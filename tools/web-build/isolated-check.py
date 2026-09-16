#!/usr/bin/env python3
"""Owned PostgreSQL/HTTP/Chrome contract check; explicitly not a game journey."""
import argparse
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import signal
import subprocess
import time
import urllib.request
import uuid

ROOT = Path(__file__).resolve().parents[2]
IMAGE = "postgres:16-alpine@sha256:cf78e76683b9ca8c5733cbbdce6c9262b45b6767934dd0a95e671f9a0fc20685"


def command(args, env=None, timeout=120):
    process = subprocess.Popen(args, cwd=ROOT, env=env, text=True, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, start_new_session=True)
    try:
        stdout, stderr = process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGTERM)
        try:
            process.communicate(timeout=10)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.communicate(timeout=5)
        raise RuntimeError(f"{args[0]} exceeded its bounded runtime; its owned process group was stopped.") from None
    if process.returncode:
        # No environment or credential-bearing process command is printed.
        raise RuntimeError(f"{args[0]} failed ({process.returncode}): {(stdout + stderr)[-4000:]}")
    return stdout.strip()

def wait_server(process, log_path):
    deadline = time.monotonic() + 35
    while time.monotonic() < deadline:
        if process.poll() is not None:
            failures = [json.loads(line).get("fields", {}) for line in log_path.read_text().splitlines() if line]
            reason = next((row.get("error_kind") for row in failures if row.get("event") == "startup_failure"), "unknown")
            raise RuntimeError(f"Owned server failed startup ({reason}); no account-only fallback was selected.")
        for line in log_path.read_text().splitlines():
            fields = json.loads(line).get("fields", {})
            if fields.get("event") == "listening":
                address = fields.get("address", "")
                if not re.fullmatch(r"127\.0\.0\.1:[0-9]+", address):
                    raise RuntimeError("Owned service did not bind loopback.")
                origin = "http://" + address
                opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
                with opener.open(origin + "/healthz", timeout=5) as health:
                    if health.status != 200:
                        raise RuntimeError("Owned service failed its real readiness check.")
                return origin
        time.sleep(0.1)
    raise RuntimeError("Owned service did not become ready.")


def start_server(binary, env, log_path):
    with log_path.open("w") as log:
        process = subprocess.Popen([str(binary)], cwd=ROOT, env=env, stdin=subprocess.DEVNULL,
                                   stdout=log, stderr=subprocess.STDOUT)
    return process


