"""Consume the authorized original HUD fixtures without rerendering or relabeling their state."""

from functools import lru_cache
import hashlib
import importlib.util
import json
import sys

from PIL import Image

from components import ROOT, SOURCE, digest, load_gzip
from catalogue import NUMERIC_PROFILES


DIRECTORY = ROOT / "assets/reference/osrs240/native-hud"
SOURCE_COMMIT = "db103ba838cc954ce9adbbaecff4cc70d6d0962c"
CAPTURES_SHA256 = "f70232054d6e4213dcaf5d83bed667c0cd7771e6832993fbd7824dd3463b8218"
PANEL_GROUPS = {
    "native-inventory": 149, "native-equipment": 387, "native-skills": 320,
    "native-combat": 593, "native-prayer": 541, "native-magic": 218,
    "native-quest-list": 399, "native-bank": 12, "native-shop": 300,
    "native-guide-dialogue": 231,
}
FAMILY_SLOTS = {
    "guide": [10, 11], "survival": [1, 3, 10, 11], "quest-guide": [1, 2, 3, 10, 11],
    "combat": [0, 1, 2, 3, 4, 10, 11], "prayer": [0, 1, 2, 3, 4, 5, 10, 11],
    "magic": [0, 1, 2, 3, 4, 5, 6, 10, 11],
}
FAMILY_PANELS = {
    "hud.classic": ["native-inventory", *["family-" + name for name in FAMILY_SLOTS]],
    "hud.minimenu": ["native-inventory"],
    "dialogue.flow": ["native-guide-dialogue", *["family-" + name for name in FAMILY_SLOTS]],
    "ui.inventory": ["native-inventory"],
    "ui.equipment": ["native-equipment"],
    "ui.skills": ["native-skills"],
    "ui.combat": ["native-combat"],
    "ui.prayer": ["native-prayer"],
    "ui.magic": ["native-magic"],
    "ui.bank": ["native-bank"],
    "ui.shop": ["native-shop"],
    "ui.quests": ["native-quest-list"],
    "ui.settings": ["family-guide"],
}
PHASE_FIXTURES = {
    "Gielinor Guide": "guide", "Survival Expert": "survival", "Master Chef": "survival",
    "Quest Guide": "quest-guide", "Mining Instructor": "quest-guide", "Combat Instructor": "combat",
    "Banking tutorial": "combat", "Prayer Tutorial": "prayer", "Magic Instructor": "magic",
}


def input_id(name):
    return "native-hud." + name


@lru_cache(maxsize=1)
def source_validator():
    path = ROOT / "tools/source-capture/capture.py"
    spec = importlib.util.spec_from_file_location("original_hud_input_validator", path)
    module = importlib.util.module_from_spec(spec)
    previous = sys.dont_write_bytecode
    sys.dont_write_bytecode = True
    try:
        spec.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous
    return module


def load_inputs():
    manifest_path = DIRECTORY / "captures.json"
    if digest(manifest_path)["sha256"] != CAPTURES_SHA256:
        raise ValueError("Authorized native HUD capture manifest changed")
    manifest = json.loads(manifest_path.read_text())
    provenance = json.loads((DIRECTORY / "provenance.json").read_text())
    if (manifest["profile"], manifest["configured_game_revision"], manifest["source_cache_id"]) != ("hud", 240, 2695):
        raise ValueError("Wrong original native HUD source identity")
    if manifest["gpu_plugin_enabled"] or provenance["authenticated"] or provenance["owner_reference_pack_approved"]:
        raise ValueError("Native HUD input misclassified or self-approved")
    source_validator().validate(DIRECTORY)
    records = []
    for capture in manifest["captures"]:
        name = capture["path"].removeprefix("hud/").removesuffix(".png")
        synthetic = [widget["text"] for widget in capture["source"]["visible_widgets"]
                     if widget["id"] == (231 << 16 | 6) and widget["text"]]
        records.append({
            **capture, "id": input_id(name),
            "path": (DIRECTORY / capture["path"]).relative_to(ROOT).as_posix(),
            "dimensions": [capture["width"], capture["height"]],
            "source_role": "current_original_runtime_fixture",
            "capture_collection": "osrs240-native-hud",
            "source_commit": SOURCE_COMMIT, "source_capture_build": 240, "source_cache_id": 2695,
            "captured_at": provenance["captured_at"],
            "full_resizable_classic_frame": True,
            "browser": None, "browser_dpr": None,
            "fixture_text_origin": "synthetic_fixture_text_not_source_dialogue",
            "synthetic_dialogue_strings": synthetic,
            "text_authority": "Pinned dynamic-text-oracles.json and original glyph/font inputs, never the synthetic fixture body.",
            "stage_mapping_role": "Controlled UI attachment/layout example, not an authenticated stage, NPC spawn "
                                  "identity, reward grant, price or exact71-state timing/visibility observation.",
            "settings_origin": "Verbatim original native capture metadata; unknown browser/live-state values are not filled.",
        })
    return records


