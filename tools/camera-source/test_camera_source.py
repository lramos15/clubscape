import gzip
import shutil
import unittest
import uuid

import probe_camera as probe


class CameraSourceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.directory = probe.OUT / "traces"
        cls.manifest = probe.cache.read_json(cls.directory / "manifest.json")
        cls.traces = {row["scenario"]: probe.cache.read_json(probe.ROOT / row["trace"]["path"])
                      for row in cls.manifest["traces"]}
        cls.contract = probe.cache.read_json(probe.OUT / "contract.json")

    def setUp(self):
        self.scratch = probe.LOCAL / "tests" / uuid.uuid4().hex
        self.scratch.mkdir(parents=True)

    def tearDown(self):
        shutil.rmtree(self.scratch)

    def rows(self, scenario, label=None):
        rows = self.traces[scenario]["traces"]
        return rows if label is None else [row for row in rows if row["label"] == label]

    def test_all_scenarios_are_actual_hash_bound_original_traces(self):
        self.assertEqual(set(self.traces), set(probe.SCENARIOS))
        for row in self.manifest["traces"]:
            probe.cache.checked_file(probe.ROOT / row["trace"]["path"], row["trace"])
            log = probe.cache.checked_file(probe.ROOT / row["native_runlog"]["path"], row["native_runlog"])
            self.assertEqual(probe.native_errors(gzip.decompress(log.read_bytes()).decode()), [])
            self.assertGreater(row["states"], 0)
        for trace in self.traces.values():
            self.assertFalse(trace["network_pump"])
            self.assertFalse(trace["original_jar_modified"])
            self.assertFalse(trace["server_camera_defaults_established"])

    def test_constructor_defaults_are_distinct_from_world_entry(self):
        defaults = self.traces["initial"]["constructor_defaults"]
        self.assertEqual((defaults["camera_mode"], defaults["target_pitch"], defaults["target_yaw"]), (0, 1024, 0))
        self.assertEqual(defaults["camera_x_height_y"], [0, 0, 0])
        self.assertEqual(defaults["native_state"]["ek"]["decoded"], -1)
        self.assertEqual(defaults["native_state"]["do"]["decoded"], 50)
        self.assertFalse(defaults["mouse_camera_enabled"])
        self.assertFalse(self.contract["initial_and_reset_provenance"]["genuine_unknowns"] == [])

    def test_original_arrow_key_mapping_and_cycle_period(self):
        for trace in self.traces.values():
            keys = trace["physical_arrow_mapping"]
            self.assertEqual([keys[name] for name in ["left_java37", "right_java39", "up_java38", "down_java40"]],
                             [96, 97, 98, 99])
            self.assertEqual(trace["native_logic_cycle_period_ns"], 20_000_000)

    def test_arrow_acceleration_and_release_use_native_signed_truncation(self):
        held = self.rows("keyboard", "left-held")
        self.assertEqual([row["native_state"]["jm"]["decoded"] for row in held[:8]],
                         [-96, -144, -168, -180, -186, -189, -190, -191])
        release = self.rows("keyboard", "released")[:8]
        self.assertEqual([row["native_state"]["jm"]["decoded"] for row in release],
                         [-95, -47, -23, -11, -5, -2, -1, 0])
        self.assertNotEqual(held[-1]["target_yaw"], release[-1]["target_yaw"])

    def test_pitch_clamps_and_yaw_modulo_are_native(self):
        rows = self.rows("clamps")
        self.assertTrue(all(1024 <= row["target_pitch"] <= 3064 for row in rows))
        self.assertEqual(max(row["target_pitch"] for row in rows), 3064)
        self.assertEqual(min(row["target_pitch"] for row in rows), 1024)
        self.assertTrue(all(0 <= row["target_yaw"] < 16384 for row in rows))

    def test_orbit_eye_changes_with_source_angles(self):
        rows = self.rows("orbit")
        by_label = {row["label"]: row for row in rows}
        forward = by_label["controlled-orbit-1024-0"]
        right = by_label["controlled-orbit-1024-4096"]
        self.assertEqual(forward["focal_x_y"], right["focal_x_y"])
        self.assertNotEqual(forward["camera_x_height_y"], right["camera_x_height_y"])
        self.assertEqual(forward["viewport_zoom"], right["viewport_zoom"])
        self.assertNotEqual(forward["camera_x_height_y"][0::2], forward["focal_x_y"])

    def test_middle_mouse_option_and_release_are_source_controlled(self):
        disabled = self.rows("mouse", "middle-disabled-source-option")[0]
        first = self.rows("mouse", "middle-first")[0]
        stationary = self.rows("mouse", "middle-stationary")[0]
        self.assertEqual((disabled["target_yaw"], disabled["target_pitch"]), (0, 1024))
        self.assertNotEqual(first["target_yaw"], stationary["target_yaw"])
        self.assertEqual(stationary["native_state"]["jm"]["decoded"],
                         int(first["native_state"]["jm"]["decoded"] / 2))
        self.assertEqual(self.rows("mouse", "released")[-1]["native_state"]["jh"]["decoded"], 0)

    def test_follow_snap_and_smoothing_have_exact_500_boundary(self):
        rows = {row["label"]: row for row in self.rows("follow-threshold")}
        self.assertEqual(rows["follow-threshold-500"]["focal_float_x_height_y"][0], 7007.25)
        self.assertEqual(rows["follow-threshold-501"]["focal_float_x_height_y"][0], 7477.0)
        self.assertEqual(rows["follow-threshold--500"]["focal_float_x_height_y"][0], 6944.75)
        self.assertEqual(rows["follow-threshold--501"]["focal_float_x_height_y"][0], 6475.0)

    def test_render_frame_elapsed_is_not_replaced_by_fixed_rate_guess(self):
        rows = {row["label"]: row for row in self.rows("frame-rates")}
        for nanos, focus, yaw in [(10_000_000, 6980, 24), (20_000_000, 6984, 48), (40_000_000, 6992, 96)]:
            row = rows[f"render-frame-{nanos}"]
            self.assertEqual(row["focal_x_y"][0], focus)
            self.assertEqual(row["target_yaw"], yaw)
            self.assertEqual(row["input"]["controlled_render_frame_nanoseconds"], nanos)

    def test_source_ground_height_and_actor_footprint_are_retained(self):
        first = self.rows("follow", "initial-follow")[0]
        self.assertEqual(first["source_player_ground_height"], -240)
        self.assertEqual(first["focal_height"], -273)
        self.assertEqual(first["source_actor_footprint_size"], 0)
        self.assertGreater(len({row["focal_height"] for row in self.rows("terrain")}), 1)

    def test_wheel_uses_original_program_and_pointer_routing(self):
        rows = self.rows("wheel")
        initial = next(row for row in rows if row["label"] == "controlled-source-zoom-512")
        out = next(row for row in rows if row["label"] == "viewport-wheel-out")
        self.assertEqual(initial["camera_varc_ints"]["73"], 512)
        self.assertEqual(out["camera_varc_ints"]["73"], 487)
        self.assertNotEqual(initial["viewport_zoom"], out["viewport_zoom"])
        self.assertEqual(initial["viewport_distance_scale_shorts"], out["viewport_distance_scale_shorts"])
        self.assertNotEqual(initial["native_state"]["do"]["decoded"], out["native_state"]["do"]["decoded"])
        sidebar_index = next(i for i, row in enumerate(rows) if row["label"] == "sidebar-wheel")
        self.assertEqual(rows[sidebar_index]["camera_varc_ints"], rows[sidebar_index - 1]["camera_varc_ints"])

    def test_wheel_bounds_and_disabled_source_varbit(self):
        lower = self.rows("wheel", "wheel-to-lower-bound")[0]
        upper = self.rows("wheel", "wheel-to-upper-bound")[0]
        disabled = self.rows("wheel", "source-wheel-disabled-varbit")[0]
        self.assertEqual((lower["camera_varc_ints"]["73"], upper["camera_varc_ints"]["73"]), (128, 896))
        self.assertEqual(disabled["camera_varc_ints"], upper["camera_varc_ints"])
        self.assertEqual(disabled["viewport_zoom"], upper["viewport_zoom"])
        self.assertEqual(disabled["wheel_varbits"][0], 1)

    def test_original_script_and_overlay_identities_are_explicit(self):
        scripts = self.traces["wheel"]["camera_script_evidence"]
        self.assertEqual(set(map(int, scripts)), {39, 42, 603, 604, 605, 626, 1045, 1046, 1049})
        self.assertTrue(scripts["39"]["matching_runtime_overlay"])
        self.assertTrue(scripts["1049"]["matching_runtime_overlay"])
        self.assertFalse(scripts["626"]["matching_runtime_overlay"])
        self.assertEqual(scripts["626"]["original_definition"]["intOperands"][:4], [128, 896, 128, 896])

    def test_distance_zoom_is_separate_from_projection_zoom(self):
        rows = self.rows("zoom-separation")
        before, after = rows[-2:]
        self.assertEqual(before["viewport_zoom"], after["viewport_zoom"])
        self.assertNotEqual(before["camera_x_height_y"], after["camera_x_height_y"])
        self.assertEqual(after["viewport_distance_scale_shorts"], [320, 384])

    def test_resize_keeps_world_focal_position_and_recalculates_projection(self):
        rows = [row for row in self.rows("resize") if row["label"].startswith("native-resize")]
        self.assertEqual([row["viewport_zoom"] for row in rows], [292, 410, 547, 410])
        self.assertEqual(len({tuple(row["focal_x_y"]) for row in rows}), 1)
        self.assertEqual(len({tuple(row["camera_x_height_y"]) for row in rows}), 1)

    def test_native_region_rebase_preserves_absolute_world_coordinates(self):
        rows = {row["label"]: row for row in self.rows("region-shift")}
        before, after, restored = [rows[name] for name in
            ["before-original-region-rebase", "after-original-region-rebase", "after-original-region-rebase-return"]]
        self.assertTrue(before["locked"])
        self.assertFalse(after["locked"])
        self.assertEqual(after["world_base"], [3176, 3176])
        for axis, camera_index in [(0, 0), (1, 2)]:
            self.assertEqual(before["world_base"][axis] * 128 + before["camera_x_height_y"][camera_index],
                             after["world_base"][axis] * 128 + after["camera_x_height_y"][camera_index])
        self.assertEqual(before["camera_x_height_y"], restored["camera_x_height_y"])
        self.assertEqual(before["focal_x_y"], restored["focal_x_y"])

    def test_missing_or_corrupted_trace_is_not_accepted(self):
        row = self.manifest["traces"][0]["trace"]
        path = self.scratch / "trace.json"
        with self.assertRaisesRegex(probe.cache.InputError, "Missing input"):
            probe.cache.checked_file(path, row)
        path.write_bytes((probe.ROOT / row["path"]).read_bytes())
        probe.cache.checked_file(path, row)
        data = bytearray(path.read_bytes())
        data[-1] ^= 1
        path.write_bytes(data)
        with self.assertRaisesRegex(probe.cache.InputError, "SHA-256"):
            probe.cache.checked_file(path, row)

    def test_native_caught_errors_are_explicit(self):
        self.assertTrue(probe.native_errors("Exception ClassCastException\n thrown in method <foo> in 'client'\n"))
        self.assertTrue(probe.native_errors("ERROR injected-client - Client error: camera failure"))

    def test_unknown_server_initialization_is_not_claimed_as_bound(self):
        self.assertFalse(self.manifest["source_spawn_camera_defaults_observed"])
        self.assertGreaterEqual(len(self.contract["initial_and_reset_provenance"]["genuine_unknowns"]), 3)
        self.assertIn("cannot represent", self.contract["downstream_requirements"]["existing_shell_shape"])
        self.assertFalse(self.contract["production_implemented"])
        self.assertFalse(self.contract["new_reference_policy_or_acceptance"])


if __name__ == "__main__":
    unittest.main()
