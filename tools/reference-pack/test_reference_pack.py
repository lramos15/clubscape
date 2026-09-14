"""Source evidence validation, including deliberate missing/corrupt/misclassified inputs."""

import copy
import io
import json
from pathlib import Path
import tempfile
import unittest

from PIL import Image

from components import ROOT, OUT, digest, find_component, text_width
from fetch import image_facts
from media import mp4_facts
from validate import EvidenceError, checked_path, decode_public, require_complete, structural, verify_file, verify_proposal


class PackTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.manifest = json.loads((OUT / "manifest.json").read_text())

    def rejects(self, mutation):
        value = copy.deepcopy(self.manifest)
        mutation(value)
        with self.assertRaises(EvidenceError):
            structural(value)

    def test_actual_structure(self):
        structural(self.manifest)

    def test_missing_required_case(self):
        self.rejects(lambda value: value["cases"].pop())

    def test_duplicate_required_case(self):
        self.rejects(lambda value: value["cases"].append(value["cases"][0]))

    def test_missing_case_input(self):
        self.rejects(lambda value: value["cases"][0]["input_ids"].append("wiki.missing"))

    def test_complete_inventory_not_duplicate_fixture(self):
        self.rejects(lambda value: value["original_inputs"][0].update(path=value["original_inputs"][1]["path"]))

    def test_missing_source_asset(self):
        self.rejects(lambda value: value["source_asset_inventory"].pop())

    def test_wrong_public_capture_role(self):
        self.rejects(lambda value: value["public_inputs"][0].update(source_role="current_original_runtime_fixture"))

    def test_wrong_public_file_identity(self):
        self.rejects(lambda value: value["public_inputs"][0].update(title="File:A different capture.png"))

    def test_crop_cannot_be_full_classic_frame(self):
        self.rejects(lambda value: value["public_inputs"][0].update(full_resizable_classic_frame=True))

    def test_icon_cannot_be_equipment_panel(self):
        def mutation(value):
            entry = next(entry for entry in value["public_inputs"] if entry["id"] == "wiki.equipment-stats-png")
            entry["media_scope"] = "ui_panel_crop"
        self.rejects(mutation)

    def test_case_cannot_replace_required_panel_with_icon(self):
        def mutation(value):
            case = next(case for case in value["cases"] if case["id"] == "case.ui.equipment")
            case["input_ids"] = ["wiki.equipment-stats-png"]
            case["input_roles"] = []
        self.rejects(mutation)

    def test_no_invented_capture_date(self):
        self.rejects(lambda value: value["public_inputs"][0].update(capture_timestamp="2026-09-14T00:00:00Z"))

    def test_no_invented_camera(self):
        self.rejects(lambda value: value["public_inputs"][0]["capture_settings"].update(camera_position=[0, 0, 0]))

    def test_no_invented_original_settings(self):
        self.rejects(lambda value: value["original_inputs"][0]["settings"].update(zoom=999))

    def test_no_self_approval(self):
        self.rejects(lambda value: value["approval"].update(owner_reference_pack_approved=True))

    def test_no_incomplete_ready_flag(self):
        self.rejects(lambda value: value.update(ready_for_owner_approval=True))

    def test_proposal_is_not_source_capture(self):
        self.rejects(lambda value: value["proposal_inputs"][0].update(source_capture=True))

    def test_predeclared_numeric_policy_cannot_be_relaxed(self):
        self.rejects(lambda value: value["cases"][0]["numeric_tolerances"].update(changed_pixel_fraction_max=1))

    def test_no_whole_panel_mask(self):
        self.rejects(lambda value: value["cases"][0].update(mask_regions=[[0, 0, 1920, 1080]]))

    def test_stage_evidence_not_promoted(self):
        self.rejects(lambda value: value["cases"][0].update(exact_stage_screenshot_available=True))

    def test_silent_audio_is_preserved(self):
        self.rejects(lambda value: value["audio"]["source_silences"][0].update(playable_output="made-up.flac"))

    def test_gaps_cannot_be_dropped(self):
        self.rejects(lambda value: value["source_gaps"].pop())

    def test_unrun_mac_not_certified(self):
        self.rejects(lambda value: value["browsers_and_hardware"].update(mac_edge_tested=True))

    def test_input_usage_complete(self):
        self.rejects(lambda value: value["input_usage"].pop())

    def test_missing_actual_file(self):
        with self.assertRaisesRegex(EvidenceError, "Missing input"):
            verify_file({"path": "assets/reference/wiki/not-present.png", "sha256": "0" * 64})

    def test_changed_actual_file_hash(self):
        entry = copy.deepcopy(self.manifest["public_inputs"][0])
        entry["sha256"] = "0" * 64
        with self.assertRaisesRegex(EvidenceError, "Changed SHA-256"):
            verify_file(entry)

    def test_actual_decode_rejects_wrong_dimensions(self):
        entry = copy.deepcopy(next(entry for entry in self.manifest["public_inputs"] if entry["decoded"]["format"] == "PNG"))
        entry["decoded"]["dimensions"][0] += 1
        with self.assertRaisesRegex(EvidenceError, "decode/dimensions"):
            decode_public(entry)

    def test_actual_decode_rejects_non_image_bytes(self):
        with self.assertRaises(OSError):
            image_facts(b"\x89PNG\r\n\x1a\nnot an actual decoded image")

    def test_source_mp4_container_and_truncation(self):
        entry = next(entry for entry in self.manifest["public_inputs"] if entry["decoded"]["format"] == "MP4")
        data = (ROOT / entry["path"]).read_bytes()
        facts = mp4_facts(data)
        self.assertEqual(facts["dimensions"], [1920, 1080])
        self.assertGreater(facts["frame_count"], 1000)
        with self.assertRaises(ValueError):
            mp4_facts(data[:-10])

    def test_path_traversal_rejected(self):
        for path in ("../other.txt", "/etc/passwd", "assets/../../outside"):
            with self.assertRaises(EvidenceError):
                checked_path(path)

    def test_source_font_metrics_exact(self):
        phrase = "Old School RuneScape  |  Attack: 1  Hitpoints: 10  Coins: 1,000"
        self.assertEqual([text_width(phrase, font) for font in (494, 495, 496, 497)], [303, 341, 403, 432])

    def test_reconciliation_compares_entire_opaque_component(self):
        sprite = Image.new("RGBA", (10, 10), (30, 40, 50, 255))
        actual = Image.new("RGB", (20, 20), (0, 0, 0))
        actual.paste(sprite, (5, 5), sprite)
        self.assertTrue(find_component(actual, sprite))
        actual.putpixel((12, 12), (255, 0, 0))
        self.assertEqual(find_component(actual, sprite), [])

    def test_small_source_is_not_a_search_error(self):
        self.assertEqual(find_component(Image.new("RGB", (2, 2)), Image.new("RGBA", (20, 20))), [])

    def test_proposal_cannot_change_unchanged_source_region(self):
        scratch = ROOT / ".local/reference-pack/tests"
        scratch.mkdir(parents=True, exist_ok=True)
        entry = copy.deepcopy(self.manifest["proposal_inputs"][0])
        with tempfile.TemporaryDirectory(dir=scratch) as directory:
            path = Path(directory) / "altered-proposal.png"
            with Image.open(ROOT / entry["path"]) as source:
                image = source.convert("RGB")
            image.putpixel((0, 0), (255, 0, 0))
            image.save(path)
            entry.update(digest(path))
            with self.assertRaisesRegex(EvidenceError, "outside its declared titlebox"):
                verify_proposal(entry)

    def test_source_completeness_gate_stays_blocked(self):
        with self.assertRaisesRegex(EvidenceError, "Mandatory source evidence"):
            require_complete(self.manifest)


if __name__ == "__main__":
    unittest.main()
