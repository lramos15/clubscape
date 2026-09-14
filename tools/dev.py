#!/usr/bin/env python3
"""Bounded local account infrastructure; never a gameplay acceptance substitute."""

import argparse
import hashlib
import json
import os
import re
import secrets
import shutil
import stat
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
POSTGRES_IMAGE = (
    "postgres:16-alpine@sha256:"
    "cf78e76683b9ca8c5733cbbdce6c9262b45b6767934dd0a95e671f9a0fc20685"
)


class DevelopmentError(RuntimeError):
    pass


def run(arguments, *, cwd=ROOT, env=None, show=False, timeout=600):
    result = subprocess.run(
        arguments, cwd=cwd, env=env, capture_output=True, text=True,
        timeout=timeout, check=False,
    )
    output = result.stdout + result.stderr
    for name in ("DATABASE_URL", "CLUBSCAPE_TEST_DATABASE_URL"):
        value = (env or {}).get(name)
        if value:
            output = output.replace(value, "<redacted database URL>")
    if result.returncode:
        raise DevelopmentError(
            f"{arguments[0]} failed (exit {result.returncode}):\n{output[-12000:]}"
        )
    if show and output:
        print(output, end="" if output.endswith("\n") else "\n")
    return result.stdout


def private_directory(path):
    if path.is_symlink():
        raise DevelopmentError(f"Refusing a symlinked private directory: {path}")
    path.mkdir(mode=0o700, parents=True, exist_ok=True)
    info = path.stat()
    if info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) & 0o077:
        raise DevelopmentError(f"Private directory must be owned by this user with mode 0700: {path}")
    return path


def write_secret(path):
    if path.is_symlink():
        raise DevelopmentError("Refusing a symlinked credential file.")
    if path.exists():
        info = path.stat()
        if not stat.S_ISREG(info.st_mode) or stat.S_IMODE(info.st_mode) != 0o600 or info.st_uid != os.getuid():
            raise DevelopmentError("Existing credentials must be an owned mode-0600 regular file.")
        value = path.read_text(encoding="ascii").strip()
        if not re.fullmatch(r"[A-Za-z0-9_-]{43}", value):
            raise DevelopmentError("Invalid existing development credentials; not overwriting them.")
        return value
    value = secrets.token_urlsafe(32)
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w", encoding="ascii") as target:
        target.write(value + "\n")
    return value


def database_url(password, port, database):
    if type(port) is not int or not 1 <= port <= 65535:
        raise DevelopmentError("Invalid isolated PostgreSQL port.")
    if database not in {"clubscape", "clubscape_m1_test"}:
        raise DevelopmentError("Unexpected project database name.")
    return f"postgresql://clubscape:{urllib.parse.quote(password, safe='')}@127.0.0.1:{port}/{database}"


def workspace_path(value):
    workspace = Path(value).resolve()
    if not workspace.is_relative_to(ROOT) or not (workspace / "Cargo.toml").is_file():
        raise DevelopmentError("Validation workspace must be a Cargo workspace inside this repository.")
    return workspace


def source_revision(workspace):
    return run(["git", "rev-parse", "HEAD"], cwd=workspace).strip()


def doctor():
    missing = [name for name in ("git", "cargo", "rustup", "docker") if shutil.which(name) is None]
    if missing:
        raise DevelopmentError(
            "Missing prerequisites: " + ", ".join(missing)
            + ". Read AGENTS.md and docs/machines/sparky.md; bootstrap does not alter global tools."
        )
    report = {
        "host": os.uname().nodename,
        "architecture": os.uname().machine,
        "rust": run(["rustc", "--version"]).strip(),
        "cargo": run(["cargo", "--version"]).strip(),
        "docker": run(["docker", "version", "--format", "{{.Server.Version}}"]).strip(),
        "gameplay_verified": False,
    }
    print(json.dumps(report))


def development_environment():
    local = private_directory(ROOT / ".local")
    password = write_secret(local / "postgres-password")
    env = os.environ.copy()
    env["DATABASE_URL"] = database_url(password, 55432, "clubscape")
    env["CLUBSCAPE_BIND"] = "127.0.0.1:4010"
    env["CLUBSCAPE_BUILD_REVISION"] = source_revision(ROOT)
    return env


def start_database():
    development_environment()
    run(
        ["docker", "compose", "-f", str(ROOT / "compose.yaml"),
         "up", "--detach", "--wait", "--wait-timeout", "90", "postgres"],
        show=True, timeout=150,
    )


def stop_process(process):
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=15)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=10)
            raise DevelopmentError("The owned test-server process required forced termination.")
    if process.returncode != 0:
        raise DevelopmentError(f"The owned test server exited unsuccessfully ({process.returncode}).")


