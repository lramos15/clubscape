#!/usr/bin/env python3
"""Validate checkpoint integrity without granting approvals or running successors."""

import argparse
import hashlib
import json
import math
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REQUIRED_GATES = (
    "reference-baseline",
    "reference-pack",
    "benchmark-contract",
    "early-presentation-benchmark",
    "real-ui-account-creation",
    "complete-tutorial",
    "legitimate-lumbridge-arrival",
    "lumbridge-activities",
    "death-and-recovery",
    "complete-cooks-assistant",
    "shared-authoritative-headless-path",
    "tutorial-and-quest-persistence",
    "content-compilation-and-stable-ids",
    "asset-manifest-validation",
    "interface-controls-and-resizing",
    "capability-feedback",
    "migration-and-extension-validation",
    "chrome-fresh-account-journey",
    "edge-fresh-account-journey",
    "source-visual-comparisons",
    "runtime-animation-evidence",
    "runtime-audio-evidence",
    "chrome-integrated-gpu-performance",
    "edge-integrated-gpu-performance",
    "independent-style-review",
    "independent-technical-review",
    "security-and-recovery",
    "owner-presentation-acceptance",
)
GATE_STATES = {"not_started", "in_progress", "implemented", "verified", "blocked", "deferred"}
TASK_STATES = {
    "BLOCKED", "READY", "CLAIMED", "IMPLEMENTING", "VALIDATING",
    "REVIEW", "INTEGRATION", "DONE", "FAILED", "PARKED",
}
INPUT_GATES = {"reference-baseline", "reference-pack", "benchmark-contract"}
REVISION = re.compile(r"[0-9a-f]{40}")
HASH = re.compile(r"[0-9a-f]{64}")


def load_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def evidence_record(root, reference):
    if not isinstance(reference, dict):
        raise ValueError("Evidence references must be objects, not claims or prose.")
    relative = reference.get("path")
    expected = reference.get("sha256")
    if not isinstance(relative, str) or not isinstance(expected, str) or not HASH.fullmatch(expected):
        raise ValueError("Evidence requires a relative path and exact SHA-256.")
    path = (root / relative).resolve()
    if Path(relative).is_absolute() or not path.is_relative_to(root.resolve()):
        raise ValueError("Evidence paths must remain inside the repository.")
    if not path.is_file() or path.stat().st_size > 5 * 1024 * 1024:
        raise ValueError(f"Missing or oversized evidence record: {relative}")
    raw = path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != expected:
        raise ValueError(f"Evidence integrity mismatch: {relative}")
    record = json.loads(raw)
    if not isinstance(record, dict):
        raise ValueError(f"Evidence record must be a JSON object: {relative}")
    return record


