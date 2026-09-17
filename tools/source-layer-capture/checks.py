"""Pixel, native-state and exact-input checks; never evaluates a candidate renderer."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path

from PIL import Image, ImageChops

ROOT = Path(__file__).resolve().parents[2]
POLICY = ROOT / "research/reference-pack/v1/comparison-policy.json"
FROZEN_SCENES = ROOT / "assets/reference/osrs240/captures.json"


def argb_bytes(image: Image.Image) -> bytes:
    rgba = image.convert("RGBA").tobytes()
    result = bytearray(len(rgba))
    result[0::4], result[1::4], result[2::4], result[3::4] = rgba[3::4], rgba[0::4], rgba[1::4], rgba[2::4]
    return bytes(result)


def image_for(directory: Path, record: dict) -> Image.Image:
    capture = record["capture"]
    path = (directory / capture["path"]).resolve()
    if not path.is_relative_to(directory.resolve()) or not path.is_file():
        raise ValueError("Missing or escaping original dynamic image")
    data = path.read_bytes()
    if len(data) != capture["size_bytes"] or hashlib.sha256(data).hexdigest() != capture["sha256"]:
        raise ValueError("Original dynamic PNG hash changed")
    with Image.open(path) as opened:
        if opened.format != "PNG" or opened.size != (1920, 1080) or opened.mode != "RGBA":
            raise ValueError("Dynamic reference is not the exact native RGBA framebuffer")
        image = opened.copy()
    if hashlib.sha256(argb_bytes(image)).hexdigest() != capture["pixel_argb32_be_sha256"]:
        raise ValueError("Decoded PNG differs from original native ARGB pixel hash")
    rgb = image.convert("RGB")
    red, green, blue = rgb.split()
    intensity = ImageChops.lighter(ImageChops.lighter(red, green), blue)
    nonblack = intensity.width * intensity.height - intensity.histogram()[0]
    colors = sum(color != (0, 0, 0) for _, color in rgb.getcolors(rgb.width * rgb.height))
    bbox = intensity.getbbox()
    if nonblack < 20000 or colors < 2 or bbox is None:
        raise ValueError("Blank original native layer capture")
    if (nonblack, colors, [bbox[0], bbox[1], bbox[2] - 1, bbox[3] - 1]) != (
            capture["nonbackground_pixels"], capture["nonbackground_colors"], capture["pixel_bounds"]):
        raise ValueError("Native nonblank pixel/color/bounds evidence differs from decoded PNG")
    return image


def geometry_values(record: dict) -> list[dict]:
    values = []
    for operation in record["capture"]["source"]["native_operations"]:
        if "geometry" in operation:
            values.append(operation["geometry"])
        for selection in operation.get("selected", []):
            values.append(selection["geometry"])
    return values


def operation(record: dict, field: str) -> dict:
    found = [value for value in record["capture"]["source"]["native_operations"] if field in value]
    if len(found) != 1:
        raise ValueError(f"Missing/ambiguous native operation {field} in {record['id']}")
    return found[0]


def validate_state(record: dict) -> None:
    capture, control = record["capture"], record["input"]
    settings, source = capture["settings"], capture["source"]
    if control["id"] != record["id"] or control != source["case"]:
        raise ValueError("Declared source control state differs from captured state")
    if source["classification"] != "controlled offline original-client rendering":
        raise ValueError("Incorrect source capture classification")
    if capture["authenticated_source_journey"] or settings["source_gameplay_observed"] or record["candidate_compared"]:
        raise ValueError("Controlled native fixture must not claim source gameplay or candidate acceptance")
    if settings["network_transport_connected"]:
        raise ValueError("Native fixture has a network transport")
    cameras = {Path(row["path"]).stem: row["settings"] for row in json.loads(FROZEN_SCENES.read_text())["captures"]
               if row["kind"] == "original-runtime-scene-fixture"}
    expected = cameras[control["camera"]]
    for field, reference in (("camera_local_units", "camera_local_units"),
                             ("draw_distance", "draw_distance"), ("pitch", "pitch_input"),
                             ("yaw", "yaw_input"), ("far_clip_units", "far_clip_units")):
        if settings[field] != expected[reference]:
            raise ValueError(f"Approved original camera/projection changed: {field}")
    if (settings["dpr"], settings["ui_scale"], settings["brightness"], settings["angle_units_per_turn"],
            settings["texture_resolution"], settings["game_cycle"]) != (1, 1, 0.8, 16384, 128, 0):
        raise ValueError("Source display/material/scene phase contract changed")
    if settings["zoom"] != 410:
        raise ValueError("Original full-HUD native viewport zoom changed")
    if not -8 <= settings["native_terrain_hue_offset"] <= 8 or not -16 <= settings["native_terrain_lightness_offset"] <= 16:
        raise ValueError("Original source terrain tint exceeds native bounds")
    if settings["layout"] != "Original Resizable-Classic group161":
        raise ValueError("Native stock layout changed")
    census = source["scene_census"]
    if census["tiles"] < 10000 or census["game_object_tile_references"] < 1000:
        raise ValueError("Missing source scenery geometry")
    for geometry in geometry_values(record):
        if geometry["vertices"] <= 0 or geometry["faces"] <= 0:
            raise ValueError("Empty native layer geometry")
        if any(low > high or abs(low) > 100000 or abs(high) > 100000
               for low, high in zip(geometry["bounds_min"], geometry["bounds_max"])):
            raise ValueError("Invalid original model bounds")
        for field in ("vertex_xyz_float32_be_sha256", "triangle_indices_int32_be_sha256"):
            if len(geometry[field]) != 64:
                raise ValueError("Missing original model-transform/topology integrity")
    for key, payload in source["layer_source_inputs"].items():
        if key != f"{payload['archive']}/{payload['group']}/{payload['file']}" or payload["size_bytes"] <= 0:
            raise ValueError("Invalid source layer archive/file identity")
        if not 0 <= payload["group_crc32"] <= 0xffffffff or len(payload["sha256"]) != 64:
            raise ValueError("Missing original payload CRC/hash")
    kind = control["layer"]
    if kind == "door":
        door = operation(record, "orientation_a")
        if (door["object_id"], door["shape"], door["model_id"], door["source_tile"]) != (9398, 0, 9476, [3098, 3107, 0]):
            raise ValueError("Starting door source model/shape/placement changed")
        if door["orientation"] != control["orientation"] or door["native_config"] & 31:
            raise ValueError("Native door orientation/shape differs from controlled request")
        if ((door["native_config"] >> 6) & 3) != control["orientation"]:
            raise ValueError("Native door config readback differs")
        if door["native_definition_actions"] != ["Open"]:
            raise ValueError("Unobserved server open definition must not be invented")
        if not {"2/6/9398", "7/9476/0"} <= source["layer_source_inputs"].keys():
            raise ValueError("Missing original door payload evidence")
    if kind == "ground":
        pile = operation(record, "selected")
        expected_slots = {"lumbridge-ground-single": [("top", 995, 1)],
                          "lumbridge-ground-stack": [("top", 995, 10000)],
                          "lumbridge-ground-top-three": [("bottom", 1277, 1), ("middle", 1925, 1), ("top", 995, 10000)]}
        selected = [(row["slot"], row["item_id"], row["quantity"]) for row in pile["selected"]]
        if selected != expected_slots[record["id"]]:
            raise ValueError("Native highest-value/distinct-item pile selection changed")
        if pile["tile"] != [3221, 3217, 0] or pile["native_draw_anchor_x_height_y"] != [6848, -240, 6336]:
            raise ValueError("Native ground item draw point moved")
    if kind == "fire":
        fire = operation(record, "requested_frame")
        if (fire["object_id"], fire["model_id"], fire["sequence_id"]) != (26185, 2260, 475):
            raise ValueError("Not the original requested fire/animation")
        if fire["native_frame"] != control["animation_frame"] or fire["requested_frame"] != fire["native_frame"]:
            raise ValueError("Native fire frame differs from declared phase")
        if fire["frame_lengths"] != [6] * 5 or fire["randomize_initial_phase"]:
            raise ValueError("Fire phase is randomized or source timing changed")
        if fire["native_animation_advance_cycles"] != {0: 0, 3: 19}[control["animation_frame"]]:
            raise ValueError("Native animation stepping changed")
        if not {"2/6/26185", "7/2260/0", "2/12/475"} <= source["layer_source_inputs"].keys():
            raise ValueError("Missing original fire/sequence payload evidence")
    roof = operation(record, "actual_locked_camera_draw_plane")
    if roof["roof_removal_plugin_mode"] != 0 or roof["hide_roofs"] != control["hide_roofs"]:
        raise ValueError("Non-stock roof setting or undeclared preference")
    if kind == "roof":
        expected_planes = {"tutorial-roofs-outside": (3, 3, 0), "tutorial-roofs-inside": (3, 0, 4),
                           "tutorial-roofs-hidden": (0, 0, 0)}
        measured = (roof["actual_locked_camera_draw_plane"], roof["normal_camera_stock_plane_selector"],
                    roof["player_source_tile_setting"])
        if measured != expected_planes[record["id"]]:
            raise ValueError("Original locked/following-camera roof rule or source inside/outside flag changed")
    if kind == "plane" and (settings["source_plane"], settings["source_scene_draw_plane"]) != (1, 1):
        raise ValueError("Native controlled plane was not applied")
    if kind == "mapped-chunk":
        mapping = operation(record, "packed_template")
        if (mapping["packed_template"], mapping["target_local_chunk"], mapping["source_chunk"],
                mapping["rotation_quarter_turns"]) != (6589588, [0, 6, 6], [0, 402, 402], 2):
            raise ValueError("Native mapped chunk readback changed")


def changed_mask(first: Image.Image, second: Image.Image) -> Image.Image:
    channels = ImageChops.difference(first.convert("RGB"), second.convert("RGB")).split()
    return ImageChops.lighter(ImageChops.lighter(channels[0], channels[1]), channels[2]).point(lambda value: 255 if value else 0)


def changed_count(mask: Image.Image) -> int:
    return mask.width * mask.height - mask.histogram()[0]


def validate_regions(record: dict, image: Image.Image) -> dict[str, list[int]]:
    regions = {}
    expected = {"root161:95": [1709, 0, 211, 207], "root161:96": [0, 915, 519, 165],
                "root161:97": [1679, 745, 241, 335], "inventory-panel": [1704, 782, 190, 261]}
    for region in record["capture"]["settings"]["native_ui_regions"]:
        name = region["name"]
        bounds = region["bounds"]
        if name in regions or expected.get(name) != bounds:
            raise ValueError("Native HUD bounds or region coverage changed")
        x, y, width, height = bounds
        decoded = image.crop((x, y, x + width, y + height))
        if hashlib.sha256(argb_bytes(decoded)).hexdigest() != region["argb32_be_sha256"]:
            raise ValueError("Original native UI region pixel hash differs")
        if region["nonblack_pixels"] < 1000 or region["colors"] < 10:
            raise ValueError("Blank native HUD region")
        regions[name] = bounds
    if regions != expected:
        raise ValueError("Missing native HUD region")
    return regions


def pair_metrics(pair: dict, before: dict, after: dict, first: Image.Image, second: Image.Image) -> dict:
    if before["capture"]["settings"]["camera_local_units"] != after["capture"]["settings"]["camera_local_units"]:
        raise ValueError("A dynamic pair changed the approved fixed camera")
    mask = changed_mask(first, second)
    total = changed_count(mask)
    regions = validate_regions(before, first)
    validate_regions(after, second)
    deltas = {}
    for name, (x, y, width, height) in regions.items():
        deltas[name] = changed_count(mask.crop((x, y, x + width, y + height)))
    if total == 0:
        raise ValueError(f"Pair has no actual source pixel change: {pair['id']}")
    if deltas["root161:96"] or deltas["inventory-panel"]:
        raise ValueError("Unchanged native chat/inventory-panel pixels changed")
    if deltas["root161:97"] and pair["id"] != "minimap-plane":
        raise ValueError("Unexpected native sidebar-container background change")
    scene_pixels = total - sum(deltas[name] for name in ("root161:95", "root161:96", "root161:97"))
    if pair["id"] in {"door", "ground-quantity", "ground-top-three", "fire", "roof-preference",
                      "minimap-plane", "minimap-mapped-chunk"} and scene_pixels <= 0:
        raise ValueError(f"Requested layer is not visibly rendered: {pair['id']}")
    if pair["id"] in {"door", "minimap-plane", "minimap-mapped-chunk"} and deltas["root161:95"] <= 0:
        raise ValueError(f"Original minimap did not reflect the declared change: {pair['id']}")
    geometries = [geometry_values(value) for value in (before, after)]
    return {**pair, "before_png_sha256": before["capture"]["sha256"], "after_png_sha256": after["capture"]["sha256"],
            "before_native_pixel_sha256": before["capture"]["pixel_argb32_be_sha256"],
            "after_native_pixel_sha256": after["capture"]["pixel_argb32_be_sha256"],
            "different_pixels_full_frame": total, "different_pixel_bounds_xyxy": mask.getbbox(),
            "different_native_minimap_pixels": deltas["root161:95"],
            "different_unchanged_chat_pixels": deltas["root161:96"],
            "different_unchanged_inventory_panel_pixels": deltas["inventory-panel"],
            "different_sidebar_container_pixels": deltas["root161:97"],
            "sidebar_container_note": "The root161:97 bounding box contains unpainted scene gutters. Plane changes alter those original scene pixels; the complete native inventory panel remains exact. No pixels are excluded from the full-frame comparison.",
            "different_scene_or_remaining_frame_pixels": scene_pixels,
            "pixel_accounting": "Full frame accounted for; no ignored geometry/panel masks. These are source-to-source control deltas, not candidate tolerances.",
            "before_layer_geometry": geometries[0], "after_layer_geometry": geometries[1],
            "before_scene_census": before["capture"]["source"]["scene_census"],
            "after_scene_census": after["capture"]["source"]["scene_census"],
            "candidate_compared": False}


def verify_all(directory: Path, manifest: dict) -> dict:
    records = manifest["cases"]
    by_id, images = {}, {}
    if not records or len(records) > 12:
        raise ValueError("Bounded native case count changed")
    for record in records:
        if record["id"] in by_id:
            raise ValueError("Duplicate source case ID")
        by_id[record["id"]] = record
        validate_state(record)
        image = image_for(directory, record)
        validate_regions(record, image)
        images[record["id"]] = image
    pairs = [pair_metrics(pair, by_id[pair["before"]], by_id[pair["after"]],
                          images[pair["before"]], images[pair["after"]]) for pair in manifest["pairs"]]
    return {"schema_version": 1, "result": "passed", "cases": len(records), "pairs": pairs,
            "native_chat_and_inventory_panels_exact": True, "source_comparison_policy": {
                "path": str(POLICY.relative_to(ROOT)), "sha256": hashlib.sha256(POLICY.read_bytes()).hexdigest(),
                "profiles": ["native_scene_model", "native_hud"], "new_or_loosened_tolerances": False},
            "classification": "controlled offline original-client rendering", "candidate_compared": False,
            "authenticated_source_gameplay": False, "new_presentation_approval": False}
