#!/usr/bin/env python3
"""Assemble a source-evidence review pack, not a product baseline or game renderer."""

from collections import Counter
from datetime import datetime, timezone
import gzip
import hashlib
import html
import json
import os
from pathlib import Path
import re
import urllib.parse

from PIL import Image, ImageDraw

from catalogue import EXTRA_CASES, NUMERIC_PROFILES, TUTORIAL_GROUPS, dispositions, music_update, update, wiki
from components import ROOT, OUT, SOURCE, digest, load_gzip, measure, proposals
from fetch import image_facts, write_json
from factoring import make_factoring, review_ready
from native_hud import load_inputs as load_native_hud_inputs, calibration as native_hud_calibration, wire_cases as wire_native_hud


CAPTURE_PATH = ROOT / "assets/reference/osrs240/captures.json"
AUDIO_PATH = ROOT / "assets/manifests/osrs/audio-runtime.json"
TUTORIAL_PATH = ROOT / "research/journey-rules/tutorial.json"


def original_id(path):
    return "original." + path.removesuffix(".png").replace("/", ".")


def local_record(path, role):
    return {**digest(ROOT / path), "source_role": role}


def original_inputs():
    data = json.loads(CAPTURE_PATH.read_text())
    provenance = json.loads((CAPTURE_PATH.parent / "provenance.json").read_text())
    records = []
    for capture in data["captures"]:
        actual = CAPTURE_PATH.parent / capture["path"]
        if digest(actual)["sha256"] != capture["sha256"]:
            raise ValueError(f"Original fixture changed: {actual}")
        records.append({
            **capture, "id": original_id(capture["path"]),
            "path": actual.relative_to(ROOT).as_posix(),
            "dimensions": [capture["width"], capture["height"]],
            "source_role": "current_original_runtime_fixture",
            "captured_at": provenance["captured_at"],
            "source_capture_build": 240, "source_cache_id": 2695,
            "full_resizable_classic_frame": False,
            "browser": None, "browser_dpr": None,
            "settings_origin": "Verbatim per-image original capture metadata, not reconstructed defaults",
        })
    return records


def published_inventory():
    published = json.loads((ROOT / "assets/manifests/osrs/cache2695-published.json").read_text())
    result = []
    for entry in published["published_files"]:
        actual = digest(ROOT / entry["path"])
        if any(actual[key] != entry[key] for key in ("sha256", "size_bytes")):
            raise ValueError(f"Published source input changed: {entry['path']}")
        result.append({**entry, "category": entry["path"].split("/")[4],
                       "role": "current_original_source_asset_not_a_rendering_approval"})
    return result


def symbols():
    path = ROOT / ".local/reference-pack/upstream/InterfaceID.java"
    output = OUT / "sources/runelite-interface-id.java.gz"
    expected = "ee9cadd2bf61059a43c9e6e4be865a34195ca5bc84e74cb94e5487d88e345753"
    raw = path.read_bytes() if path.exists() else gzip.decompress(output.read_bytes())
    if hashlib.sha256(raw).hexdigest() != expected:
        raise ValueError("Wrong pinned InterfaceID source")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(gzip.compress(raw, mtime=0))
    selected = set(json.loads((ROOT / "research/current-source/m1-request.json").read_text())["interface_groups"])
    names = {int(value): name for name, value in re.findall(
        r"^\tpublic static final int (\w+) = (\d+);", raw.decode(), re.M) if int(value) in selected}
    if set(names) != selected:
        raise ValueError("Incomplete selected widget symbol mapping")
    return {
        "source": digest(output), "uncompressed_sha256": expected,
        "url": "https://raw.githubusercontent.com/runelite/runelite/"
               "ac79ed8bd8926bec7bf172aa291574b4d944b0e7/"
               "runelite-api/src/main/java/net/runelite/api/gameval/InterfaceID.java",
        "revision": "ac79ed8bd8926bec7bf172aa291574b4d944b0e7",
        "notice": "Original BSD notice retained in the complete compressed source file.",
        "groups": [{"id": group, "name": names[group],
                    "input": digest(SOURCE / f"interfaces/{group}.json.gz")}
                   for group in sorted(names)],
    }


def document_bindings():
    paths = [
        "AGENTS.md", "prompt.md", "milestones/m1-owner-followups.json",
        "spec/interface-parity.md", "spec/art-style.md", "spec/world.md",
        "research/current-source/selection.json", "research/current-source/extraction-contract.json",
        "research/current-source/runtime-config.ws", "research/current-source/m1-request.json",
        "research/source-capture/coverage.json", "research/source-capture/handoff.json",
        "assets/reference/osrs240/captures.json", "assets/reference/osrs240/provenance.json",
        "assets/manifests/osrs/cache2695-published.json", "assets/manifests/osrs/cache2695-full-bundle.json.gz",
        "assets/manifests/osrs/audio-runtime.json", "research/audio-source/source-map.json",
        "research/audio-source/references.json", "research/audio-source/browser-decode.json",
        "research/audio-source/conversion-evidence.json.gz", "research/audio-source/extra-inputs.json.gz",
        "research/audio-source/layout-check.json", "tools/audio-import/dependencies.json",
        "tools/audio-import/codec.py", "tools/source-capture/capture.py",
        "tools/source-capture/NATIVE_HUD.md",
        "assets/reference/osrs240/reference-sets.json",
        "assets/reference/osrs240/native-hud/captures.json",
        "assets/reference/osrs240/native-hud/provenance.json",
        "assets/reference/osrs240/native-hud/hud-input-contract.json",
        "research/source-capture/native-hud-bindings.json",
        "research/source-capture/native-hud-validation.json",
        "research/journey-rules/tutorial.json", "research/journey-rules/cooks-assistant.json",
        "research/journey-rules/activities.json", "research/journey-rules/initial-state.json",
        "research/journey-rules/sources.json", "research/journey-rules/decisions.json",
        "assets/source/osrs/NOTICE.txt", "assets/source/osrs/audio-runtime/NOTICE.txt",
    ]
    return [local_record(path, "hash_bound_existing_contract_or_inventory") for path in paths]


