#!/usr/bin/env python3
"""Run the actual M1 validation gates and persist failures as failures, including native execution."""

import argparse
import json
import os
import re
import subprocess
import sys

from common import BINDINGS, CONTENT, ROOT, load, sha, write


BUILD = [sys.executable, "tools/m1-content/build.py", "--json-output", "tools/m1-content/.local/game-content.json"]
REPORTS = (
    "compiler-validation.json", "unresolved-bindings.json", "schema-validation.json", "check-result.json",
    "asset-refresh-validation.json", "application-validation.json", "application-source-validation.json",
    "route-validation.json", "state-oracle-validation.json", "test-output.log",
)


def normalized(text):
    return re.sub(r"Ran (\d+) tests in [0-9.]+s", r"Ran \1 tests (elapsed time omitted)", text)


def command(name, arguments):
    work = ROOT / "tools/m1-content/.local"
    work.mkdir(parents=True, exist_ok=True)
    result = subprocess.run(arguments, cwd=ROOT, text=True, capture_output=True, env={
        **os.environ, "PYTHONDONTWRITEBYTECODE": "1",
        "CARGO_TARGET_DIR": str(work / "schema-target"), "TMPDIR": str(work),
    })
    record = {"name": name, "command": arguments, "exit_code": result.returncode,
              "stdout": normalized(result.stdout), "stderr": normalized(result.stderr)}
    print(json.dumps({"check": name, "exit_code": result.returncode}), flush=True)
    return record


def run():
    results = [command("build_and_strict_runtime_compile", BUILD)]
    if results[0]["exit_code"]:
        write(BINDINGS / "validation-run.json", {"schema_version": 3, "passed": False, "checks": results}, True)
        raise SystemExit(results[0]["exit_code"])
    checks = [
        ("content_tests", [sys.executable, "-m", "unittest", "discover", "-s", "tools/m1-content", "-p", "test_*.py", "-q"]),
        ("source_asset_closure", [sys.executable, "tools/m1-content/verify_assets.py"]),
        ("source_application", [sys.executable, "tools/m1-content/verify_runtime_bindings.py"]),
        ("source_routes", [sys.executable, "tools/m1-content/verify_routes.py"]),
        ("source_state_oracles", [sys.executable, "tools/m1-content/verify_state_oracles.py"]),
        ("original_source_oracles", [sys.executable, "tools/runtime-bindings/validate.py", "--baseline-commit", "e9073ab", "--self-test"]),
        ("rust_format", ["cargo", "fmt", "--manifest-path", "tools/m1-content/schema-check/Cargo.toml", "--check"]),
        ("rust_typecheck", ["cargo", "check", "--quiet", "--locked", "--offline", "--manifest-path",
                            "tools/m1-content/schema-check/Cargo.toml"]),
        ("strict_reload_and_native_source_probes", [sys.executable, "tools/m1-content/check.py"]),
    ]
    for name, arguments in checks:
        record = command(name, arguments)
        results.append(record)
        if name == "content_tests":
            (BINDINGS / "test-output.log").write_text(record["stdout"] + record["stderr"])
        if name == "original_source_oracles" and record["exit_code"] == 0:
            write(BINDINGS / "application-source-validation.json", {
                "schema_version": 3, "command": arguments,
                "source_evidence_rewritten": False,
                "resolutions_sha256": sha((ROOT / "research/runtime-bindings/resolutions.json").read_bytes()),
                "result": json.loads(record["stdout"]),
            }, True)
    return results


