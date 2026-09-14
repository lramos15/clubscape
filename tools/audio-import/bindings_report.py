"""Publish compact, confidence-separated evidence for the two narrow audio pack gaps."""

import gzip
import hashlib
import json
from pathlib import Path
from collections import Counter

from audio_import import file_record, json_bytes, write_json

WORK = Path(".local/audio-bindings")
DEST = Path("research/audio-source")


def main():
    extracted = WORK / "extracted"
    scan = json.loads((extracted / "script-scan.json").read_text())
    sequences = json.loads((extracted / "sequences.json").read_text())
    native = json.loads((WORK / "native-queue-evidence.json").read_text())
    if native["result"] != "passed" or scan["decoded_scripts"] != 9801:
        raise ValueError("Native evidence is incomplete")
    counts = Counter(row["opcode"] for row in scan["audio_calls"])
    if counts[3202] != 0:
        raise ValueError("A current script jingle selector needs investigation")
    selection = Path("research/current-source/selection.json")
    source = json.loads(selection.read_text())
    audit = {
        "schema_version": 1,
        "selection": file_record(selection),
        "native_runtime_artifact": source["runtime"]["artifacts"][0],
        "sequences": sequences,
        "widget_callbacks": json.loads((extracted / "widget-callbacks.json").read_text()),
        "native_script_decoder": scan["native_decoder"],
        "decoded_scripts": scan["decoded_scripts"],
        "script_payload_sha256": scan["script_sha256"],
        "non_script_payloads": scan["non_script_payloads_rejected_by_native_decoder"],
        "audio_opcode_counts": dict(sorted(counts.items())),
        "callback_roots": scan["root_scripts"],
        "callback_closure": scan["reachable_scripts"],
        "quest_callback_scripts": {
            str(script): json.loads((extracted / f"scripts/{script}.json").read_text())
            for script in (118, 6816, 6817, 55)
        },
        "native_queue_tests": native,
        "interpretation": "The source script corpus and relevant widget closure do not contain opcode3202. The original packet reader accepts a numeric cue from the server; native client queue rules do not choose a quest's cue.",
    }
    audit_path = DEST / "binding-native-evidence.json.gz"
    audit_path.write_bytes(gzip.compress(json_bytes(audit), compresslevel=9, mtime=0))
    media = WORK / "public-recordings"
    recordings = []
    for video in ("uWc4W3yUcMw", "5A8ZEsQmzrg", "1lHcLieBdLk", "kdk7teYIOQk", "5PazG956MZo", "sEVlD6oBcxs"):
        metadata = json.loads((media / f"{video}.info.json").read_text())
        recordings.append({
            "video_id": video,
            "url": f"https://www.youtube.com/watch?v={video}",
            "title": metadata["title"], "uploader": metadata["uploader"],
            "upload_date": metadata["upload_date"], "declared_duration_seconds": metadata["duration"],
            "retrieved_audio": file_record(media / f"{video}.m4a"),
            "format_id": metadata["format_id"],
            "recording_build": None,
            "rights": "Public third-party recording of Jagex material; used only for bounded source research, not redistributed as a game asset.",
        })
    comparisons = {}
    for name in (
        "cooks-corrected-waveform.json", "cooks-corrected-spectral.json",
        "tutorial-corrected-spectral.json", "recent-music-only-quest-matches.json",
        "tutorial-candidate-sfx-matches.json", "goblin-candidate-matches.json",
        "wiki154-corrected-correlation.json", "wiki154-prior-renderer-correlation.json",
    ):
        comparisons[name] = json.loads((media / name).read_text())
    public_path = DEST / "binding-public-evidence.json.gz"
    public_path.write_bytes(gzip.compress(json_bytes({
        "recordings": recordings, "comparisons": comparisons,
        "source_dictionary": {
            "url": "https://github.com/AlterRSPS/Alter/blob/9ddbbbe3bdf47d79ff919a12d01ce1a7e6be6169/game-api/src/main/kotlin/org/alter/api/cfg/Sound.kt",
            "git_blob": "ad3fcc79a63d200585735cf3cad6a32a16a8effe",
            "sha256": hashlib.sha256((WORK / "references/Alter-Sound.kt").read_bytes()).hexdigest(),
            "role": "Independent candidate names, not authoritative current server-trigger logic.",
        },
        "jingle_recording_metadata": json.loads((media / "wiki-jingle-file-info.json").read_text()),
        "search_evidence": {
            name: json.loads((media / name).read_text()) for name in
            ("targeted-audio-search.json", "sfx-file-search.json", "named-sfx-file-info.json")
        },
    }), compresslevel=9, mtime=0))
    result = {
        "schema_version": 1,
        "task": "m1-audio-bindings",
        "source_selection": source["selection_id"],
        "reference_pack_gap_ids": ["input.required_effect_bindings", "input.quest_jingle_binding"],
        "native_evidence": file_record(audit_path),
        "public_evidence": file_record(public_path),
        "native_queue_rules": {
            "script_sfx_opcode": 3200,
            "arguments": ["sound_id", "repeat_count", "client_cycle_delay"],
            "nonpositional_volume_zero_or_repeat_zero": "No enqueue",
            "capacity": 50,
            "full_queue": "New entry dropped, existing FIFO entries retained; no kind-based priority",
            "countdown": "Each source client cycle decrements delay; dispatch when negative, set delay=-100, remove next cycle. Submitted delay2 dispatches on the third processing call.",
            "source_silence": "Preserve2411 and its74/100 branch probability, not a missing-file fallback.",
            "offsets": "Published SFX already include source instrument offsets. Source runtime trims whole20ms leading delay and adds it to the queue; using the untrimmed asset must not add that trim again.",
            "jingle_opcode": 3202,
            "jingle_arguments": ["source_index11_group", "unused_auxiliary_integer"],
            "jingle_acceptance": "Music volume nonzero and group != -1",
            "jingle_priority": "Last accepted request replaces pending work and clears active music streams, retaining the remembered background playlist. No quest-vs-skill ranking; auxiliary values0/1/255/60000 do not change this.",
            "jingle_sentinel": "Group -1 does not start or erase an earlier accepted request",
            "native_loop": False,
            "request_delay_fade_arguments": [0, 0, 0, 0],
            "source_clock_units_not_hardware_latency": True,
        },
        "cue_bindings": [
            {
                "action": "eating",
                "source_sound_ids": [2393],
                "status": "verified_current_sequence_cue",
                "sequence_id": 12526, "frame": 1, "repeat_count": 1, "range": 5, "retain": 0, "weight": 100,
                "frame_start_cycle_sum": sum(sequences["12526"]["definition"]["frameLengths"][:1]),
                "alternate_sequence": 829,
                "alternate_relationship": "Identical original motion/timing, but829 has no frame sound",
                "residual": "Ordinary food's server choice of829 plus a game-triggered cue versus sound-enabled12526 is not encoded in client action metadata. Do not dispatch2393 twice.",
            },
            {
                "action": "ordinary_unarmed_goblin",
                "source_sound_ids": [469, 472, 471],
                "event_roles": ["attack", "hit", "death"],
                "source_sequences": [6184, 6183, 6182],
                "status": "identified_original_signals_with_historical_public_action_evidence",
                "public_matches": {"attack_peak": 0.7873672946663144, "hit_peak": 0.9777869588842321, "death_peak": 0.9845552823159313},
                "frame_events": [],
                "residual": "No current cache frame sound selects these cues; exact current game-trigger delay and armed goblin variants remain server-dependent, not inferred from historical recordings.",
            },
            {
                "action": "tutorial_rat",
                "source_npc_ids": [3313, 3314, 3315],
                "source_sequences": {"attack": 4933, "hit": 4934, "death": 4935},
                "source_sound_ids": {"hit": 713, "death": 711},
                "status": "identified_original_hit_death_signals_with_historical_tutorial_evidence",
                "public_matches": {"hit_peak": 0.9421424429479718, "death_peak": 0.8224174870665013},
                "attack_candidate": 710,
                "frame_events": [],
                "residual": "Attack710 lacks a decisive recording match; server-trigger phase/delay is not encoded in493x. Legacy138/139/141 is not this source NPC's animation family.",
            },
            {
                "action": "ordinary_shortbow",
                "status": "unresolved_source_event_selector",
                "source_sequence": 426,
                "frame_events": [],
                "independently_named_candidates": {"arrow_launch": 2692, "arrowlaunch2": 2693, "shortbow": 2702},
                "residual": "Names and weak/nonunique historical recording correlations do not identify whether draw/release/projectile callbacks emit one or several cues, or their exact delay.",
            },
            {
                "action": "bronze_smelting",
                "status": "unresolved_current_game_trigger",
                "source_sequence": 899, "other_source_sequence": 3243,
                "relationship": "Same frame lengths/other sequence settings, different actual frame IDs, no embedded sound in either",
                "candidate_sound": 2725,
                "public_candidate_peak": 0.2568288958395292,
                "residual": "The named furnace candidate and weak historical correlation are not a verified current899 bronze-smelt callback. Do not relabel3243 evidence as899.",
            },
        ],
        "quest_jingles": {
            "status": "unresolved_per_quest_server_selector",
            "quests": ["Learning the Ropes", "Cook's Assistant"],
            "candidate_groups": [152, 153, 154],
            "possible_no_request_outcome_retained": True,
            "quest_widget_callback": "153:0 -> script118 ->6816/6817; these format quest-point fields, not audio selection",
            "current_script_corpus": "9801 valid native-decoded scripts; zero opcode3202 occurrences; source script0 payload0009 is rejected by native parser and not in required closure",
            "packet_selector_boundary": "Original client.ia's jingle branch reads numeric group and auxiliary value, maps65535 to-1, then calls bl.bp. No quest ID or skill class is passed.",
            "priority_closed": "Generic last-accepted-request-wins, no class priority and no use of the auxiliary integer",
            "exact_residual": "Actual server-emitted MIDI_JINGLE group/no-request and packet order after valid tutorial Wind Strike and Cook's Assistant reward commit, relative to skill-level jingle emissions. This is not recoverable from the quest widget or the numeric receiver alone.",
            "public_recording_result": "Recent named quest/tutorial guides plus a recent no-commentary tutorial were retrieved without login. Their candidate ending intervals did not produce a decisive match against any152/153/154 template, so neither154 nor silence is asserted.",
        },
        "necessary_source_correction": {
            "native_factory": "dg.ay -> new nu(player) -> nu.ap(9,128,-27396)",
            "previous_offline_renderer": "Omitted the source startup percussion bank",
            "action": "Correct/re-render musical inputs only; preserve all existing SFX hashes and silent weighted entries",
            "not_a_source_selection_change": True,
        },
        "pack_gate_status": {
            "input.required_effect_bindings": "not_fully_closed",
            "input.quest_jingle_binding": "not_fully_closed",
        },
        "blanket_account_requirement": False,
        "original_source_cache_modified": False,
        "runtime_browser_audio_accepted": False,
        "owner_reference_pack_approved": False,
        "m1_accepted": False,
    }
    observations_path = DEST / "selector-observations.json"
    if observations_path.exists():
        observations = json.loads(observations_path.read_text())
        result["latest_selector_observations"] = file_record(observations_path)
        result["superseded_candidate_notes"] = (
            "The dated, source-state-verified observations identify2693 for ordinary shortbow release, "
            "710 for tutorial-rat attack,2725 for copper/tin smelt start, and152 for a normal Cook's "
            "Assistant completion. Use the attached observations rather than the earlier weak-match notes. "
            "Their recording dates remain explicit; Learning-the-Ropes and literal315/2309 selectors remain unresolved."
        )
        for cue in result["cue_bindings"]:
            if cue["action"] == "ordinary_shortbow":
                cue.update({
                    "status": "source_signal_and_public_event_identified",
                    "source_sound_ids": [2693],
                    "event_boundary": "ordinary shortbow projectile release",
                    "recording_date": "20240521",
                    "correlation": 0.994381682692452,
                    "residual": "No animation426 frame cue; retain source game-trigger/received queue delay. Recording date is not relabeled as build240.",
                })
            elif cue["action"] == "tutorial_rat":
                cue["source_sound_ids"]["attack"] = 710
                cue["status"] = "source_attack_hit_death_signals_and_public_events_identified"
                cue["public_matches"]["attack_peak"] = 0.9880185670344562
                cue["residual"] = "Game-triggered attack/hit/death cues, not embedded493x frame events; dated source observation qualification retained."
            elif cue["action"] == "bronze_smelting":
                cue.update({
                    "status": "source_signal_and_public_smelt_start_identified",
                    "source_sound_ids": [2725],
                    "event_boundary": "accepted copper/tin furnace operation starts",
                    "recording_date": "20240521",
                    "residual": "Bound by sample-coherent recording and actual smelt-start state, not by treating3243 as899. Current server-build capture is not claimed.",
                })
        result["quest_jingles"]["cooks_assistant_source_observation"] = observations["cooks_assistant_observation"]
        result["quest_jingles"]["exact_residual"] = (
            "Learning-the-Ropes actual current quest-jingle/no-request selector and its emission order "
            "against simultaneous level-ups. Cook's Assistant152 followed by modal-deferred level-up33 "
            "is now source-observed, with the2017 recording qualification retained."
        )
        result["remaining_selector_rules"] = [row["rule"] for row in observations["still_unresolved"]]
    write_json(DEST / "bindings.json", result)
    print("Published exact native queue/metadata evidence and explicit per-selector residuals.")


if __name__ == "__main__":
    main()
