#!/usr/bin/env python3
"""Run owned bounded native preflights/live attempts, preserving exits and cleaning only owned processes."""
from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import secrets
import selectors
import signal
import subprocess
import time
import urllib.request
import uuid

from prepare import ROOT, LOCAL, digest
import renewal

PG_IMAGE = "postgres:16-alpine@sha256:cf78e76683b9ca8c5733cbbdce6c9262b45b6767934dd0a95e671f9a0fc20685"


def clean_environment(home):
    environment = {
        key: value for key, value in os.environ.items()
        if not key.startswith("JX_") and key not in {
            "_JAVA_OPTIONS", "JAVA_TOOL_OPTIONS", "JDK_JAVA_OPTIONS", "DATABASE_URL",
            "CLUBSCAPE_TEST_DATABASE_URL", "CLUBSCAPE_WEB_ROOT", "CLUBSCAPE_GAME_ROOT",
        }
    }
    environment.update({"HOME": str(home), "TMPDIR": str(home), "XDG_CACHE_HOME": str(home / "cache")})
    return environment


def run_command(command, *, environment=None, timeout=120):
    return subprocess.run(command, cwd=ROOT, env=environment, text=True,
                          capture_output=True, timeout=timeout, check=False)


def require(command, **kwargs):
    result = run_command(command, **kwargs)
    if result.returncode:
        raise RuntimeError(f"{command[0]} exited {result.returncode}: {result.stderr[-1600:]}")
    return result.stdout.strip()


def stop(process):
    if process is None or process.poll() is not None:
        return
    process.terminate()
    try:
        process.wait(timeout=20)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)


def display(output, environment):
    read_fd, write_fd = os.pipe()
    log = (output / "xvfb.log").open("w")
    command = [
        "/usr/bin/Xvfb", "-displayfd", str(write_fd), "-screen", "0", "1920x1080x24",
        "-nolisten", "tcp", "-nolisten", "unix", "-nolock",
    ]
    process = subprocess.Popen(command, cwd=ROOT, env=environment, stdout=log, stderr=subprocess.STDOUT,
                               pass_fds=(write_fd,))
    os.close(write_fd)
    selector = selectors.DefaultSelector()
    selector.register(read_fd, selectors.EVENT_READ)
    try:
        if not selector.select(timeout=15):
            raise RuntimeError("Owned Xvfb did not announce an abstract loopback display")
        value = os.read(read_fd, 32).decode().strip()
        if not value.isdecimal() or process.poll() is not None:
            raise RuntimeError("Owned Xvfb failed: " + (output / "xvfb.log").read_text()[-1600:])
        environment["DISPLAY"] = ":" + value
        probe = run_command(["xdpyinfo", "-display", environment["DISPLAY"]], environment=environment, timeout=15)
        if probe.returncode:
            raise RuntimeError("Owned Xvfb is not responsive: " + probe.stderr[-1600:])
        return process, log, command
    except BaseException:
        stop(process)
        log.close()
        raise
    finally:
        selector.close()
        os.close(read_fd)


def verify_cache():
    records = json.loads((ROOT / "research/current-source/cache-files.json").read_text())
    for record in records:
        path = LOCAL / "cache-2695" / record["name"]
        if path.stat().st_size != record["size_bytes"] or digest(path) != record["sha256"]:
            raise RuntimeError("Original cache input changed: " + record["name"])
    for record in json.loads((ROOT / "research/runelite-feasibility/build-inputs.json").read_text())["libraries"]:
        path = LOCAL / "libraries" / record["name"]
        if path.stat().st_size != record["size_bytes"] or digest(path) != record["sha256"]:
            raise RuntimeError("Pinned official runtime library changed: " + record["name"])


def native_errors(path):
    if not path.exists():
        return []
    import re
    lines = path.read_text(errors="replace").splitlines()
    errors = []
    for index, line in enumerate(lines):
        if "thrown in " not in line or "method <" not in line:
            continue
        owner = re.search(r" in '([^']+)'", line)
        if owner and re.fullmatch(r"[a-z]{1,3}|rl\d+", owner.group(1)):
            context = "\n".join(lines[max(0, index - 2):index + 2])
            if re.search(r"NullPointerException|IndexOutOfBoundsException|ArithmeticException|IllegalStateException|OutOfMemoryError", context):
                errors.append(context)
    return errors[:20]


