"""Source transcript projections and native-font metrics, not generated capture baselines."""

import gzip
import hashlib
import html
import json
from pathlib import Path
import re

from components import ROOT, SOURCE, digest


TRANSCRIPTS = (
    "Transcript:Learning the Ropes",
    "Transcript:Cook's Assistant",
    "Transcript:Cook's Assistant/Journal",
    "Transcript:Death (NPC)",
)
DESKTOP_VARIANTS = {
    "[Click/Tap]": "Click", "[Click/tap]": "Click", "[click/tap]": "click",
    "[clicking/tapping]": "clicking",
    "[swipe across the screen/use your arrow keys]": "use your arrow keys",
    "[use your arrow keys/swipe across the screen]": "use your arrow keys",
    "[tap and hold/right-click on]": "right-click on",
}


def split_fields(value):
    fields = []
    start = 0
    template_depth = link_depth = 0
    index = 0
    while index < len(value):
        token = value[index:index + 2]
        if token == "{{":
            template_depth += 1
            index += 2
        elif token == "}}":
            template_depth -= 1
            index += 2
        elif token == "[[":
            link_depth += 1
            index += 2
        elif token == "]]":
            link_depth -= 1
            index += 2
        else:
            if value[index] == "|" and not template_depth and not link_depth:
                fields.append(value[start:index])
                start = index + 1
            index += 1
        if template_depth < 0 or link_depth < 0:
            raise ValueError("Unbalanced source markup")
    if template_depth or link_depth:
        raise ValueError("Unbalanced source markup")
    return fields + [value[start:]]


def end_token(value, start, opening, closing):
    depth = 1
    index = start + len(opening)
    while index < len(value):
        if value.startswith(opening, index):
            depth += 1
            index += len(opening)
        elif value.startswith(closing, index):
            depth -= 1
            if not depth:
                return index
            index += len(closing)
        else:
            index += 1
    raise ValueError(f"Unterminated source {opening} markup")


def runs_from_wikitext(value, color=None, strike=False):
    runs = []
    index = 0
    buffer = []

    def flush():
        if buffer:
            text = html.unescape("".join(buffer)).replace("'''", "").replace("''", "")
            runs.append({"text": text, "color": color, "strikethrough": strike})
            buffer.clear()

    while index < len(value):
        if value.startswith("{{", index):
            flush()
            end = end_token(value, index, "{{", "}}")
            fields = split_fields(value[index + 2:end])
            name = fields[0].strip().lower()
            if name == "colour":
                if len(fields) != 3 or not re.fullmatch(r"#[0-9a-fA-F]{6}", fields[1]):
                    raise ValueError(f"Unsupported source colour: {fields}")
                runs.extend(runs_from_wikitext(fields[2], fields[1].lower(), strike))
            elif name in ("tbox", "topt", "tselect"):
                positional = [field for field in fields[1:] if not field.startswith("pic=")]
                runs.extend(runs_from_wikitext("|".join(positional), color, strike))
            elif name == "!":
                runs.append({"text": "|", "color": color, "strikethrough": strike})
            elif name == "sic":
                if len(fields) > 1:
                    runs.extend(runs_from_wikitext(fields[1], color, strike))
            elif name == "tmissing":
                raise ValueError("Missing source prose cannot be projected as known text")
            else:
                raise ValueError(f"Unsupported DISPLAY template: {name}")
            index = end + 2
        elif value.startswith("[[", index):
            flush()
            end = end_token(value, index, "[[", "]]")
            fields = split_fields(value[index + 2:end])
            runs.extend(runs_from_wikitext(fields[-1], color, strike))
            index = end + 2
        elif re.match(r"<br\s*/?>", value[index:], re.I):
            flush()
            match = re.match(r"<br\s*/?>", value[index:], re.I)
            runs.append({"text": "\n", "color": color, "strikethrough": strike})
            index += match.end()
        elif value.startswith("<s>", index):
            flush()
            end = value.find("</s>", index + 3)
            if end < 0:
                raise ValueError("Unterminated source strikethrough")
            runs.extend(runs_from_wikitext(value[index + 3:end], color, True))
            index = end + 4
        else:
            buffer.append(value[index])
            index += 1
    flush()
    for run in runs:
        for original, desktop in DESKTOP_VARIANTS.items():
            run["text"] = run["text"].replace(original, desktop)
    return runs


