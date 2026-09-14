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
from factoring import review_ready
from text_oracles import check_full_panel_partition, check_source_values, check_text_projection, project_record


class PackTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.manifest = json.loads((OUT / "manifest.json").read_text())
        cls.oracles = json.loads((OUT / "dynamic-text-oracles.json").read_text())

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

    def test_no_microstate_capture_gate(self):
        factor = self.manifest["evidence_factorization"]
        self.assertEqual(len(factor["state_bindings"]), 71)
        self.assertFalse(any(row["separate_source_screenshot_required"] for row in factor["state_bindings"]))
        self.assertFalse(any(gap["id"] in ("gap.tutorial_matched_states", "gap.arrival_state")
                             for gap in self.manifest["source_gaps"]))

    def test_missing_visual_family(self):
        self.rejects(lambda value: value["evidence_factorization"]["families"].pop())

    def test_duplicate_visual_family(self):
        self.rejects(lambda value: value["evidence_factorization"]["families"].append(
            value["evidence_factorization"]["families"][0]))

    def test_distinct_visual_mode_cannot_disappear(self):
        def mutation(value):
            family = next(row for row in value["evidence_factorization"]["families"] if row["id"] == "ui.magic")
            family["required_visual_variants"].remove("level_filtered")
        self.rejects(mutation)

    def test_player_dialogue_not_collapsed_into_npc(self):
        def mutation(value):
            family = next(row for row in value["evidence_factorization"]["families"] if row["id"] == "dialogue.flow")
            family["required_visual_variants"].remove("player_speaker")
        self.rejects(mutation)

    def test_factoring_cannot_drop_semantic_state(self):
        self.rejects(lambda value: value["evidence_factorization"]["state_bindings"].pop())

    def test_factoring_cannot_demand_71_source_screens(self):
        self.rejects(lambda value: value["evidence_factorization"]["state_bindings"][0].update(
            separate_source_screenshot_required=True))

    def test_instructor_phase_cannot_disappear(self):
        self.rejects(lambda value: value["evidence_factorization"]["phases"].pop())

    def test_hud_signature_cannot_disappear(self):
        self.rejects(lambda value: value["evidence_factorization"]["hud_signatures"].pop())

    def test_unmentioned_tabs_not_guessed_hidden(self):
        self.rejects(lambda value: value["evidence_factorization"]["hud_signatures"][0].update(
            unmentioned_controls="all hidden"))

    def test_dynamic_scalar_wrong_reward_rejected(self):
        self.rejects(lambda value: value["evidence_factorization"]["dynamic_value_oracles"]["source_facts"].update(
            cooks_assistant_cooking_xp_tenths=300))

    def test_journal_carried_not_collapsed_into_delivered(self):
        self.rejects(lambda value: value["evidence_factorization"]["dynamic_value_oracles"][
            "quest_journal_distinct_states"]["per_ingredient_states"].remove("carried"))

    def test_dynamic_source_text_selector_required(self):
        self.rejects(lambda value: value["evidence_factorization"]["state_bindings"][0].update(
            source_text_record_ids=[]))

    def test_family_cannot_mask_panel(self):
        self.rejects(lambda value: value["evidence_factorization"]["families"][0].update(
            required_panel_pixel_coverage=0.5))

    def test_final_acceptance_obligations_remain(self):
        self.rejects(lambda value: value["evidence_factorization"]["acceptance_obligations"].pop())

    def test_readiness_is_not_approval_or_always_false(self):
        requirements = [{"status": "available", "evidence_ids": ["synthetic-unit-test-source"]}]
        self.assertTrue(review_ready(requirements))
        self.assertFalse(any(self.manifest["approval"].values()))
        self.assertFalse(review_ready([]))
        self.assertFalse(review_ready([{"status": "available", "evidence_ids": []}]))
        self.assertFalse(review_ready([{"status": "missing_native_reference", "evidence_ids": ["x"]}]))

    def test_pinned_text_and_native_font_projection(self):
        record = next(row for row in self.oracles["records"]
                      if row["static_source_font_binding"] and not row["conditional_source_tokens"])
        expected = project_record(record)
        font_id = record["static_source_font_binding"]["font_id"]
        advances = expected["metrics_by_font"][str(font_id)]["codepoint_advances_px"]
        result = check_text_projection(record, expected["text"], font_id, advances, actual_runs=expected["runs"])
        self.assertTrue(result["static_font_binding_checked"])
        self.assertFalse(result["pixel_or_candidate_acceptance"])
        with self.assertRaisesRegex(ValueError, "differs from pinned"):
            check_text_projection(record, "A plausible but invented line.", font_id, advances, actual_runs=expected["runs"])
        altered = copy.deepcopy(advances)
        altered[0][0] += 1
        with self.assertRaisesRegex(ValueError, "advances differ"):
            check_text_projection(record, expected["text"], font_id, altered, actual_runs=expected["runs"])
        with self.assertRaisesRegex(ValueError, "Wrong font"):
            check_text_projection(record, expected["text"], 494, expected["metrics_by_font"]["494"]["codepoint_advances_px"],
                                  actual_runs=expected["runs"])
        wrong_style = copy.deepcopy(expected["runs"])
        wrong_style[0]["strikethrough"] = not wrong_style[0]["strikethrough"]
        with self.assertRaisesRegex(ValueError, "colour/strikethrough"):
            check_text_projection(record, expected["text"], font_id, advances, actual_runs=wrong_style)

    def test_unrecorded_source_line_is_not_fabricated(self):
        record = next(row for row in self.oracles["records"] if row["desktop_text"] is None)
        with self.assertRaisesRegex(ValueError, "No independent source text"):
            project_record(record)

    def test_conditional_source_branch_requires_explicit_selection(self):
        record = next(row for row in self.oracles["records"] if row["conditional_source_tokens"])
        with self.assertRaisesRegex(ValueError, "explicit permitted branch"):
            project_record(record)
        choices = {token: token[1:-1].split("/")[0] for token in record["conditional_source_tokens"]}
        self.assertIsInstance(project_record(record, choices)["text"], str)
        choices[record["conditional_source_tokens"][0]] = "invented"
        with self.assertRaisesRegex(ValueError, "Invented source branch"):
            project_record(record, choices)

    def test_journal_source_strikethrough_and_colour_preserved(self):
        journal = [row for row in self.oracles["records"] if row["kind"] == "journal"]
        self.assertTrue(any(run["strikethrough"] for row in journal for run in row["runs"]))
        self.assertTrue(any(run["color"] == "#800000" for row in journal for run in row["runs"]))
        self.assertTrue(any(row["desktop_text"] == "QUEST COMPLETE!" for row in journal))

    def test_dynamic_value_oracle_checks_actual_values(self):
        facts = self.manifest["evidence_factorization"]["dynamic_value_oracles"]["source_facts"]
        self.assertTrue(check_source_values(facts, {"tutorial_bank_first_open_coins": 25},
                                           ["tutorial_bank_first_open_coins"]))
        with self.assertRaisesRegex(ValueError, "differs from source"):
            check_source_values(facts, {"tutorial_bank_first_open_coins": 1000}, ["tutorial_bank_first_open_coins"])

    def test_full_panel_partition_no_mask_or_unchecked_pixels(self):
        fixed = {"rectangle": [0, 0, 20, 10], "verifier": "source_pixels", "source_input_ids": ["unit.source.frame"]}
        dynamic = {"rectangle": [0, 10, 20, 10], "verifier": "source_background_and_glyphs",
                   "source_input_ids": ["unit.source.background", "unit.source.font"]}
        self.assertTrue(check_full_panel_partition(20, 20, [fixed, dynamic]))
        with self.assertRaisesRegex(ValueError, "unchecked"):
            check_full_panel_partition(20, 20, [fixed])
        with self.assertRaisesRegex(ValueError, "never masked"):
            check_full_panel_partition(20, 20, [fixed, {**dynamic, "skip": True}])
        with self.assertRaisesRegex(ValueError, "multiply-owned"):
            check_full_panel_partition(20, 20, [fixed, fixed, dynamic])
        with self.assertRaisesRegex(ValueError, "independent source"):
            check_full_panel_partition(20, 20, [{**fixed, "source_input_ids": []}, dynamic])

    def test_empty_panel_cannot_vacuously_pass(self):
        for width, height in ((0, 0), (10, 0), (-1, 10), (True, 10)):
            with self.assertRaisesRegex(ValueError, "positive bounded"):
                check_full_panel_partition(width, height, [])

    def test_fractional_pixel_partition_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "integer coordinates"):
            check_full_panel_partition(10, 10, [{"rectangle": [0.5, 0, 9, 10],
                "verifier": "source_pixels", "source_input_ids": ["unit.source"]}])


if __name__ == "__main__":
    unittest.main()