def stop_server(process):
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=20)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)
            raise RuntimeError("Owned source server required forced termination.")
    if process.returncode != 0:
        raise RuntimeError("Owned source server exited unsuccessfully.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--game-root", type=Path, help="Use the real pinned canonical world, never a fixture or reseeded artifact.")
    arguments = parser.parse_args()
    run_id = uuid.uuid4().hex[:16]
    evidence = (ROOT / os.environ.get("CLUBSCAPE_BROWSER_EVIDENCE", ".local/evidence/browser-shell")).resolve()
    if evidence == ROOT or not evidence.is_relative_to(ROOT):
        raise RuntimeError("Browser evidence must stay in a worktree-owned directory.")
    local = ROOT / ".local" / "browser-shell-checks" / run_id
    local.mkdir(parents=True, mode=0o700)
    runtime = local / "runtime"
    runtime.mkdir(mode=0o700)
    evidence.mkdir(parents=True, exist_ok=True)
    container = "clubscape-browser-shell-" + run_id
    created = False
    process = None
    source_pin = None
    game_root = arguments.game_root or os.environ.get("CLUBSCAPE_GAME_DESCRIPTOR_PROBE")
    try:
        command(["cargo", "build", "--quiet", "-p", "clubscape-server"], timeout=300)
        target = Path(json.loads(command(["cargo", "metadata", "--no-deps", "--format-version=1"]))["target_directory"])
        password = secrets.token_urlsafe(32)
        database_env = dict(os.environ, POSTGRES_PASSWORD=password)
        command([
            "docker", "run", "--detach", "--rm", "--name", container,
            "--label", "clubscape.scope=m1-browser-shell-infrastructure",
            "--cpus=2", "--memory=512m", "--pids-limit=256", "--read-only",
            "--tmpfs", "/var/lib/postgresql/data:rw,nosuid,nodev,size=384m",
            "--tmpfs", "/var/run/postgresql:rw,nosuid,nodev,size=16m",
            "--env", "POSTGRES_USER=clubscape", "--env", "POSTGRES_DB=clubscape_browser",
            "--env", "POSTGRES_PASSWORD", "--publish", "127.0.0.1::5432", IMAGE,
        ], env=database_env)
        created = True
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            ready = subprocess.run(["docker", "exec", container, "pg_isready", "-h", "127.0.0.1",
                                    "-U", "clubscape", "-d", "clubscape_browser"],
                                   cwd=ROOT, capture_output=True, timeout=10)
            if ready.returncode == 0:
                break
            time.sleep(0.25)
        else:
            raise RuntimeError("Owned PostgreSQL did not become ready.")
        binding = command(["docker", "port", container, "5432/tcp"])
        match = re.fullmatch(r"127\.0\.0\.1:([0-9]+)", binding)
        if not match:
            raise RuntimeError("Owned database is not exclusively on a random loopback port.")
        env = dict(os.environ)
        env.pop("CLUBSCAPE_GAME_ROOT", None)
        env["DATABASE_URL"] = f"postgresql://clubscape:{password}@127.0.0.1:{match[1]}/clubscape_browser"
        env["CLUBSCAPE_BIND"] = "127.0.0.1:0"
        web_root = Path(command(["node", "tools/web-build/output.ts"]))
        env["CLUBSCAPE_WEB_ROOT"] = str(web_root)
        env["CLUBSCAPE_BUILD_REVISION"] = command(["git", "rev-parse", "HEAD"])
        env["TMPDIR"] = str(runtime)
        if game_root:
            game_root = (ROOT / game_root).resolve()
            if game_root == ROOT or not game_root.is_relative_to(ROOT):
                raise RuntimeError("The source run must use an owned worktree bundle.")
            source_pin = json.loads(command(["node", "tools/web-build/run-pins.ts", str(game_root)]))
            build = json.loads((web_root / "client/build.json").read_text())
            if not build.get("content") or build["content"]["owner"] != "game" or build["content"]["sha256"] != source_pin["contentManifestSha256"]:
                raise RuntimeError("Build this exact source manifest with CLUBSCAPE_CONTENT_OWNER=game before real world integration; overlapping web/game assets are not allowed.")
            env["CLUBSCAPE_GAME_ROOT"] = str(game_root)
            standalone = dict(env)
            standalone.pop("CLUBSCAPE_WEB_ROOT")
            standalone_log = local / "standalone.jsonl"
            process = start_server(target / "debug/clubscape-server", standalone, standalone_log)
            wait_server(process, standalone_log)
            stop_server(process)
            process = None
            after = json.loads(command(["node", "tools/web-build/run-pins.ts", str(game_root)]))
            if after != source_pin:
                raise RuntimeError("Canonical artifact changed during standalone source startup.")
            (evidence / "game-root-startup.json").write_text(json.dumps({
                "kind": "actual-canonical-game-root-startup", "result": "passed",
                "testedRevision": env["CLUBSCAPE_BUILD_REVISION"], "sourceRunPin": source_pin,
                "standaloneWithoutWebRoot": True, "descriptorBytes": (game_root / "clubscape-game.json").stat().st_size,
                "descriptorLimit": 512 * 1024, "gameplayAccepted": False,
            }, indent=2) + "\n")
        log_path = local / "server.jsonl"
        process = start_server(target / "debug/clubscape-server", env, log_path)
        origin = wait_server(process, log_path)
        browser_env = {key: value for key, value in os.environ.items()
                       if key not in ("DATABASE_URL", "CLUBSCAPE_TEST_DATABASE_URL", "POSTGRES_PASSWORD")}
        browser_env["CLUBSCAPE_BROWSER_ORIGIN"] = origin
        browser_env["CLUBSCAPE_BROWSER_EVIDENCE"] = str(evidence)
        browser_env["TMPDIR"] = str(runtime)
        browser_env["TMP"] = str(runtime)
        browser_env["TEMP"] = str(runtime)
        if game_root:
            browser_env["CLUBSCAPE_SOURCE_GAME_ROOT"] = str(game_root)
            browser_env["CLUBSCAPE_SOURCE_SERVER_BINARY"] = str(target / "debug/clubscape-server")
            browser_env["CLUBSCAPE_SOURCE_SERVER_PID"] = str(process.pid)
            browser_env["CLUBSCAPE_SOURCE_DATABASE_URL"] = env["DATABASE_URL"]
        if not browser_env.get("CLUBSCAPE_BROWSER_EXECUTABLE"):
            raise RuntimeError("Set CLUBSCAPE_BROWSER_EXECUTABLE after verifying this host's machine guide.")
        authority = local / "Xauthority"
        authority.touch(mode=0o600)
        args = [
            "xvfb-run", "--auto-servernum", "--auth-file", str(authority),
            "--error-file", str(local / "xvfb.log"),
            "--server-args=-screen 0 2560x1440x24 -nolisten tcp" if game_root else "--server-args=-screen 0 1280x800x24 -nolisten tcp",
            "node", "tools/web-build/source-browser-check.ts" if game_root else "tools/web-build/browser-check.ts",
        ]
        print(command(args, env=browser_env, timeout=240 if game_root else 180))
        if source_pin is not None:
            after = json.loads(command(["node", "tools/web-build/run-pins.ts", str(game_root)]))
            if after != source_pin:
                raise RuntimeError("Source artifact/run identity changed during the isolated check; no refreshed or reseeded run can count as a restart.")
            (evidence / "source-run-pin.json").write_text(json.dumps({
                "kind": "unchanged-source-run-pin", "result": "passed", "pin": source_pin,
                "scope": "Actual canonical startup, UI onboarding and pinned restart; not full-journey/presentation acceptance.",
            }, indent=2) + "\n")
    finally:
        cleanup_failure = None
        if process is not None and process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=20)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
                cleanup_failure = "Owned account server required forced termination."
        if process is not None and process.returncode != 0:
            cleanup_failure = "Owned account server exited unsuccessfully."
        if created:
            try:
                command(["docker", "rm", "--force", container], timeout=30)
            except RuntimeError:
                cleanup_failure = f"Owned container cleanup failed: {container}."
        shutil.rmtree(local)
        for name in ("profile", "runtime"):
            path = evidence / name
            if path.exists():
                shutil.rmtree(path)
        if cleanup_failure:
            raise RuntimeError(cleanup_failure)
        print("Owned account server, database, Xvfb/browser profile and scratch resources cleaned.")


if __name__ == "__main__":
    main()