def metrics_for(text):
    lines = text.split("\n")
    result = {}
    for identifier in (494, 495, 496, 497):
        metrics = json.loads((SOURCE / f"fonts/{identifier}.json").read_text())
        advances = [[metrics["advances"][code] for code in line.encode("cp1252", errors="strict")] for line in lines]
        result[str(identifier)] = {
            "ascent_px": metrics["ascent"], "logical_line_advances_px": [sum(line) for line in advances],
            "codepoint_advances_px": advances, "kerning": None,
        }
    return result


def source_page(pages, title):
    matches = [page for page in pages if page["title"] == title]
    if len(matches) != 1:
        raise ValueError(f"Missing/duplicate pinned transcript: {title}")
    page = matches[0]
    path = ROOT / page["snapshot"]["path"]
    if digest(path)["sha256"] != page["snapshot"]["sha256"]:
        raise ValueError(f"Changed transcript snapshot: {title}")
    source = json.loads(gzip.decompress(path.read_bytes()))["page"]["revisions"][0]
    if source["revid"] != page["revision"]:
        raise ValueError(f"Wrong transcript revision: {title}")
    return page, source["slots"]["main"]["content"]


def source_blocks(content):
    lines = content.splitlines()
    index = 0
    while index < len(lines):
        start = index
        raw = lines[index]
        index += 1
        if raw.startswith("*"):
            while raw.count("{{") > raw.count("}}"):
                if index >= len(lines) or lines[index].startswith(("*", "==")):
                    raise ValueError(f"Unterminated source display block at line{start + 1}")
                raw += "\n" + lines[index]
                index += 1
        yield start + 1, raw


def make_text_oracles(pages):
    records = []
    inputs = []
    for title in TRANSCRIPTS:
        page, content = source_page(pages, title)
        inputs.append({"title": title, "revision": page["revision"], "url": page["url"],
                       "snapshot": page["snapshot"], "wikitext_sha256": hashlib.sha256(content.encode()).hexdigest()})
        headings = {}
        for line_number, raw in source_blocks(content):
            header = re.fullmatch(r"(={2,})\s*(.*?)\s*\1", raw)
            if header:
                level = len(header[1])
                headings = {depth: text for depth, text in headings.items() if depth < level}
                headings[level] = header[2]
                continue
            if not raw.startswith("*"):
                continue
            value = re.sub(r"\n\s*(?=\}\})", "", raw.lstrip("*").strip())
            missing = "{{tmissing" in value.lower()
            speaker = re.match(r"'''(.+?):'''\s*(.*)", value)
            template = re.match(r"\{\{(tbox|topt|tselect)\|", value, re.I)
            journal = title.endswith("/Journal") and ("{{Colour|" in value or "<s>" in value)
            if not (speaker or template or journal or missing):
                continue
            kind = "source_text_unrecorded" if missing else (
                "npc_or_player_dialogue" if speaker else "journal" if journal else template[1].lower())
            if speaker:
                value = speaker[2]
            if journal:
                editorial = re.match(r"''\((.*?)\)'':\s*", value)
                if editorial:
                    value = value[editorial.end():]
            runs = None if missing else runs_from_wikitext(value)
            text = None if missing else "".join(run["text"] for run in runs)
            if text is not None and ("{{" in text or "[[" in text):
                raise ValueError(f"Unprocessed source display markup: {title}:{line_number}")
            conditional_tokens = [] if text is None else sorted(set(re.findall(r"\[[^\[\]\n]+/[^\[\]\n]+\]", text)))
            identifier = f"text.{page['page_id']}.{page['revision']}.{line_number}"
            font_binding = None
            if speaker and speaker[1] != "Player":
                widget_path = SOURCE / "interfaces/231.json.gz"
                widgets = json.loads(gzip.decompress(widget_path.read_bytes()))
                body = next(widget for widget in widgets if widget["id"] == (231 << 16) | 6)
                font_binding = {"font_id": body["fontId"], "widget_id": body["id"],
                                "widget_input": digest(widget_path),
                                "scope": "Actual current static widget font; native script overrides must be recorded if present."}
            records.append({
                "id": identifier, "kind": kind, "source_page": title, "source_revision": page["revision"],
                "source_url": page["url"], "source_snapshot": page["snapshot"], "source_line": line_number,
                "section_path": [headings[depth] for depth in sorted(headings)],
                "raw_wikitext": raw, "raw_line_sha256": hashlib.sha256(raw.encode()).hexdigest(),
                "speaker": speaker[1] if speaker else None, "runs": runs, "desktop_text": text,
                "conditional_source_tokens": conditional_tokens,
                "metrics_by_font": None if text is None else metrics_for(text),
                "source_role": "pinned_source_transcript_projection" if text is not None else "explicit_source_text_gap",
                "font_binding": "Use the original widget/native-panel font, not whichever font fits best. "
                                "These are metrics for all four AVAILABLE source fonts, not four interchangeable styles.",
                "static_source_font_binding": font_binding,
                "evidence_gate": "candidate_text_and_state_validation; not a separate screenshot prerequisite",
            })
    return {
        "schema_version": 1, "inputs": inputs, "records": records,
        "source_capture": False, "candidate_evaluated": False,
        "normalization": {
            "desktop_alternatives": DESKTOP_VARIANTS,
            "links": "Use source display label; wiki links/editorial bold/italic are not native glyphs.",
            "colour_and_strike": "Retain source colour runs and strikethrough independently of glyph advances.",
            "conditional_tokens": "Keep non-device alternatives unresolved until a source-permitted branch is chosen.",
            "missing_text": "Never substitute a fabricated line; retain the source gap for exact dialogue validation.",
        },
        "font_inputs": [digest(SOURCE / f"fonts/{identifier}.json") for identifier in (494, 495, 496, 497)],
    }


