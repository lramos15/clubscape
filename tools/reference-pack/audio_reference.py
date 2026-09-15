"""Current audio payloads, dated source observations and the two exact owner-approved adaptations."""

import argparse
from array import array
from datetime import datetime, timezone
import gzip
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import wave
from functools import lru_cache

from components import ROOT, OUT, digest
from catalogue import NUMERIC_PROFILES


APPROVAL_PATH = "milestones/m1-audio-trigger-approval.json"
APPROVAL_SHA256 = "d6f187cd182741421cbc14420dbb8eb60411cd478f3c15be3cd740c081731c30"
APPROVAL_COMMIT = "5df8420"
BASE_COMMIT = "e74026a3e9764ea9dd81a19ddd6f87abc32a70bb"
TEMPLATE_GROUPS = (2693, 710)
TEMPLATE_INDEX = OUT / "audio-templates.json"
ADAPTATIONS = (
    "adaptation.m1.learning_the_ropes_completion_audio",
    "adaptation.m1.ordinary_food_audio",
)


def read_json(path):
    return json.loads((ROOT / path).read_text())


def compressed(path):
    return json.loads(gzip.decompress((ROOT / path).read_bytes()))


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, ensure_ascii=True) + "\n")


def wave_facts(path):
    with wave.open(str(path), "rb") as source:
        if (source.getnchannels(), source.getsampwidth(), source.getframerate()) != (1, 2, 22050):
            raise ValueError("Expected original mono22050-Hz signed16 cue template")
        frames = source.getnframes()
        data = source.readframes(frames)
        if len(data) != frames * 2 or frames <= 0:
            raise ValueError("Truncated/empty source cue template")
    samples = array("h", data)
    return {
        "sample_rate": 22050, "channels": 1, "frames": frames,
        "duration_seconds": frames / 22050,
        "pcm_s16le_sha256": hashlib.sha256(data).hexdigest(),
        "peak": max(abs(sample) for sample in samples) / 32768,
        "export_gain_numerator": 1, "export_gain_denominator": 1,
        "scope": "Exact original native comparison PCM, not a newly converted runtime FLAC or calibrated live mixer level.",
    }


def template_expectations():
    evidence = compressed("research/audio-source/binding-public-evidence.json.gz")
    matches = evidence["comparisons"]["tutorial-candidate-sfx-matches.json"]["matches"]
    return {row["sound_id"]: row["template_wav_sha256"] for row in matches if row["sound_id"] in TEMPLATE_GROUPS}


def materialize_templates(source):
    expected = template_expectations()
    records = []
    for group in TEMPLATE_GROUPS:
        wave_path = source / f"sfx-{group}.wav"
        proof_path = source / f"sfx-{group}.json"
        data = wave_path.read_bytes()
        proof = json.loads(proof_path.read_text())
        facts = wave_facts(wave_path)
        if hashlib.sha256(data).hexdigest() != expected[group] or facts["pcm_s16le_sha256"] != proof["pcm_s16le_sha256"]:
            raise ValueError(f"Source template does not match retained native/public evidence: {group}")
        if proof["group"] != group or proof["index"] != 4 or proof["frames"] != facts["frames"]:
            raise ValueError("Wrong native cue provenance")
        target = ROOT / f"assets/reference/wiki/audio-source/sfx-{group}.wav"
        provenance = OUT / f"sources/native-cue-{group}.json"
        target.parent.mkdir(parents=True, exist_ok=True)
        provenance.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(wave_path, target)
        shutil.copyfile(proof_path, provenance)
        records.append({
            "id": f"reference.audio.sfx.{group}", **digest(target),
            "source_role": "current_original_audio_template",
            "source_index": 4, "source_group": group, "source_file": 0, "source_cache_id": 2695,
            "kind": "sfx_reference_template", "name": f"Original source cue {group}",
            "source_provenance": digest(provenance),
            "source_comparison_evidence": digest(ROOT / "research/audio-source/binding-public-evidence.json.gz"),
            "signal": facts, "copied_without_conversion": True,
            "retained_source_path": str(wave_path),
            "copy_verified_at": datetime.now(timezone.utc).isoformat(),
            "notice": "assets/reference/wiki/NOTICE.txt",
        })
    write_json(TEMPLATE_INDEX, records)
    return records


