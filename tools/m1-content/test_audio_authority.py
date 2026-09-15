"""Source-authority facts, not client playback preferences or a full music-content expansion."""
from copy import deepcopy
import unittest

from audio_authority4 import INPUT, LUMBRIDGE_GROUPS, MENU_GROUPS, bind_audio_authority, contains
from common import CONTENT, load


class AudioAuthoritySources(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.content = load(CONTENT / "game-content.json.gz")
        cls.inputs = load(INPUT)
        cls.audio, cls.proof = bind_audio_authority(cls.content)

    def test_exact_eight_published_menu_groups_exclude_the_title(self):
        self.assertEqual({int(group) for group in self.audio["tracks"]}, MENU_GROUPS)
        self.assertNotIn("0", self.audio["tracks"])
        self.assertTrue(self.audio["tracks"]["62"]["automatic"])
        self.assertEqual(self.inputs["tracks"]["62"]["automatic_unlock"], 1)

    def test_each_lumbridge_track_has_independent_arrival_unlock_evidence(self):
        for group in LUMBRIDGE_GROUPS:
            track = self.audio["tracks"][str(group)]
            self.assertFalse(track["automatic"])
            self.assertEqual(track["area"], "lumbridge")
            self.assertEqual(track["conserved"], "counter.tutorial.departed")
            self.assertEqual(self.inputs["tracks"][str(group)]["hint"], "in Lumbridge.")
            self.assertTrue(any("individual track explicitly unlocks" in row["notes"] for row in track["source"]))
        self.assertFalse(self.proof["modern_membership_is_unlock_proof"])
        self.assertFalse(self.proof["client_preferences_grant_tracks"])

    def test_correct_harmony_page_is_not_the_old_disambiguation_snapshot(self):
        harmony = self.inputs["supplemental_pages"]["Harmony (music track)"]
        self.assertEqual(harmony["revision"], 15330698)
        self.assertIn("|cacheid = 76", harmony["text"])
        self.assertIn("first arrives in Lumbridge", harmony["text"])

    def test_conserved_cave_and_departure_counters_prove_completed_landings(self):
        cave = self.proof["conserved_facts"]["tutorial_cave"]
        self.assertEqual(cave["counter"], "counter.tutorial.quest_ladder.completed")
        self.assertEqual(cave["phase"], "completed")
        self.assertEqual(cave["landings"], [{"x": 3088, "y": 9520, "plane": 0}])
        for location in self.proof["conserved_facts"]["lumbridge"]["landings"]:
            self.assertTrue(contains(self.audio["areas"]["lumbridge"], location))

    def test_queued_or_arbitrary_events_cannot_become_historical_unlock_proof(self):
        content = deepcopy(self.content)
        edge = content["tutorial"]["stage.tutorial.quest_ladder"]["transitions"][0]
        edge["event"] = "interacted"
        with self.assertRaisesRegex(ValueError, "completed travel"):
            bind_audio_authority(content)

    def test_known_plane_and_region_bounds_are_not_mode_dependent(self):
        area = self.audio["areas"]["lumbridge"]
        self.assertTrue(contains(area, {"x": 3232, "y": 3233, "plane": 0}))
        self.assertFalse(contains(area, {"x": 3232, "y": 3233, "plane": 1}))
        self.assertFalse(contains(area, self.content["initial_state"]["tile"]))
        self.assertFalse(contains(area, {"x": 3174, "y": 5726, "plane": 0}))
        self.assertTrue(contains(self.audio["areas"]["tutorial_cave"], {"x": 3088, "y": 9520, "plane": 0}))

    def test_native_morph_word_claims_only_two_source_qualified_bits(self):
        self.assertEqual(self.proof["native_varp"], {"id": 491, "known_bits": 20, "value": 0})
        self.assertEqual(self.inputs["variable_symbols"]["VarPlayerID.java"]["symbols"]["491"], "ABYSSAL_WARP")
        self.assertEqual(self.inputs["variable_symbols"]["VarbitID.java"]["symbols"]["609"], "RC_NO_TALLY_REQUIRED_WATER")
        self.assertEqual(self.inputs["variable_symbols"]["VarbitID.java"]["symbols"]["611"], "RC_NO_TALLY_REQUIRED_FIRE")

    def test_expanded_equipment_cannot_silently_reuse_the_zero_default(self):
        content = deepcopy(self.content)
        content["items"]["item.chefs_hat"]["source_id"] = 5531
        with self.assertRaisesRegex(ValueError, "equipment"):
            bind_audio_authority(content)


if __name__ == "__main__":
    unittest.main()
