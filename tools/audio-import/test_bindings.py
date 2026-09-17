import gzip
import hashlib
import importlib.util
import json
from pathlib import Path
import unittest


RESEARCH = Path("research/audio-source")
MEDIA_AVAILABLE = importlib.util.find_spec("numpy") is not None and importlib.util.find_spec("av") is not None


class BindingEvidenceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.bindings = json.loads((RESEARCH / "bindings.json").read_text())
        cls.native = json.loads(gzip.decompress((RESEARCH / "binding-native-evidence.json.gz").read_bytes()))
        cls.public = json.loads(gzip.decompress((RESEARCH / "binding-public-evidence.json.gz").read_bytes()))

    def test_bound_evidence_hashes(self):
        for key in ("native_evidence", "public_evidence"):
            record = self.bindings[key]
            raw = Path(record["path"]).read_bytes()
            self.assertEqual(len(raw), record["size_bytes"])
            self.assertEqual(hashlib.sha256(raw).hexdigest(), record["sha256"])

    def test_native_script_scan_does_not_invent_a_quest_selector(self):
        self.assertEqual(self.native["decoded_scripts"], 9801)
        self.assertEqual(self.native["audio_opcode_counts"].get("3202", 0), 0)
        self.assertEqual(self.native["non_script_payloads"][0]["payload_hex"], "0009")
        self.assertNotIn(0, self.native["callback_closure"])
        source = self.native["quest_callback_scripts"]["118"]["definition"]
        self.assertEqual(
            [value for op, value in zip(source["instructions"], source["intOperands"]) if op == 40],
            [6816, 6817],
        )

    def test_eating_motion_variant_and_sound_are_exact(self):
        old = self.native["sequences"]["829"]["definition"]
        current = self.native["sequences"]["12526"]["definition"]
        self.assertEqual({key for key in old if old[key] != current[key]}, {"id", "frameSounds"})
        self.assertEqual(old["frameSounds"], {})
        self.assertEqual(current["frameSounds"], {
            "1": [{"id": 2393, "loops": 1, "location": 5, "retain": 0, "weight": 100}],
        })

    def test_smelting_animation_identity_is_not_assumed(self):
        a = self.native["sequences"]["899"]["definition"]
        b = self.native["sequences"]["3243"]["definition"]
        self.assertEqual(a["frameLengths"], b["frameLengths"])
        self.assertNotEqual(a["frameIDs"], b["frameIDs"])
        self.assertEqual(a["frameSounds"], {})
        self.assertEqual(b["frameSounds"], {})
        entry = next(row for row in self.bindings["cue_bindings"] if row["action"] == "bronze_smelting")
        self.assertEqual(entry["status"], "source_signal_and_public_smelt_start_identified")

    def test_tutorial_rat_uses_current_source_family(self):
        npcs = json.loads(gzip.decompress(Path("assets/source/osrs/cache2695/collections/npc.json.gz").read_bytes()))
        for source_id in (3313, 3314, 3315):
            npc = npcs[f"asset.source.osrs.cache2695.npc.{source_id}"]
            self.assertEqual(npc["standingAnimation"], 4932)
            self.assertEqual(npc["walkingAnimation"], 4931)
        for sequence in (4933, 4934, 4935):
            self.assertEqual(self.native["sequences"][str(sequence)]["definition"]["frameSounds"], {})

    def test_native_request_order_is_not_quest_priority(self):
        cases = self.native["native_queue_tests"]["cases"]
        order_cases = [case for case in cases if case["case"] == "jingle-request-order"]
        self.assertEqual(len(order_cases), 4)
        for case in order_cases:
            self.assertEqual(case["pending"], [case["submitted"][-1]])
        arguments = [case for case in cases if case["case"] == "jingle-auxiliary-argument"]
        self.assertEqual({case["argument"] for case in arguments}, {0, 1, 255, 60000})
        self.assertTrue(all(case["pending"] == [154] for case in arguments))

    def test_actual_sfx_tick_is_recorded(self):
        case = next(case for case in self.native["native_queue_tests"]["cases"] if case["case"] == "native-client-sfx-tick-delay")
        self.assertEqual([row["queue_size"] for row in case["observed"]], [1, 1, 1, 0])
        self.assertEqual([row["entry"]["delay"] for row in case["observed"][:3]], [1, 0, -100])

    def test_original_startup_percussion_is_not_constructor_default(self):
        case = next(case for case in self.native["native_queue_tests"]["cases"] if case["case"] == "native-startup-percussion-bank")
        self.assertEqual(case["constructor_default_bank_channel9"], 0)
        self.assertEqual(case["source_startup_default_bank_channel9"], 128)
        self.assertIn("dg.ay", case["source_startup_call"])

    def test_native_leading_delay_does_not_require_a_second_asset_offset(self):
        case = next(case for case in self.native["native_queue_tests"]["cases"] if case["case"] == "native-leading-delay-trim-is-exact")
        self.assertEqual(case["trim_client_cycles"], 1)
        self.assertEqual(case["trim_frames"], 441)
        self.assertEqual(case["full_frames"] - case["trimmed_frames"], 441)
        self.assertTrue(case["remaining_pcm_identical"])

    def test_unresolved_selectors_are_not_promoted_to_pack_approval(self):
        self.assertTrue(all(value == "not_fully_closed" for value in self.bindings["pack_gate_status"].values()))
        self.assertFalse(self.bindings["blanket_account_requirement"])
        self.assertFalse(self.bindings["owner_reference_pack_approved"])
        self.assertFalse(self.bindings["runtime_browser_audio_accepted"])
        self.assertTrue(self.bindings["quest_jingles"]["possible_no_request_outcome_retained"])
        self.assertEqual(self.bindings["quest_jingles"]["candidate_groups"], [152, 153, 154])

    def test_published_sound_roles_use_actual_matched_ids(self):
        records = {row["action"]: row for row in self.bindings["cue_bindings"]}
        self.assertEqual(records["ordinary_unarmed_goblin"]["source_sound_ids"], [469, 472, 471])
        self.assertEqual(records["tutorial_rat"]["source_sound_ids"], {"attack": 710, "hit": 713, "death": 711})
        self.assertGreater(records["ordinary_unarmed_goblin"]["public_matches"]["hit_peak"], 0.97)
        self.assertGreater(records["tutorial_rat"]["public_matches"]["hit_peak"], 0.94)
        self.assertEqual(records["ordinary_shortbow"]["source_sound_ids"], [2693])
        self.assertEqual(records["ordinary_shortbow"]["status"], "source_signal_and_public_event_identified")