def template_inputs():
    records = json.loads(TEMPLATE_INDEX.read_text())
    expected = template_expectations()
    if len(records) != 2 or {row["source_group"] for row in records} != set(TEMPLATE_GROUPS):
        raise ValueError("Missing/duplicate required original cue templates")
    for row in records:
        actual = digest(ROOT / row["path"])
        if actual["sha256"] != row["sha256"] or row["sha256"] != expected[row["source_group"]]:
            raise ValueError("Changed original cue template")
        if row["source_role"] != "current_original_audio_template" or not row["copied_without_conversion"]:
            raise ValueError("Cue template misclassified as a new conversion or source observation")
    return records


def approval_record():
    actual = digest(ROOT / APPROVAL_PATH)
    if actual["sha256"] != APPROVAL_SHA256:
        raise ValueError("Owner audio approval record changed; do not infer renewed or broader approval")
    record = read_json(APPROVAL_PATH)
    entries = {entry["id"]: entry for entry in record["adaptations"]}
    if record["authority"] != "owner" or record["decision"] != "approved" or set(entries) != set(ADAPTATIONS):
        raise ValueError("Missing or broadened owner audio adaptation approval")
    for entry in entries.values():
        if entry["classification"] != "approved_adaptation":
            raise ValueError("Approved audio choice relabeled as verified OSRS behavior")
    if entries[ADAPTATIONS[0]]["verified_current_source_selector"] is not False:
        raise ValueError("Tutorial adaptation falsely claimed to be an observed source selector")
    if entries[ADAPTATIONS[1]]["verified_current_ordinary_food_selector"] is not False:
        raise ValueError("Ordinary-food adaptation falsely claimed to be an observed source selector")
    return {
        "input": actual, "source_commit": APPROVAL_COMMIT,
        "adaptations_spec": digest(ROOT / "spec/adaptations.md"),
        "record": record, "scope": "These two selector choices only; not reference-pack or product acceptance.",
    }


def current_audio_proof(audio):
    correction_path = "research/audio-source/musical-startup-correction.json"
    correction = read_json(correction_path)
    manifest_record = digest(ROOT / "assets/manifests/osrs/audio-runtime.json")
    if manifest_record["sha256"] != correction["updated_manifest_sha256"]:
        raise ValueError("Current musical correction and audio manifest disagree")
    if audio["counts"] != {"music": 5, "jingle": 30, "sfx": 229} or len(audio["assets"]) != 264:
        raise ValueError("Incomplete corrected published audio inventory")
    if (audio["settings"]["native_startup_percussion_channel"],
            audio["settings"]["native_startup_percussion_bank"]) != (9, 128):
        raise ValueError("Source startup percussion bank correction was lost")
    previous = previous_audio_inventory()
    old = {entry["asset_id"]: entry for entry in previous["assets"]}
    current = {entry["asset_id"]: entry for entry in audio["assets"]}
    changed = sorted(key for key in old if old[key]["kind"] != "sfx" and old[key]["sha256"] != current[key]["sha256"])
    effects = [key for key in old if old[key]["kind"] == "sfx"]
    if changed != sorted(correction["changed_musical_asset_ids"]) or len(changed) != 26:
        raise ValueError("Unexpected musical payload delta")
    if len(effects) != 223 or any(old[key]["sha256"] != current[key]["sha256"] for key in effects):
        raise ValueError("A prior original SFX payload changed")
    if previous["source_silences"] != audio["source_silences"]:
        raise ValueError("Original source silence/probability input changed")
    browser = read_json("research/audio-source/browser-decode.json")
    if browser["manifest_sha256"] != manifest_record["sha256"] or browser["count"] != 264:
        raise ValueError("Browser PCM evidence uses obsolete pre-correction audio")
    return {
        "current_manifest": manifest_record,
        "correction_input": digest(ROOT / correction_path),
        "browser_pcm_evidence": digest(ROOT / "research/audio-source/browser-decode.json"),
        "previous_comparison_commit": BASE_COMMIT,
        "changed_musical_asset_ids": changed, "musical_assets_checked": 35,
        "prior_sfx_hashes_preserved": 223, "source_silence_preserved": True,
        "active_payload_records": "Only the current264 published assets; previous musical hashes are not active baselines.",
        "native_startup": {"channel": 9, "bank": 128, "call": correction["native_call"]},
    }