def native(java_home, output, mode, environment, catalog=None, deadline=None):
    verify_cache()
    xvfb = log = None
    try:
        xvfb, log, display_command = display(output, environment)
        classpath = os.pathsep.join([str(LOCAL / "classes"), (LOCAL / "classpath.txt").read_text()])
        arguments = [
            str(java_home / "bin/java"), "-ea", "-Xmx3072m", "-Xss2m", "-XX:ActiveProcessorCount=2",
            "-Djava.awt.headless=false", f"-Duser.home={environment['HOME']}",
            f"-Djava.io.tmpdir={environment['HOME']}", "-Djagex.disableBouncyCastle=true",
            f"-Djagex.userhome={environment['HOME']}",
            "--add-opens=java.base/java.net=ALL-UNNAMED", "--add-opens=java.base/java.io=ALL-UNNAMED",
            "--add-opens=java.base/java.lang=ALL-UNNAMED",
            f"-Xlog:exceptions=info:file={output / 'native-exceptions.log'}",
            "-cp", classpath, "RuneLiteComposition", str(ROOT), str(output), mode,
        ]
        if catalog is not None:
            arguments.append(str(catalog))
        with (output / "runtime.log").open("w") as runtime_log:
            process = subprocess.Popen(arguments, cwd=ROOT, env=environment, stdout=runtime_log, stderr=subprocess.STDOUT)
            try:
                budget = 150 if mode == "preflight" else min(570, max(1, deadline - time.monotonic() - 30))
                exit_code = process.wait(timeout=budget)
            finally:
                stop(process)
        verify_cache()
        errors = native_errors(output / "native-exceptions.log")
        return {"command": arguments, "exit_code": exit_code, "display_command": display_command,
                "display_responsive": True, "native_render_errors": errors,
                "runtime_log": str((output / "runtime.log").relative_to(ROOT)),
                "evidence": str((output / "events.jsonl").relative_to(ROOT))}
    finally:
        stop(xvfb)
        if log:
            log.close()


def write_secret(secret_path):
    password = secrets.token_urlsafe(32)
    descriptor = os.open(secret_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w") as secret:
        secret.write(password)
    return password


def preserve_owned_database(name, output, environment, report):
    path = output / "private-world-after-run.pgcustom"
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "wb") as destination:
        result = subprocess.run(
            ["docker", "exec", name, "pg_dump", "--username=clubscape", "--format=custom", "clubscape"],
            cwd=ROOT, env=environment, stdout=destination, stderr=subprocess.PIPE, timeout=30, check=False)
    report["database"]["private_backup"] = {
        "exit_code": result.returncode, "path": str(path.relative_to(ROOT)), "size_bytes": path.stat().st_size,
        "sha256": digest(path), "read_only_snapshot": True, "publish_contents": False,
        "scope": "Owned synthetic world/account database only; private RNG/auth material stays ignored",
    }
    if result.returncode:
        raise RuntimeError("Could not preserve the owned synthetic world before cleanup")


def start_database(output, environment, report):
    name = "clubscape-rl-" + uuid.uuid4().hex[:16]
    secret_path = output / "postgres-password"
    password = write_secret(secret_path)
    command = [
        "docker", "run", "--detach", "--rm", "--name", name, "--cpus=2", "--memory=512m",
        "--publish", "127.0.0.1::5432",
        "--mount", f"type=bind,src={secret_path},dst=/run/secrets/password,readonly",
        "--env", "POSTGRES_PASSWORD_FILE=/run/secrets/password",
        "--env", "POSTGRES_USER=clubscape", "--env", "POSTGRES_DB=clubscape", PG_IMAGE,
    ]
    require(command, environment=environment)
    report["database"] = {"name": name, "image": PG_IMAGE, "command": command, "cpus": 2, "memory_mib": 512}
    try:
        for _ in range(100):
            result = run_command(["docker", "exec", name, "pg_isready", "--username=clubscape", "--dbname=clubscape"],
                                 environment=environment, timeout=10)
            if result.returncode == 0:
                address = require(["docker", "port", name, "5432/tcp"], environment=environment)
                if not address.startswith("127.0.0.1:") or "\n" in address:
                    raise RuntimeError("Owned PostgreSQL is not loopback-only")
                report["database"]["responsive"] = True
                return name, f"postgresql://clubscape:{password}@{address}/clubscape"
            time.sleep(0.3)
        raise RuntimeError("Owned PostgreSQL did not become ready")
    except BaseException:
        require(["docker", "stop", "--time", "10", name], environment=environment)
        raise


def wait_server(process, path):
    deadline = time.monotonic() + 45
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f"Strict authoritative server exited {process.returncode} before listening")
        for line in path.read_text(errors="replace").splitlines():
            try:
                fields = json.loads(line).get("fields", {})
            except json.JSONDecodeError:
                continue
            if fields.get("event") == "listening":
                address = fields.get("address", "")
                if not address.startswith("127.0.0.1:"):
                    raise RuntimeError("Authoritative service is not loopback-only")
                opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
                with opener.open("http://" + address + "/healthz", timeout=5) as response:
                    if response.status != 200:
                        raise RuntimeError("Real PostgreSQL-backed health failed")
                return "http://" + address
        time.sleep(0.1)
    raise RuntimeError("No bounded service readiness; not proof of impossibility")