def case_base(identifier, title, family, input_ids, widget_groups, profile):
    return {
        "id": "case." + identifier, "title": title, "family": family,
        "input_ids": list(dict.fromkeys(input_ids)),
        "current_widget_groups": widget_groups,
        "comparison_profile": profile,
        "numeric_tolerances": NUMERIC_PROFILES[profile],
        "comparison_procedure": [
            "Validate exact source hashes, dimensions and declared source role before opening any candidate.",
            "Use the native-size source inputs and recorded fixture settings. Do not fit/scale/warp a candidate to the source.",
            "Compare every unchanged source region. Native sprites/fonts use the zero-error profile in addition to this case profile.",
            "Unknown public settings remain unknown. Factor data-only states through independent source "
            "text/value/font oracles; absent genuine layout/calibration inputs remain explicit, never masks.",
            "Evaluate penguin/equipment, name/logo and web-only proposals separately after owner approval.",
            "Store source/candidate hashes, state, full images, overlay, difference, per-region numeric results and failures.",
        ],
        "primary_target": [1920, 1080], "target_dpr": 1, "target_ui_scale": 1,
        "capture_settings_reference": "Input record settings; public unknowns are explicit and never copied from a fixture.",
        "interaction_variants_required": ["successful", "locked_or_disabled", "cancelled", "rejected",
                                          "reconnect_without_duplicate_effect"],
        "source_gap_refs": [], "product_acceptance": "not_run",
        "evidence_status": "component_reference_for_owner_review",
    }


def make_cases(originals, public, proposal_inputs, audio):
    tutorial = json.loads(TUTORIAL_PATH.read_text())
    by_title = {entry["title"].removeprefix("File:"): entry["id"] for entry in public}
    public_ids = set(by_title.values())
    original_ids = {entry["id"] for entry in originals}
    proposal_ids = {entry["id"] for entry in proposal_inputs}
    audio_by_key = {f"{entry['kind']}.{entry['source_group']}": entry["asset_id"]
                    for entry in audio["assets"]}
    audio_map = json.loads((ROOT / "research/audio-source/source-map.json").read_text())
    audio_families = {
        "gathering": ["rule.mining.", "rule.woodcutting.", "rule.fishing.", "rule.firemaking."],
        "production": ["rule.cooking.", "rule.smelting.", "rule.smithing."],
        "combat": ["rule.combat.", "rule.magic.", "rule.food.", "rule.goblin.", "rule.death."],
        "interaction": ["rule.inventory.", "rule.equipment.", "rule.prayer.", "rule.bank.", "rule.shop."],
    }
    groups = {}
    for suffixes, section, titles, widgets, scene in TUTORIAL_GROUPS:
        for suffix in suffixes.split():
            if suffix in groups:
                raise ValueError(f"Duplicate authored tutorial mapping: {suffix}")
            groups[suffix] = (section, titles, widgets, scene)
    actual_suffixes = {state["id"].removeprefix("stage.tutorial.") for state in tutorial["states"]}
    if set(groups) != actual_suffixes:
        raise ValueError(f"Tutorial case mapping mismatch: {set(groups) ^ actual_suffixes}")
    cases = []
    unlocked = set()
    permanent = {"ui.settings", "ui.inventory", "ui.skills", "ui.quests", "ui.equipment",
                 "ui.combat", "ui.account", "ui.logout", "ui.prayer", "ui.magic"}
    for index, state in enumerate(tutorial["states"]):
        suffix = state["id"].removeprefix("stage.tutorial.")
        section, titles, widgets, scene = groups[suffix]
        identifiers = [by_title[title] for title in titles]
        if scene:
            identifiers.append(original_id(f"scenes/tutorial-{scene}.png"))
        identifiers.extend(original_id(capture["path"].removeprefix("assets/reference/osrs240/"))
                           for capture in originals if capture["kind"] == "original-runtime-item-icon")
        case = case_base("tutorial." + suffix, state["instruction"], "tutorial", identifiers,
                         widgets, "public_lossless_ui")
        unlocked.update(set(state.get("ui_unlock_refs", [])) & permanent)
        case.update({
            "journey_state_id": state["id"], "journey_order": index,
            "source_transcript": {
                "page": "Transcript:Learning the Ropes", "revision": 15309360,
                "section": section,
                "url": "https://oldschool.runescape.wiki/w/Transcript:Learning_the_Ropes?oldid=15309360#"
                       + urllib.parse.quote(section.replace(" ", "_")),
            },
            "expected_instruction": state["instruction"],
            "instruction_kind": "Existing semantic contract; exact source prose remains in the pinned transcript snapshot.",
            "declared_controls": state.get("ui_unlock_refs", []),
            "cumulative_expected_tabs": sorted(unlocked),
            "unlock_evidence_role": "inference_from_symbolic_contract_not_observed_numeric_widget_visibility",
            "source_numeric_progress": state["source_numeric_progress"],
            "evidence_scope": "Public lesson/interface/portrait pixels plus exact current assets and transcript. "
                              "Not a manufactured full-stage source screenshot or proof of legitimate progression.",
        })
        if suffix == "experience":
            case["input_ids"] = [by_title[update(4)]]
        if suffix == "departure_offer":
            case["input_ids"] = [by_title["Learning the Ropes reward scroll.png"]]
        if suffix in ("magic_open", "magic_supply", "wind_strike"):
            case["variant_note"] = "The public full spellbook is NOT the tutorial's initially level-filtered list; "
            case["variant_note"] += "require the current filtered state, not every spell shown in the illustration."
        if suffix in ("journal_open", "quest_explanation"):
            case["variant_note"] = "Official October 2025 parchment/list says Tutorial Island. Use only its "
            case["variant_note"] += "components; current source names the quest Learning the Ropes."
        if suffix == "mainland":
            case["input_ids"].extend([original_id("scenes/lumbridge-castle-plaza.png"),
                                      by_title["Adventure Paths interface.png"]])
        cases.append(case)
    audio_ids = [entry["asset_id"] for entry in audio["assets"]]
    for suffix, title, kind, names, title_indices, widgets in EXTRA_CASES:
        profile = "public_lossless_ui"
        identifiers = []
        audio_rules = []
        if kind == "title":
            identifiers = [original_id(f"title/state-10-login-index-{index}.png") for index in title_indices]
            profile = "native_sprite_font"
        elif kind == "loading":
            identifiers = [original_id("title/state-0-login-index-0.png")]
            profile = "native_sprite_font"
        elif kind == "proposal":
            identifiers = ["proposal." + name for name in names]
            profile = "owner_composition"
        elif kind == "scene":
            identifiers = [original_id("scenes/" + name + ".png") for name in names]
            profile = "native_scene_model"
        elif kind == "model":
            fragment = {"tree": "tree-1277", "goblin": "npc-3028", "penguin": "npc-2063"}[names[0]]
            identifiers = [entry["id"] for entry in originals if fragment in entry["id"]]
            profile = "penguin_adaptation" if names[0] == "penguin" else "native_scene_model"
        elif kind == "audio":
            profile = "audio_source"
            if names[0].startswith("music."):
                identifiers = [audio_by_key[names[0]]]
            elif names[0] == "jingles":
                identifiers = [entry["asset_id"] for entry in audio["assets"] if entry["kind"] == "jingle"]
            elif names[0] in ("controls", "transitions", "reconnect"):
                identifiers = [by_title["Audio options interface.png"], by_title["Music tab.png"],
                               by_title[music_update(1, "gif")], by_title[music_update(4)],
                               by_title[music_update(2, "mp4")]]
                identifiers += [entry["asset_id"] for entry in audio["assets"] if entry["kind"] == "music"]
            else:
                sounds = set()
                if names[0] == "ambient":
                    for obj in audio_map["ambient_objects"]:
                        fields = obj["source_fields"]
                        if fields["ambientSoundId"] >= 0:
                            sounds.add(fields["ambientSoundId"])
                        sounds.update(fields["ambientSoundIds"] or [])
                else:
                    for action in audio_map["actions"]:
                        if any(action["journey_rule_id"].startswith(prefix)
                               for prefix in audio_families[names[0]]):
                            audio_rules.append(action["journey_rule_id"])
                            sounds.update(action["identified_sound_ids"])
                    if names[0] == "production":
                        sounds.add(2725)
                    if names[0] == "interaction":
                        for action in audio_map["quest_and_gathering"]:
                            if "sound_ids" in action:
                                sounds.update(action["sound_ids"])
                            elif "jingle_candidates" not in action:
                                raise ValueError(f"Unknown quest audio source-map shape: {action['action']}")
                identifiers = [entry["asset_id"] for entry in audio["assets"]
                               if entry["kind"] == "sfx" and entry["source_group"] in sounds]
        else:
            identifiers = [by_title[name] for name in names]
        case = case_base(suffix, title, kind, identifiers, widgets, profile)
        if kind in ("title", "loading", "scene", "model"):
            case["evidence_status"] = "original_fixture_reference_ready"
            case["evidence_scope"] = "Exactly the controlled state in the original capture metadata, not a source account journey."
        if kind == "proposal":
            case["evidence_status"] = "owner_review_proposal"
            case["approval_required"] = True
        if suffix == "hud.resizable_classic":
            case["evidence_scope"] = (
                "Complete Classic side-panel crop + actual minimap/chat inputs + "
                "current native root anchors. No retrieved full Classic frame is asserted."
            )
        if suffix == "world.cooks_route":
            case["input_ids"].extend(by_title[name] for name in (
                "Cook's Assistant.png", "Mill Lane Mill (interior, ground floor).png",
                "Mill Lane Mill (interior, 1st floor).png", "Mill Lane Mill (interior, 2nd floor).png"))
        if suffix == "model.penguin":
            case["evidence_status"] = "owner_review_adaptation_using_original_npc"
            case["approval_required"] = True
            case["adaptation_note"] = "NPC2063 is a source reference, not self-approved as the player. "
            case["adaptation_note"] += "Retain every equipment slot; attachments and fitting remain product work."
        if kind == "audio":
            case["source_map"] = "research/audio-source/source-map.json"
            case["action_rule_ids"] = audio_rules
            case["unbound_trigger_policy"] = "An empty identified_sound_ids list means unverified, not silence."
            if names[0] == "production":
                case["candidate_only_sound_ids"] = [2725]
            case["evidence_scope"] = "Exact original FLAC inputs and source-cache relationships; no new conversion. "
            case["evidence_scope"] += "Public playback controls/recording supplement these. Live triggers/mixer are not guessed."
        cases.append(case)
    all_ids = original_ids | public_ids | proposal_ids | set(audio_ids)
    inputs = {entry["id"]: entry for entry in originals + public + proposal_inputs}
    for case in cases:
        missing = set(case["input_ids"]) - all_ids
        if not case["input_ids"] or missing:
            raise ValueError(f"Missing actual inputs for {case['id']}: {missing}")
        case["input_roles"] = []
        for identifier in case["input_ids"]:
            if identifier in audio_ids:
                role, profile = "current_original_audio_input", "audio_source"
            else:
                entry = inputs[identifier]
                role = entry["source_role"]
                if identifier.startswith("proposal."):
                    profile = "owner_composition"
                elif identifier.startswith("original."):
                    profile = ("native_scene_model" if entry["kind"] in
                               ("original-runtime-model", "original-runtime-scene-fixture")
                               else "native_sprite_font")
                else:
                    profile = ("public_lossy_motion" if entry["decoded"]["format"] in ("MP4", "GIF", "JPEG")
                               else "public_lossless_ui")
            case["input_roles"].append({
                "input_id": identifier, "source_role": role, "comparison_profile": profile,
                "numeric_tolerances": NUMERIC_PROFILES[profile],
                "normative_scope": "Only the actual input's declared scope/settings and reconciled unchanged regions; "
                                   "not an invented full-frame state.",
            })
    return cases