@lru_cache(maxsize=1)
def previous_audio_inventory():
    return json.loads(subprocess.check_output(
        ["git", "show", f"{BASE_COMMIT}:research/reference-pack/v1/manifest.json"], cwd=ROOT))["audio"]


def source_contract():
    audio = read_json("assets/manifests/osrs/audio-runtime.json")
    approval = approval_record()
    bindings = read_json("research/audio-source/bindings.json")
    observations = read_json("research/audio-source/selector-observations.json")
    native = compressed("research/audio-source/binding-native-evidence.json.gz")
    templates = template_inputs()
    proof = current_audio_proof(audio)
    observed = {row["rule"]: row for row in observations["observed_selectors"]}
    cues = {row["action"]: row for row in bindings["cue_bindings"]}
    assets = {f"{row['kind']}.{row['source_group']}": row["asset_id"] for row in audio["assets"]}
    assets.update({f"sfx.{row['source_group']}": row["id"] for row in templates})

    def payloads(kind, ids):
        return [assets[f"{kind}.{identifier}"] for identifier in ids]

    shortbow = observed["rule.combat.ranged"]
    rat = observed["rule.combat.tutorial_rat"]
    smelt = observed["rule.smelting.bronze"]
    goblin = cues["ordinary_unarmed_goblin"]
    food = cues["eating"]
    cook = observations["cooks_assistant_observation"]
    if shortbow["sound_id"] != 2693 or rat["sound_ids"] != {"attack": 710, "hit": 713, "death": 711}:
        raise ValueError("Unexpected observed source selector")
    if smelt["sound_id"] != 2725 or goblin["source_sound_ids"] != [469, 472, 471] or cook["quest_jingle"] != 152:
        raise ValueError("Current source cue identity changed")
    if (food["sequence_id"], food["frame"], food["frame_start_cycle_sum"], food["source_sound_ids"]) != (12526, 1, 4, [2393]):
        raise ValueError("Eating source animation cue changed")
    selectors = [
        {
            "id": "selector.shortbow", "rule_ids": ["rule.combat.ranged"],
            "classification": "dated_public_source_observation", "payload_ids": payloads("sfx", [2693]),
            "sound_ids": {"release": 2693}, "source_sequence": 426,
            "event": shortbow["event"], "timing": shortbow["source_relative_timing"],
            "recording_dates": [shortbow["recording_date"]], "recording_urls": [f"https://www.youtube.com/watch?v={shortbow['video']}"],
            "verified_current_build_selector": False,
        },
        {
            "id": "selector.tutorial_rat", "rule_ids": ["rule.combat.tutorial_rat"],
            "classification": "dated_public_source_observation", "payload_ids": payloads("sfx", [710, 713, 711]),
            "sound_ids": rat["sound_ids"], "source_sequences": rat["source_sequences"],
            "event": rat["event"], "timing": "Separate received/game-triggered attack, hit and death events; preserve native queue delay.",
            "recording_dates": [rat["recording_date"]], "recording_urls": [f"https://www.youtube.com/watch?v={rat['video']}"],
            "verified_current_build_selector": False,
        },
        {
            "id": "selector.goblin", "rule_ids": ["rule.goblin.level_2"],
            "classification": "dated_public_source_observation", "payload_ids": payloads("sfx", [469, 472, 471]),
            "sound_ids": dict(zip(goblin["event_roles"], goblin["source_sound_ids"], strict=True)),
            "source_sequences": goblin["source_sequences"],
            "event": "Ordinary unarmed goblin attack, damage reaction and death.",
            "timing": "Game-triggered source event boundaries; no fabricated618x frame sound or armed-variant substitution.",
            "recording_dates": ["20180510", "20190720"],
            "recording_urls": ["https://www.youtube.com/watch?v=kdk7teYIOQk", "https://www.youtube.com/watch?v=5PazG956MZo"],
            "verified_current_build_selector": False,
        },
        {
            "id": "selector.bronze_smelting", "rule_ids": ["rule.smelting.bronze"],
            "classification": "dated_public_source_observation", "payload_ids": payloads("sfx", [2725]),
            "sound_ids": {"start": 2725}, "source_sequence": 899,
            "event": smelt["event"], "timing": smelt["source_relative_timing"],
            "recording_dates": [smelt["recording_date"]], "recording_urls": [f"https://www.youtube.com/watch?v={smelt['video']}"],
            "verified_current_build_selector": False,
        },
        {
            "id": "selector.ordinary_food", "rule_ids": ["rule.food.healing"],
            "classification": "approved_adaptation", "payload_ids": payloads("sfx", [2393]),
            "adaptation_id": ADAPTATIONS[1], "owner_record": approval["input"],
            "sound_ids": {"eat": 2393}, "source_item_ids": [315, 2309],
            "source_sequence": 12526, "frame": 1, "source_client_cycle": 4, "repeat_count": 1,
            "event": "Legitimate ordinary shrimp/bread consumption; source12526 first sound cue.",
            "timing": "One dispatch atframe1 after four source client cycles; animation and game callback may not both emit it.",
            "baked_offset_added_again": False, "duplicate_dispatches_allowed": 0,
            "current_native_sequence_cue_verified": True, "verified_current_build_selector": False,
        },
        {
            "id": "selector.learning_the_ropes", "quest_refs": ["quest.learning_the_ropes"],
            "classification": "approved_adaptation", "payload_ids": payloads("jingle", [152]),
            "adaptation_id": ADAPTATIONS[0], "owner_record": approval["input"],
            "jingle_id": 152, "event": "Legitimate committed Learning the Ropes completion.",
            "timing": "Submit once on committed completion; do not replay on duplicate completion, reconnect or state reload.",
            "duplicate_dispatches_allowed": 0, "native_last_accepted_request_wins_preserved": True,
            "verified_current_build_selector": False,
        },
        {
            "id": "selector.cooks_assistant", "quest_refs": ["quest.cooks_assistant"],
            "classification": "dated_public_source_observation", "payload_ids": payloads("jingle", [152, 33, 34]),
            "jingle_id": 152, "event": "Normal-account quest completion/reward scroll.",
            "timing": cook["ordering_rule_observed"],
            "reward_levelup_after_scroll_dismissal": cook["levelup_after_scroll_dismissal"],
            "later_levelup_not_simultaneous": cook["later_actual_cooking_levelup"],
            "recording_dates": [cook["recording_date"]],
            "recording_urls": [f"https://www.youtube.com/watch?v={cook['normal_account_video']}"],
            "verified_current_build_selector": False,
            "not_a_class_priority": True,
        },
    ]
    evidence = [
        "research/audio-source/bindings.json", "research/audio-source/selector-observations.json",
        "research/audio-source/binding-native-evidence.json.gz", "research/audio-source/binding-public-evidence.json.gz",
        "research/audio-source/selector-observation-evidence.json.gz",
        "research/audio-source/musical-startup-correction.json", "research/audio-source/selector-observation-validation.json",
        "research/audio-source/binding-test-results.json", "research/audio-source/validation.json",
    ]
    return {
        "schema_version": 1, "source_selection": audio["source_selection"],
        "current_audio_proof": proof, "owner_audio_adaptations": approval,
        "selectors": selectors, "reference_templates": templates,
        "native_queue_rules": bindings["native_queue_rules"],
        "native_queue_cases": native["native_queue_tests"]["cases"],
        "evidence_inputs": [digest(ROOT / path) for path in evidence],
        "historical_qualification": observations["current_scope_limit"],
        "supersession": "Current selector observations supersede older candidate notes; the exact owner record supplies "
                       "only the two remaining choices. Upstream pre-approval residual/status prose is historical context, "
                       "not a reason to discard observed IDs or the approved adaptations.",
        "missing_reference_inputs": [],
        "candidate_validation_obligations": [
            "Verify legitimate triggers, once-only dispatch and persistence against authoritative state.",
            "Preserve native FIFO/last-accepted-request-wins and waveform offsets; measure browser/device timing separately.",
            "Use source scene/frame triggers and current PCM; retain dated-source qualifications through the owner playtest.",
            "Test actual audible playback, region/loop/fade behavior, gestures and reconnects in the implemented client.",
        ],
        "reference_pack_approved": False, "candidate_or_mac_accepted": False,
    }