def validate_pack_selection(game_root, catalog):
    descriptor = json.loads((game_root / "clubscape-game.json").read_text())
    canonical = json.loads((ROOT / "content/m1/manifest.json").read_text())["compiled_artifact"]
    if descriptor["sha256"] != canonical["uncompressed_sha256"]:
        raise ValueError("Selected game pack is not the current canonical artifact; choose a current --game-root.")
    public = json.loads((game_root / "manifest.json").read_text())
    adapter = json.loads(catalog.read_text())
    if adapter["content_revision"] != public["content_revision"]:
        raise ValueError("Selected --catalog belongs to a different source content revision.")


def validate_bundle_storage(game_root):
    manifest = json.loads((game_root / "clubscape-game-assets.json").read_text())
    allowed = {".html", ".js", ".css", ".wasm", ".json", ".png", ".jpg", ".jpeg", ".webp",
               ".ogg", ".flac", ".wav", ".glb", ".bin", ".ktx2", ".woff", ".woff2", ".ttf", ".svg"}
    total = 0
    for record in manifest["files"]:
        path = game_root / record["path"]
        if (Path(record["path"]).is_absolute() or ".." in Path(record["path"]).parts
                or path.suffix not in allowed or path.is_symlink()
                or not path.resolve().is_relative_to(game_root.resolve()) or not path.is_file()):
            raise ValueError("Invalid public bundle storage path; preserve the server's extension/path guards")
        if path.stat().st_size > 64 * 1024 * 1024 or digest(path) != record["sha256"]:
            raise ValueError("Public bundle storage bytes/hash differ from the pinned descriptor")
        total += path.stat().st_size
    if total > 512 * 1024 * 1024:
        raise ValueError("Public asset memory bound exceeded")
    descriptor = json.loads((game_root / "clubscape-game.json").read_text())
    artifact = game_root / descriptor["artifact"]
    if digest(artifact) != descriptor["sha256"]:
        raise ValueError("Pinned world artifact bytes changed")
    return {"files": len(manifest["files"]), "bytes": total, "storage_extensions_and_hashes": "verified",
            "artifact_sha256": descriptor["sha256"], "world_artifact_repacked": False}


