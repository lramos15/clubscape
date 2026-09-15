import copy
import hashlib
import shutil
import unittest
import uuid

from PIL import Image

import capture
import checks


class SourceLayerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.directory = capture.OUTPUT
        cls.manifest = capture.cache.read_json(cls.directory / "case-index.json")
        cls.records = {record["id"]: record for record in cls.manifest["cases"]}
        cls.metrics = capture.cache.read_json(cls.directory / "pair-metrics.json")

    def setUp(self):
        self.scratch = capture.LOCAL / "tests" / uuid.uuid4().hex
        self.scratch.mkdir(parents=True)

    def tearDown(self):
        shutil.rmtree(self.scratch)

    def test_complete_bounded_original_set(self):
        result = capture.validate(self.directory)
        self.assertEqual((result["cases"], len(result["pairs"])), (12, 8))
        self.assertFalse(result["candidate_compared"])
        self.assertFalse(result["authenticated_source_gameplay"])

    def test_case_selection_rejects_unknown_duplicate_and_thirteenth_case(self):
        contract = capture.cache.read_json(capture.TOOL / "cases.json")
        for requested in (["unknown"], ["tutorial-door-open", "tutorial-door-open"]):
            with self.assertRaises(ValueError):
                capture.case_selection(contract, requested)
        contract["cases"].append({**contract["cases"][0], "id": "thirteenth"})
        with self.assertRaises(ValueError):
            capture.case_selection(contract, None)

    def test_output_cannot_rewrite_frozen_pack_or_images(self):
        for path in (capture.ROOT / "assets/reference/osrs240", capture.ROOT / "research/reference-pack/v1",
                     capture.ROOT / "tools/source-capture", capture.ROOT / "assets/source/osrs"):
            with self.subTest(path=path), self.assertRaises(ValueError):
                capture.output_directory(path)
        self.assertEqual(capture.output_directory(capture.OUTPUT), capture.OUTPUT)

    def test_escaped_case_id_is_rejected(self):
        with self.assertRaises(ValueError):
            capture.case_selection({"cases": [{"id": "../frozen"}]}, None)

    def test_all_native_cameras_and_stock_material_settings(self):
        for record in self.records.values():
            checks.validate_state(record)
            self.assertEqual(record["capture"]["settings"]["zoom"], 410)
            self.assertEqual(record["capture"]["settings"]["game_cycle"], 0)

    def test_camera_or_model_viewport_zoom_substitution_fails(self):
        for field, value in (("zoom", 662), ("camera_local_units", [0, 0, 0]), ("pitch", 2047)):
            record = copy.deepcopy(self.records["tutorial-door-closed"])
            record["capture"]["settings"][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                checks.validate_state(record)

    def test_original_door_shape_and_actual_placement(self):
        before = checks.operation(self.records["tutorial-door-closed"], "orientation_a")
        after = checks.operation(self.records["tutorial-door-open"], "orientation_a")
        self.assertEqual((before["object_id"], before["shape"], before["model_id"]), (9398, 0, 9476))
        self.assertEqual((before["orientation_a"], after["orientation_a"]), (1, 2))
        self.assertEqual(before["world_position"], after["world_position"])
        self.assertEqual(before["geometry"]["vertices"], after["geometry"]["vertices"])
        self.assertNotEqual(before["geometry"]["vertex_xyz_float32_be_sha256"],
                            after["geometry"]["vertex_xyz_float32_be_sha256"])

    def test_diagonal_door_model_or_relocated_tile_is_rejected(self):
        for field, value in (("model_id", 9477), ("source_tile", [1, 1, 0]), ("shape", 9)):
            record = copy.deepcopy(self.records["tutorial-door-open"])
            checks.operation(record, "orientation_a")[field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                checks.validate_state(record)

    def test_native_ground_quantity_and_top_three(self):
        one = checks.operation(self.records["lumbridge-ground-single"], "selected")
        stack = checks.operation(self.records["lumbridge-ground-stack"], "selected")
        three = checks.operation(self.records["lumbridge-ground-top-three"], "selected")
        self.assertEqual(one["selected"][0]["geometry"]["vertices"], 16)
        self.assertEqual(stack["selected"][0]["geometry"]["vertices"], 121)
        self.assertEqual([item["item_id"] for item in three["selected"]], [1277, 1925, 995])
        self.assertEqual(three["native_draw_anchor_x_height_y"], [6848, -240, 6336])

    def test_fabricated_item_order_or_drawpoint_is_rejected(self):
        record = copy.deepcopy(self.records["lumbridge-ground-top-three"])
        checks.operation(record, "selected")["selected"].reverse()
        with self.assertRaises(ValueError):
            checks.validate_state(record)
        record = copy.deepcopy(self.records["lumbridge-ground-single"])
        checks.operation(record, "selected")["native_draw_anchor_x_height_y"][1] = 0
        with self.assertRaises(ValueError):
            checks.validate_state(record)

    def test_fire_uses_exact_source_frames_not_random_start(self):
        first = checks.operation(self.records["lumbridge-fire-frame-zero"], "requested_frame")
        later = checks.operation(self.records["lumbridge-fire-frame-three"], "requested_frame")
        self.assertEqual((first["native_frame"], later["native_frame"]), (0, 3))
        self.assertEqual((first["native_animation_advance_cycles"], later["native_animation_advance_cycles"]), (0, 19))
        self.assertEqual(first["geometry"]["triangle_indices_int32_be_sha256"],
                         later["geometry"]["triangle_indices_int32_be_sha256"])
        self.assertNotEqual(first["geometry"]["vertex_xyz_float32_be_sha256"],
                            later["geometry"]["vertex_xyz_float32_be_sha256"])
        self.assertEqual((first["geometry"]["vertices"], first["geometry"]["faces"]), (112, 121))

    def test_wrong_or_random_fire_frame_fails(self):
        for field, value in (("native_frame", 2), ("randomize_initial_phase", True)):
            record = copy.deepcopy(self.records["lumbridge-fire-frame-three"])
            checks.operation(record, "requested_frame")[field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                checks.validate_state(record)

    def test_stock_locked_and_normal_roof_paths_stay_distinct(self):
        outside = checks.operation(self.records["tutorial-roofs-outside"], "actual_locked_camera_draw_plane")
        inside = checks.operation(self.records["tutorial-roofs-inside"], "actual_locked_camera_draw_plane")
        hidden = checks.operation(self.records["tutorial-roofs-hidden"], "actual_locked_camera_draw_plane")
        self.assertEqual((outside["actual_locked_camera_draw_plane"], inside["actual_locked_camera_draw_plane"],
                          hidden["actual_locked_camera_draw_plane"]), (3, 3, 0))
        self.assertEqual((outside["normal_camera_stock_plane_selector"], inside["normal_camera_stock_plane_selector"]), (3, 0))
        pair = next(pair for pair in self.metrics["pairs"] if pair["id"] == "roof-player-location")
        self.assertEqual(pair["different_scene_or_remaining_frame_pixels"], 0)
        self.assertGreater(pair["different_native_minimap_pixels"], 0)

    def test_nonstock_roof_plugin_is_rejected(self):
        record = copy.deepcopy(self.records["tutorial-roofs-outside"])
        checks.operation(record, "actual_locked_camera_draw_plane")["roof_removal_plugin_mode"] = 1
        with self.assertRaises(ValueError):
            checks.validate_state(record)

    def test_native_plane_and_qualified_chunk_mapping(self):
        plane = self.records["lumbridge-minimap-plane-one"]["capture"]["settings"]
        self.assertEqual((plane["source_plane"], plane["source_scene_draw_plane"]), (1, 1))
        mapping = checks.operation(self.records["lumbridge-minimap-mapped-chunk"], "packed_template")
        self.assertEqual(mapping["source_chunk"], [0, 402, 402])
        self.assertEqual(mapping["packed_template"], 6589588)
        pair = next(pair for pair in self.metrics["pairs"] if pair["id"] == "minimap-mapped-chunk")
        self.assertEqual(pair["before_scene_census"], pair["after_scene_census"])
        self.assertGreater(pair["different_native_minimap_pixels"], 0)

    def test_all_pair_pixels_accounted_and_panels_unchanged(self):
        for pair in self.metrics["pairs"]:
            self.assertGreater(pair["different_pixels_full_frame"], 0)
            self.assertEqual(pair["different_unchanged_chat_pixels"], 0)
            self.assertEqual(pair["different_unchanged_inventory_panel_pixels"], 0)
            self.assertEqual(pair["different_pixels_full_frame"],
                             pair["different_native_minimap_pixels"] + pair["different_sidebar_container_pixels"]
                             + pair["different_scene_or_remaining_frame_pixels"])

    def test_identical_images_cannot_claim_dynamic_evidence(self):
        pair = self.manifest["pairs"][0]
        record = self.records[pair["before"]]
        image = checks.image_for(self.directory, record)
        with self.assertRaisesRegex(ValueError, "no actual source pixel change"):
            checks.pair_metrics(pair, record, record, image, image)

    def test_missing_and_corrupt_pngs_fail(self):
        record = copy.deepcopy(self.records["tutorial-door-open"])
        record["capture"]["path"] = "image.png"
        path = self.scratch / "image.png"
        with self.assertRaisesRegex(ValueError, "Missing"):
            checks.image_for(self.scratch, record)
        original = self.directory / self.records["tutorial-door-open"]["capture"]["path"]
        data = bytearray(original.read_bytes())
        data[-1] ^= 1
        path.write_bytes(data)
        with self.assertRaisesRegex(ValueError, "hash changed"):
            checks.image_for(self.scratch, record)

    def test_blank_image_with_self_consistent_hash_is_rejected(self):
        record = copy.deepcopy(self.records["tutorial-door-closed"])
        image = Image.new("RGBA", (1920, 1080), (0, 0, 0, 255))
        path = self.scratch / "blank.png"
        image.save(path)
        record["capture"].update(path="blank.png", size_bytes=path.stat().st_size,
                                 sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                                 pixel_argb32_be_sha256=hashlib.sha256(checks.argb_bytes(image)).hexdigest())
        with self.assertRaisesRegex(ValueError, "Blank"):
            checks.image_for(self.scratch, record)

    def test_missing_source_payload_or_empty_geometry_fails(self):
        record = copy.deepcopy(self.records["tutorial-door-open"])
        del record["capture"]["source"]["layer_source_inputs"]["7/9476/0"]
        with self.assertRaisesRegex(ValueError, "payload evidence"):
            checks.validate_state(record)
        record = copy.deepcopy(self.records["tutorial-door-open"])
        checks.operation(record, "orientation_a")["geometry"]["vertices"] = 0
        with self.assertRaisesRegex(ValueError, "Empty"):
            checks.validate_state(record)

    def test_unsupported_authentication_or_transport_claim_fails(self):
        for field in ("source_gameplay_observed", "network_transport_connected"):
            record = copy.deepcopy(self.records["tutorial-door-open"])
            record["capture"]["settings"][field] = True
            with self.subTest(field=field), self.assertRaises(ValueError):
                checks.validate_state(record)

    def test_caught_native_errors_are_not_silently_accepted(self):
        log = "[ERROR injected-client - Client error: native fixture failed]\n"
        self.assertTrue(capture.render_failures(log))
        log = "Exception NullPointerException\n thrown in method <foo> in 'dy'\n"
        self.assertTrue(capture.render_failures(log))
        log = "Exception ClassCastException\n thrown in method <foo> in 'dy'\n"
        self.assertTrue(capture.render_failures(log))


if __name__ == "__main__":
    unittest.main()