def reset_test_database(container):
    if not re.fullmatch(r"clubscape-m1-test-[0-9a-f]{16}", container):
        raise DevelopmentError("Refusing to reset a database outside the owned disposable container.")
    run(["docker", "exec", container, "dropdb", "--username=clubscape", "clubscape_m1_test"])
    run(["docker", "exec", container, "createdb", "--username=clubscape", "clubscape_m1_test"])


def wait_server(process, log_path):
    deadline = time.monotonic() + 20
    with log_path.open(encoding="utf-8") as log:
        while time.monotonic() < deadline:
            if process.poll() is not None:
                raise DevelopmentError("Test server exited before readiness; inspect the isolated server log.")
            line = log.readline()
            if line:
                event = json.loads(line)
                fields = event.get("fields", {})
                if fields.get("event") == "listening":
                    address = fields.get("address", "")
                    if not re.fullmatch(r"127\.0\.0\.1:[0-9]+", address):
                        raise DevelopmentError("Test server did not announce its loopback listener.")
                    origin = "http://" + address
                    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
                    with opener.open(origin + "/healthz", timeout=5) as response:
                        if response.status != 200:
                            raise DevelopmentError("Test server failed its real database readiness check.")
                    return origin
            else:
                time.sleep(0.05)
    raise DevelopmentError("Timed out waiting for the test-server listening event.")


def write_report(path, report):
    resolved = (ROOT / path).resolve()
    if Path(path).is_absolute() or not resolved.is_relative_to(ROOT):
        raise DevelopmentError("Evidence reports must stay inside the repository.")
    resolved.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile("w", dir=resolved.parent, delete=False, encoding="utf-8") as temporary:
        temporary_path = Path(temporary.name)
        try:
            json.dump(report, temporary, indent=2, sort_keys=True)
            temporary.write("\n")
            temporary.flush()
            os.fsync(temporary.fileno())
            os.replace(temporary_path, resolved)
        finally:
            temporary_path.unlink(missing_ok=True)