def live(java_home, output, environment, report, server_binary, game_root, catalog):
    name = None
    server = None
    started = time.monotonic()
    deadline = started + 600
    try:
        name, database_url = start_database(output, environment, report)
        server_environment = {**environment, "DATABASE_URL": database_url,
            "CLUBSCAPE_GAME_ROOT": str(game_root), "CLUBSCAPE_BIND": "127.0.0.1:0",
            "CLUBSCAPE_BUILD_REVISION": require(["git", "rev-parse", "HEAD"]),
            "TOKIO_WORKER_THREADS": "2",
        }
        report["server"] = {"command": [str(server_binary)], "sha256": digest(server_binary),
                            "game_descriptor_sha256": digest(game_root / "clubscape-game.json"),
                            "game_root": str(game_root), "adapter_catalog": str(catalog)}
        log_path = output / "server.log"
        with log_path.open("w") as log:
            server = subprocess.Popen([str(server_binary)], cwd=ROOT, env=server_environment,
                                      stdout=log, stderr=subprocess.STDOUT)
            report["phase"] = "strict_authoritative_service_readiness"
            origin = wait_server(server, log_path)
            report["server"]["origin"] = origin
            report["server"]["responsive"] = True
            report["phase"] = "real_runtime_scene_state_plugin"
            report["native"] = native(java_home, output, origin, environment, catalog, deadline)
            report["exit_code"] = report["native"]["exit_code"]
            events = [json.loads(line) for line in (output / "events.jsonl").read_text().splitlines()]
            report["complete_tuple"] = next((event for event in events if event["kind"] == "complete_tuple"), None)
            report["compatibility_verified"] = bool(report["complete_tuple"] and report["exit_code"] == 0
                                                   and not report["native"]["native_render_errors"])
    finally:
        if name:
            try:
                preserve_owned_database(name, output, environment, report)
            except Exception as error:
                report["preservation_error"] = str(error)
                report["compatibility_verified"] = False
        stop(server)
        if server is not None:
            report["server"]["exit_code"] = server.returncode
        if name:
            cleanup = run_command(["docker", "stop", "--time", "10", name], environment=environment, timeout=30)
            report["database"]["cleanup_exit_code"] = cleanup.returncode
        (output / "postgres-password").unlink(missing_ok=True)
        report["elapsed_seconds_including_cleanup"] = round(time.monotonic() - started, 3)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=["preflight", "live"])
    parser.add_argument("--java-home", type=Path, required=True)
    parser.add_argument("--name", required=True)
    parser.add_argument("--attempt", type=int)
    parser.add_argument("--hypothesis")
    parser.add_argument("--server-binary", type=Path, default=LOCAL / "rust-target/debug/clubscape-server")
    parser.add_argument("--game-root", type=Path, default=LOCAL / "game")
    parser.add_argument("--catalog", type=Path)
    parser.add_argument("--renewal-record", type=Path)
    args = parser.parse_args()
    if not args.name.replace("-", "").isalnum():
        parser.error("Use an alphanumeric/hyphen owned evidence directory name")
    game_root = args.game_root.resolve()
    catalog = (args.catalog or (game_root / "adapter-catalog.json"
        if (game_root / "adapter-catalog.json").exists() else LOCAL / "catalog.json")).resolve()
    if not game_root.is_relative_to(LOCAL) or not catalog.is_relative_to(LOCAL):
        parser.error("Game roots and catalogs must remain in the owned artifacts directory.")
    experiment_path = ROOT / "research/runelite-feasibility/experiment.json"
    experiment = json.loads(experiment_path.read_text())
    renewed = None
    bundle_check = None
    if args.mode == "live":
        if not args.hypothesis:
            parser.error("Each live attempt requires its own falsifiable --hypothesis")
        validate_pack_selection(game_root, catalog)
        bundle_check = validate_bundle_storage(game_root)
        if args.renewal_record:
            artifact = json.loads((game_root / "clubscape-game.json").read_text())["sha256"]
            renewed = renewal.load_allocation(args.renewal_record, args.attempt, artifact)
        else:
            if args.attempt not in (1, 2, 3) or any(a["number"] == args.attempt for a in experiment["live_attempts"]):
                parser.error("A new explicitly numbered live attempt within the original allocation is required")
            if len(experiment["live_attempts"]) >= 3:
                parser.error("The original bound is exhausted; an explicit renewal record is required for 4/5")
    output = LOCAL / args.name
    output.mkdir(mode=0o700)
    home = output / "home"
    home.mkdir(mode=0o700)
    environment = clean_environment(home)
    report = {
        "schema_version": 1, "mode": args.mode, "name": args.name,
        "recorded_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "architecture": "A", "compatibility_verified": False, "exit_code": 1,
        "global_invocation": args.attempt, "allocation": "renewal" if renewed is not None else "original",
        "evidence_directory": str(output.relative_to(ROOT)),
        "bundle_preflight": bundle_check,
    }
    if args.mode == "live":
        entry = {
            "number": args.attempt, "architecture": "A", "name": args.name,
            "hypothesis": args.hypothesis,
            "diagnostics": ["strict server readiness/exit", "actual protobuf receipts", "native scene/player", "real plugin output"],
            "status": "started", "report": f"research/runelite-feasibility/{args.name}.json",
        }
        if renewed is not None:
            renewed["invocations"].append(entry)
            renewal.save(renewed)
        else:
            experiment["live_attempts"].append(entry)
            experiment_path.write_text(json.dumps(experiment, indent=2) + "\n")
    try:
        if args.mode == "preflight":
            report["native"] = native(args.java_home.resolve(), output, "preflight", environment, catalog)
            report["exit_code"] = report["native"]["exit_code"]
        else:
            live(args.java_home.resolve(), output, environment, report, args.server_binary.resolve(), game_root, catalog)
    except BaseException as error:
        report["error"] = f"{type(error).__name__}: {error}"
    finally:
        report_path = ROOT / f"research/runelite-feasibility/{args.name}.json"
        report_path.write_text(json.dumps(report, indent=2) + "\n")
        if args.mode == "live":
            current = json.loads((renewal.LEDGER if renewed is not None else experiment_path).read_text())
            attempts = current["invocations"] if renewed is not None else current["live_attempts"]
            attempt = next(a for a in attempts if a["number"] == args.attempt)
            attempt["status"] = "passed" if report["compatibility_verified"] else "failed"
            attempt["phase"] = report.get("phase")
            attempt["exit_code"] = report["exit_code"]
            if renewed is not None:
                current["compatibility_verified"] = report["compatibility_verified"]
                current["state"] = "passed_and_stopped" if report["compatibility_verified"] else (
                    "exhausted_and_stopped" if len(attempts) == 2 else "one_remaining_invocation")
                renewal.save(current)
            else:
                experiment_path.write_text(json.dumps(current, indent=2) + "\n")
    print(json.dumps({k: v for k, v in report.items() if k not in ["native", "server", "database"]}))
    return report["exit_code"]


if __name__ == "__main__":
    raise SystemExit(main())