def calibration(records):
    by_name = {entry["id"].removeprefix("native-hud."): entry for entry in records}
    inventory = by_name["native-inventory"]
    dialogue = by_name["native-guide-dialogue"]
    widgets = load_gzip(SOURCE / "interfaces/231.json.gz")
    fonts = {widget["id"]: widget for widget in widgets if widget["fontId"] >= 0}
    bounds = []
    for widget in dialogue["source"]["visible_widgets"]:
        if widget["id"] in fonts:
            definition = fonts[widget["id"]]
            bounds.append({
                "widget_id": widget["id"], "native_canvas_rectangle": [
                    widget["x"], widget["y"], widget["width"], widget["height"]],
                "font_id": definition["fontId"], "source_text_color": definition["textColor"],
                "font_basis": "Current source widget definition, with original native font loading/painting. "
                              "visible_widgets records actual coordinates, not a separate font-ID readback.",
                "fixture_text": widget["text"],
                "text_role": "synthetic_fixture_body" if widget["id"] == (231 << 16 | 6) else "native_fixture_widget_value",
            })
    contract_path = DIRECTORY / "hud-input-contract.json"
    contract = json.loads(contract_path.read_text())
    return {
        "schema_version": 1, "source_commit": SOURCE_COMMIT,
        "manifest": digest(DIRECTORY / "captures.json"),
        "provenance": digest(DIRECTORY / "provenance.json"),
        "input_contract": digest(contract_path),
        "native_bindings": digest(ROOT / "research/source-capture/native-hud-bindings.json"),
        "source_validation": digest(ROOT / "research/source-capture/native-hud-validation.json"),
        "documentation": digest(ROOT / "tools/source-capture/NATIVE_HUD.md"),
        "full_frame_input_ids": [entry["id"] for entry in records],
        "active_panel_groups": {name: entry["source"]["active_interface"] for name, entry in by_name.items()},
        "native_root": 161,
        "native_scripts": {"onload": 901, "redraw": 907, "tab_listener": 914, "chat_rebuild": 216},
        "native_region_rectangles": {region["name"]: region["bounds"] for region in inventory["source"]["native_ui_regions"]},
        "dialogue_anchor": {"parent_widget": 162 << 16 | 567, "interface_group": 231,
                            "native_panel_rectangle": next(
                                region["bounds"] for region in dialogue["source"]["native_ui_regions"]
                                if region["name"] == "active-panel")},
        "dialogue_font_and_layout": bounds,
        "dialogue_definition": digest(SOURCE / "interfaces/231.json.gz"),
        "scene_preparation": {
            "input": digest(ROOT / "tools/source-capture/WorldCapture.java"),
            "method": "prepareForHud -> view(lumbridge-hud,...,renderCapture=false)",
            "settings_role": "Exact source-code fixture configuration; not per-HUD native camera readbacks.",
            "base_world_tile": [3168, 3168], "camera_world_tile_xz": [3222, 3208],
            "height_offset_from_source_ground": -1300, "focal_world_tile": [3222, 3218],
            "pitch_input": 2048, "yaw_input": 0, "angle_units_per_turn": 16384,
            "draw_distance": 25, "far_clip_units": 32768,
            "per_hud_camera_readbacks": None,
            "existing_calibrated_scene_id": "original.scenes.lumbridge-castle-plaza",
        },
        "attachment_families": [
            {"name": name, "input_id": input_id("family-" + name), "enabled_tab_slots": slots,
             "native_component_links": by_name["family-" + name]["source"]["component_links"],
             "role": "controlled_attachment_family_not_exact_tutorial_progress"}
            for name, slots in FAMILY_SLOTS.items()
        ],
        "source_derived_fixture_values": {
            "sword_category": contract["sword_category_source_row"],
            "quest_counters": contract["quest_counter_fixture"],
            "shop_main": contract["shop_main"], "shop_side": contract["shop_side"],
        },
        "comparison_scope": "Whole original frames, scene, minimap/chat/sidebar and native panel backgrounds/geometry. "
                            "For the same controlled fixture state compare synthetic body pixels as fixture pixels ONLY. "
                            "For journey text/values use the independent source oracles with full-panel pixel accounting.",
        "scope_limits": [
            "The16 native frames do not certify authenticated progression, source71-state unlock timing or arrival state.",
            "Six enabled-slot sets are explicitly chosen rendering scenarios, not the semantic11-signature table.",
            "The captured survival-family NPC choice is a fixture input; it does not override the pinned tutorial instructor identity.",
            "NPC dialogue231 is demonstrated. Player/item-message variants remain in factored source-state checks; "
            "do not claim separate217/229 screenshots or require a separate live screenshot for each variant.",
            "The selected Lumbridge scene is controlled. HUD records do not contain per-frame camera readbacks; "
            "use their hash-bound original WorldCapture preparation code and the existing calibrated scene reference, "
            "without inventing additional measured camera fields.",
        ],
        "owner_approved": False, "authenticated_progression_verified": False,
    }