def resolved_action_map(contract):
    actions = read_json("research/audio-source/source-map.json")["actions"]
    by_rule = {rule: selector for selector in contract["selectors"] for rule in selector.get("rule_ids", [])}
    result = []
    for action in actions:
        selector = by_rule.get(action["journey_rule_id"])
        if selector is None:
            result.append(action)
            continue
        resolved = {
            **action, "identified_sound_ids": list(selector["sound_ids"].values()),
            "source_sequence_ids": ([selector["source_sequence"]] if "source_sequence" in selector else
                                    list(selector["source_sequences"].values()) if isinstance(selector.get("source_sequences"), dict)
                                    else selector.get("source_sequences", [])),
            "selector_ref": selector["id"], "selector_classification": selector["classification"],
            "note": selector["event"] + " " + selector["timing"],
            "source_session_trigger_verified": False,
        }
        if selector["id"] == "selector.ordinary_food":
            resolved["source_frame_events"] = [{
                "id": 2393, "sequence_id": 12526, "frame": 1, "source_frame_start_cycle_sum": 4,
                "loops": 1, "range": 5, "retain": 0, "weight": 100,
                "selector_classification": "approved_adaptation",
                "no_duplicate_game_callback": True, "do_not_add_baked_offset_again": True,
            }]
        result.append(resolved)
    return result


