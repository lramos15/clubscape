#!/usr/bin/env python3
"""Fail closed on changed/missing/misclassified evidence; report source gaps separately."""

import argparse
from collections import Counter
from datetime import datetime, timezone
import gzip
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import sys

from PIL import Image, ImageChops

from catalogue import EXTRA_CASES, NUMERIC_PROFILES, dispositions, wiki
from components import ROOT, OUT, digest, load_gzip, sprite, text_width, widget_label
from fetch import image_facts, write_json
from media import mp4_facts
from factoring import (
    ACCEPTANCE_OBLIGATIONS, COMPARISON_FACTORIZATION, FAMILY_DEFINITIONS,
    assess_source_requirements, review_ready, stage_visual_map, text_selectors,
)
from text_oracles import make_text_oracles
from native_hud import (
    FAMILY_PANELS, calibration as native_hud_calibration, case_input_ids as native_hud_case_ids,
    input_id as native_hud_id, validate_records as validate_native_hud,
)
from audio_reference import source_contract as current_audio_contract, validate_contract as validate_audio_contract, wave_facts, case_selector_ids, resolved_action_map


class EvidenceError(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise EvidenceError(message)


def checked_path(path):
    candidate = ROOT / path
    require(not Path(path).is_absolute() and ".." not in Path(path).parts, f"Unsafe input path: {path}")
    require(candidate.resolve().is_relative_to(ROOT.resolve()), f"Input escapes worktree: {path}")
    return candidate


def verify_file(record):
    path = checked_path(record["path"])
    require(path.is_file(), f"Missing input: {record['path']}")
    actual = digest(path)
    require(actual["sha256"] == record["sha256"], f"Changed SHA-256 input: {record['path']}")
    if "size_bytes" in record:
        require(actual["size_bytes"] == record["size_bytes"], f"Incorrect byte length: {record['path']}")
    return path


def unique(records, key, label):
    values = [record[key] for record in records]
    require(len(values) == len(set(values)), f"Duplicate {label}")
    return set(values)


def structural(manifest):
    require(manifest["schema_version"] == 1 and manifest["pack_version"] == "1.3.0", "Unsupported pack schema/version")
    require(manifest["status"] == "awaiting_owner_approval", "Pack must remain awaiting_owner_approval")
    factoring = manifest["evidence_factorization"]
    ready = review_ready(factoring["source_review_requirements"])
    require(manifest["ready_for_owner_review"] == ready and manifest["ready_for_owner_approval"] == ready,
            "Readiness must follow actual source-input satisfaction, not a fixed flag or candidate results")
    require(not any(manifest["approval"].values()), "Evidence preparation cannot grant product/owner acceptance")
    require(manifest["source_build"] == 240 and manifest["source_cache"] == 2695, "Wrong current source selection")
    require(manifest["primary_target"]["logical_viewport"] == [1920, 1080], "Wrong primary target")
    require(manifest["primary_target"]["dpr"] == 1 and manifest["primary_target"]["ui_scale"] == 1, "Wrong native UI/DPR")
    require(manifest["primary_target"]["layout"] == "Resizable - Classic", "Wrong stock target layout")
    require(manifest["resize_proposal"]["min_logical_viewport"] == [1024, 768], "Changed resize floor")
    require(manifest["resize_proposal"]["max_logical_viewport"] == [2560, 1440], "Changed resize ceiling")
    require(not manifest["resize_proposal"]["game_resize_verified"], "Gallery/anchor checks are not game resize proof")
    require(not manifest["browsers_and_hardware"]["mac_chrome_tested"]
            and not manifest["browsers_and_hardware"]["mac_edge_tested"], "Unrun Mac result asserted")
    original_ids = unique(manifest["original_inputs"], "id", "original input ID")
    native_hud_ids = unique(manifest["native_hud_inputs"], "id", "native HUD input ID")
    public_ids = unique(manifest["public_inputs"], "id", "public input ID")
    proposal_ids = unique(manifest["proposal_inputs"], "id", "proposal ID")
    frame_ids = unique(manifest["recording_frames"], "id", "recording frame ID")
    audio_ids = unique(manifest["audio"]["assets"], "asset_id", "audio asset ID")
    template_ids = unique(manifest["audio"]["reference_templates"], "id", "source audio template ID")
    all_ids = original_ids | native_hud_ids | public_ids | proposal_ids | frame_ids | audio_ids | template_ids
    require(len(all_ids) == sum(map(len, (original_ids, native_hud_ids, public_ids, proposal_ids, frame_ids, audio_ids, template_ids))),
            "Cross-class duplicate input ID")
    cases = manifest["cases"]
    ids = unique(cases, "id", "required case ID")
    tutorial = json.loads((ROOT / "research/journey-rules/tutorial.json").read_text())
    expected = {"case.tutorial." + state["id"].removeprefix("stage.tutorial.") for state in tutorial["states"]}
    expected |= {"case." + row[0] for row in EXTRA_CASES}
    require(ids == expected, f"Missing/extra required cases: {sorted(ids ^ expected)}")
    catalogue = json.loads(verify_file(manifest["required_case_catalogue"]).read_text())
    require(set(catalogue["required_ids"]) == expected and len(catalogue["required_ids"]) == len(expected),
            "Required case catalogue has missing/duplicate IDs")
    states = {state["id"]: state for state in tutorial["states"]}
    require(len(states) == 71, "Unexpected tutorial contract state count")
    require(manifest["counts"]["required_cases"] == len(ids), "Incorrect required case count")
    role_by_id = {entry["id"]: entry["source_role"] for entry in
                  manifest["original_inputs"] + manifest["native_hud_inputs"] + manifest["public_inputs"] + manifest["proposal_inputs"]}
    role_by_id.update({entry["asset_id"]: "current_original_audio_input" for entry in manifest["audio"]["assets"]})
    role_by_id.update({entry["id"]: "current_original_audio_template" for entry in manifest["audio"]["reference_templates"]})
    for case in cases:
        require(case["input_ids"] and set(case["input_ids"]) <= all_ids, f"Missing case input: {case['id']}")
        require(len(case["input_ids"]) == len(set(case["input_ids"])), f"Duplicate case input: {case['id']}")
        require(case["numeric_tolerances"] == NUMERIC_PROFILES[case["comparison_profile"]],
                f"Changed or absent pre-candidate numeric tolerance: {case['id']}")
        require("mask_regions" not in case, f"Undeclared exclusion mask: {case['id']}")
        require(case["product_acceptance"] == "not_run", "Source case promoted to product acceptance")
        require([entry["input_id"] for entry in case["input_roles"]] == case["input_ids"],
                f"Incomplete per-case source roles: {case['id']}")
        for reference in case["input_roles"]:
            require(reference["source_role"] == role_by_id[reference["input_id"]], "Incorrect per-case source role")
            require(reference["numeric_tolerances"] == NUMERIC_PROFILES[reference["comparison_profile"]],
                    "Missing/changed per-input numeric policy")
            if reference["input_id"] in native_hud_ids:
                require(reference["comparison_profile"] == "native_hud", "Native full-frame tolerance profile changed")
        if case["family"] == "tutorial":
            state = states[case["journey_state_id"]]
            require(case["declared_controls"] == state.get("ui_unlock_refs", []), "Changed tutorial unlock declaration")
            require(case["expected_instruction"] == state["instruction"], "Changed source semantic stage")
            require(case["source_numeric_progress"] is None, "Invented numeric source tutorial progress")
            require("exact_stage_screenshot_available" not in case, "Per-microstate screenshot predicate reintroduced")
            require(case["source_pixel_evidence"] ==
                    "shared_actual_family_inputs; no separate source-session capture claimed",
                    "Shared evidence relabeled as a captured source session")
    by_case = {case["id"]: case for case in cases}
    for suffix, _, kind, names, _, _ in EXTRA_CASES:
        if kind == "public":
            require({wiki(name) for name in names} <= set(by_case["case." + suffix]["input_ids"]),
                    f"Required actual panel/capture replaced by an unrelated component: {suffix}")
    policy = dispositions()
    require(public_ids == set(policy), "Missing/extra public originals or unclassified acquisition")
    metrics = json.loads(verify_file(manifest["component_reconciliation"]).read_text())
    for entry in manifest["public_inputs"]:
        require(entry["id"] == wiki(entry["title"].removeprefix("File:")), "Public source ID/title mismatch")
        for key, value in policy[entry["id"]].items():
            require(entry[key] == value, f"Incorrect public image classification: {entry['id']} / {key}")
        matches = metrics["public_component_matches"].get(entry["id"], [])
        text_matches = metrics["public_text_matches"].get(entry["id"], [])
        role = "reconciled_public_source_capture" if matches or text_matches else "public_source_capture_unreconciled"
        require(entry["source_role"] == role, f"Unproven reconciliation role: {entry['id']}")
        require(entry["native_component_match_count"] == len(matches), "Incorrect component proof count")
        require(entry["native_text_match_count"] == len(text_matches), "Incorrect text proof count")
        require(entry["capture_timestamp"] is None and entry["capture_build"] is None, "Invented public capture date/build")
        settings = entry["capture_settings"]
        require(all(settings[key] is None for key in settings if key != "unknown_policy"),
                "Unknown public settings silently filled")
        require(entry["mediawiki_original_sha1_verified"] is True, "Unverified original media bytes")
    canonical = json.loads((ROOT / "assets/reference/osrs240/captures.json").read_text())
    require(len(original_ids) == 93, "Original fixture inventory is incomplete")
    by_path = {entry["path"]: entry for entry in canonical["captures"]}
    require(unique(manifest["original_inputs"], "path", "original fixture path") ==
            {"assets/reference/osrs240/" + path for path in by_path}, "Missing original fixture/frame")
    require(len(unique(manifest["public_inputs"], "path", "public original path")) == len(public_ids),
            "Public original path duplicated")
    for entry in manifest["original_inputs"]:
        source = by_path[entry["path"].removeprefix("assets/reference/osrs240/")]
        for key in ("sha256", "size_bytes", "source", "settings", "kind", "width", "height"):
            require(entry[key] == source[key], f"Changed original capture metadata: {entry['id']} / {key}")
        require(entry["source_role"] == "current_original_runtime_fixture"
                and entry["source_capture_build"] == 240 and not entry["full_resizable_classic_frame"],
                "Original controlled fixture misclassified as a complete source journey/HUD")
        require(entry["browser"] is None and entry["browser_dpr"] is None, "Invented original browser settings")
    try:
        validate_native_hud(manifest["native_hud_inputs"])
    except ValueError as error:
        raise EvidenceError(str(error)) from error
    require(manifest["counts"]["original_pre_hud_images"] == 93
            and manifest["counts"]["native_hud_original_images"] == 16
            and manifest["counts"]["original_runtime_images"] == 109, "Wrong original/native image inventory")
    audio = json.loads((ROOT / "assets/manifests/osrs/audio-runtime.json").read_text())
    require(manifest["audio"]["assets"] == audio["assets"], "Existing original audio inventory changed")
    require(len(audio_ids) == 264 and len(template_ids) == 2, "Missing corrected audio or required source cue templates")
    require(manifest["audio"]["source_silences"] == audio["source_silences"], "Source silence2411 changed/dropped")
    require(manifest["audio"]["source_silences"][0]["playable_output"] is None, "Silence falsely assigned playable file")
    require(not manifest["audio"]["conversion_repeated"]
            and not manifest["audio"]["live_source_mixer_calibrated"], "False audio conversion/calibration claim")
    validate_audio_application(manifest)
    validate_factoring(manifest, factoring)
    requirements = {row["id"]: row for row in factoring["source_review_requirements"]}
    gaps = {entry["id"]: entry for entry in manifest["source_gaps"]}
    require(set(gaps) == {key for key, row in requirements.items() if row["status"] != "available"},
            "Source gaps must be exactly the unsatisfied literal input requirements")
    for identifier, gap in gaps.items():
        affected = {case["id"] for case in cases if identifier in case["source_gap_refs"]}
        require(affected and affected == set(gap["required_case_ids"]), f"Incomplete exact gap mapping: {identifier}")
        require(not gap["account_login_required_by_pack"], "Blanket account blocker reintroduced")
    require(all(set(case["source_gap_refs"]) <= set(gaps) for case in cases), "Dangling source-gap reference")
    require(manifest["counts"]["source_gap_cases"] == sum(bool(case["source_gap_refs"]) for case in cases),
            "Incorrect incomplete-case count")
    require(unique(manifest["input_usage"], "input_id", "input usage ID") == all_ids, "Incomplete inventory usage map")
    for usage in manifest["input_usage"]:
        expected_cases = {case["id"] for case in cases if usage["input_id"] in case["input_ids"]}
        require(set(usage["case_ids"]) == expected_cases, "Incorrect actual input-to-case mapping")
    for proposal in manifest["proposal_inputs"]:
        require(proposal["source_role"] == "composition_proposal" and not proposal["source_capture"]
                and not proposal["approved"] and not proposal["game_client"], "Proposal promoted to capture/approval/client")
        require("OWNER-REVIEW PROPOSAL" in proposal["label"], "Missing visible proposal classification")
    require(len(proposal_ids) == 7, "Missing concrete web-only proposal")
    inventory = json.loads((ROOT / "assets/manifests/osrs/cache2695-published.json").read_text())["published_files"]
    actual_inventory = manifest["source_asset_inventory"]
    require(len(unique(actual_inventory, "path", "published source asset path")) == 559,
            "Missing original published source asset")
    require({entry["path"]: entry["sha256"] for entry in inventory} ==
            {entry["path"]: entry["sha256"] for entry in actual_inventory}, "Original source inventory altered")
    return metrics


def validate_audio_application(manifest):
    contract = current_audio_contract()
    require(manifest["approved_audio_adaptations"] == contract["owner_audio_adaptations"],
            "Exact owner audio approval scope/hash or adaptation classification changed")
    require(manifest["audio"]["selectors"] == contract["selectors"],
            "Qualified/approved audio selectors changed or falsely certified current")
    require(manifest["audio"]["reference_templates"] == contract["reference_templates"],
            "Missing/changed original cue templates")
    require(manifest["audio"]["current_audio_proof"] == contract["current_audio_proof"],
            "Obsolete pre-correction musical proof retained")
    require(manifest["audio"]["native_queue_rules"] == contract["native_queue_rules"],
            "Native queue/offset/priority semantics changed")
    require(manifest["audio"]["actions"] == resolved_action_map(contract),
            "Stale or incorrect active source cue bindings")
    require(manifest["audio"]["remaining_bindings"] == [], "Closed audio blockers restated as missing inputs")
    selectors = {row["id"]: row for row in contract["selectors"]}
    for case in manifest["cases"]:
        expected = case_selector_ids(case, selectors)
        require(case["audio_selector_refs"] == expected, "Missing case audio selector mapping")
        adaptations = [selectors[key]["adaptation_id"] for key in expected if selectors[key]["classification"] == "approved_adaptation"]
        require(case["approved_audio_adaptation_refs"] == adaptations, "Case lost exact approved-adaptation label")
        for key in expected:
            require(set(selectors[key]["payload_ids"]) <= set(case["input_ids"]), "Selected cue lacks actual case input bytes")
    require(manifest["counts"]["playable_original_audio_files"] == 264
            and manifest["counts"]["additional_original_cue_wav_references"] == 2
            and manifest["counts"]["owner_approved_audio_selector_adaptations"] == 2, "Incorrect corrected audio/approval counts")


def validate_factoring(manifest, factoring):
    definitions = {row[0]: row for row in FAMILY_DEFINITIONS}
    families = {row["id"]: row for row in factoring["families"]}
    require(len(families) == len(factoring["families"]) and set(families) == set(definitions),
            "Missing/duplicate required visual family")
    input_ids = {row["id"] for row in manifest["original_inputs"] + manifest["native_hud_inputs"] + manifest["public_inputs"]
                 + manifest["proposal_inputs"] + manifest["recording_frames"]}
    input_ids |= {row["asset_id"] for row in manifest["audio"]["assets"]}
    input_ids |= {row["id"] for row in manifest["audio"]["reference_templates"]}
    case_map = {case["id"]: case for case in manifest["cases"]}
    all_variants = set()
    for identifier, definition in definitions.items():
        family = families[identifier]
        require(family["required_visual_variants"] == definition[2].split(),
                f"Distinct visual state disappeared/changed: {identifier}")
        require(family["pixel_or_audio_input_ids"] and set(family["pixel_or_audio_input_ids"]) <= input_ids,
                f"Missing source inputs for family: {identifier}")
        require(family["representative_input_ids"] and set(family["representative_input_ids"]) <=
                set(family["pixel_or_audio_input_ids"]), "Family representative is not an actual input")
        require(family["required_panel_pixel_coverage"] == 1.0
                and family["candidate_panel_pixel_coverage_measured"] is None,
                "Whole-panel checking was relaxed or an unrun candidate result asserted")
        require(family["comparison_lanes"] ==
                ["whole_representative_source_state", "full_coverage_dynamic_state_projection"],
                "Dynamic factoring removed the whole-source-state comparison")
        expected_cases = {case["id"] for case in manifest["cases"] if identifier in case["reference_family_ids"]}
        require(set(family["case_ids"]) == expected_cases, "Incorrect family-to-case map")
        expected_native = [native_hud_id(name) for name in FAMILY_PANELS.get(identifier, [])]
        require(family["native_hud_input_ids"] == expected_native
                and set(expected_native) <= set(family["pixel_or_audio_input_ids"]),
                "Native panel/family evidence omitted")
        all_variants.update(identifier + "." + variant for variant in family["required_visual_variants"])
    require(factoring["counts"]["families"] == len(families)
            and factoring["counts"]["distinct_visual_variants"] == len(all_variants), "Incorrect family/variant count")
    tutorial = json.loads((ROOT / "research/journey-rules/tutorial.json").read_text())
    expected_states = {state["id"] for state in tutorial["states"]}
    bindings = {row["state_id"]: row for row in factoring["state_bindings"]}
    require(len(bindings) == len(factoring["state_bindings"]) == 71 and set(bindings) == expected_states,
            "A factored tutorial state disappeared or was duplicated")
    require(factoring["counts"]["tutorial_states"] == 71, "Factoring reduced the tutorial")
    phase_members = [state for phase in factoring["phases"] for state in phase["state_ids"]]
    require(len(phase_members) == 71 and set(phase_members) == expected_states
            and len(factoring["phases"]) == 11, "Instructor phase/state coverage changed")
    phases = {phase["id"]: phase for phase in factoring["phases"]}
    for case in manifest["cases"]:
        expected_native = native_hud_case_ids(case, phases)
        require(case["native_hud_input_ids"] == expected_native
                and set(expected_native) <= set(case["input_ids"]), "Native evidence dropped from an applicable case")
    signatures = {row["id"]: row for row in factoring["hud_signatures"]}
    signature_members = [state for row in signatures.values() for state in row["state_ids"]]
    require(len(signature_members) == 71 and set(signature_members) == expected_states
            and len(signatures) == len(factoring["hud_signatures"]), "Missing/duplicate progressive HUD signature state")
    visual_map = stage_visual_map()
    selectors = text_selectors()
    text = json.loads(verify_file(manifest["dynamic_text_oracles"]).read_text())
    text_ids = {record["id"] for record in text["records"]}
    introduced = set()
    permanent = {"ui.settings", "ui.inventory", "ui.skills", "ui.quests", "ui.equipment",
                 "ui.combat", "ui.account", "ui.logout", "ui.prayer", "ui.magic"}
    for state in tutorial["states"]:
        suffix = state["id"].removeprefix("stage.tutorial.")
        binding = bindings[state["id"]]
        case = case_map["case.tutorial." + suffix]
        require(binding["case_id"] == case["id"], "Factored state points at another case")
        require(not binding["separate_source_screenshot_required"] and binding["distinct_layout_still_required"],
                "Invented screenshot gate or omitted distinct visual state")
        require(set(visual_map[suffix]) <= set(binding["visual_variant_ids"]) <= all_variants,
                "A genuinely distinct state variant was collapsed")
        require(binding["visual_variant_ids"] == case["distinct_visual_variant_ids"]
                and set(binding["family_ids"]) == set(case["reference_family_ids"]), "Case/family factoring inconsistent")
        require(state["id"] in phases[binding["phase_id"]]["state_ids"], "Wrong instructor phase")
        selector = selectors[suffix]
        chapter = selector["chapter_override"] or phases[binding["phase_id"]]["source_section"]
        require(binding["source_text_selector"] == {"chapter": chapter, "sections": selector["sections"]},
                "Incorrect source text section assigned to state")
        selected_records = {
            record["id"] for record in text["records"]
            if record["source_page"] == "Transcript:Learning the Ropes" and record["section_path"]
            and record["section_path"][0] == chapter
            and any((len(record["section_path"]) == 1 if section is None else section in record["section_path"][1:])
                    for section in selector["sections"])
        }
        require(selected_records and set(binding["source_text_record_ids"]) == selected_records
                and set(case["source_text_record_ids"]) == selected_records, "Dynamic text lost its exact source selector")
        introduced.update(set(state.get("ui_unlock_refs", [])) & permanent)
        signature = signatures[binding["hud_signature_id"]]
        require(signature["expected_introduced_tabs"] == sorted(introduced)
                and state["id"] in signature["state_ids"], "Progressive HUD unlock family disappeared/changed")
        require(signature["unmentioned_controls"].startswith("unknown"), "Unmentioned controls guessed hidden")
        require(signature["display_states_required"] == ["source_locked", "source_highlighted", "unlocked"],
                "Locked/highlighted/unlocked distinction omitted")
        require(binding["declared_controls"] == state.get("ui_unlock_refs", []), "Source control introduction changed")
        require(set(binding["source_text_record_ids"]) <= text_ids, "Unknown text oracle")
    require(factoring["counts"]["hud_signatures"] == len(signatures), "Wrong progressive signature count")
    require(factoring["comparison_factorization"] == COMPARISON_FACTORIZATION,
            "Factoring relaxed numeric pixel/value/whole-panel requirements")
    require(factoring["acceptance_obligations"] == ACCEPTANCE_OBLIGATIONS,
            "Final live journey/source-fidelity/platform/owner gates disappeared")
    require(factoring["source_review_requirements"] ==
            assess_source_requirements(manifest["original_inputs"], manifest["public_inputs"], manifest["native_hud_inputs"]),
            "Source input readiness was changed without qualifying source evidence")
    values = factoring["dynamic_value_oracles"]
    require(values["source_facts"] == {
        "skill_count": 24, "inventory_slots": 28, "tutorial_bank_first_open_coins": 25,
        "initial_ranged_arrow_grant": 50, "initial_magic_grant_air": 5, "initial_magic_grant_mind": 5,
        "learning_the_ropes_quest_points": 1, "cooks_assistant_quest_points": 1,
        "cooks_assistant_cooking_xp_tenths": 3000, "poll_vote_skill_total": 300, "poll_support_percent": 70,
    }, "Pinned dynamic scalar facts changed")
    require(values["quest_journal_distinct_states"]["per_ingredient_states"] == ["missing", "carried", "delivered"],
            "Carried/delivered journal distinction disappeared")
    require(values["arrival"]["camera_role"].startswith("actual recorded controlled fixture"),
            "Arrival fixture relabeled as observed live camera")
    require(manifest["counts"]["separate_tutorial_microstate_source_captures_required"] == 0,
            "A separate source capture was demanded for each micro-transition")


def decode_original(entry):
    path = verify_file(entry)
    data = path.read_bytes()
    facts = image_facts(data)
    require(facts["dimensions"] == [entry["width"], entry["height"]], "Incorrect original decoded dimensions")
    with Image.open(io.BytesIO(data)) as image:
        red, green, blue, alpha = image.convert("RGBA").split()
        argb = Image.merge("RGBA", (alpha, red, green, blue)).tobytes()
    require(hashlib.sha256(argb).hexdigest() == entry["pixel_argb32_be_sha256"], "Original pixels differ from native buffer")


def decode_public(entry):
    path = verify_file(entry)
    data = path.read_bytes()
    require(hashlib.sha1(data).hexdigest() == entry["imageinfo"]["sha1"], "MediaWiki original SHA-1 mismatch")
    require(len(data) == entry["imageinfo"]["size"], "MediaWiki original byte length mismatch")
    source = load_gzip(verify_file(entry["source_snapshot"]))["page"]
    require(source["pageid"] == entry["file_page_id"]
            and source["revisions"][0]["revid"] == entry["file_page_revision"], "Wrong pinned file revision")
    require(source["title"] == entry["title"] and source["imageinfo"][0] == entry["imageinfo"],
            "Changed original MediaWiki identity")
    require(source["revisions"][0]["slots"]["main"]["content"], "Missing source media notice/description")
    facts = mp4_facts(data) if entry["decoded"]["format"] == "MP4" else image_facts(data)
    require(facts == entry["decoded"], f"Incorrect decode/dimensions/frame metadata: {entry['id']}")
    require(facts["dimensions"] == [entry["imageinfo"]["width"], entry["imageinfo"]["height"]],
            "Imageinfo dimensions do not match actual bytes")


def verify_matches(manifest, metrics):
    public = {entry["id"]: entry for entry in manifest["public_inputs"]}
    count = 0
    for mapping, text_mode in ((metrics["public_component_matches"], False), (metrics["public_text_matches"], True)):
        for identifier, matches in mapping.items():
            if not matches:
                continue
            with Image.open(ROOT / public[identifier]["path"]) as source:
                source.seek(0)
                actual = source.convert("RGBA")
            for match in matches:
                reference, _ = widget_label(match["widget_id"]) if text_mode else sprite(match["sprite_group"])
                x, y, width, height = match["image_rectangle"]
                require([width, height] == list(reference.size), "Reconciliation component dimensions changed")
                require(x >= 0 and y >= 0 and x + width <= actual.width and y + height <= actual.height,
                        "Reconciliation rectangle outside actual input")
                candidate = actual.crop((x, y, x + width, y + height))
                opaque = 0
                for expected_pixel, actual_pixel in zip(reference.getdata(), candidate.getdata(), strict=True):
                    if expected_pixel[3] == 255:
                        opaque += 1
                        require(expected_pixel[:3] == actual_pixel[:3], "False exact source-component reconciliation")
                require(opaque == match["fully_opaque_pixels_compared"] and opaque >= 32
                        and match["different_pixels"] == 0, "Incomplete native component footprint proof")
                count += 1
    widths = {494: 303, 495: 341, 496: 403, 497: 432}
    ascents = {494: 10, 495: 12, 496: 12, 497: 15}
    for font in metrics["fonts"]:
        require(font["ascent"] == ascents[font["font_id"]] and font["glyph_count"] == 256,
                "Wrong source font ascent/glyph inventory")
        require(font["text_width_px"] == widths[font["font_id"]] == text_width(font["calibration_text"], font["font_id"]),
                "Source font advance calibration differs")
    require([row["logical_viewport"] for row in metrics["static_resize_analysis"]]
            == manifest["resize_proposal"]["required_checkpoints"], "Wrong resize comparison checkpoints")
    for row in metrics["static_resize_analysis"]:
        width, height = row["logical_viewport"]
        require(row["rectangles"]["minimap_container"] == [width - 211, 0, 211, 207], "Wrong native minimap anchor")
        require(row["rectangles"]["chat_container"] == [0, height - 165, 519, 165], "Wrong native chat anchor")
        require(row["rectangles"]["side_panel_container"] == [width - 241, height - 335, 241, 335],
                "Wrong native Classic side-panel anchor")
    return count


def verify_proposal(proposal):
    with Image.open(verify_file(proposal)) as image:
        actual = image.convert("RGBA")
    with Image.open(verify_file(proposal["base_input"])) as image:
        original = image.convert("RGBA")
    require(list(actual.size) == proposal["dimensions"] == [1920, 1080], "Wrong proposal canvas")
    x, y, width, height = proposal["titlebox_rectangle"]
    difference = ImageChops.difference(actual, original).convert("RGB")
    changed = difference.getbbox()
    require(changed is not None and x <= changed[0] and y <= changed[1]
            and changed[2] <= x + width and changed[3] <= y + height,
            "Proposal changed source pixels outside its declared titlebox")
    for control in proposal["controls"]:
        left, top, control_width, control_height = control["rectangle"]
        require(control_width == 147 and control_height == 41, "Source button stretched")
        require(x <= left and y <= top and left + control_width <= x + width
                and top + control_height <= y + height, "Proposal control escaped source frame")


def decode_audio(manifest):
    path = ROOT / "tools/audio-import/codec.py"
    spec = importlib.util.spec_from_file_location("source_audio_codec", path)
    module = importlib.util.module_from_spec(spec)
    previous_bytecode_policy = sys.dont_write_bytecode
    sys.dont_write_bytecode = True
    try:
        spec.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous_bytecode_policy
    decoder = module.Flac(ROOT / "tools/audio-import/dependencies.json")
    frames = 0
    for entry in manifest["audio"]["assets"]:
        samples, info = decoder.read(verify_file(entry))
        require(hashlib.sha256(samples.tobytes()).hexdigest() == entry["encoding"]["decoded_pcm_sha256"],
                f"Decoded original PCM changed: {entry['asset_id']}")
        require(info.frames == entry["signal"]["frames"]
                and info.channels == entry["signal"]["channels"]
                and info.samplerate == entry["signal"]["sample_rate"], "Invalid audio frame/rate/channel metadata")
        frames += info.frames
    return {"files_decoded": len(manifest["audio"]["assets"]), "audio_frames": frames,
            "reencoded_files": 0, "audible_product_playback_asserted": False}


def require_complete(manifest):
    require(review_ready(manifest["evidence_factorization"]["source_review_requirements"])
            and manifest["ready_for_owner_review"] and not manifest["source_gaps"],
            "Mandatory source evidence remains: " + ", ".join(gap["id"] for gap in manifest["source_gaps"]))


def validate_owner_review(manifest):
    review = json.loads((OUT / "owner-review.json").read_text())
    require(review["manifest"] == digest(OUT / "manifest.json"), "Owner review cites a stale manifest")
    require(review["ready_for_owner_review"] == manifest["ready_for_owner_review"]
            and review["status"] == "awaiting_owner_approval" and not review["reference_pack_approved"],
            "Owner review falsely grants pack approval or contradicts readiness")
    require(review["already_approved_audio_only"] == manifest["approved_audio_adaptations"],
            "Owner summary broadens or hides the two approved audio adaptations")
    decisions = {row["id"]: row for row in review["decisions_requested"]}
    require(set(decisions) == {"review.reference_pack", "review.web_compositions",
                              "review.penguin_and_equipment", "review.viewport_and_tolerances"},
            "Incomplete/broadened final owner decision scope")
    require(decisions["review.web_compositions"]["proposal_ids"] == [row["id"] for row in manifest["proposal_inputs"]],
            "Owner review omitted a concrete web-only proposal")
    facts = image_facts(verify_file(review["contact_sheet"]).read_bytes())
    require(facts["dimensions"] == [1600, 1020] and len(review["contact_sheet"]["input_ids"]) == 12,
            "Owner review contact sheet is incomplete")
    require(review["manifest"]["sha256"] in (OUT / "owner-review.html").read_text(), "Owner page hides the exact manifest hash")
    return {"decision_groups": 4, "web_proposals": 7, "contact_images": 12, "pack_approved": False}


def validate(manifest, *, full_audio=True):
    metrics = structural(manifest)
    records = []
    for key in ("bound_existing_documents", "source_asset_inventory", "original_inputs", "native_hud_inputs", "public_inputs",
                "proposal_inputs", "recording_frames", "tool_inputs", "source_snapshot_inventory", "owned_document_inputs"):
        records.extend(manifest[key])
    records.extend(manifest["audio"]["assets"])
    records.extend(manifest["audio"]["reference_templates"])
    records.extend(entry["source_provenance"] for entry in manifest["audio"]["reference_templates"])
    records.extend(entry["snapshot"] for entry in manifest["public_page_revisions"])
    records.extend(entry["source_snapshot"] for entry in manifest["public_inputs"])
    records += [manifest["source_widget_symbols"]["source"], manifest["comparison_policy"],
                manifest["factorization_document"], manifest["dynamic_text_oracles"], manifest["audio_binding_audit_sources"],
                manifest["native_hud_calibration"], manifest["remaining_audio_handoff"],
                manifest["audio_reference_contract"], manifest["approved_audio_adaptations"]["input"],
                manifest["approved_audio_adaptations"]["adaptations_spec"],
                manifest["audio"]["browser_source_recording_decode"]]
    unique_files = {}
    for record in records:
        path = record["path"]
        if path in unique_files:
            require(unique_files[path]["sha256"] == record["sha256"], f"Conflicting input hashes: {path}")
        else:
            unique_files[path] = record
            verify_file(record)
    for entry in manifest["original_inputs"]:
        decode_original(entry)
    native_regions = validate_native_hud(manifest["native_hud_inputs"], pixels=True)
    for entry in manifest["public_inputs"]:
        decode_public(entry)
    for entry in manifest["proposal_inputs"]:
        verify_proposal(entry)
    for entry in manifest["recording_frames"]:
        facts = image_facts(verify_file(entry).read_bytes())
        require(facts["dimensions"] == entry["dimensions"], "Wrong decoded recording-frame dimensions")
    source_pngs = 0
    for entry in manifest["source_asset_inventory"]:
        if entry["path"].endswith(".png"):
            image_facts((ROOT / entry["path"]).read_bytes())
            source_pngs += 1
    expected_media = {entry["path"] for entry in manifest["public_inputs"] + manifest["recording_frames"]
                      + manifest["audio"]["reference_templates"]}
    actual_media = {path.relative_to(ROOT).as_posix() for path in (ROOT / "assets/reference/wiki").rglob("*")
                    if path.is_file() and path.suffix.lower() in (".png", ".jpg", ".jpeg", ".gif", ".mp4", ".wav")}
    require(expected_media == actual_media, "Unindexed/missing media file in owned asset inventory")
    expected_snapshots = {entry["path"] for entry in manifest["source_snapshot_inventory"]}
    actual_snapshots = {path.relative_to(ROOT).as_posix() for path in (OUT / "sources").glob("*") if path.is_file()}
    require(expected_snapshots == actual_snapshots, "Unindexed/missing source snapshot")
    for page in manifest["public_page_revisions"]:
        source = load_gzip(verify_file(page["snapshot"]))["page"]
        require(source["pageid"] == page["page_id"] and source["revisions"][0]["revid"] == page["revision"],
                "Wrong public behavior-page revision")
    factored_file = json.loads(verify_file(manifest["factorization_document"]).read_text())
    require(factored_file == manifest["evidence_factorization"], "Factoring handoff differs from reviewed manifest")
    expected_text = make_text_oracles(manifest["public_page_revisions"])
    actual_text = json.loads(verify_file(manifest["dynamic_text_oracles"]).read_text())
    require(actual_text == expected_text, "Pinned dynamic text, style, native metrics or source lines changed")
    actual_calibration = json.loads(verify_file(manifest["native_hud_calibration"]).read_text())
    require(actual_calibration == native_hud_calibration(manifest["native_hud_inputs"]),
            "Native source geometry/font/background calibration differs")
    synthetic = {text for entry in manifest["native_hud_inputs"] for text in entry["synthetic_dialogue_strings"]}
    require(not any(record["desktop_text"] in synthetic for record in actual_text["records"]),
            "Synthetic native fixture body was promoted to source dialogue")
    audio_contract = json.loads(verify_file(manifest["audio_reference_contract"]).read_text())
    audio_application = validate_audio_contract(audio_contract, decode_templates=True)
    for notice in manifest["notices"]:
        require(checked_path(notice).is_file(), f"Missing source notice: {notice}")
    contact_sheets = json.loads((OUT / "gallery/contact-sheets.json").read_text())
    for sheet in contact_sheets:
        facts = image_facts(verify_file(sheet).read_bytes())
        require(facts["dimensions"] == [1600, 1200], "Malformed contact sheet")
    audio = decode_audio(manifest) if full_audio else {"status": "not_run", "files_decoded": 0}
    owner_review = validate_owner_review(manifest)
    return {
        "schema_version": 1, "validated_at": datetime.now(timezone.utc).isoformat(),
        "result": "passed_source_input_integrity_and_case_index",
        "manifest": digest(OUT / "manifest.json"),
        "hash_bound_files_checked": len(unique_files),
        "original_images_decoded": len(manifest["original_inputs"]),
        "native_hud_images_decoded": len(manifest["native_hud_inputs"]),
        "native_ui_region_hashes_checked": native_regions,
        "public_images_decoded": sum(entry["decoded"]["format"] != "MP4" for entry in manifest["public_inputs"]),
        "public_recording_containers_checked": sum(entry["decoded"]["format"] == "MP4" for entry in manifest["public_inputs"]),
        "recording_frames_decoded": len(manifest["recording_frames"]),
        "published_source_sprite_atlases_decoded": source_pngs,
        "exact_component_or_text_proofs_rechecked": verify_matches(manifest, metrics),
        "audio": audio, "required_case_ids_checked": len(manifest["cases"]),
        "audio_reference_application": audio_application,
        "owner_review_summary": owner_review,
        "tutorial_states_mapped": 71,
        "visual_families_checked": manifest["counts"]["visual_families"],
        "distinct_visual_variants_checked": manifest["counts"]["distinct_visual_variants"],
        "progressive_hud_signatures_checked": manifest["counts"]["progressive_hud_signatures"],
        "source_text_records_checked": len(actual_text["records"]),
        "complete_reference_pack": manifest["ready_for_owner_review"],
        "ready_for_owner_review": manifest["ready_for_owner_review"],
        "ready_for_owner_approval": manifest["ready_for_owner_approval"], "status": "awaiting_owner_approval",
        "source_gap_categories": len(manifest["source_gaps"]),
        "source_gap_cases": manifest["counts"]["source_gap_cases"],
        "owner_approved": False, "candidate_tested": False, "mac_target_tested": False,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", action="store_true")
    parser.add_argument("--require-complete", action="store_true")
    args = parser.parse_args()
    try:
        lock = json.loads((OUT / "manifest-lock.json").read_text())
        manifest = json.loads(verify_file(lock["manifest"]).read_text())
        report = validate(manifest)
        if args.report:
            write_json(OUT / "validation.json", report)
        print(json.dumps(report))
        if args.require_complete:
            require_complete(manifest)
    except (EvidenceError, OSError, ValueError, KeyError) as error:
        print(f"REFERENCE EVIDENCE FAILURE: {error}", file=sys.stderr)
        raise SystemExit(2) from error


if __name__ == "__main__":
    main()