def structure_errors(state, tasks):
    errors = []
    if not isinstance(state, dict) or not isinstance(tasks, dict):
        return ["Milestone and task records must be objects."]
    for name, value in [("milestone", state), ("tasks", tasks)]:
        if value.get("schema_version") != 1 or value.get("milestone") != "m1-starter-journey":
            errors.append(f"{name}: unsupported schema or milestone identity")
    if state.get("status") not in {"in_progress", "blocked", "accepted"}:
        errors.append("Unknown milestone status.")
    if state.get("later_milestone_authorized") is not False:
        errors.append("M1 cannot authorize a later milestone.")
    scope = state.get("scope_approval")
    if not isinstance(scope, dict) or scope.get("authority") != "owner" or not scope.get("record"):
        errors.append("The current milestone requires its owner scope-approval record.")
    gates = state.get("gates")
    if not isinstance(gates, list) or any(not isinstance(gate, dict) for gate in gates):
        return errors + ["Gates must be a list of objects."]
    gate_ids = [gate.get("id") for gate in gates]
    if any(not isinstance(identifier, str) for identifier in gate_ids):
        return errors + ["Every gate requires a string ID."]
    if Counter(gate_ids) != Counter(REQUIRED_GATES):
        errors.append("The complete required M1 gate set must appear exactly once.")
    for gate in gates:
        if gate.get("status") not in GATE_STATES:
            errors.append(f"{gate['id']}: invalid gate state")
        if not isinstance(gate.get("evidence"), list):
            errors.append(f"{gate['id']}: evidence must be a list")
        if gate.get("status") == "blocked" and not gate.get("blocker"):
            errors.append(f"{gate['id']}: blocked state requires a reason")

    entries = tasks.get("tasks")
    if not isinstance(entries, list) or any(not isinstance(task, dict) for task in entries):
        return errors + ["Tasks must be a list of objects."]
    identifiers = [task.get("id") for task in entries]
    if any(not isinstance(identifier, str) for identifier in identifiers):
        return errors + ["Every task requires a string ID."]
    if len(identifiers) != len(set(identifiers)):
        errors.append("Task IDs must be unique.")
    by_id = {task["id"]: task for task in entries}
    for task in entries:
        if task.get("status") not in TASK_STATES:
            errors.append(f"{task['id']}: invalid task state")
        for field in (
            "specs", "dependencies", "allowed_paths", "forbidden_paths", "outputs",
            "validation_commands", "test_requirements", "acceptance",
            "reserved_namespaces", "source_notes", "parity_entries", "adaptations", "evidence",
        ):
            if not isinstance(task.get(field), list):
                errors.append(f"{task['id']}: missing list field {field}")
        dependencies = task.get("dependencies", [])
        if not isinstance(dependencies, list) or any(not isinstance(dep, str) for dep in dependencies):
            errors.append(f"{task['id']}: dependency IDs must be strings")
            continue
        for dependency in dependencies:
            if dependency not in by_id:
                errors.append(f"{task['id']}: unknown dependency {dependency}")
        if task["id"] == "M1-PRESENTATION" and task.get("status") not in {"BLOCKED", "PARKED"}:
            if state.get("reference_pack_approval") is None:
                errors.append("Presentation cannot start without owner reference-pack approval.")

    visiting, visited = set(), set()

    def visit(identifier):
        if identifier in visiting:
            errors.append(f"Task dependency cycle at {identifier}.")
            return
        if identifier in visited:
            return
        visiting.add(identifier)
        dependencies = by_id[identifier].get("dependencies", [])
        if isinstance(dependencies, list):
            for dependency in dependencies:
                if isinstance(dependency, str) and dependency in by_id:
                    visit(dependency)
        visiting.remove(identifier)
        visited.add(identifier)

    for identifier in by_id:
        visit(identifier)

    budget = tasks.get("agent_budget", {})
    if not isinstance(budget, dict):
        return errors + ["Agent budget must be an object."]
    if budget.get("max_active_ai_agents") != 25:
        errors.append("The owner-approved project-wide agent ceiling is exactly 25.")
    limit = budget.get("effective_limit")
    if type(limit) is not int or not 1 <= limit <= 25:
        errors.append("The effective agent limit must be between 1 and 25.")
    slots = []
    for key in ("active", "reserved", "queued"):
        values = budget.get(key)
        if not isinstance(values, list) or any(not isinstance(value, dict) for value in values):
            errors.append(f"Agent {key} must be a list of objects.")
        elif key != "queued":
            slots.extend(values)
    ids = [slot.get("id") for slot in slots]
    if any(not isinstance(identifier, str) for identifier in ids):
        errors.append("Agent IDs must be strings.")
    elif len(ids) != len(set(ids)):
        errors.append("An active/reserved worker cannot occupy duplicate slots.")
    if len(slots) > 25:
        errors.append("Active and reserved workers exceed the shared ceiling.")
    for slot in slots:
        if slot.get("task") not in by_id:
            errors.append("Each agent reservation must identify a recorded task.")
    if budget.get("nested_delegation_allowed") is not False:
        errors.append("Unbudgeted nested delegation is forbidden in this execution.")
    return errors