@unittest.skipUnless(MEDIA_AVAILABLE, "Use the pinned project-local analysis requirements for recording tests")
class RecordingMatcherTests(unittest.TestCase):
    def test_normalized_match_recovers_exact_offset_and_gain(self):
        import numpy as np
        from public_audio_match import correlate
        rng = np.random.default_rng(812)
        template = rng.normal(0, 0.1, 800).astype(np.float32)
        recording = rng.normal(0, 0.0001, 4000).astype(np.float32)
        recording[1200:2000] += template * 0.25
        result = correlate(recording, template, 1000, 1)
        self.assertEqual(result[0]["offset_seconds"], 1.2)
        self.assertGreater(result[0]["coefficient"], 0.999)

    def test_unrelated_sound_does_not_match(self):
        import numpy as np
        from public_audio_match import correlate
        rng = np.random.default_rng(814)
        result = correlate(rng.normal(size=4000), rng.normal(size=800), 1000, 1)
        self.assertLess(abs(result[0]["coefficient"]), 0.2)

    def test_invalid_signal_window_rejected(self):
        import numpy as np
        from public_audio_match import correlate
        with self.assertRaises(ValueError):
            correlate(np.zeros(20), np.zeros(200), 1000)


if __name__ == "__main__":
    unittest.main()
