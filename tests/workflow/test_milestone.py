import copy
import hashlib
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("milestone", ROOT / "tools/milestone.py")
milestone = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(milestone)


class MilestoneTests(unittest.TestCase):
    def setUp(self):
        self.state = milestone.load_json(ROOT / "milestones/m1-starter-journey.json")
        self.tasks = milestone.load_json(ROOT / "milestones/m1-tasks.json")
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.addCleanup(self.temp.cleanup)

    def write_record(self, name, record):
        raw = json.dumps(record, sort_keys=True).encode()
        (self.root / name).write_bytes(raw)
        return {"path": name, "sha256": hashlib.sha256(raw).hexdigest()}

    def accepted_fixture(self):
        revision = "1" * 40
        self.state.update(status="accepted", candidate_revision=revision, accepted_revision=revision)
        self.tasks["agent_budget"].update(active=[], reserved=[])
        for gate in self.state["gates"]:
            record = {"gate": gate["id"], "result": "passed", "revision": revision}
            if gate["id"] == "reference-pack":
                record["reference_pack_sha256"] = "a" * 64
            if gate["id"] == "benchmark-contract":
                record["benchmark_contract_sha256"] = "b" * 64
            if gate["id"].endswith("integrated-gpu-performance"):
                record.update(
                    hardware={
                        "gpu_kind": "integrated", "gpu_model": "synthetic unit fixture",
                        "cpu": "unit fixture", "memory": "16 GiB", "os": "unit fixture",
                        "driver": "unit fixture",
                    },
                    browser={
                        "name": "Chrome" if gate["id"].startswith("chrome-") else "Edge",
                        "version": "synthetic unit fixture",
                    },
                    viewport=[1920, 1080], backend="WebGPU", measured_rendered_fps=60,
                    window_seconds=120, frame_time_p95_ms=16, frame_time_p99_ms=20,
                    max_stall_ms=30, workloads=["tutorial", "lumbridge"],
                    reference_pack_sha256="a" * 64, benchmark_contract_sha256="b" * 64,
                )
            gate.update(
                status="verified", blocker=None,
                evidence=[self.write_record(gate["id"] + ".json", record)],
            )
        for field, kind in [
            ("reference_pack_approval", "reference_pack"), ("presentation_acceptance", "presentation")
        ]:
            self.state[field] = self.write_record(field + ".json", {
                "authority": "owner", "decision": "approved", "kind": kind,
                "recorded_at": "2026-09-13T00:00:00Z", "owner_record": "Synthetic unit fixture only",
                "reference_pack_sha256": "a" * 64, "revision": revision,
                "evidence_bundle_sha256": milestone.evidence_bundle_hash(self.state),
            })

    def test_current_checkpoint_is_structurally_valid_but_not_accepted(self):
        self.assertEqual(milestone.structure_errors(self.state, self.tasks), [])
        self.assertTrue(milestone.acceptance_errors(self.state, self.tasks, self.root, False))

    def test_synthetic_complete_record_needs_a_real_preserved_commit_in_live_mode(self):
        self.accepted_fixture()
        self.assertEqual(milestone.acceptance_errors(self.state, self.tasks, self.root, False), [])
        self.assertIn(
            "The candidate build is not preserved as a local Git commit.",
            milestone.acceptance_errors(self.state, self.tasks, self.root, True),
        )
        self.assertTrue(milestone.record_errors(self.state, self.tasks, self.root))

    def test_removing_or_duplicating_a_required_gate_is_rejected(self):
        for change in ("remove", "duplicate"):
            state = copy.deepcopy(self.state)
            if change == "remove":
                state["gates"].pop()
            else:
                state["gates"].append(copy.deepcopy(state["gates"][0]))
            self.assertTrue(milestone.structure_errors(state, self.tasks))

    def test_passing_labels_without_evidence_and_approvals_do_not_pass(self):
        for gate in self.state["gates"]:
            gate.update(status="verified", blocker=None, evidence=[])
        errors = milestone.acceptance_errors(self.state, self.tasks, self.root, False)
        self.assertTrue(any("require evidence" in error for error in errors))
        self.assertIn("Missing owner presentation_acceptance.", errors)

    def test_account_foundations_cannot_substitute_for_browser_gameplay_evidence(self):
        self.accepted_fixture()
        gate = next(g for g in self.state["gates"] if g["id"] == "real-ui-account-creation")
        gate["evidence"] = [self.write_record("foundations.json", {
            "kind": "account_foundations", "result": "passed", "revision": "1" * 40,
            "gameplay_verified": False, "browser_signup_verified": False,
            "milestone_accepted": False,
        })]
        errors = milestone.acceptance_errors(self.state, self.tasks, self.root, False)
        self.assertTrue(any("Evidence must report this exact gate" in error for error in errors))

    def test_agent_self_approval_stale_build_and_changed_evidence_fail(self):
        for mutation in ("agent", "stale_build", "bundle"):
            self.accepted_fixture()
            approval = milestone.evidence_record(self.root, self.state["presentation_acceptance"])
            if mutation == "agent":
                approval["authority"] = "implementation-agent"
            elif mutation == "stale_build":
                approval["revision"] = "2" * 40
            else:
                approval["evidence_bundle_sha256"] = "c" * 64
            self.state["presentation_acceptance"] = self.write_record("owner.json", approval)
            self.assertTrue(milestone.acceptance_errors(self.state, self.tasks, self.root, False))

    def test_hash_and_path_integrity_are_enforced(self):
        reference = self.write_record("evidence.json", {"result": "passed"})
        (self.root / "evidence.json").write_text("changed")
        with self.assertRaisesRegex(ValueError, "integrity"):
            milestone.evidence_record(self.root, reference)
        with self.assertRaisesRegex(ValueError, "inside"):
            milestone.evidence_record(self.root, {"path": "../outside.json", "sha256": "0" * 64})
        with self.assertRaisesRegex(ValueError, "objects"):
            milestone.evidence_record(self.root, "I ran all tests")

    def test_dedicated_gpu_lower_fps_wrong_browser_or_window_cannot_pass(self):
        self.accepted_fixture()
        gate = next(g for g in self.state["gates"] if g["id"] == "chrome-integrated-gpu-performance")
        record = milestone.evidence_record(self.root, gate["evidence"][0])
        for field, value in [
            ("measured_rendered_fps", 59.9), ("measured_rendered_fps", True),
            ("measured_rendered_fps", float("nan")), ("viewport", [1280, 720]),
            ("window_seconds", 0), ("backend", "software"),
        ]:
            changed = copy.deepcopy(record)
            changed[field] = value
            self.assertTrue(milestone.performance_errors(changed, "Chrome"))
        for kind in ("dedicated", "software"):
            changed = copy.deepcopy(record)
            changed["hardware"]["gpu_kind"] = kind
            self.assertTrue(milestone.performance_errors(changed, "Chrome"))
        self.assertTrue(milestone.performance_errors(record, "Edge"))

    def test_no_automatic_next_milestone_or_early_presentation(self):
        self.state["later_milestone_authorized"] = True
        self.assertTrue(milestone.structure_errors(self.state, self.tasks))
        self.state["later_milestone_authorized"] = False
        self.state["reference_pack_approval"] = None
        next(task for task in self.tasks["tasks"] if task["id"] == "M1-PRESENTATION")["status"] = "IMPLEMENTING"
        self.assertTrue(milestone.structure_errors(self.state, self.tasks))

    def test_dangling_or_agent_reference_approval_cannot_start_presentation(self):
        self.state["reference_pack_approval"] = {"path": "missing.json", "sha256": "a" * 64}
        self.assertTrue(milestone.reference_approval_errors(self.state, self.root))
        self.accepted_fixture()
        self.assertEqual(milestone.reference_approval_errors(self.state, self.root), [])
        record = milestone.evidence_record(self.root, self.state["reference_pack_approval"])
        record["authority"] = "agent"
        self.state["reference_pack_approval"] = self.write_record("reference-owner.json", record)
        self.assertTrue(milestone.reference_approval_errors(self.state, self.root))

    def test_task_cycles_unknown_dependencies_and_oversubscription_fail(self):
        original = copy.deepcopy(self.tasks)
        self.tasks["tasks"][0]["dependencies"] = [self.tasks["tasks"][0]["id"]]
        self.assertTrue(milestone.structure_errors(self.state, self.tasks))
        self.tasks = copy.deepcopy(original)
        self.tasks["tasks"][0]["dependencies"] = ["unknown"]
        self.assertTrue(milestone.structure_errors(self.state, self.tasks))
        self.tasks = copy.deepcopy(original)
        self.tasks["agent_budget"].update(
            reserved=[],
            active=[{"id": str(index), "task": "M1-TOOLING"} for index in range(26)],
        )
        self.assertTrue(milestone.structure_errors(self.state, self.tasks))

    def test_workers_must_be_parked_before_acceptance(self):
        self.accepted_fixture()
        self.tasks["agent_budget"]["active"] = [{"id": "worker", "task": "M1-ACCOUNTS"}]
        self.assertTrue(milestone.acceptance_errors(self.state, self.tasks, self.root, False))

    def test_malformed_record_types_produce_explicit_errors(self):
        self.assertTrue(milestone.structure_errors([], self.tasks))
        self.state["gates"][0]["id"] = []
        self.assertTrue(milestone.structure_errors(self.state, self.tasks))


if __name__ == "__main__":
    unittest.main()
