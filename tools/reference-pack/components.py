"""Source-only bitmap measurement and explicitly labeled review compositions."""

import gzip
import hashlib
import json
from pathlib import Path

from PIL import Image, ImageDraw


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "assets/source/osrs/cache2695"
OUT = ROOT / "research/reference-pack/v1"


class MatchLimitError(ValueError):
    pass


def digest(path):
    data = Path(path).read_bytes()
    return {"path": Path(path).relative_to(ROOT).as_posix(),
            "sha256": hashlib.sha256(data).hexdigest(), "size_bytes": len(data)}


def load_gzip(path):
    return json.loads(gzip.decompress(Path(path).read_bytes()))


def sprite(group, frame=0):
    metadata = load_gzip(SOURCE / f"sprites/{group}.json.gz")["frames"][frame]
    with Image.open(SOURCE / f"sprites/{group}.png") as atlas:
        x, y = metadata["atlas_x"], metadata["atlas_y"]
        image = atlas.convert("RGBA").crop(
            (x, y, x + metadata["width"], y + metadata["height"]))
    return image, metadata


def native_text(image, text, x, baseline, font_id=495, color=(255, 255, 255), shadow=True):
    metrics = json.loads((SOURCE / f"fonts/{font_id}.json").read_text())
    start = x
    for character in text.encode("cp1252", errors="strict"):
        glyph, frame = sprite(font_id, character)
        if glyph.width and glyph.height:
            ink = glyph.getchannel("A").point(lambda value: 255 if value else 0)
            xy = (x + frame["offset_x"], baseline - metrics["ascent"] + frame["offset_y"])
            if shadow:
                image.paste((0, 0, 0), (xy[0] + 1, xy[1] + 1), ink)
            image.paste(color, xy, ink)
        x += metrics["advances"][character]
    return x - start


def text_width(text, font_id=495):
    metrics = json.loads((SOURCE / f"fonts/{font_id}.json").read_text())
    return sum(metrics["advances"][byte] for byte in text.encode("cp1252", errors="strict"))