def validate_records(records, *, pixels=False):
    manifest = json.loads((DIRECTORY / "captures.json").read_text())
    canonical = {capture["path"]: capture for capture in manifest["captures"]}
    expected = set(PANEL_GROUPS) | {"family-" + name for name in FAMILY_SLOTS}
    names = [entry["id"].removeprefix("native-hud.") for entry in records]
    if len(names) != 16 or set(names) != expected:
        raise ValueError("Missing/duplicate native full-frame panel or attachment family")
    regions_checked = 0
    for entry in records:
        relative = entry["path"].removeprefix("assets/reference/osrs240/native-hud/")
        if relative not in canonical:
            raise ValueError("Unknown native source frame path")
        if entry["id"] != input_id(relative.removeprefix("hud/").removesuffix(".png")):
            raise ValueError("Native input ID/path mismatch")
        source = canonical[relative]
        for key in source:
            if key != "path" and entry[key] != source[key]:
                raise ValueError(f"Changed original HUD metadata: {entry['id']} / {key}")
        if (entry["kind"] != "original-runtime-hud-fixture"
                or entry["source_role"] != "current_original_runtime_fixture"
                or entry["full_resizable_classic_frame"] is not True
                or entry["fixture_text_origin"] != "synthetic_fixture_text_not_source_dialogue"
                or entry["source_commit"] != SOURCE_COMMIT
                or entry["source_capture_build"] != 240 or entry["source_cache_id"] != 2695
                or entry["browser"] is not None or entry["browser_dpr"] is not None):
            raise ValueError("Native fixture/camera/text/source-role misclassification")
        synthetic = [widget["text"] for widget in source["source"]["visible_widgets"]
                     if widget["id"] == (231 << 16 | 6) and widget["text"]]
        if entry["synthetic_dialogue_strings"] != synthetic:
            raise ValueError("Synthetic fixture text was changed, omitted or relabeled")
        name = entry["id"].removeprefix("native-hud.")
        slots = FAMILY_SLOTS[name.removeprefix("family-")] if name.startswith("family-") else list(range(14))
        if entry["settings"]["enabled_tab_slots"] != slots:
            raise ValueError("Native attachment family changed")
        source_validator().validate_hud_record(entry)
        if not pixels:
            continue
        path = ROOT / entry["path"]
        if digest(path)["sha256"] != entry["sha256"]:
            raise ValueError("Changed native source image bytes")
        with Image.open(path) as image:
            image.load()
            rgba = image.convert("RGBA")
        if list(rgba.size) != [1920, 1080]:
            raise ValueError("Native HUD source image was cropped or rescaled")
        red, green, blue, alpha = rgba.split()
        argb = Image.merge("RGBA", (alpha, red, green, blue))
        if hashlib.sha256(argb.tobytes()).hexdigest() != entry["pixel_argb32_be_sha256"]:
            raise ValueError("Native frame pixels differ from original buffer")
        for region in entry["source"]["native_ui_regions"]:
            x, y, width, height = region["bounds"]
            rectangle = (x, y, x + width, y + height)
            if hashlib.sha256(argb.crop(rectangle).tobytes()).hexdigest() != region["argb32_be_sha256"]:
                raise ValueError("Native UI region pixel hash differs")
            colors = rgba.crop(rectangle).convert("RGB").getcolors(width * height)
            nonblack = sum(count for count, color in colors if color != (0, 0, 0))
            if len(colors) != region["colors"] or nonblack != region["nonblack_pixels"]:
                raise ValueError("Native region substantive pixel evidence differs")
            regions_checked += 1
    return regions_checked


