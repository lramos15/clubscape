import copy
import shutil
import unittest
import uuid

import probe_phases as probe


class SceneryPhaseTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.old = probe.old_index()
        cls.sources = {row["id"]: row for row in cls.old["cases"]}
        cls.index = probe.cache.read_json(probe.OUTPUT / "phase-index.json")
        cls.sidecars = {
            row["case_id"]: probe.cache.read_json(probe.ROOT / row["sidecar"]["path"])
            for row in cls.index["cases"]
        }

    def setUp(self):
        self.scratch = probe.LOCAL / "tests" / uuid.uuid4().hex
        self.scratch.mkdir(parents=True)

    def tearDown(self):
        shutil.rmtree(self.scratch)

    def test_all_original_replays_and_phase_observations_verify(self):
        result = probe.validate(probe.OUTPUT)
        self.assertEqual((result["cases"], result["placements_per_case"], result["observations"]), (3, 3, 27))
        self.assertTrue(result["original_png_and_native_pixel_hashes_unchanged"])

    def test_original_case_index_remains_bit_identical(self):
        self.assertEqual(probe.cache.digest(probe.original.OUTPUT / "case-index.json"), probe.INDEX_SHA)
        self.assertEqual(len(self.old["cases"]), 12)
        self.assertEqual(tuple(self.sidecars), probe.CASE_IDS)

    def test_observed_active_frames_are_placement_specific_not_zero_defaults(self):
        frames = {(24969, 3095, 3102, 0): (477, 4),
                  (196, 3096, 3105, 0): (481, 3),
                  (196, 3096, 3110, 0): (481, 2)}
        for sidecar in self.sidecars.values():
            for row in sidecar["observations"]:
                key = (row["source_object_id"], *row["world_tile"])
                self.assertEqual((row["sequence_id"], row["native_frame"]), frames[key])
                self.assertEqual(row["rendering_controller"], "dy.ac")

    def test_observed_clock_boundary_is_retained_including_terminal_flame_frame(self):
        for sidecar in self.sidecars.values():
            for row in sidecar["observations"]:
                post = row["observation_phase"] == "after-original-draw"
                self.assertEqual(row["source_cycle"], 0)
                self.assertEqual(row["last_update_cycle"], 0 if post else -1)
                self.assertEqual(row["active_controller"]["frame_cycle"], 1 if post else 0)
                self.assertEqual(row["pending_source_cycle_delta"], 0 if post else 1)
                if post and row["source_object_id"] == 24969:
                    self.assertEqual(row["active_controller"]["frame_lengths"][4], 1)
                    self.assertEqual(row["native_frame"], 4)

    def test_unavailable_api_cycle_is_not_substituted(self):
        for sidecar in self.sidecars.values():
            self.assertTrue(all(row["native_api_anim_cycle"] == -1 for row in sidecar["observations"]))
        sidecar = copy.deepcopy(self.sidecars[probe.CASE_IDS[0]])
        sidecar["observations"][0]["native_api_anim_cycle"] = 0
        with self.assertRaisesRegex(ValueError, "must not be invented"):
            probe.validate_case(sidecar, self.sources[probe.CASE_IDS[0]])

    def test_tampered_native_frame_or_multiplier_fails(self):
        for field, value in (("native_frame", 0), ("last_update_decode_multiplier", 1)):
            sidecar = copy.deepcopy(self.sidecars[probe.CASE_IDS[0]])
            sidecar["observations"][0][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                probe.validate_case(sidecar, self.sources[probe.CASE_IDS[0]])

    def test_duplicate_or_missing_placement_observation_fails(self):
        for duplicate in (True, False):
            sidecar = copy.deepcopy(self.sidecars[probe.CASE_IDS[0]])
            if duplicate:
                sidecar["observations"][1] = sidecar["observations"][0]
            else:
                sidecar["observations"].pop()
            with self.subTest(duplicate=duplicate), self.assertRaises(ValueError):
                probe.validate_case(sidecar, self.sources[probe.CASE_IDS[0]])

    def test_wrong_placement_type_or_coordinate_fails(self):
        for field, value in (("placement_type", 9), ("world_tile", [3095, 3103, 0])):
            sidecar = copy.deepcopy(self.sidecars[probe.CASE_IDS[0]])
            sidecar["observations"][0][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                probe.validate_case(sidecar, self.sources[probe.CASE_IDS[0]])

    def test_changed_source_image_or_native_pixels_fail(self):
        for field in ("png_sha256", "native_argb32_be_sha256"):
            sidecar = copy.deepcopy(self.sidecars[probe.CASE_IDS[0]])
            sidecar["source_image_replay"][field] = "0" * 64
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "pixels changed"):
                probe.validate_case(sidecar, self.sources[probe.CASE_IDS[0]])

    def test_candidate_evidence_or_animation_mutation_is_rejected(self):
        for field in ("candidate_images_or_frame_guesses_read", "animation_state_modified_by_probe"):
            sidecar = copy.deepcopy(self.sidecars[probe.CASE_IDS[0]])
            sidecar[field] = True
            with self.subTest(field=field), self.assertRaises(ValueError):
                probe.validate_case(sidecar, self.sources[probe.CASE_IDS[0]])

    def test_probe_output_cannot_overwrite_existing_references(self):
        for path in (probe.original.OUTPUT, probe.ROOT / "tools/source-layer-capture",
                     probe.ROOT / "research/reference-pack", probe.ROOT / "assets/source/osrs"):
            with self.subTest(path=path), self.assertRaises(ValueError):
                probe.selected_output(path)
        self.assertEqual(probe.selected_output(probe.OUTPUT), probe.OUTPUT)

    def test_all_observation_stages_retain_same_source_clock(self):
        sidecar = copy.deepcopy(self.sidecars[probe.CASE_IDS[0]])
        row = sidecar["observations"][-1]
        row["source_cycle"] = 1
        row["source_cycle_raw"] = pow(1612595797, -1, 1 << 32)
        with self.assertRaisesRegex(ValueError, "original replay state"):
            probe.validate_case(sidecar, self.sources[probe.CASE_IDS[0]])

    def test_probe_observer_does_not_step_animations_or_fetch_models(self):
        source = (probe.TOOL / "SceneryPhaseProbe.java").read_text()
        observer = source[source.index("private Map<String,Object> controller"):source.index("public static void main")]
        for forbidden in (".getModel(", ".rf(", ".setSeed(", ".logicalInt(", ".next", "Math.random(", ".setInt(", "field.set("):
            self.assertNotIn(forbidden, observer)

    def test_no_extra_reference_images_are_published(self):
        self.assertEqual(list(probe.OUTPUT.rglob("*.png")), [])
        for sidecar in self.sidecars.values():
            self.assertFalse(sidecar["source_image_replay"]["replay_png_published_as_new_case"])


if __name__ == "__main__":
    unittest.main()