def case_selector_ids(case, selectors):
    extra = {
        "case.audio.jingles": ["selector.learning_the_ropes", "selector.cooks_assistant"],
        "case.ui.quest_rewards": ["selector.learning_the_ropes", "selector.cooks_assistant"],
        "case.tutorial.wind_strike": ["selector.learning_the_ropes"],
        "case.tutorial.departure_offer": ["selector.learning_the_ropes"],
        "case.tutorial.ranged_rat": ["selector.shortbow", "selector.tutorial_rat"],
        "case.tutorial.melee_rat": ["selector.tutorial_rat"],
        "case.tutorial.smelt_bronze": ["selector.bronze_smelting"],
    }
    by_rule = {rule: row["id"] for row in selectors.values() for rule in row.get("rule_ids", [])}
    references = list(extra.get(case["id"], []))
    references.extend(by_rule[rule] for rule in case.get("action_rule_ids", []) if rule in by_rule)
    return sorted(set(references))


def wire_cases(cases, contract, audio):
    selectors = {row["id"]: row for row in contract["selectors"]}
    templates = {row["id"]: row for row in contract["reference_templates"]}
    published = {row["asset_id"]: row for row in audio["assets"]}
    for case in cases:
        references = case_selector_ids(case, selectors)
        case["audio_selector_refs"] = references
        case["approved_audio_adaptation_refs"] = [
            selectors[identifier]["adaptation_id"] for identifier in references
            if selectors[identifier]["classification"] == "approved_adaptation"
        ]
        case.pop("candidate_only_sound_ids", None)
        existing = set(case["input_ids"])
        for identifier in references:
            for input_id in selectors[identifier]["payload_ids"]:
                if input_id in existing:
                    continue
                if input_id not in published and input_id not in templates:
                    raise ValueError("Selector has no actual original audio input")
                case["input_ids"].append(input_id)
                case["input_roles"].append({
                    "input_id": input_id,
                    "source_role": "current_original_audio_template" if input_id in templates else "current_original_audio_input",
                    "comparison_profile": "audio_source", "numeric_tolerances": NUMERIC_PROFILES["audio_source"],
                    "normative_scope": "Current original PCM identity; selector classification/date/owner policy "
                                       "is recorded separately and does not grant product acceptance.",
                })
                existing.add(input_id)
        if references:
            case["audio_selector_scope"] = "Qualified source observations and two exact approved adaptations; "
            case["audio_selector_scope"] += "historical/approved selectors are not labeled verified current OSRS selectors."


