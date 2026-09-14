import copy
import importlib.util
import json
from pathlib import Path
import unittest

SPEC = importlib.util.spec_from_file_location("capture_hud_tests", Path(__file__).with_name("capture.py"))
CAPTURE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CAPTURE)


class NativeHudTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.manifest = json.loads((CAPTURE.HUD_OUTPUT / "captures.json").read_text())
        cls.records = {record["path"]: record for record in cls.manifest["captures"]}

    def record(self, name):
        return copy.deepcopy(self.records[f"hud/{name}.png"])

    def test_complete_native_frame_and_panel_family(self):
        result = CAPTURE.validate(CAPTURE.HUD_OUTPUT)
        self.assertEqual(result["captures"], 16)
        self.assertEqual(result["counts"], {"original-runtime-hud-fixture": 16})
        self.assertFalse(result["owner_reference_pack_approved"])

    def test_native_geometry_not_zero_placeholder_coordinates(self):
        record = self.record("native-inventory")
        regions = {region["name"]: region for region in record["source"]["native_ui_regions"]}
        self.assertEqual(regions["minimap"]["bounds"], [1709, 0, 211, 207])
        self.assertEqual(regions["chat"]["bounds"], [0, 915, 519, 165])
        self.assertEqual(regions["sidebar"]["bounds"], [1679, 745, 241, 335])
        self.assertEqual(regions["active-panel"]["bounds"], [1704, 782, 190, 261])

    def test_missing_native_chat_attachment_rejected(self):
        record = self.record("native-inventory")
        record["source"]["component_links"] = [
            link for link in record["source"]["component_links"]
            if int(link["parent_component"]) != (161 << 16 | 96)
        ]
        with self.assertRaisesRegex(ValueError, "chatbox/orbs"):
            CAPTURE.validate_hud_record(record)

    def test_blank_panel_not_hidden_by_nonblank_world(self):
        record = self.record("native-inventory")
        record["source"]["native_ui_regions"][-1]["colors"] = 1
        with self.assertRaisesRegex(ValueError, "region is blank"):
            CAPTURE.validate_hud_record(record)

    def test_fabricated_unlock_family_rejected(self):
        record = self.record("family-guide")
        record["settings"]["enabled_tab_slots"] = list(range(14))
        with self.assertRaisesRegex(ValueError, "unlock family"):
            CAPTURE.validate_hud_record(record)

    def test_transport_or_login_in_fixture_rejected(self):
        for field in ["network_transport_connected", "login_handler_invoked"]:
            record = self.record("native-inventory")
            record["settings"][field] = True
            with self.assertRaisesRegex(ValueError, "transport"):
                CAPTURE.validate_hud_record(record)

    def test_native_weapon_name_and_combat_style_are_consistent(self):
        record = self.record("native-combat")
        labels = " ".join(widget["text"] for widget in record["source"]["visible_widgets"]
                          if widget["id"] >> 16 == 593)
        self.assertIn("Bronze sword", labels)
        self.assertIn("Stab", labels)
        self.assertNotIn("Punch", labels)

    def test_native_script_errors_fail_even_when_caught(self):
        errors = CAPTURE.native_render_errors("[main] ERROR injected-client - Client error: 901 3408\n")
        self.assertEqual(len(errors), 1)

    def test_progressive_attachments_are_actual_native_state(self):
        guide = self.record("family-guide")
        survival = self.record("family-survival")
        magic = self.record("family-magic")
        self.assertEqual(guide["settings"]["enabled_tab_slots"], [10, 11])
        self.assertEqual(survival["settings"]["enabled_tab_slots"], [1, 3, 10, 11])
        self.assertEqual(magic["settings"]["enabled_tab_slots"], [0, 1, 2, 3, 4, 5, 6, 10, 11])
        for record in [guide, survival, magic]:
            CAPTURE.validate_hud_record(record)
            self.assertFalse(record["authenticated_source_journey"])


if __name__ == "__main__":
    unittest.main()