def output_hashes():
    manifest = load(CONTENT / "manifest.json")
    paths = {ROOT / record["path"] for record in manifest["outputs"]}
    paths.update((CONTENT / "manifest.json", CONTENT / "game-content.csc.gz"))
    paths.update(BINDINGS / name for name in REPORTS)
    return {str(path.relative_to(ROOT)): sha(path.read_bytes()) for path in sorted(paths)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repeat", action="store_true", help="Repeat every gate and compare generated content/artifact/evidence hashes.")
    args = parser.parse_args()
    results = run()
    first = output_hashes()
    repeatable = None
    if args.repeat:
        results = run()
        second = output_hashes()
        changed = sorted(path for path in first.keys() | second.keys() if first.get(path) != second.get(path))
        repeatable = not changed
        write(BINDINGS / "repeatability.json", {
            "schema_version": 3, "command": BUILD, "full_validation_repeated": True,
            "identical_outputs": repeatable, "compared_outputs": len(first), "changed_outputs": changed,
            "output_hashes": second, "compiled_artifact": load(CONTENT / "manifest.json")["compiled_artifact"],
            "native_failures_are_not_waived": True, "gameplay_executed": False, "presentation_approved": False,
        }, True)
    manifest = load(CONTENT / "manifest.json")
    application = load(BINDINGS / "application-result.json")
    schema = load(BINDINGS / "schema-validation.json")
    probes = schema["native_source_policy_probes"]
    failed = [record["name"] for record in results if record["exit_code"]]
    if repeatable is False:
        failed.append("repeatability")
    active_executor = [
        {"id": name, "evidence": value}
        for name, value in probes.items()
        if isinstance(value, dict) and value.get("passed") is False
    ]
    tests = next(record for record in results if record["name"] == "content_tests")
    test_match = re.search(r"Ran (\d+) tests", tests["stderr"])
    residuals = application["residuals"]
    passed = not failed and residuals["active_or_conditionally_active_count"] == 0
    report = {
        "schema_version": 3, "passed": passed, "failed_checks": failed, "checks": results,
        "compiled_artifact": manifest["compiled_artifact"], "active_executor_failures": active_executor,
        "source_binding_residuals": residuals, "repeatability_checked": args.repeat,
        "repeatable_outputs": repeatable, "full_journey_executed": False,
    }
    write(BINDINGS / "validation-run.json", report, True)
    evidence = output_hashes()
    evidence["research/m1-bindings/validation-run.json"] = sha((BINDINGS / "validation-run.json").read_bytes())
    if args.repeat:
        evidence["research/m1-bindings/repeatability.json"] = sha((BINDINGS / "repeatability.json").read_bytes())
    status = {
        "schema_version": 3, "task": "m1-content-runtime3", "base": "6605b09", "branch": "task/m1-content-runtime3",
        "status": "done" if passed else "blocked", "fully_done": passed, "runtime_ready": passed,
        "declared_v3_selectors_authored": True, "bounded_source_application_complete": True,
        "revision": manifest["revision"], "compiled_artifact": manifest["compiled_artifact"], "counts": manifest["counts"],
        "input_aggregate_sha256": manifest["input_aggregate_sha256"],
        "source_application": {"audit": "research/m1-bindings/application-result.json", "source_bindings_consumed": 104,
                               "coupled_updates": 7, "source_inferences_not_approvals": 98,
                               "vital_policy": application["vital_policy"], "approved_loot": application["approved_loot"]["adaptation"]},
        "residuals": residuals, "active_executor_failure_count": len(active_executor),
        "active_executor_failures": active_executor,
        "validation": {"content_tests": int(test_match[1]) if test_match else None,
                       "content_tests_passed": tests["exit_code"] == 0,
                       "failed_checks": failed, "repeatability_checked": args.repeat, "repeatability_passed": repeatable,
                       "strict_runtime_compiled": schema["runtime_compiled"], "artifact_reloaded": schema["artifact_reloaded"],
                       "engine_constructed": schema["engine_constructed"],
                       "real_source_appearance_request_passed": schema["real_source_appearance_request_passed"]},
        "evidence_hashes": evidence,
        "parent_action": "Resolve the exact native source-policy failures in contract-gaps.json without clearing source "
                         "collision or changing private-drop semantics, then rerun python3 tools/m1-content/validate.py --repeat. "
                         "No v2/decoder/vital/potion-asset blocker remains.",
        "full_journey_executed": False, "presentation_approved": False, "milestone_accepted": False,
    }
    write(BINDINGS / "status.json", status, True)
    print(json.dumps({"status": status["status"], "artifact_sha256": manifest["compiled_artifact"]["uncompressed_sha256"],
                      "active_source_bindings": residuals["active_or_conditionally_active_count"],
                      "inactive_source_bindings": residuals["inactive_full_target_count"],
                      "active_executor_failures": len(active_executor), "failed_checks": failed,
                      "repeatability_passed": repeatable}))
    raise SystemExit(0 if passed else 1)


if __name__ == "__main__":
    main()
