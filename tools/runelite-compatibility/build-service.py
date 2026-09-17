#!/usr/bin/env python3
"""Build the real shared service's public-configuration probe, preserving locked inputs."""
import argparse
import json
import os
from pathlib import Path
import subprocess

from prepare import ROOT, LOCAL, digest

MANIFEST = ROOT / "runelite/integration-tests/service-probe/Cargo.toml"

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", type=Path, default=ROOT / "research/runelite-feasibility/service-probe-build.json")
    args = parser.parse_args()
    report_path = args.report.resolve()
    if not report_path.is_relative_to(ROOT / "research/runelite-feasibility"):
        parser.error("Build reports must remain in the owned research directory.")
    home = LOCAL / "build-home"
    home.mkdir(parents=True, exist_ok=True)
    environment = {**os.environ, "CARGO_BUILD_JOBS": "2",
                   "CARGO_TARGET_DIR": str(LOCAL / "rust-target"), "TMPDIR": str(home)}
    command = ["cargo", "build", "--quiet", "--locked", "--offline", "--manifest-path", str(MANIFEST)]
    result = subprocess.run(command, cwd=ROOT, env=environment, text=True, capture_output=True, timeout=600)
    (LOCAL / "service-probe-build.log").write_text(result.stdout + result.stderr)
    binary = LOCAL / "rust-target/debug/clubscape-runelite-service-probe"
    report = {
        "command": command, "exit_code": result.returncode,
        "source_revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "root_lock_sha256": digest(ROOT / "Cargo.lock"),
        "probe_lock_sha256": digest(MANIFEST.with_name("Cargo.lock")),
        "probe_main_sha256": digest(MANIFEST.parent / "src/main.rs"),
        "server_config_sha256": digest(ROOT / "crates/server/src/config.rs"),
        "server_content_loader_sha256": digest(ROOT / "crates/server/src/game_service/content.rs"),
        "shared_authority_modified": False,
        "binary_sha256": digest(binary) if result.returncode == 0 else None,
        "compatibility_verified": False,
    }
    report_path.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report))
    if result.returncode:
        print((result.stdout + result.stderr)[-8000:])
    raise SystemExit(result.returncode)
