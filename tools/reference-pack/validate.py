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
    require(manifest["schema_version"] == 1 and manifest["pack_version"] == "1.0.0", "Unsupported pack schema/version")
    require(manifest["status"] == "awaiting_owner_approval", "Pack must remain awaiting_owner_approval")
    require(manifest["ready_for_owner_approval"] is False, "Known mandatory source deficits cannot be self-cleared")
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
    public_ids = unique(manifest["public_inputs"], "id", "public input ID")
    proposal_ids = unique(manifest["proposal_inputs"], "id", "proposal ID")
    frame_ids = unique(manifest["recording_frames"], "id", "recording frame ID")
    audio_ids = unique(manifest["audio"]["assets"], "asset_id", "audio asset ID")
    all_ids = original_ids | public_ids | proposal_ids | frame_ids | audio_ids
    require(len(all_ids) == sum(map(len, (original_ids, public_ids, proposal_ids, frame_ids, audio_ids))),
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
                  manifest["original_inputs"] + manifest["public_inputs"] + manifest["proposal_inputs"]}
    role_by_id.update({entry["asset_id"]: "current_original_audio_input" for entry in manifest["audio"]["assets"]})
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
        if case["family"] == "tutorial":
            state = states[case["journey_state_id"]]
            require(case["declared_controls"] == state.get("ui_unlock_refs", []), "Changed tutorial unlock declaration")
            require(case["expected_instruction"] == state["instruction"], "Changed source semantic stage")
            require(case["source_numeric_progress"] is None, "Invented numeric source tutorial progress")
            suffix = case["id"].removeprefix("case.tutorial.")
            require(case["exact_stage_screenshot_available"] == (suffix in ("experience", "departure_offer")),
                    "Lesson/component image incorrectly promoted to exact stage capture")
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
    audio = json.loads((ROOT / "assets/manifests/osrs/audio-runtime.json").read_text())
    require(manifest["audio"]["assets"] == audio["assets"], "Existing original audio inventory changed")
    require(len(audio_ids) == 258, "Missing playable audio files")
    require(manifest["audio"]["source_silences"] == audio["source_silences"], "Source silence2411 changed/dropped")
    require(manifest["audio"]["source_silences"][0]["playable_output"] is None, "Silence falsely assigned playable file")
    require(not manifest["audio"]["conversion_repeated"]
            and not manifest["audio"]["live_source_mixer_calibrated"], "False audio conversion/calibration claim")
    gaps = {entry["id"]: entry for entry in manifest["source_gaps"]}
    require(set(gaps) == {"gap.full_classic_frame", "gap.tutorial_matched_states",
                         "gap.arrival_state", "gap.required_audio_bindings"}, "Known source deficits disappeared")
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
    require(manifest["ready_for_owner_approval"] and not manifest["source_gaps"],
            "Mandatory source evidence remains: " + ", ".join(gap["id"] for gap in manifest["source_gaps"]))


def validate(manifest, *, full_audio=True):
    metrics = structural(manifest)
    records = []
    for key in ("bound_existing_documents", "source_asset_inventory", "original_inputs", "public_inputs",
                "proposal_inputs", "recording_frames", "tool_inputs", "source_snapshot_inventory", "owned_document_inputs"):
        records.extend(manifest[key])
    records.extend(manifest["audio"]["assets"])
    records.extend(entry["snapshot"] for entry in manifest["public_page_revisions"])
    records.extend(entry["source_snapshot"] for entry in manifest["public_inputs"])
    records += [manifest["source_widget_symbols"]["source"], manifest["comparison_policy"],
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
    expected_media = {entry["path"] for entry in manifest["public_inputs"] + manifest["recording_frames"]}
    actual_media = {path.relative_to(ROOT).as_posix() for path in (ROOT / "assets/reference/wiki").rglob("*")
                    if path.is_file() and path.suffix.lower() in (".png", ".jpg", ".jpeg", ".gif", ".mp4")}
    require(expected_media == actual_media, "Unindexed/missing media file in owned asset inventory")
    expected_snapshots = {entry["path"] for entry in manifest["source_snapshot_inventory"]}
    actual_snapshots = {path.relative_to(ROOT).as_posix() for path in (OUT / "sources").glob("*") if path.is_file()}
    require(expected_snapshots == actual_snapshots, "Unindexed/missing source snapshot")
    for page in manifest["public_page_revisions"]:
        source = load_gzip(verify_file(page["snapshot"]))["page"]
        require(source["pageid"] == page["page_id"] and source["revisions"][0]["revid"] == page["revision"],
                "Wrong public behavior-page revision")
    for notice in manifest["notices"]:
        require(checked_path(notice).is_file(), f"Missing source notice: {notice}")
    contact_sheets = json.loads((OUT / "gallery/contact-sheets.json").read_text())
    for sheet in contact_sheets:
        facts = image_facts(verify_file(sheet).read_bytes())
        require(facts["dimensions"] == [1600, 1200], "Malformed contact sheet")
    audio = decode_audio(manifest) if full_audio else {"status": "not_run", "files_decoded": 0}
    return {
        "schema_version": 1, "validated_at": datetime.now(timezone.utc).isoformat(),
        "result": "passed_source_input_integrity_and_case_index",
        "manifest": digest(OUT / "manifest.json"),
        "hash_bound_files_checked": len(unique_files),
        "original_images_decoded": len(manifest["original_inputs"]),
        "public_images_decoded": sum(entry["decoded"]["format"] != "MP4" for entry in manifest["public_inputs"]),
        "public_recording_containers_checked": sum(entry["decoded"]["format"] == "MP4" for entry in manifest["public_inputs"]),
        "recording_frames_decoded": len(manifest["recording_frames"]),
        "published_source_sprite_atlases_decoded": source_pngs,
        "exact_component_or_text_proofs_rechecked": verify_matches(manifest, metrics),
        "audio": audio, "required_case_ids_checked": len(manifest["cases"]),
        "tutorial_states_mapped": 71, "complete_reference_pack": False,
        "ready_for_owner_approval": False, "status": "awaiting_owner_approval",
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
