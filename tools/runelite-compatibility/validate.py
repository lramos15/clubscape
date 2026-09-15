#!/usr/bin/env python3
"""Record small owned checks only; no runtime launch, live request, or new integration admission."""
import argparse
import json
import os
from pathlib import Path
import subprocess

from prepare import ROOT, LOCAL
from run import clean_environment


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--java-home", type=Path, required=True)
    parser.add_argument("--report", type=Path, default=ROOT / "research/runelite-feasibility/validation.json")
    args = parser.parse_args()
    report_path = args.report.resolve()
    if not report_path.is_relative_to(ROOT / "research/runelite-feasibility"):
        parser.error("Validation reports must remain in the owned research directory.")
    home = LOCAL / "build-home"
    home.mkdir(parents=True, exist_ok=True)
    classpath = os.pathsep.join([str(LOCAL / "classes"), (LOCAL / "classpath.txt").read_text()])
    java_environment = clean_environment(home)
    rust_environment = {**os.environ, "CARGO_BUILD_JOBS": "2",
                        "CARGO_TARGET_DIR": str(LOCAL / "rust-target"), "TMPDIR": str(home)}
    manifest = "runelite/integration-tests/service-probe/Cargo.toml"
    checks = [
        ("synthetic_java_codec_projection", [
            str(args.java_home.resolve() / "bin/java"), "-ea", "-Xmx256m", "-XX:ActiveProcessorCount=2",
            f"-Duser.home={home}", f"-Djava.io.tmpdir={home}", "-cp", classpath, "ProjectionChecks"
        ], java_environment),
        ("synthetic_java_shop_identity", [
            str(args.java_home.resolve() / "bin/java"), "-ea", "-Xmx256m", "-XX:ActiveProcessorCount=2",
            f"-Duser.home={home}", f"-Djava.io.tmpdir={home}", "-cp", classpath, "ShopIdentityChecks"
        ], java_environment),
        ("python_harness", [
            "python3", "-m", "unittest", "discover", "-s", "tools/runelite-compatibility", "-p", "test_*.py", "-q"
        ], rust_environment),
        ("public_service_probe_clippy", [
            "cargo", "clippy", "--quiet", "--locked", "--offline", "--manifest-path", manifest, "--", "-D", "warnings"
        ], rust_environment),
        ("public_service_probe_format", [
            "cargo", "fmt", "--check", "--manifest-path", manifest
        ], rust_environment),
        ("shared_shop_wire_and_v1_hash_goldens", [
            "cargo", "test", "--quiet", "--locked", "--offline", "-p", "clubscape-protocol",
            "-p", "clubscape-server", "--test", "shop_identity", "--test", "shop_intent_compatibility"
        ], rust_environment),
    ]
    records = []
    for name, command, environment in checks:
        result = subprocess.run(command, cwd=ROOT, env=environment, text=True,
                                capture_output=True, timeout=180, check=False)
        records.append({"name": name, "command": command, "exit_code": result.returncode,
                        "stdout": result.stdout, "stderr": result.stderr})
    report = {"schema_version": 1, "checks": records, "compatibility_verified": False,
              "all_small_checks_passed": all(record["exit_code"] == 0 for record in records)}
    report_path.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"all_small_checks_passed": report["all_small_checks_passed"],
                      "checks": [{"name": r["name"], "exit_code": r["exit_code"]} for r in records],
                      "live_compatibility_proof": False}))
    raise SystemExit(0 if report["all_small_checks_passed"] else 1)