def project_record(record, choices=None):
    if record["desktop_text"] is None:
        raise ValueError("No independent source text for this unrecorded line")
    choices = {} if choices is None else choices
    if set(choices) != set(record["conditional_source_tokens"]):
        raise ValueError("Every conditional source token requires an explicit permitted branch")
    runs = [dict(run) for run in record["runs"]]
    for token, replacement in choices.items():
        if replacement not in token[1:-1].split("/"):
            raise ValueError(f"Invented source branch text for {token}")
        for run in runs:
            run["text"] = run["text"].replace(token, replacement)
    text = "".join(run["text"] for run in runs)
    return {"text": text, "runs": runs, "metrics_by_font": metrics_for(text)}


def check_text_projection(record, actual_text, font_id, actual_advances, *, actual_runs, choices=None):
    expected = project_record(record, choices)
    if actual_text != expected["text"]:
        raise ValueError("Dynamic text differs from pinned source transcript")
    if actual_runs != expected["runs"]:
        raise ValueError("Source colour/strikethrough/text runs differ")
    if str(font_id) not in expected["metrics_by_font"]:
        raise ValueError("Not an original source font")
    binding = record["static_source_font_binding"]
    if binding is not None and font_id != binding["font_id"]:
        raise ValueError("Wrong font for the current source widget")
    if actual_advances != expected["metrics_by_font"][str(font_id)]["codepoint_advances_px"]:
        raise ValueError("Native glyph advances differ")
    return {"text_matches": True, "source_style_matches": True, "native_advances_match": True,
            "static_font_binding_checked": binding is not None,
            "pixel_or_candidate_acceptance": False}


def check_source_values(source_facts, actual_values, required_fields):
    if set(actual_values) != set(required_fields) or not set(required_fields) <= set(source_facts):
        raise ValueError("Missing/extra independently specified dynamic value")
    for field in required_fields:
        if type(actual_values[field]) is not type(source_facts[field]) or actual_values[field] != source_facts[field]:
            raise ValueError(f"Dynamic value differs from source: {field}")
    return True


def check_full_panel_partition(width, height, checks):
    if any(type(value) is not int or not 0 < value <= 16384 for value in (width, height)):
        raise ValueError("Source panel dimensions must be positive bounded integers")
    allowed = {"source_pixels", "source_background_and_glyphs", "source_background_and_item_sprite",
               "source_widget_primitive"}
    rows = [[] for _ in range(height)]
    for check in checks:
        if check["verifier"] not in allowed or check.get("skip", False):
            raise ValueError("A dynamic field is checked by a source oracle, never masked")
        x, y, w, h = check["rectangle"]
        if any(type(value) is not int for value in (x, y, w, h)):
            raise ValueError("Source pixel rectangles require integer coordinates")
        if min(x, y) < 0 or min(w, h) <= 0 or x + w > width or y + h > height:
            raise ValueError("Comparison rectangle outside source panel")
        if not check["source_input_ids"]:
            raise ValueError("Every pixel verifier requires independent source inputs")
        for row in range(y, y + h):
            rows[row].append((x, x + w))
    for intervals in rows:
        cursor = 0
        for start, end in sorted(intervals):
            if start != cursor:
                raise ValueError("Panel partition has unchecked or multiply-owned pixels")
            cursor = end
        if cursor != width:
            raise ValueError("Panel partition leaves source pixels unchecked")
    return True