def gaps(factoring):
    return [
        {"id": row["id"], "status": row["status"], "required_case_ids": row["case_ids"],
         "source_family_ids": row["family_ids"], "literal_basis": row["literal_basis"],
         "exact_deficit": row["missing"], "useful_evidence_already_in_pack": row["available"],
         "bounded_resolution": row["not_required"], "account_login_required_by_pack": False}
        for row in factoring["source_review_requirements"] if row["status"] != "available"
    ]


def audio_handoff(factoring):
    source_map_path = ROOT / "research/audio-source/source-map.json"
    source_map = json.loads(source_map_path.read_text())
    by_rule = {row["journey_rule_id"]: row for row in source_map["actions"]}
    requirement = next(row for row in factoring["source_review_requirements"]
                       if row["id"] == "input.required_effect_bindings")
    anchors = {
        "rule.combat.ranged": {"item_ids": [841]},
        "rule.goblin.level_2": {"npc_ids": [3028]},
        "rule.combat.tutorial_rat": {"npc_ids": [3313, 3314, 3315]},
        "rule.food.healing": {"item_ids": [315, 2309]},
        "rule.smelting.bronze": {"item_ids": [436, 438, 2349], "candidate_sound_ids": [2725]},
    }
    return {
        "schema_version": 1, "scope": "Only remaining source-audio identity/event/precedence inputs; no HUD blocker.",
        "source_map": digest(source_map_path),
        "playable_source_manifest": digest(AUDIO_PATH),
        "unresolved_actions": [
            {"rule_id": rule, **anchors[rule],
             "retained_sequence_ids": by_rule[rule]["source_sequence_ids"],
             "sequence_role": "Existing source-map anchors/candidates, not a newly verified event binding.",
             "identified_sound_ids": by_rule[rule]["identified_sound_ids"],
             "source_note": by_rule[rule]["note"],
             "needed": "Actual source sound ID/variant and source-relative event boundary, with loops/weights "
                       "where relevant. Do not replace it with a generic click, grunt, silence or plausible candidate."}
            for rule in requirement["unbound_rule_ids"]
        ],
        "quest_selection": {
            "quest_refs": ["quest.learning_the_ropes", "quest.cooks_assistant"],
            "retained_source_jingle_ids": [152, 153, 154],
            "current_best_candidate": 154,
            "candidate_basis": "Pinned wiki says154 usually accompanies Beginner/Easy quests; not a per-quest selector.",
            "needed": ["exact per-quest jingle ID/selection rule",
                       "quest-completion versus simultaneous level-up jingle ordering/interrupt/queue precedence"],
        },
        "not_requested_again": ["cache acquisition", "audio conversion", "native HUD/panel rendering",
                                "per-microstate authenticated screenshots", "a blanket source account handoff"],
        "later_candidate_checks": ["audible output", "gain/device latency", "bound event synchronization",
                                  "region transitions/loops", "gesture and reconnect behavior"],
        "owner_approved": False,
    }


