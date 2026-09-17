"""Explicit second allocation; never rewrite the original three-attempt experiment."""
import hashlib
import json
from pathlib import Path
import subprocess

from prepare import ROOT

AUTHORITY = ROOT / "milestones/m1-runelite-live-renewal.json"
AUTHORITY_SHA256 = "f2563301e272cf3f95608be247b30d71326c337e45c5c51f12b5727d34e3f13b"
LEDGER = ROOT / "research/runelite-feasibility/live-renewal.json"
ORIGINAL_FILES = [
    "research/runelite-feasibility/experiment.json",
    "research/runelite-feasibility/assessment.json",
]
EXPECTED_BOUNDS = {
    "additional_live_invocations": 2, "global_invocation_numbers": [4, 5],
    "total_live_invocations_maximum": 5, "total_materially_distinct_architectures_maximum": 2,
    "live_attempt_timeout_seconds": 600, "jvm_heap_mib": 3072, "jvm_active_processors": 2,
    "rust_build_jobs": 2, "database_cpus": 2, "database_memory_mib": 512, "nested_agents": 0,
}


def hash_bytes(data):
    return hashlib.sha256(data).hexdigest()


def validate_bounds(authority, phase, attempt, original):
    if authority.get("task") != "M1-RUNELITE-FEASIBILITY" or authority.get("decision") != "admitted_bounded_followup":
        raise ValueError("Not the reserved task's explicit renewal")
    if authority.get("followup_bounds") != EXPECTED_BOUNDS:
        raise ValueError("Renewal bounds differ from the explicitly approved two-invocation allocation")
    prior = original.get("live_attempts", [])
    if [a["number"] for a in prior] != [1, 2, 3]:
        raise ValueError("Do not reset or renumber the original allocation")
    calls = phase.get("invocations", [])
    if [a["number"] for a in calls] != [4, 5][:len(calls)] or len(calls) >= 2:
        raise ValueError("Renewed allocation exhausted or ledger sequence invalid")
    if any(a.get("status") == "passed" for a in calls):
        raise ValueError("A complete tuple already passed; stop rather than consume another invocation")
    if attempt != 4 + len(calls):
        raise ValueError("Use only the next admitted global invocation, 4 then 5")
    architectures = {a["architecture"] for a in [*prior, *calls]} | {"A"}
    if len(architectures) > 2:
        raise ValueError("The overall architecture bound is exhausted")


def original_integrity(authority):
    revision = authority["prior_allocation"]["record_revision"]
    records = []
    for path in ORIGINAL_FILES:
        expected = subprocess.check_output(["git", "show", f"{revision}:{path}"], cwd=ROOT)
        actual = (ROOT / path).read_bytes()
        if expected != actual:
            raise ValueError(f"Original evidence changed: {path}")
        records.append({"path": path, "sha256": hash_bytes(actual)})
    return records


def load_allocation(path, attempt, artifact_hash):
    if path.resolve() != AUTHORITY or hash_bytes(AUTHORITY.read_bytes()) != AUTHORITY_SHA256:
        raise ValueError("Only the exact committed renewal authority is admitted")
    authority = json.loads(AUTHORITY.read_text())
    if artifact_hash != authority["prerequisites"]["source_artifact_sha256"]:
        raise ValueError("Renewal is pinned to the existing canonical df3e artifact")
    preserved = original_integrity(authority)
    original = json.loads((ROOT / ORIGINAL_FILES[0]).read_text())
    if LEDGER.exists():
        phase = json.loads(LEDGER.read_text())
        if phase["authority_sha256"] != AUTHORITY_SHA256 or phase["original_evidence"] != preserved:
            raise ValueError("Renewal provenance changed; do not reset the phase ledger")
    else:
        phase = {
            "schema_version": 1, "task": authority["task"],
            "authority_record": str(AUTHORITY.relative_to(ROOT)), "authority_sha256": AUTHORITY_SHA256,
            "authority_commit": "eec354c00d67d88686794085de2c8ecb31ef91df",
            "original_evidence": preserved, "original_invocations_consumed": 3,
            "bounds": EXPECTED_BOUNDS, "artifact_sha256": artifact_hash, "invocations": [],
            "compatibility_verified": False, "tier": None,
        }
    validate_bounds(authority, phase, attempt, original)
    return phase


def save(phase):
    for record in phase["original_evidence"]:
        if hash_bytes((ROOT / record["path"]).read_bytes()) != record["sha256"]:
            raise ValueError("Original evidence was modified during the renewed invocation")
    pending = LEDGER.with_suffix(".json.next")
    pending.write_text(json.dumps(phase, indent=2) + "\n")
    pending.replace(LEDGER)