def evidence_bundle_hash(state):
    values = sorted(
        (
            {"id": gate["id"], "evidence": gate["evidence"]}
            for gate in state["gates"]
            if gate["id"] != "owner-presentation-acceptance"
        ),
        key=lambda gate: gate["id"],
    )
    return hashlib.sha256(json.dumps(values, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def reference_approval_errors(state, root):
    try:
        approval = evidence_record(root, state.get("reference_pack_approval"))
        if (
            approval.get("authority") != "owner"
            or approval.get("decision") != "approved"
            or approval.get("kind") != "reference_pack"
            or not approval.get("recorded_at")
            or not approval.get("owner_record")
        ):
            raise ValueError("Requires a dated, explicit owner reference-pack approval.")
        gate = next(gate for gate in state["gates"] if gate["id"] == "reference-pack")
        if gate["status"] != "verified" or not gate["evidence"]:
            raise ValueError("Reference-pack approval requires its verified, nonempty input gate.")
        for reference in gate["evidence"]:
            record = evidence_record(root, reference)
            pack_hash = record.get("reference_pack_sha256")
            if (
                record.get("gate") != "reference-pack"
                or record.get("result") != "passed"
                or not isinstance(pack_hash, str)
                or not HASH.fullmatch(pack_hash)
                or approval.get("reference_pack_sha256") != pack_hash
            ):
                raise ValueError("Reference-pack approval and input evidence must identify the same pack.")
        return []
    except (OSError, ValueError, TypeError) as error:
        return [f"reference_pack_approval: {error}"]


def performance_errors(record, expected_browser):
    errors = []
    hardware = record.get("hardware", {})
    browser = record.get("browser", {})
    if not isinstance(hardware, dict) or not isinstance(browser, dict):
        return ["Performance evidence requires hardware and browser records."]
    if hardware.get("gpu_kind") != "integrated":
        errors.append("Dedicated/software graphics cannot pass integrated-GPU acceptance.")
    for field in ("gpu_model", "cpu", "memory", "os", "driver"):
        if not isinstance(hardware.get(field), str) or not hardware[field].strip():
            errors.append(f"Missing pinned hardware field: {field}")
    if browser.get("name") != expected_browser or not browser.get("version"):
        errors.append(f"Performance evidence must identify the tested {expected_browser} version.")
    if record.get("viewport") != [1920, 1080] or record.get("backend") != "WebGPU":
        errors.append("Performance requires 1920x1080 WebGPU.")
    fps = record.get("measured_rendered_fps")
    if type(fps) not in (int, float) or not math.isfinite(fps) or fps < 60:
        errors.append("Measured rendered FPS must attain the unchanged 60 FPS target.")
    for field in ("window_seconds", "frame_time_p95_ms", "frame_time_p99_ms", "max_stall_ms"):
        value = record.get(field)
        if type(value) not in (int, float) or not math.isfinite(value) or value <= 0:
            errors.append(f"Performance requires a positive measured {field}.")
    if record.get("workloads") != ["tutorial", "lumbridge"]:
        errors.append("Both representative tutorial and Lumbridge workloads are required.")
    for field in ("reference_pack_sha256", "benchmark_contract_sha256"):
        if not isinstance(record.get(field), str) or not HASH.fullmatch(record[field]):
            errors.append(f"Performance evidence must bind the frozen {field}.")
    return errors


def acceptance_errors(state, tasks, root=ROOT, verify_git=True):
    errors = structure_errors(state, tasks)
    if errors:
        return errors
    revision = state.get("candidate_revision")
    if not isinstance(revision, str) or not REVISION.fullmatch(revision):
        errors.append("No exact candidate build revision is recorded.")
    elif verify_git:
        result = subprocess.run(
            ["git", "cat-file", "-t", revision], cwd=root, capture_output=True, text=True, check=False
        )
        if result.returncode != 0 or result.stdout.strip() != "commit":
            errors.append("The candidate build is not preserved as a local Git commit.")
    if state.get("status") != "accepted" or state.get("accepted_revision") != revision or revision is None:
        errors.append("No accepted revision matching the reviewed candidate is recorded.")
    records = {}
    for gate in state["gates"]:
        identifier = gate["id"]
        if gate["status"] != "verified":
            errors.append(f"{identifier}: {gate['status']}")
            continue
        if not gate["evidence"]:
            errors.append(f"{identifier}: verified claims require evidence")
        for reference in gate["evidence"]:
            try:
                record = evidence_record(root, reference)
                if record.get("gate") != identifier or record.get("result") != "passed":
                    raise ValueError("Evidence must report this exact gate as passed.")
                if identifier not in INPUT_GATES and record.get("revision") != revision:
                    raise ValueError("Evidence belongs to a different or unknown candidate revision.")
                records[identifier] = record
                if identifier.endswith("integrated-gpu-performance"):
                    browser = "Chrome" if identifier.startswith("chrome-") else "Edge"
                    errors.extend(f"{identifier}: {error}" for error in performance_errors(record, browser))
            except (OSError, ValueError, TypeError) as error:
                errors.append(f"{identifier}: {error}")
    for field, kind in (
        ("reference_pack_approval", "reference_pack"),
        ("presentation_acceptance", "presentation"),
    ):
        reference = state.get(field)
        if reference is None:
            errors.append(f"Missing owner {field}.")
            continue
        try:
            approval = evidence_record(root, reference)
            if (
                approval.get("authority") != "owner"
                or approval.get("decision") != "approved"
                or approval.get("kind") != kind
                or not approval.get("recorded_at")
                or not approval.get("owner_record")
            ):
                raise ValueError("Requires a dated, explicit owner approval, not agent certification.")
            pack_hash = records.get("reference-pack", {}).get("reference_pack_sha256")
            if not isinstance(pack_hash, str) or not HASH.fullmatch(pack_hash):
                raise ValueError("The source reference pack identity is missing.")
            if approval.get("reference_pack_sha256") != pack_hash:
                raise ValueError("Approval does not cover this reference pack.")
            if kind == "presentation":
                if approval.get("revision") != revision:
                    raise ValueError("Owner acceptance belongs to a different build.")
                if approval.get("evidence_bundle_sha256") != evidence_bundle_hash(state):
                    raise ValueError("Owner acceptance does not cover the reviewed evidence bundle.")
        except (OSError, ValueError, TypeError) as error:
            errors.append(f"{field}: {error}")
    for identifier, record in records.items():
        if identifier.endswith("integrated-gpu-performance"):
            for field, input_gate, input_field in (
                ("reference_pack_sha256", "reference-pack", "reference_pack_sha256"),
                ("benchmark_contract_sha256", "benchmark-contract", "benchmark_contract_sha256"),
            ):
                if record.get(field) != records.get(input_gate, {}).get(input_field):
                    errors.append(f"{identifier}: measurement does not match the frozen {input_gate}")
    if tasks["agent_budget"]["reserved"] or any(
        slot["id"] != "director" for slot in tasks["agent_budget"]["active"]
    ):
        errors.append("Park or finish all workers and stop new admissions before acceptance.")
    return errors


def record_errors(state, tasks, root=ROOT):
    errors = structure_errors(state, tasks)
    if not errors and state.get("reference_pack_approval") is not None:
        errors.extend(reference_approval_errors(state, root))
    if not errors and state["status"] == "accepted":
        errors.extend(acceptance_errors(state, tasks, root))
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=["status", "validate", "check"])
    args = parser.parse_args()
    try:
        state = load_json(ROOT / "milestones/m1-starter-journey.json")
        tasks = load_json(ROOT / "milestones/m1-tasks.json")
        errors = record_errors(state, tasks)
        if errors:
            print(json.dumps({"record_valid": False, "errors": errors}, indent=2))
            return 1
        if args.operation == "validate":
            print(json.dumps({"record_valid": True, "milestone_accepted": state["status"] == "accepted"}))
            return 0
        failures = acceptance_errors(state, tasks)
        report = {
            "milestone": state["milestone"],
            "status": state["status"],
            "candidate_revision": state["candidate_revision"],
            "gate_counts": dict(Counter(gate["status"] for gate in state["gates"])),
            "milestone_accepted": not failures,
            "acceptance_blockers": failures,
            "later_milestone_authorized": False,
        }
        print(json.dumps(report, indent=2))
        return 1 if args.operation == "check" and failures else 0
    except (OSError, ValueError) as error:
        print(f"Checkpoint error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