def validate_contract(contract, *, decode_templates=False):
    expected = source_contract()
    if contract != expected:
        raise ValueError("Audio source identity, dated qualification, queue rule or exact approved adaptation changed")
    by_id = {row["id"]: row for row in contract["selectors"]}
    if len(by_id) != 7:
        raise ValueError("Missing/duplicate audio selector")
    adapted = [row for row in by_id.values() if row["classification"] == "approved_adaptation"]
    if {row["adaptation_id"] for row in adapted} != set(ADAPTATIONS):
        raise ValueError("The audio approval may authorize only the exact two adaptations")
    if any(row["verified_current_build_selector"] for row in by_id.values()):
        raise ValueError("Historical or approved selectors cannot be certified as current-build observations")
    cases = contract["native_queue_cases"]
    startup = next(row for row in cases if row["case"] == "native-startup-percussion-bank")
    if startup["source_startup_default_bank_channel9"] != 128:
        raise ValueError("Incorrect native percussion setup")
    delay = next(row for row in cases if row["case"] == "native-client-sfx-tick-delay")
    if delay["submitted_delay"] != 2 or [row["queue_size"] for row in delay["observed"]] != [1, 1, 1, 0]:
        raise ValueError("Native delay/dispatch/removal observations changed")
    if [row["entry"]["delay"] for row in delay["observed"][:3]] != [1, 0, -100]:
        raise ValueError("Native client-cycle countdown changed")
    for row in cases:
        if row["case"] == "jingle-request-order" and row["pending"] != [row["submitted"][-1]]:
            raise ValueError("Invented quest/skill jingle priority")
        if row["case"] == "jingle-auxiliary-argument" and row["pending"] != [154]:
            raise ValueError("Unused source jingle auxiliary argument acquired a priority")
    source_native = compressed("research/audio-source/binding-native-evidence.json.gz")
    eating = source_native["sequences"]["12526"]["definition"]
    silent = source_native["sequences"]["829"]["definition"]
    if eating["frameLengths"][0] != 4 or eating["frameSounds"]["1"] != [
            {"id": 2393, "location": 5, "loops": 1, "retain": 0, "weight": 100}]:
        raise ValueError("Source eating frame/cycle event changed")
    if silent["frameSounds"] or eating["frameIDs"] != silent["frameIDs"] or eating["frameLengths"] != silent["frameLengths"]:
        raise ValueError("Eating motion/timing relationship lost")
    for template in contract["reference_templates"]:
        provenance = read_json(template["source_provenance"]["path"])
        if decode_templates:
            actual = wave_facts(ROOT / template["path"])
            if actual != template["signal"] or actual["pcm_s16le_sha256"] != provenance["pcm_s16le_sha256"]:
                raise ValueError("Original cue reference PCM changed")
    return {"published_flacs": 264, "reference_wavs": 2, "selectors": 7,
            "approved_adaptations": 2, "historical_qualifications_retained": True,
            "native_queue_rules_checked": True, "reconverted_files": 0, "pack_approved": False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--materialize-templates", action="store_true")
    parser.add_argument("--source", type=Path)
    args = parser.parse_args()
    if args.materialize_templates:
        if args.source is None:
            parser.error("--source is required for exact template reuse")
        records = materialize_templates(args.source)
        print(json.dumps({"copied_exact_native_templates": len(records), "reconverted_files": 0}))
    else:
        contract = source_contract()
        write_json(OUT / "audio-reference.json", contract)
        print(json.dumps({"published_flacs": 264, "original_reference_wavs": 2,
                          "selectors": len(contract["selectors"]), "owner_approved_adaptations": 2,
                          "pack_approved": False}))


if __name__ == "__main__":
    main()