def search_log():
    path = OUT / "search-log.json"
    previous = json.loads(path.read_text()) if path.exists() else None
    records = []
    for cache in sorted((ROOT / ".local/reference-pack/api").glob("*.json")):
        record = json.loads(cache.read_text())
        response = record["response"]
        params = urllib.parse.parse_qs(urllib.parse.urlparse(record["url"]).query)
        query = response.get("query", {})
        records.append({
            "api_url": record["url"], "retrieved_at": record["retrieved_at"],
            "search_term": params.get("srsearch", [None])[0],
            "namespace": params.get("srnamespace", [None])[0],
            "matching_titles": [row["title"] for row in query.get("search", [])],
            "total_hits": query.get("searchinfo", {}).get("totalhits"),
            "missing_titles": [row["title"] for row in query.get("pages", []) if row.get("missing")],
            "continuation": response.get("continue"),
            "scope": "Bounded discovery; no assumption that an unconsumed continuation was exhausted.",
        })
    if previous is not None:
        merged = {(row["api_url"], row["retrieved_at"]): row for row in previous["mediawiki_queries"]}
        merged.update({(row["api_url"], row["retrieved_at"]): row for row in records})
        previous["mediawiki_queries"] = sorted(merged.values(), key=lambda row: (row["retrieved_at"], row["api_url"]))
        return previous
    return {
        "schema_version": 1, "mediawiki_queries": records,
        "distinct_alternatives": [
            {"source": "Internet Archive public advancedsearch",
             "query": 'title:("Tutorial Island") AND (title:(OSRS) OR title:(RuneScape) OR description:(OSRS))',
             "result": "One result: https://archive.org/details/2022-11-14-18-07-29, HDOS - Tutorial Island. "
                       "Rejected because it is HDOS, not stock matching visuals."},
            {"source": "Official Steam app1343370 public API",
             "url": "https://store.steampowered.com/api/appdetails?appids=1343370&filters=basic,screenshots",
             "result": "Ten actual 1920x1080 source marketing images retrieved and decoded in ignored scratch. "
                       "All omit the HUD, so none establishes the required Classic layout. Not pack inputs."},
            {"source": "Public RuneLite issue search / file history",
             "result": "Bounded issue examples involve GPU/stretched/status-bar plugins; no logs/credentials "
                       "downloaded. RuneLite client.png has one Modern-layout upload; Interface.png history "
                       "contains fixed 765x503 variants, not Classic resizable."},
            {"source": "Public RuneHQ candidate walkthrough URLs",
             "urls": ["https://www.runehq.com/oldschoolquest/tutorial-island",
                      "https://www.runehq.com/oldschoolquest/learning-the-ropes"],
             "result": "Both returned HTTP404."},
            {"source": "Search-engine suggestions verified against MediaWiki",
             "result": "Suggested Resizable Classic Layout.png, Character Creator interface.png and Lumbridge "
                       "arrival.png do not exist in the exact File API. Rejected rather than trusted."},
            {"source": "Original-file delivery",
             "result": "Cloudflare Polish changed cached PNG bytes. Both ordinary URLs and cached download=1 "
                       "responses were rejected when different from imageinfo SHA-1/size. Fresh public query "
                       "keys returned exact original bytes. Every selected MediaWiki original is SHA-1 checked."},
        ],
        "authorization": "Public reading/retrieval only. No source account creation, login, secret reads, purchases "
                         "or public-world load tests. Owner terms authorization is recorded, not exercised by these queries.",
    }