def centered(image, text, center, baseline, font_id=495, color=(255, 255, 255)):
    return native_text(image, text, center - text_width(text, font_id) // 2, baseline, font_id, color)


def find_component(image, component, max_matches=4, raw=None):
    """Exact all-opaque-pixel comparison; the anchor only accelerates searching."""
    if component.width > image.width or component.height > image.height:
        return []
    rgba = list(component.getdata())
    opaque = [(i % component.width, i // component.width, pixel[:3])
              for i, pixel in enumerate(rgba) if pixel[3] == 255]
    if len(opaque) < 32:
        return []
    longest = None
    for y in range(component.height):
        x = 0
        while x < component.width:
            start = x
            while x < component.width and rgba[y * component.width + x][3] == 255:
                x += 1
            if x > start and (longest is None or x - start > longest[2]):
                longest = (start, y, x - start)
            x += 1
    if longest is None or longest[2] < 4:
        return []
    anchor_x, anchor_y, length = longest
    length = min(32, length)
    anchor = bytes(channel for x in range(anchor_x, anchor_x + length)
                   for channel in rgba[anchor_y * component.width + x][:3])
    raw = image.convert("RGB").tobytes() if raw is None else raw
    matches = []
    offset = -1
    attempts = 0
    while len(matches) < max_matches:
        offset = raw.find(anchor, offset + 1)
        if offset < 0:
            break
        if offset % 3:
            continue
        x = (offset // 3) % image.width - anchor_x
        y = (offset // 3) // image.width - anchor_y
        if x < 0 or y < 0 or x + component.width > image.width or y + component.height > image.height:
            continue
        attempts += 1
        if attempts > 5000:
            raise MatchLimitError("Source component search exceeded its explicit 5000-anchor bound")
        if all(raw[((y + dy) * image.width + x + dx) * 3:
                   ((y + dy) * image.width + x + dx) * 3 + 3] == bytes(rgb)
               for dx, dy, rgb in opaque):
            matches.append({
                "image_rectangle": [x, y, component.width, component.height],
                "fully_opaque_pixels_compared": len(opaque), "different_pixels": 0,
                "transparent_pixels": component.width * component.height - len(opaque),
                "scope": "Entire native component's opaque footprint, not the surrounding panel.",
            })
    return matches


def widget_label(widget_id):
    group = widget_id >> 16
    definition = next(w for w in load_gzip(SOURCE / f"interfaces/{group}.json.gz") if w["id"] == widget_id)
    font_id = definition["fontId"]
    text = definition["text"]
    metrics = json.loads((SOURCE / f"fonts/{font_id}.json").read_text())
    image = Image.new("RGBA", (text_width(text, font_id) + 2, metrics["ascent"] + 14))
    color = definition["textColor"]
    native_text(image, text, 0, metrics["ascent"], font_id,
                ((color >> 16) & 255, (color >> 8) & 255, color & 255), definition["textShadowed"])
    return image, {
        "widget_id": widget_id, "font_id": font_id, "text": text,
        "color": color, "shadow": definition["textShadowed"],
        "advance_width_px": text_width(text, font_id),
        "widget_input": digest(SOURCE / f"interfaces/{group}.json.gz"),
        "font_input": digest(SOURCE / f"fonts/{font_id}.json"),
        "glyph_input": digest(SOURCE / f"sprites/{font_id}.png"),
    }


def measure(public):
    sprite_ids = sorted(int(path.stem) for path in (SOURCE / "sprites").glob("*.png")
                        if int(path.stem) not in (494, 495, 496, 497))
    components = {group: sprite(group) for group in sprite_ids}
    label_ids = [(84 << 16) | child for child in (23, 29, 35, 40, 43)]
    label_ids += [(153 << 16) | child for child in (3, 8)]
    label_ids += [(399 << 16) | 1, (239 << 16) | 3, (300 << 16) | 7]
    labels = {identifier: widget_label(identifier) for identifier in label_ids}
    matches = {}
    text_matches = {}
    inconclusive = []
    for entry in public:
        if entry["decoded"]["format"] == "MP4":
            continue
        with Image.open(ROOT / entry["path"]) as image:
            image.seek(0)
            image = image.convert("RGBA").convert("RGB")
            raw = image.tobytes()
            found = []
            for group, (component, frame) in components.items():
                try:
                    candidates = find_component(image, component, raw=raw)
                except MatchLimitError as error:
                    inconclusive.append({"public_id": entry["id"], "sprite_group": group, "error": str(error)})
                    continue
                for match in candidates:
                    found.append({"sprite_group": group, "sprite_frame": 0,
                                  "frame_metadata": {k: frame[k] for k in
                                                     ("width", "height", "offset_x", "offset_y",
                                                      "canvas_width", "canvas_height")},
                                  "sprite_input": digest(SOURCE / f"sprites/{group}.png"),
                                  "frame_metadata_input": digest(SOURCE / f"sprites/{group}.json.gz"),
                                  **match})
            matches[entry["id"]] = found
            found_text = []
            if image.width <= 900 and image.height <= 650:
                for identifier, (component, metadata) in labels.items():
                    try:
                        candidates = find_component(image, component, raw=raw)
                    except MatchLimitError as error:
                        inconclusive.append({"public_id": entry["id"], "widget_id": identifier, "error": str(error)})
                        continue
                    found_text.extend({**metadata, **match} for match in candidates)
            text_matches[entry["id"]] = found_text
    calibration_text = "Old School RuneScape  |  Attack: 1  Hitpoints: 10  Coins: 1,000"
    fonts = []
    for font_id in (494, 495, 496, 497):
        metrics = json.loads((SOURCE / f"fonts/{font_id}.json").read_text())
        fonts.append({
            "font_id": font_id, "ascent": metrics["ascent"],
            "advances": metrics["advances"], "kerning": metrics["kerning"],
            "glyph_count": len(load_gzip(SOURCE / f"sprites/{font_id}.json.gz")["frames"]),
            "calibration_text": calibration_text, "text_width_px": text_width(calibration_text, font_id),
            "metrics_input": digest(SOURCE / f"fonts/{font_id}.json"),
            "glyph_input": digest(SOURCE / f"sprites/{font_id}.png"),
        })
    widgets = load_gzip(SOURCE / "interfaces/161.json.gz")
    anchor_children = {w["id"] & 65535: w for w in widgets if w["id"] & 65535 in (95, 96, 97)}
    resize = []
    for width, height in ((1024, 768), (1280, 800), (1920, 1080), (2560, 1440)):
        rectangles = {}
        for child, label in ((95, "minimap_container"), (96, "chat_container"), (97, "side_panel_container")):
            w = anchor_children[child]
            x = width - w["originalWidth"] - w["originalX"] if w["xPositionMode"] == 2 else w["originalX"]
            y = height - w["originalHeight"] - w["originalY"] if w["yPositionMode"] == 2 else w["originalY"]
            rectangles[label] = [x, y, w["originalWidth"], w["originalHeight"]]
        resize.append({"logical_viewport": [width, height], "rectangles": rectangles,
                       "classification": "static current widget anchor arithmetic; not original-script execution"})
    result = {
        "schema_version": 1, "scope": "Reference-to-source-asset reconciliation; no ClubScape candidate evaluated",
        "matching_rule": "Native size only; exact RGB on every alpha=255 source component pixel. "
                         "No image normalization, whole-panel exclusion or best-fit scale.",
        "public_component_matches": matches, "public_text_matches": text_matches, "fonts": fonts,
        "inconclusive_searches": inconclusive,
        "static_resize_analysis": resize,
        "widget_input": digest(SOURCE / "interfaces/161.json.gz"),
        "limits": "A matching icon/border does not verify other pixels, hidden state, client scripts, "
                  "camera, lighting, full-frame layout, recording cadence or stock plugin settings.",
    }
    return result


PROPOSALS = {
    "registration": {
        "heading": "Create a ClubScape account",
        "lines": ["Login name: reference_user", "Password: ************", "Confirm:  ************"],
        "buttons": ["Create account", "Cancel"],
        "note": "Login-name/password/confirmation composition only. Real account fields, validation and "
                "persistence must use the existing account contract; this image makes no request.",
    },
    "registration-rejected": {
        "heading": "Account not created",
        "lines": ["That login name is unavailable.", "Your entered name is preserved.", "Choose another name and try again."],
        "buttons": ["Try again", "Cancel"],
        "note": "Explicit rejected outcome; do not report success or discard unrelated entered fields.",
    },
    "connecting": {
        "heading": "Connecting to ClubScape",
        "lines": ["Connecting to the game server...", "Preparing required source assets.", "Progress must reflect actual completed work."],
        "buttons": ["Cancel"],
        "note": "Browser-only composition, not a captured original game-state-20 painter.",
    },
    "capability": {
        "heading": "Unable to start",
        "lines": ["WebGPU is required for ClubScape.", "Use desktop Chrome or Edge.", "No alternate renderer has been started."],
        "buttons": ["Try again", "Back"],
        "note": "No WebGL fallback, fake running world, new dashboard or sandbox-disabling instruction.",
    },
    "runtime-error": {
        "heading": "Unable to continue",
        "lines": ["A required game resource could not load.", "Saved progress is checked after login.", "Retry, or return to the login screen."],
        "buttons": ["Retry", "Back"],
        "note": "Message is an owner-review wording proposal. Show actual error category; never claim "
                "progress was saved unless authoritative acknowledgement proves it.",
    },
    "unavailable": {
        "heading": "Not available in this slice",
        "lines": ["This control is outside the current journey.", "Its source position is preserved.", "Required tutorial features remain available."],
        "buttons": ["Back"],
        "note": "Proposal for scope feedback. In-world equivalent uses the same source chat/continue "
                "visual language; no claim of an implemented overlay or permission to hide required controls.",
    },
    "branding": {
        "heading": "Welcome to ClubScape",
        "lines": ["Source title composition retained.", "Brand text uses the native source font.", "The original source artwork remains credited."],
        "buttons": ["New account", "Existing user"],
        "note": "Minimal name-string proposal inside the source titlebox. The OSRS logo is deliberately "
                "retained as source evidence; final ClubScape logo artwork is not fabricated or approved.",
    },
}


def proposals():
    base_path = ROOT / "assets/reference/osrs240/title/state-10-login-index-0.png"
    box, _ = sprite(499)
    button, _ = sprite(500)
    result = []
    for name, proposal in PROPOSALS.items():
        with Image.open(base_path) as original:
            image = original.convert("RGB")
        # The titlebox is the actual source component; only its controlled content is proposed.
        x, y = 779, 171
        image.paste(box, (x, y), box)
        center = x + box.width // 2
        centered(image, proposal["heading"], center, y + 31, 496, (255, 255, 0))
        for index, line in enumerate(proposal["lines"]):
            if text_width(line, 495) > box.width - 24:
                raise ValueError(f"Proposal text exceeds source titlebox: {name}: {line}")
            centered(image, line, center, y + 59 + index * 21, 495)
        controls = []
        centers = [center] if len(proposal["buttons"]) == 1 else [center - 80, center + 80]
        for label, control_center in zip(proposal["buttons"], centers, strict=True):
            left, top = control_center - button.width // 2, y + 132
            image.paste(button, (left, top), button)
            centered(image, label, control_center, top + 25, 496)
            controls.append({"label": label, "rectangle": [left, top, button.width, button.height]})
        path = OUT / "proposals" / f"{name}.png"
        path.parent.mkdir(parents=True, exist_ok=True)
        image.save(path)
        result.append({
            "id": "proposal." + name, **digest(path), "dimensions": list(image.size),
            "source_role": "composition_proposal",
            "label": "OWNER-REVIEW PROPOSAL - NOT AN OSRS SOURCE CAPTURE OR GAME FRONTEND",
            "approved": False, "source_capture": False, "game_client": False,
            "base_input": digest(base_path),
            "component_inputs": [digest(SOURCE / f"sprites/{group}.png") for group in (499, 500, 495, 496)],
            "titlebox_rectangle": [x, y, box.width, box.height],
            "unchanged_region": "Every pixel outside the titlebox; inside it the reused source "
                                "frame/font/button constraints still apply, not a whole-panel mask.",
            "controls": controls, "content": proposal,
            "target_canvas": [1920, 1080], "source_sprite_scale": 1,
            "position_classification": "Explicit proposal coordinates within the native 765-wide title composition",
            "glyphs": "Actual CP1252 source glyph masks/advances at scale 1; no system font substitution.",
        })
    return result
