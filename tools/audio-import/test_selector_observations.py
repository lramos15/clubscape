import gzip
import hashlib
import importlib.util
import json
from pathlib import Path
import unittest


class SelectorObservationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.report = json.loads(Path("research/audio-source/selector-observations.json").read_text())
        cls.evidence = json.loads(gzip.decompress(Path(cls.report["evidence"]["path"]).read_bytes()))

    def test_evidence_is_hash_bound(self):
        record = self.report["evidence"]
        raw = Path(record["path"]).read_bytes()
        self.assertEqual(len(raw), record["size_bytes"])
        self.assertEqual(hashlib.sha256(raw).hexdigest(), record["sha256"])

    def test_normal_cooks_selector_is_observed152_not_difficulty_guess154(self):
        result = self.report["cooks_assistant_observation"]
        self.assertEqual(result["quest_jingle"], 152)
        self.assertGreater(result["five_second_correlation"], 0.98)
        self.assertLess(result["quest_cue_start_seconds"], result["levelup_after_scroll_dismissal"]["cue_start_seconds"])
        self.assertEqual(result["levelup_after_scroll_dismissal"]["jingle"], 33)
        self.assertTrue(result["later_actual_cooking_levelup"]["not_a_simultaneous_quest_reward"])
        self.assertNotEqual(result["recording_date"][:4], "2026")

    def test_bow_release_rejects_name_only_shortbow_candidate(self):
        result = next(row for row in self.report["observed_selectors"] if row["rule"] == "rule.combat.ranged")
        self.assertEqual(result["item_ids"], [841])
        self.assertEqual(result["sound_id"], 2693)
        self.assertIn(2702, result["rejected_name_only_alternatives"])
        self.assertGreater(min(result["correlations"]), 0.97)
        self.assertEqual(result["source_sequence"], 426)

    def test_furnace_bands_identify_same_original_sample_without_aliasing_sequences(self):
        bands = self.evidence["furnace-independent-band-coherence.json"]
        times = [row["peaks"][0]["video_time"] for row in bands]
        self.assertLessEqual(max(times) - min(times), 1 / 22050 + 1e-10)
        self.assertFalse(self.evidence["furnace_frame_relation"]["all_raw_frame_payloads_identical"])
        result = next(row for row in self.report["observed_selectors"] if row["rule"] == "rule.smelting.bronze")
        self.assertEqual(result["sound_id"], 2725)
        self.assertEqual(result["source_sequence"], 899)
        self.assertFalse(result["frame_payloads_899_3243_identical"])

    def test_exact_remaining_selectors_are_not_filled_with_silence(self):
        remaining = {row["rule"] for row in self.report["still_unresolved"]}
        self.assertEqual(remaining, {"quest.learning_the_ropes", "rule.food.healing"})
        food = next(row for row in self.report["still_unresolved"] if row["rule"] == "rule.food.healing")
        self.assertEqual(food["item_ids"], [315, 2309])
        self.assertFalse(self.report["owner_pack_approved"])
        self.assertFalse(self.report["running_browser_audio_accepted"])
        self.assertTrue(self.report["no_existing_audio_converted"])


@unittest.skipUnless(importlib.util.find_spec("numpy") and importlib.util.find_spec("av"),
                     "Use pinned local analysis dependencies")
class FixedBandTests(unittest.TestCase):
    def test_identifies_masked_source_without_fitting_filter_to_the_recording(self):
        import numpy as np
        from public_audio_match import bandpass, correlate
        rate = 22050
        rng = np.random.default_rng(2702)
        source = rng.normal(0, 0.05, rate)
        recording = np.zeros(rate * 4)
        recording[rate * 2:rate * 3] += source
        recording += 0.8 * np.sin(2 * np.pi * 220 * np.arange(len(recording)) / rate)
        raw = correlate(recording, source, rate, 1)[0]
        isolated = correlate(bandpass(recording, 1500, 9000), bandpass(source, 1500, 9000), rate, 1)[0]
        self.assertLess(abs(raw["coefficient"]), 0.2)
        self.assertEqual(isolated["offset_seconds"], 2)
        self.assertGreater(isolated["coefficient"], 0.99)

    def test_invalid_band_is_rejected(self):
        import numpy as np
        from public_audio_match import bandpass
        with self.assertRaises(ValueError):
            bandpass(np.ones(100), 9000, 500)


if __name__ == "__main__":
    unittest.main()