def case_input_ids(case, phases):
    names = []
    for family_id in case["reference_family_ids"]:
        names.extend(FAMILY_PANELS.get(family_id, []))
    if case["family"] == "tutorial":
        section = phases[case["phase_id"]]["source_section"]
        if section in PHASE_FIXTURES:
            preferred = "family-" + PHASE_FIXTURES[section]
            names = [preferred] + [name for name in names if not name.startswith("family-")]
    elif case["id"] != "case.hud.tabs_unlocks":
        names = [name for name in names if not name.startswith("family-")]
    return list(dict.fromkeys(input_id(name) for name in names))


def wire_cases(cases, factoring, records):
    by_id = {entry["id"]: entry for entry in records}
    phases = {phase["id"]: phase for phase in factoring["phases"]}
    for case in cases:
        ids = case_input_ids(case, phases)
        case["native_hud_input_ids"] = ids
        if ids:
            case["input_ids"] = ids + [identifier for identifier in case["input_ids"] if identifier not in ids]
            case["input_roles"] = [
                {"input_id": identifier, "source_role": "current_original_runtime_fixture",
                 "comparison_profile": "native_hud", "numeric_tolerances": NUMERIC_PROFILES["native_hud"],
                 "normative_scope": "Original complete native frame/layout and declared fixture inputs; synthetic "
                                    "dialogue is never source transcript text or a legitimate journey assertion."}
                for identifier in ids
            ] + case["input_roles"]
            groups = {by_id[identifier]["source"]["active_interface"] for identifier in ids}
            case["native_executed_interface_groups"] = sorted(groups)
            case["native_reference_state_scope"] = "controlled_frame_and_panel_evidence_not_authenticated_state"
    for family in factoring["families"]:
        native_ids = [input_id(name) for name in FAMILY_PANELS.get(family["id"], [])]
        family["native_hud_input_ids"] = native_ids
        family["pixel_or_audio_input_ids"] = sorted(set(family["pixel_or_audio_input_ids"]) | set(native_ids))
        family["representative_input_ids"] = native_ids + [
            identifier for identifier in family["representative_input_ids"] if identifier not in native_ids]
        family["native_evidence_scope"] = "actual native geometry/background/font/panel/attachment examples; "
        family["native_evidence_scope"] += "source text and exact semantic state stay independently checked"