def integration(workspace, report_path):
    local = private_directory(ROOT / ".local")
    revision = source_revision(workspace)
    report = {
        "schema_version": 1,
        "kind": "account_foundations",
        "revision": revision,
        "recorded_at": datetime.now(timezone.utc).isoformat(),
        "workspace_dirty": bool(run(["git", "status", "--porcelain"], cwd=workspace).strip()),
        "cargo_lock_sha256": hashlib.sha256((workspace / "Cargo.lock").read_bytes()).hexdigest(),
        "database_image": POSTGRES_IMAGE,
        "resource_limits": {"database_cpus": 2, "database_memory_mib": 512},
        "commands": [],
        "result": "failed",
        "gameplay_verified": False,
        "browser_signup_verified": False,
        "milestone_accepted": False,
    }
    created = False
    container = "clubscape-m1-test-" + uuid.uuid4().hex[:16]
    try:
        build = ["cargo", "build", "--quiet", "--locked", "-p", "clubscape-server", "-p", "clubscape-sim"]
        run(build, cwd=workspace, show=True)
        report["commands"].append({"command": " ".join(build), "result": "passed"})
        metadata = json.loads(run(["cargo", "metadata", "--no-deps", "--format-version=1", "--locked"], cwd=workspace))
        binary_dir = Path(metadata["target_directory"]) / "debug"
        with tempfile.TemporaryDirectory(prefix="integration-", dir=local) as directory:
            temporary = Path(directory)
            secret_path = temporary / "postgres-password"
            password = write_secret(secret_path)
            run([
                "docker", "run", "--detach", "--rm", "--name", container,
                "--label", "clubscape.scope=m1-integration", "--cpus=2", "--memory=512m",
                "--pids-limit=256", "--read-only",
                "--tmpfs", "/var/lib/postgresql/data:rw,nosuid,nodev,size=384m",
                "--tmpfs", "/var/run/postgresql:rw,nosuid,nodev,size=16m",
                "--tmpfs", "/tmp:rw,nosuid,nodev,size=16m",
                "--mount", f"type=bind,src={secret_path},dst=/run/secrets/postgres-password,readonly",
                "--env", "POSTGRES_USER=clubscape", "--env", "POSTGRES_DB=clubscape_m1_test",
                "--env", "POSTGRES_PASSWORD_FILE=/run/secrets/postgres-password",
                "--publish", "127.0.0.1::5432", POSTGRES_IMAGE,
            ], timeout=120)
            created = True
            deadline = time.monotonic() + 60
            while time.monotonic() < deadline:
                ready = subprocess.run(
                    ["docker", "exec", container, "pg_isready", "-h", "127.0.0.1",
                     "-U", "clubscape", "-d", "clubscape_m1_test"],
                    capture_output=True, timeout=10, check=False,
                )
                if ready.returncode == 0:
                    break
                time.sleep(0.25)
            else:
                raise DevelopmentError("Isolated PostgreSQL did not become ready within 60 seconds.")
            binding = run(["docker", "port", container, "5432/tcp"]).strip()
            match = re.fullmatch(r"127\.0\.0\.1:([0-9]+)", binding)
            if match is None:
                raise DevelopmentError("PostgreSQL is not exclusively bound to a random loopback port.")
            env = os.environ.copy()
            url = database_url(password, int(match[1]), "clubscape_m1_test")
            env["CLUBSCAPE_TEST_DATABASE_URL"] = url
            env["DATABASE_URL"] = url
            env["CLUBSCAPE_BUILD_REVISION"] = revision
            tests = [
                "cargo", "test", "--quiet", "-p", "clubscape-server", "--tests",
                "--locked", "--", "--ignored", "--test-threads=1",
            ]
            output = run(tests, cwd=workspace, env=env, show=True)
            counts = re.findall(r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored", output)
            if not counts or sum(int(count[0]) for count in counts) == 0 or any(
                count[1:] != ("0", "0") for count in counts
            ):
                raise DevelopmentError("The real database integration suite did not run and pass all selected tests.")
            report["commands"].append({
                "command": " ".join(tests),
                "passed_tests": sum(int(count[0]) for count in counts),
                "result": "passed",
            })
            # Adversarial cases deliberately corrupt schema; the independent client owns a fresh database.
            reset_test_database(container)
            report["independent_client_database"] = "Fresh isolated database after adversarial integration fixtures."
            env["CLUBSCAPE_BIND"] = "127.0.0.1:0"
            log_path = temporary / "server.jsonl"
            with log_path.open("w", encoding="utf-8") as log:
                process = subprocess.Popen(
                    [str(binary_dir / "clubscape-server")], cwd=workspace, env=env,
                    stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT,
                )
                try:
                    origin = wait_server(process, log_path)
                    output = run(
                        [str(binary_dir / "clubscape-sim"), "account-lifecycle", "--url", origin],
                        cwd=workspace, env=env, show=True,
                    )
                    lifecycle = json.loads(output)
                    if lifecycle.get("result") != "passed" or lifecycle.get("milestone_accepted") is not False:
                        raise DevelopmentError("The independent account client did not report a bounded successful check.")
                    report["commands"].append({
                        "command": "clubscape-sim account-lifecycle --url <isolated-loopback-server>",
                        "result": "passed", "checks": lifecycle["checks"],
                    })
                except (DevelopmentError, OSError, ValueError) as error:
                    diagnostic = log_path.read_text(encoding="utf-8")[-8000:].replace(password, "<redacted>")
                    raise DevelopmentError(f"{error}\nIsolated server diagnostic:\n{diagnostic}") from error
                finally:
                    stop_process(process)
            report["result"] = "passed"
    except (DevelopmentError, OSError, ValueError, subprocess.TimeoutExpired) as error:
        report["failure"] = str(error)[-12000:]
        raise
    finally:
        if created:
            try:
                run(["docker", "rm", "--force", container], timeout=30)
                report["cleanup"] = "Owned test server/container stopped; disposable database removed."
            except (DevelopmentError, subprocess.TimeoutExpired):
                report["result"] = "failed"
                report["cleanup"] = f"Cleanup failed for owned container {container}; manual attention required."
                write_report(report_path, report)
                raise
        write_report(report_path, report)
    print(json.dumps({
        "result": report["result"], "evidence": report_path,
        "revision": revision, "milestone_accepted": False,
    }))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=["doctor", "bootstrap", "db-start", "db-stop", "server", "integration"])
    parser.add_argument("--workspace", default=str(ROOT))
    parser.add_argument("--report", default=".local/evidence/accounts-integration.json")
    args = parser.parse_args()
    try:
        if args.operation == "doctor":
            doctor()
        elif args.operation in {"bootstrap", "db-start"}:
            doctor()
            start_database()
            if args.operation == "bootstrap":
                run(["cargo", "build", "--quiet", "--workspace", "--locked"], show=True)
                run(["cargo", "test", "--quiet", "--workspace", "--locked"], show=True)
                print("Account infrastructure built. No game content, WASM renderer or accepted slice exists.")
        elif args.operation == "db-stop":
            run(["docker", "compose", "-f", str(ROOT / "compose.yaml"), "stop", "postgres"], show=True)
        elif args.operation == "server":
            env = development_environment()
            os.execvpe("cargo", ["cargo", "run", "--quiet", "--locked", "-p", "clubscape-server"], env)
        else:
            integration(workspace_path(args.workspace), args.report)
        return 0
    except (DevelopmentError, OSError, ValueError, subprocess.TimeoutExpired, urllib.error.URLError) as error:
        print(f"Development command failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
