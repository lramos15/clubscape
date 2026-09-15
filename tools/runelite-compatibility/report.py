#!/usr/bin/env python3
"""Join the bounded evidence into a compact assessment; never launch or fabricate a live run."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import shutil
import struct
import subprocess
import zipfile

from prepare import ROOT, LOCAL, digest

RESEARCH = ROOT / "research/runelite-feasibility"
NAMES = ["preflight-a1", "preflight-a2", "preflight-a3", "preflight-a4", "preflight-a5",
         "live-a1", "live-a2", "live-a3"]


def read(path):
    return json.loads(path.read_text())


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def main():
    inputs = read(RESEARCH / "build-inputs.json")
    libraries = {record["name"]: record for record in inputs["libraries"]}
    experiment = read(RESEARCH / "experiment.json")
    reports = {name: read(RESEARCH / (name + ".json")) for name in NAMES}
    if len(experiment["live_attempts"]) != 3 or any(reports[name]["exit_code"] != 1 for name in NAMES[-3:]):
        raise ValueError("This assessment must describe the actual recorded three failed invocations")
    validation = read(RESEARCH / "validation.json")
    if not validation["all_small_checks_passed"]:
        raise ValueError("Owned targeted validation is incomplete")
    evidence = RESEARCH / "evidence"
    evidence.mkdir(exist_ok=True)
    inventory = []
    for name in NAMES:
        directory = LOCAL / name
        for filename in ["runtime.log", "events.jsonl", "server.log", "xvfb.log"]:
            source = directory / filename
            if not source.exists():
                continue
            if source.stat().st_size > 128 * 1024:
                raise ValueError("Unexpectedly large text evidence; review before publication")
            target = evidence / (name + "-" + filename)
            shutil.copyfile(source, target)
            inventory.append({"path": str(target.relative_to(ROOT)), "size_bytes": target.stat().st_size,
                              "sha256": digest(target), "classification": reports[name]["mode"]})
        for source in sorted(directory.glob("*.png")):
            data = source.read_bytes()
            if data[:8] != b"\x89PNG\r\n\x1a\n":
                raise ValueError("Invalid image artifact")
            width, height = struct.unpack(">II", data[16:24])
            inventory.append({
                "path": str(source.relative_to(ROOT)), "size_bytes": len(data), "sha256": digest(source),
                "width": width, "height": height,
                "classification": "developer-only offline preflight; never live compatibility evidence",
                "committed": False,
            })
    write(RESEARCH / "evidence-inventory.json", {"schema_version": 1, "files": inventory})
    final_events = [json.loads(line) for line in
                    (LOCAL / "preflight-a5/events.jsonl").read_text().splitlines()]
    event = {value["kind"]: value for value in final_events}
    if reports["preflight-a5"]["native"]["exit_code"] != 0:
        raise ValueError("Final offline preflight did not pass")
    if event["actual_window_pixel_readback"]["equal_native_pixels"] != event["actual_window_pixel_readback"]["total_pixels"]:
        raise ValueError("Final actual-window match was not exact")
    if event["developer_preflight_result"]["server_connected"] or event["developer_preflight_result"]["xp_events_posted"]:
        raise ValueError("Do not relabel an offline preflight as actual live events")
    with zipfile.ZipFile(LOCAL / "libraries/client-1.12.38.jar") as jar:
        plugin_class_hash = hashlib.sha256(jar.read(
            "net/runelite/client/plugins/xptracker/XpTrackerPlugin.class")).hexdigest()
    cleanup = []
    for name in ["live-a2", "live-a3"]:
        report = reports[name]
        container = report["database"]["name"]
        remaining = subprocess.check_output([
            "docker", "ps", "--all", "--filter", f"name=^/{container}$", "--format", "{{.ID}}"
        ], text=True).strip()
        if remaining or report["database"]["cleanup_exit_code"] != 0:
            raise ValueError("Owned integration database was not removed")
        if (LOCAL / name / "postgres-password").exists():
            raise ValueError("Owned credential file was not removed")
        cleanup.append({"attempt": name, "container": container, "container_absent": True,
                        "credential_file_removed": True, "server_exit_code": report["server"]["exit_code"]})
    write(RESEARCH / "cleanup.json", {"schema_version": 1, "owned_services": cleanup,
                                    "jvm_xvfb": "Owned Popen processes waited/stopped in each recorded runner's finally block"})
    summaries = [
        {"number": 1, "exit_code": 1, "phase": "harness_before_services",
         "result": "Path.open(opener=...) TypeError; corrected and covered by private-file unit test",
         "actual_http_connection": False},
        {"number": 2, "exit_code": 1, "phase": "real_http_hello",
         "result": "Actual original runtime reached real PostgreSQL-backed HTTP Hello, then refused gameplay-unavailable readiness",
         "actual_http_connection": True, "authenticated_game_world": False},
        {"number": 3, "exit_code": 1, "phase": "strict_source_service_startup",
         "result": "Existing public Config::with_game_root and unchanged Service::bind rejected full source descriptor: game_file_size",
         "actual_http_connection": False},
    ]
    experiment["architectures"][0]["status"] = "implemented; original-runtime offline preflight passed; live tuple unverified"
    experiment["architectures"][1]["status"] = "not executed; client lifecycle interception cannot solve reproduced shared service prerequisites"
    experiment["live_attempts"][0]["hypothesis"] = "The owned integration harness can start isolated real services and execute the original-runtime adapter."
    experiment["live_attempts"][0]["diagnostic_result"] = summaries[0]["result"]
    experiment["live_attempts"][1]["hypothesis"] = "The documented game-root-only environment launch activates the actual authoritative game service."
    experiment["live_attempts"][1]["diagnostic_result"] = summaries[1]["result"]
    experiment["live_attempts"][2]["diagnostic_result"] = summaries[2]["result"]
    experiment["assessment_complete"] = True
    experiment["live_invocation_bound_exhausted"] = True
    experiment["stop_state"] = "parked after three conservatively counted invocations; no fourth run or unapproved later milestone"
    write(RESEARCH / "experiment.json", experiment)
    capacity = read(RESEARCH / "strict-compiler-capacity.json")
    assessment = {
        "schema_version": 1, "task": experiment["task"], "base_commit": experiment["base_commit"],
        "assessment_complete": True, "compatibility_verified": False, "tier": None,
        "desktop_deferral_approved": False, "native_client_substitution_authorized": False,
        "browser_headless_acceptance_claimed": False,
        "bounds": {"architectures_maximum": 2, "architectures_executed": 1,
                   "live_invocations_maximum": 3, "numbered_invocations_executed": 3,
                   "first_invocation_failed_before_services_and_is_conservatively_counted": True,
                   "actual_http_invocations": 1, "offline_native_preflights": 5,
                   "nested_agents": 0, "exhausted": True},
        "tuple": {
            "runtime": libraries["client-1.12.38.jar"],
            "injected_client": libraries["injected-client-1.12.38.jar"],
            "api": libraries["runelite-api-1.12.38-runtime.jar"],
            "plugin": {"name": "XP Tracker", "class": "net.runelite.client.plugins.xptracker.XpTrackerPlugin",
                       "class_sha256": plugin_class_hash, "modified": False},
            "revision_before_original_init": event["original_init"]["before"],
            "revision_after_original_init": event["original_init"]["after"],
            "opaque_build_id": event["original_init"]["build_id"],
            "cache_id": 2695,
            "cache_file_manifest_sha256": digest(ROOT / "research/current-source/cache-files.json"),
            "decoder": libraries["cache-1.12.39-SNAPSHOT.jar"],
            "decoder_revision": "ac79ed8bd8926bec7bf172aa291574b4d944b0e7",
            "jdk": inputs["jdk"], "protoc": inputs["protoc"],
            "launcher_used": False, "entry": "RuneLiteComposition with real RuneLiteModule/ClientUI/PluginManager/Hooks",
            "upstream_bytecode_patches": [],
        },
        "verified_separately": {
            "original_runtime_ui_plugin_offline": True,
            "native_scene_offline": event["native_scene_loaded"],
            "original_penguin_offline": event["native_player"],
            "original_startup_canvas": event["native_canvas_binding"],
            "native_actor_visibility_offline": event["native_actor_visibility"],
            "actual_window_readback_offline": event["actual_window_pixel_readback"],
            "offline_xp_callbacks_posted": 0,
            "real_postgres_http_hello": True,
            "strict_compiler_capacity": capacity,
            "strict_service_rejection": "game_content/game_file_size",
        },
        "unverified": [
            "Actual ClubScape signup/login from this runtime",
            "Authoritative game-world join and player projections",
            "Live source scene/penguin reflecting authoritative movement or events",
            "Genuine XP Tracker gain over committed server XP events",
            "Any support tier, other plugin, complete RuneLite entry lifecycle, full native HUD or broad API coverage",
        ],
        "invocations": summaries,
        "required_director_extensions": [
            "Move game-root environment parsing outside the optional web-root branch, with subprocess configuration tests",
            "Increase the bounded game descriptor capacity to fit the real source pack, retaining all strict guards and adding actual source startup coverage",
        ],
        "recommendation": {
            "action": "Continue narrowly after both shared prerequisites are fixed, pending owner review and renewed bounded M1 integration authorization",
            "status": "proposed; not automatic execution or approved deferral",
            "shared_fixes_estimated_engineer_days": [0.5, 1],
            "first_xp_integration_estimated_engineer_days": [2, 5],
            "untested_lifecycle_interception_alternative_estimated_engineer_days": [5, 10],
            "costs_are_estimates_not_success_or_deadline_claims": True,
        },
        "validation": {"final_java_compile_exit": 0, "synthetic_java_checks": 9,
                       "python_harness_tests": 3, "owned_rust_probe_build_exit": 0,
                       "owned_rust_probe_clippy_exit": 0, "owned_rust_probe_rustfmt_exit": 0,
                       "final_native_render_errors": reports["preflight-a5"]["native"]["native_render_errors"]},
        "validation_commands": "research/runelite-feasibility/validation.json",
        "evidence_inventory": "research/runelite-feasibility/evidence-inventory.json",
        "review": "research/runelite-feasibility/review.md",
        "cleanup": "research/runelite-feasibility/cleanup.json",
        "stop_state": "Bound exhausted; preserving evidence and parking. No scheduled work, new native client, or later milestone.",
    }
    write(RESEARCH / "assessment.json", assessment)
    print(json.dumps({"assessment_complete": True, "compatibility_verified": False, "tier": None,
                      "invocations": 3, "architectures_executed": 1, "evidence_files": len(inventory)}))


if __name__ == "__main__":
    main()