def gallery(manifest):
    directory = OUT / "gallery"
    directory.mkdir(parents=True, exist_ok=True)
    all_media = {entry["id"]: entry for entry in manifest["original_inputs"] + manifest["public_inputs"]
                 + manifest["proposal_inputs"] + manifest["recording_frames"] + manifest["native_hud_inputs"]}
    audio = {entry["asset_id"]: entry for entry in manifest["audio"]["assets"]}
    factoring = manifest["evidence_factorization"]
    text_records = {record["id"]: record for record in
                    json.loads((ROOT / manifest["dynamic_text_oracles"]["path"]).read_text())["records"]}

    def link(path):
        return html.escape(os.path.relpath(ROOT / path, directory).replace(os.sep, "/"), quote=True)

    def image_markup(entry):
        if entry.get("decoded", {}).get("format") == "MP4":
            return f'<video controls preload="none" src="{link(entry["path"])}"></video>'
        return (f'<a href="{link(entry["path"])}"><img loading="eager" decoding="async" '
                f'src="{link(entry["path"])}" alt="{html.escape(entry["id"], quote=True)}"></a>')

    cards = []
    for case in manifest["cases"]:
        previews = [all_media[key] for key in case["input_ids"] if key in all_media][:3]
        status = "SOURCE GAP" if case["source_gap_refs"] else case["evidence_status"]
        refs = []
        for key in case["input_ids"]:
            entry = all_media.get(key) or audio[key]
            refs.append(f'<li><a href="{link(entry["path"])}">{html.escape(key)}</a> '
                        f'<code>{entry["sha256"][:16]}</code></li>')
        cards.append(
            f'<article data-case-id="{case["id"]}" data-family="{case["family"]}" '
            f'data-search="{html.escape(case["id"] + " " + case["title"], quote=True)}">'
            f'<h3 id="{case["id"]}"><a href="#{case["id"]}">{html.escape(case["id"])}</a></h3>'
            f'<p>{html.escape(case["title"])}</p><p class="status">{html.escape(status)}</p>'
            + ''.join(image_markup(entry) for entry in previews)
            + f'<p>Profile: <code>{case["comparison_profile"]}</code>. '
              f'Gaps: {html.escape(", ".join(case["source_gap_refs"]) or "none for the declared reference scope")}.</p>'
            + '<p>Shared families: ' + ', '.join(
                f'<a href="#family-{identifier}">{html.escape(identifier)}</a>'
                for identifier in case["reference_family_ids"]) + '</p>'
            + ('<p><strong>Data-factored state, not a separate captured session.</strong> '
               f'HUD signature: <code>{case["hud_signature_id"]}</code>. '
               f'{len(case["source_text_record_ids"])} pinned source text records.</p>'
               + '<details><summary>Source text examples (not generated screenshots)</summary>'
               + ''.join('<p>' + html.escape(
                   text_records[identifier]["desktop_text"] or "Explicit unrecorded source line; do not invent it."
               ) + '</p>' for identifier in case["source_text_record_ids"][:3])
               + '<a href="../dynamic-text-oracles.json">All exact records, source lines and native-font metrics</a></details>'
               if case["family"] == "tutorial" else '')
            + '<details><summary>Every input and hash</summary><ul>' + ''.join(refs) + '</ul></details></article>'
        )
    family_cards = []
    for family in factoring["families"]:
        previews = [all_media[identifier] for identifier in family["representative_input_ids"]
                    if identifier in all_media][:2]
        family_cards.append(
            f'<article data-reference-family-id="{family["id"]}"><h3 id="family-{family["id"]}">'
            f'{html.escape(family["title"])}</h3><p class="status">{family["agreement_input_status"]}</p>'
            + ''.join(image_markup(entry) for entry in previews)
            + '<p><strong>Required distinct states:</strong> '
            + html.escape(', '.join(family["required_visual_variants"])) + '</p>'
            + f'<p>{len(family["case_ids"])} case bindings; '
              f'{len(family["pixel_or_audio_input_ids"])} actual input references. '
              'Full-panel source checks plus independent dynamic text/value checks; no masked panels.</p></article>'
        )
    native_cards = []
    for entry in manifest["native_hud_inputs"]:
        regions = "; ".join(region["name"] + " " + str(region["bounds"])
                            for region in entry["source"]["native_ui_regions"])
        native_cards.append(
            f'<article data-native-hud-id="{entry["id"]}"><h3>{html.escape(entry["id"])}</h3>'
            + image_markup(entry)
            + '<p><strong>Original full1920x1080 Resizable-Classic controlled fixture.</strong> '
              'Not authenticated tutorial progression. Dialogue body is synthetic fixture text, '
              'not a source transcript.</p>'
            + f'<p>Native active group {entry["source"]["active_interface"]}; '
              f'enabled slots {entry["settings"]["enabled_tab_slots"]}.</p>'
            + f'<p>{html.escape(regions)}</p><code>{entry["sha256"]}</code></article>'
        )
    signature_rows = ''.join(
        f'<tr data-hud-signature-id="{signature["id"]}"><td><code>{signature["id"]}</code></td>'
        f'<td>{html.escape(", ".join(signature["expected_introduced_tabs"]) or "(no introduced lesson tabs)")}</td>'
        f'<td>{len(signature["state_ids"])}</td><td>Unmentioned controls remain unknown, never presumed hidden.</td></tr>'
        for signature in factoring["hud_signatures"])
    later_rows = ''.join(
        f'<li><strong>{entry["id"]}</strong>: {html.escape(entry["scope"])} '
        f'{html.escape(entry["proof_needed"])}</li>' for entry in factoring["acceptance_obligations"])
    inventory = []
    for entry in manifest["public_inputs"]:
        inventory.append(
            f'<article><h3>{html.escape(entry["title"])}</h3>{image_markup(entry)}'
            f'<p><strong>{entry["media_scope"]}</strong> / {entry["source_role"]}</p>'
            f'<p>{html.escape(entry["compatibility_note"])}</p>'
            f'<p>{entry["decoded"]["dimensions"]}; uploaded {entry["upload_timestamp"]}; '
            'capture build/date/camera unknown.</p>'
            f'<p><a href="{html.escape(entry["file_page_url"], quote=True)}">Pinned file revision</a> | '
            f'<a href="{link(entry["source_snapshot"]["path"])}">Original notice + metadata</a></p></article>'
        )
    audio_rows = [
        f'<tr><td>{html.escape(entry["asset_id"])}</td><td>{html.escape(entry.get("name", entry["kind"]))}</td>'
        f'<td><audio controls preload="none" src="{link(entry["path"])}"></audio></td>'
        f'<td><code>{entry["sha256"][:16]}</code></td></tr>' for entry in manifest["audio"]["assets"]
    ]
    gap_html = ''.join(
        f'<li><strong>{entry["id"]}</strong>: {html.escape(entry["exact_deficit"])} '
        f'{html.escape(entry["bounded_resolution"])}</li>' for entry in manifest["source_gaps"])
    page = """<!doctype html><html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1"><link rel="icon" href="data:,">
<title>M1 source reference review - awaiting owner approval</title>
<style>
*{box-sizing:border-box}body{margin:0;background:#f5f3ed;color:#171717;font:16px/1.45 system-ui,sans-serif}
header,main{padding:20px;max-width:1800px;margin:auto}header{border-bottom:5px solid #995d08}
a{color:#18467c}code{overflow-wrap:anywhere}p,li,td,h3{overflow-wrap:anywhere}
.banner{padding:16px;border:2px solid #995d08;background:#fff1cf;font-weight:700}
.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,340px),1fr));gap:16px}
article{padding:12px;border:1px solid #999;background:white;min-width:0}h3{font-size:15px}
img,video{max-width:100%;height:auto;max-height:260px;object-fit:contain;image-rendering:pixelated;background:#252525}
.status{font-weight:700;color:#7b3700}input,select{max-width:100%;padding:8px;font:inherit}
table{width:100%;table-layout:fixed;border-collapse:collapse}td{border-bottom:1px solid #aaa;padding:8px}
audio{max-width:100%;width:260px}[hidden]{display:none!important}summary{cursor:pointer}
</style></head><body><header><h1>M1 source-reference review, v1.2</h1>
<p class="banner">AWAITING OWNER APPROVAL. Readiness follows the named source-family/calibration inputs,
not a separate authenticated screenshot for every micro-transition. This gallery displays original
reference media and labeled composition proposals. It is NOT the ClubScape client, a renderer demonstration,
a passed player journey, or visual/audio acceptance. No owner approval has been granted.</p>
<nav><a href="#native-hud">16 original native HUD/panel frames</a> | <a href="#families">29 visual families</a> | <a href="#signatures">Progressive HUD signatures</a> |
<a href="#cases">All126 cases /71 tutorial states</a> | <a href="#sources">Public originals</a> | <a href="#audio">258 original audio files</a> |
<a href="../manifest.json">Manifest</a> | <a href="../native-metrics.json">Exact component/font metrics</a> |
<a href="../comparison-policy.json">Pre-candidate numeric policy</a> | <a href="../search-log.json">Acquisition/search evidence</a> |
<a href="../evidence-families.json">Literal requirement audit</a> | <a href="../dynamic-text-oracles.json">Pinned text/value oracles</a> |
<a href="../native-hud-integration.json">Native calibration</a> | <a href="../audio-handoff.json">Minimal remaining audio inputs</a> |
<a href="contact-sheets.json">Contact-sheet index</a></nav>
<p>Primary product proposal: 1920x1080 / DPR1 / UI scale1. Proposed range: 1024x768 through 2560x1440.
Gallery resizing is not game resizing/performance evidence. Owner-run M-series Mac Chrome/Edge results are unrun.</p>
<h2>Minimal missing reference inputs</h2><ul>""" + gap_html + """</ul>
<p><strong>The native HUD/panel reference gap is closed.</strong> Source commit db103ba supplies the
original full frame, native161/CS2, minimap/chat/sidebar/panel coordinate readbacks and six actual
attachment families. These six controlled examples do not replace the71-state source text/value map
or claim exact tutorial unlock timing.</p>
<details><summary>Later candidate/source-fidelity/owner acceptance obligations - not invented pack prerequisites</summary>
<ul>""" + later_rows + """</ul></details></header><main>
<h2 id="native-hud">All16 original native full-frame inputs</h2>
<p>Scene and UI pixels are original rendering, not pasted images. Synthetic fixture body strings are kept
only for reproducing each controlled pixel state; the698 pinned transcript records remain the authority
for actual dialogue. The Lumbridge location, player, containers and instructor choices are explicit fixture
inputs, not grants, prices, spawn observations or authenticated Tutorial Island states.</p>
<section class="grid">""" + ''.join(native_cards) + """</section>
<h2 id="families">Required source families and distinct visual states</h2>
<p>Evidence is factored, not gameplay. All71 semantic states and every listed variant remain required.
Reuse a source-backed frame; check changed strings, values, highlights, filters and item fields with the pinned
source transcripts, rules and native glyphs. Every unchanged panel pixel is checked; nothing is masked.</p>
<section class="grid">""" + ''.join(family_cards) + """</section>
<h2 id="signatures">Progressive HUD introduction signatures</h2><p>Named introductions are source/contract-backed.
This is not a claim of observed numeric source progress, nor a licence to hide optional or unimplemented controls.</p>
<table><thead><tr><th>Signature</th><th>Introduced tabs</th><th>States</th><th>Limit</th></tr></thead><tbody>""" + signature_rows + """</tbody></table>
<h2 id="cases">Required case index</h2><label>Filter cases <input id="search" type="search" placeholder="case ID or subject"></label>
<p id="count"></p><section class="grid" id="case-grid">""" + ''.join(cards) + """</section>
<h2 id="sources">Every retrieved public original</h2><p>Native originals are linked; the scaled previews are navigation
only, never comparison baselines. Source cropping, legacy layouts and official-client-only effects are identified.</p>
<section class="grid">""" + ''.join(inventory) + """</section>
<h2 id="audio">All original audio inputs</h2><p>Click to listen. No autoplay. These are the existing lossless source
conversions, not newly converted assets or accepted source mixer/trigger behavior. The verified silent SFX2411 has no
playable file and is separately preserved in the manifest.</p><table><tbody>""" + ''.join(audio_rows) + """</tbody></table>
<p>Original game media: Copyright Jagex Ltd. Wiki contributor histories and file notices are preserved per input.
Source access does not imply endorsement, an open-source asset license or product acceptance.</p></main>
<script>
const search=document.querySelector('#search'),cards=[...document.querySelectorAll('[data-case-id]')];
function filter(){const term=search.value.toLowerCase();for(const card of cards)card.hidden=!card.dataset.search.toLowerCase().includes(term);
document.querySelector('#count').textContent=cards.filter(card=>!card.hidden).length+' / '+cards.length+' cases';}
search.addEventListener('input',filter);filter();
</script></body></html>"""
    (directory / "index.html").write_text(page)
    sheets = []
    entries = manifest["original_inputs"] + manifest["public_inputs"] + manifest["proposal_inputs"] + manifest["native_hud_inputs"]
    entries = [entry for entry in entries if entry.get("decoded", {}).get("format") != "MP4"]
    for offset in range(0, len(entries), 20):
        chunk = entries[offset:offset + 20]
        sheet = Image.new("RGB", (1600, 1200), (26, 26, 26))
        draw = ImageDraw.Draw(sheet)
        for index, entry in enumerate(chunk):
            with Image.open(ROOT / entry["path"]) as source:
                source.seek(0)
                preview = source.convert("RGBA")
                preview.thumbnail((385, 194))
                x, y = (index % 4) * 400, (index // 4) * 240
                sheet.paste(preview, (x + 5, y + 36), preview)
                label = entry["id"]
                draw.text((x + 5, y + 3), label[:57], fill="white")
                draw.text((x + 5, y + 18), "PREVIEW ONLY - click original in HTML gallery", fill=(255, 210, 120))
        path = directory / f"contact-{offset // 20 + 1:02}.png"
        sheet.save(path)
        sheets.append({**digest(path), "input_ids": [entry["id"] for entry in chunk],
                       "source_role": "review_contact_sheet_not_a_baseline",
                       "transformation": "Full original frame 0, aspect-preserving thumbnail; no source crop."})
    write_json(directory / "contact-sheets.json", sheets)


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    originals = original_inputs()
    native_hud_inputs = load_native_hud_inputs()
    native_calibration = native_hud_calibration(native_hud_inputs)
    write_json(OUT / "native-hud-integration.json", native_calibration)
    raw_public = json.loads((OUT / "public-media.json").read_text())
    for entry in raw_public:
        if "bytes_verified_at" not in entry:
            entry["bytes_verified_at"] = entry["bytes_retrieved_at"]
            if not entry["delivery_attempts"]:
                entry["bytes_retrieved_at"] = None
                entry["retrieval_url"] = None
                entry["delivery_context"] = (
                    "Exact original reused after an earlier partial acquisition. The precise successful "
                    "network-transfer URL/time were not retained; they are not guessed. Original imageinfo "
                    "URL, upload identity, actual bytes and later verification time are retained."
                )
            else:
                entry["delivery_context"] = "Exact original acquired through the recorded public delivery URL."
    write_json(OUT / "public-media.json", raw_public)
    policy = dispositions()
    if set(policy) != {entry["id"] for entry in raw_public}:
        raise ValueError(f"Unclassified/absent public originals: {set(policy) ^ {entry['id'] for entry in raw_public}}")
    measured = measure(raw_public)
    write_json(OUT / "native-metrics.json", measured)
    public = []
    for entry in raw_public:
        matches = measured["public_component_matches"].get(entry["id"], [])
        text_matches = measured["public_text_matches"].get(entry["id"], [])
        public.append({
            **entry, **policy[entry["id"]],
            "source_role": "reconciled_public_source_capture" if matches or text_matches else "public_source_capture_unreconciled",
            "reconciliation_scope": "Only exact matched source components; never the full image/build/state.",
            "native_component_match_count": len(matches),
            "native_text_match_count": len(text_matches),
            "capture_settings": {
                "build": None, "capture_date": None, "camera_position": None,
                "camera_pitch_yaw_zoom": None, "draw_distance": None, "lighting": None,
                "texture_sampling": None, "font_rendering_settings": None, "display_dpr": None,
                "ui_scale": None, "plugins": None,
                "unknown_policy": "File dimensions/upload timestamp are known; capture settings cannot be inferred from them.",
            },
        })
    proposal_inputs = proposals()
    write_json(OUT / "proposals.json", proposal_inputs)
    audio = json.loads(AUDIO_PATH.read_text())
    cases = make_cases(originals, public, proposal_inputs, audio)
    browser = json.loads((OUT / "browser-media.json").read_text())
    frames = []
    for recording in browser["recordings"]:
        for frame in recording["frames"]:
            frames.append({**frame, "id": f"recording.music-transition.{frame['requested_time_seconds']}s",
                           "source_role": "decoded_public_recording_frame",
                           "full_resizable_classic_frame": False})
    inventory = published_inventory()
    docs = document_bindings()
    source_symbols = symbols()
    pages = json.loads((OUT / "pages.json").read_text())
    for page in pages:
        page["images"] = sorted({title.removeprefix("File:") for title in page["images"]
                                 if "[" not in title and "]" not in title})
    write_json(OUT / "pages.json", pages)
    factoring, text_oracles = make_factoring(cases, originals, public, pages, native_hud_inputs)
    wire_native_hud(cases, factoring, native_hud_inputs)
    factoring["native_hud_calibration"] = digest(OUT / "native-hud-integration.json")
    for case in cases:
        if case["id"] == "case.hud.resizable_classic":
            case["evidence_scope"] = "Complete original native161 frame with recorded scene/minimap/chat/sidebar "
            case["evidence_scope"] += "and source panel geometry. Controlled offline inputs, not a live account."
    write_json(OUT / "evidence-families.json", factoring)
    write_json(OUT / "dynamic-text-oracles.json", text_oracles)
    source_gaps = gaps(factoring)
    write_json(OUT / "audio-handoff.json", audio_handoff(factoring))
    ready = review_ready(factoring["source_review_requirements"])
    for case in cases:
        if case["family"] not in ("audio", "model", "scene"):
            case["dynamic_comparison_profile"] = "dynamic_state_text"
    policy_record = {
        "schema_version": 1, "established_before_candidate_evaluation": True,
        "candidate_evaluations_performed": 0, "owner_approved": False,
        "profiles": NUMERIC_PROFILES,
        "selection_vs_acceptance": "Source-to-source component checks do not evaluate any ClubScape candidate. "
                                   "Source reference approval, candidate fidelity, behavior, performance and "
                                   "RuneLite compatibility are separate gates.",
        "mask_policy": "No whole-panel/creature/scenery masks; no post-hoc tolerances. Store full images and "
                       "explicit source-derived edge bands. Data-only states use source-backed glyph/value "
                       "checks with full panel pixel accounting, never a masked success.",
        "factorization": factoring["comparison_factorization"],
    }
    write_json(OUT / "comparison-policy.json", policy_record)
    write_json(OUT / "search-log.json", search_log())
    manifest = {
        "schema_version": 1, "pack_id": "m1-public-reference-pack-v1", "pack_version": "1.2.0",
        "assembled_on": "2026-09-14",
        "status": "awaiting_owner_approval",
        "ready_for_owner_review": ready,
        "ready_for_owner_approval": ready,
        "readiness_reason": "Readiness is computed from the literal source-family/calibration requirements, not "
                            "71 separately matched source screenshots, branch container dumps or unrun candidate/Mac tests.",
        "evidence_factorization": factoring,
        "factorization_document": digest(OUT / "evidence-families.json"),
        "dynamic_text_oracles": digest(OUT / "dynamic-text-oracles.json"),
        "audio_binding_audit_sources": digest(OUT / "audio-binding-audit-sources.json"),
        "native_hud_calibration": digest(OUT / "native-hud-integration.json"),
        "native_hud_inputs": native_hud_inputs,
        "remaining_audio_handoff": digest(OUT / "audio-handoff.json"),
        "source_selection": json.loads((ROOT / "research/current-source/selection.json").read_text())["selection_id"],
        "source_build": 240, "source_cache": 2695,
        "source_runtime": "SHA-pinned original injected-client-1.12.38",
        "owner_reference_direction": {
            **json.loads((ROOT / "milestones/m1-owner-followups.json").read_text())["reference_direction"],
            "terms_acceptance_performed_by_this_task": False,
            "account_login_performed_by_this_task": False,
            "legacy_handoff_scope": "The hash-bound source-capture handoff/selection remaining-gate text is "
                                    "historical preparation context. The later owner public-reference direction "
                                    "takes precedence; those old account/terms requests are not this pack's gates.",
        },
        "primary_target": {
            "logical_viewport": [1920, 1080], "dpr": 1, "ui_scale": 1, "browser_zoom_percent": 100,
            "layout": "Resizable - Classic", "stock_visuals": True,
            "appearance_changing_plugins": False, "hd_plugins": False,
            "source_fixture_pixels": [1920, 1080],
            "fixture_dpr_note": "Original fixtures are native JVM pixel buffers, not browser/DPR observations.",
        },
        "resize_proposal": {
            "status": "owner_review_proposal", "min_logical_viewport": [1024, 768],
            "max_logical_viewport": [2560, 1440], "dpr": 1, "ui_scale": 1,
            "required_checkpoints": [[1024, 768], [1280, 800], [1920, 1080], [2560, 1440]],
            "range_rule": "Both dimensions within these inclusive bounds; preserve native UI pixels/anchors, "
                          "do not scale the complete canvas or hide unfinished controls.",
            "game_resize_verified": False, "static_source_anchor_arithmetic": measured["static_resize_analysis"],
        },
        "comparison_settings": {
            "world_tile_source_units": 128,
            "original_fixture_settings": "Per-image original_inputs[].settings and source; copied verbatim.",
            "scene_angle_units_per_turn": 16384, "model_angle_units_per_turn": 2048,
            "fixture_scene_draw_distance": 25, "fixture_brightness": 0.8,
            "fixture_texture_resolution": 128, "fixture_far_clip_units": 32768,
            "scope": "These are actual controlled fixture choices, not observed live-session defaults or "
                     "guesses about public images. Unknown original metadata is not filled.",
            "ui": "Exact original palettes/glyph masks/advances; native pixel scale1, no smoothing.",
            "animation": "All captured frame indices and native client-cycle lengths retained; no inferred "
                         "wall-clock cadence from GIFs. Proposed 20ms cycle calibration remains separate.",
        },
        "browsers_and_hardware": {
            "source_media_decode": {"browser": browser["browser"], "os": "Linux ARM64 on Sparky",
                                    "headless": True, "sandbox_enabled": True},
            "development_evidence_only": True,
            "owner_target": "Owner-run M-series Mac; exact model, macOS, Chrome and Edge versions/results pending.",
            "mac_chrome_tested": False, "mac_edge_tested": False, "game_performance_tested": False,
        },
        "bound_existing_documents": docs,
        "owned_document_inputs": [digest(ROOT / path) for path in (
            "research/reference-pack/README.md", "tools/reference-pack/README.md", "assets/reference/wiki/NOTICE.txt")],
        "source_widget_symbols": source_symbols,
        "source_snapshot_inventory": [
            {**digest(path), "role": "retained_exact_source_or_discovery_snapshot_not_a_game_capture"}
            for path in sorted((OUT / "sources").glob("*")) if path.is_file()
        ],
        "source_asset_inventory": inventory,
        "original_inputs": originals, "public_inputs": public,
        "public_page_revisions": pages, "proposal_inputs": proposal_inputs,
        "recording_frames": frames,
        "audio": {
            "manifest": digest(AUDIO_PATH), "assets": audio["assets"],
            "source_silences": audio["source_silences"], "settings": audio["settings"],
            "source_map": digest(ROOT / "research/audio-source/source-map.json"),
            "actions": json.loads((ROOT / "research/audio-source/source-map.json").read_text())["actions"],
            "all_source_map_fields_retained_in_bound_input": True,
            "remaining_bindings": audio["remaining_bindings"],
            "public_controls": [wiki("Audio options interface.png"), wiki("Music tab.png"), wiki(music_update(1, "gif"))],
            "public_transition_recording": wiki(music_update(2, "mp4")),
            "public_policy": {
                "references": ["Music Player", "Audio", "Update:Music System Improvements & Deadman Tweaks"],
                "area_mode": "Region selection; current public documentation distinguishes Classic region music and Modern area playlists.",
                "single_mode": "Chosen unlocked track repeats indefinitely; do not loop the exported one-second release tail.",
                "shuffle_mode": "Random unlocked track; Skip selects the next, three playlists up to100 unlocked tracks.",
                "volume": "Public Audio page documents20 notches at5% for SFX/area; February2026 sliders drag smoothly. "
                          "Actual mixer gain curves/defaults are not inferred from thumb positions.",
                "colors": {"locked": "#ff0000", "unlocked": "#0dc10d", "unavailable": "#9f9f9f", "playing": "#3ce6e6"},
                "synthesis_version_caution": "Current public Audio page records the August2026 MIDI correction and "
                                             "September2025 16-bit SFX change. Do not use older 8-bit or generic-soundfont variants.",
            },
            "conversion_repeated": False, "live_source_mixer_calibrated": False,
            "browser_source_recording_decode": digest(OUT / "browser-media.json"),
            "runtime_product_playback_accepted": False,
        },
        "comparison_policy": digest(OUT / "comparison-policy.json"),
        "component_reconciliation": digest(OUT / "native-metrics.json"),
        "cases": cases, "source_gaps": source_gaps,
        "product_decisions": [
            "Bounded owner reference-pack approval remains ungranted; source gaps are not a waiver request.",
            "Review the seven explicit web-only/title composition proposals and logo substitution direction.",
            "Review reuse of original Penguin NPC2063 at source75/128 scale and subsequent modular equipment-fitting gates.",
            "Review proposed1920x1080/DPR1/scale1 primary, resize range and pre-candidate numeric tolerances.",
            "Final real Chrome/Edge M-series Mac visual/audio/performance and full fresh-account journey acceptance are unrun.",
        ],
        "approval": {"owner_reference_pack_approved": False, "candidate_fidelity_accepted": False,
                     "gameplay_accepted": False, "performance_accepted": False, "runelite_compatibility_accepted": False},
        "counts": {
            "required_cases": len(cases), "tutorial_cases": sum(case["family"] == "tutorial" for case in cases),
            "original_runtime_images": len(originals) + len(native_hud_inputs),
            "original_pre_hud_images": len(originals),
            "native_hud_original_images": len(native_hud_inputs),
            "native_attachment_families": len(native_calibration["attachment_families"]),
            "retrieved_public_images": sum(entry["decoded"]["format"] != "MP4" for entry in public),
            "retrieved_public_recordings": sum(entry["decoded"]["format"] == "MP4" for entry in public),
            "derived_recording_frames": len(frames), "owner_review_proposals": len(proposal_inputs),
            "playable_original_audio_files": len(audio["assets"]),
            "verified_source_silences_without_file": len(audio["source_silences"]),
            "published_original_asset_files": len(inventory),
            "source_gap_cases": sum(bool(case["source_gap_refs"]) for case in cases),
            "source_gap_categories": len(source_gaps),
            "visual_families": factoring["counts"]["families"],
            "distinct_visual_variants": factoring["counts"]["distinct_visual_variants"],
            "instructor_phases": factoring["counts"]["phases"],
            "progressive_hud_signatures": factoring["counts"]["hud_signatures"],
            "source_text_records": len(text_oracles["records"]),
            "separate_tutorial_microstate_source_captures_required": 0,
        },
        "notices": ["assets/source/osrs/NOTICE.txt", "assets/source/osrs/audio-runtime/NOTICE.txt",
                    "assets/reference/wiki/NOTICE.txt", "research/reference-pack/README.md"],
        "gallery": "research/reference-pack/v1/gallery/index.html",
    }
    manifest["input_usage"] = [
        {"input_id": entry.get("id", entry.get("asset_id")),
         "path": entry["path"],
         "case_ids": [case["id"] for case in cases
                      if entry.get("id", entry.get("asset_id")) in case["input_ids"]],
         "unassigned_role": "explicit_supplementary_source_inventory_not_used_to_claim_case_completion"}
        for entry in originals + native_hud_inputs + public + proposal_inputs + frames + audio["assets"]
    ]
    required = {"schema_version": 1, "source_contract": digest(TUTORIAL_PATH),
                "required_ids": [case["id"] for case in cases],
                "basis": "Every existing71 tutorial state plus independently authored Section30 case families."}
    write_json(OUT / "required-cases.json", required)
    manifest["required_case_catalogue"] = digest(OUT / "required-cases.json")
    manifest["tool_inputs"] = [digest(path) for path in sorted((ROOT / "tools/reference-pack").glob("*"))
                               if path.is_file() and path.suffix in (".py", ".mjs")]
    write_json(OUT / "manifest.json", manifest)
    gallery(manifest)
    write_json(OUT / "manifest-lock.json", {
        "schema_version": 1, "manifest": digest(OUT / "manifest.json"),
        "meaning": "Immutable review identity, NOT an approval record; changing inputs requires regenerating/reviewing the pack.",
    })
    print(json.dumps({"status": manifest["status"], "ready_for_owner_review": ready, **manifest["counts"]}))


if __name__ == "__main__":
    main()
