"""Record measured source observations without repeating audio conversion."""

import gzip
import hashlib
import json
from pathlib import Path

from audio_import import file_record, json_bytes, write_json


def main():
    root = Path(".local/audio-bindings/public-recordings")
    files = [
        "normal-cooks-completion-matches.json", "normal-cooks-state-grid.json",
        "band-limited-handoff-cue-matches.json", "furnace-independent-band-coherence.json",
        "tutorial-source-action-grid.json", "native-legacy-cue-matches.json",
        "cooks-speedrun-guide-matches.json", "cooks-short-jingle-prefixes.json",
        "cooks-levelup-state-grid.json", "tutorial-full-jingle-matches.json",
        "recent-tutorial-full-jingle-matches.json", "current-cooks-jingle-matches.json",
        "current-tutorial-jingle-matches.json", "current-new-player-jingle-matches.json",
        "current-tutorial-completion-matches.json", "current-tutorial-band-jingle-matches.json",
        "handoff-eating-matches.json", "current-food-cue-matches.json",
        "current-month-selector-search.json", "exact-handoff-file-search.json",
        "quest-selector-document-search.json",
    ]
    evidence = {name: json.loads((root / name).read_text()) for name in files}
    videos = []
    for ident in ["ppHJckQj7Vk", "fu46Z8ECdVg", "Zv2zZAsgOQ4", "1lHcLieBdLk",
                  "GdRycOHOhZM", "PhUBpN6rMYM", "JJKkr6zZubE", "0ZFXGZD4fnE"]:
        metadata = json.loads((root / f"{ident}.info.json").read_text())
        videos.append({
            "id": ident, "url": f"https://www.youtube.com/watch?v={ident}",
            "title": metadata["title"], "uploader": metadata["uploader"],
            "upload_date": metadata["upload_date"], "duration": metadata["duration"],
            "audio_file": file_record(root / f"{ident}.m4a"),
            "recording_build": None,
        })
    evidence["recordings"] = videos
    evidence["bounded_current_stream"] = json.loads((root / "current-tutorial-first450s.json").read_text())
    evidence["furnace_frame_relation"] = json.loads(Path(".local/audio-bindings/furnace-frame-relation.json").read_text())
    path = Path("research/audio-source/selector-observation-evidence.json.gz")
    path.write_bytes(gzip.compress(json_bytes(evidence), compresslevel=9, mtime=0))
    result = {
        "schema_version": 1,
        "source_selection": "osrs-live-240-cache2695-injected1.12.38-20260913",
        "parent_handoff": {
            "path": "research/reference-pack/v1/audio-handoff.json",
            "parent_commit": "9740655", "read_only": True,
        },
        "evidence": file_record(path),
        "method": {
            "fixed_bands_hz": [[500, 9000], [1500, 9000], [3500, 10000]],
            "filter": "Same zero-phase FFT band weighting for recording/template, 200Hz transitions; normalized waveform correlation.",
            "reason": "Unfiltered correlation was depressed by unrelated background music. Fixed-band tests recover the original source waveform rather than choosing by name or perceived similarity.",
            "not_product_comparison_tolerances": True,
        },
        "observed_selectors": [
            {
                "rule": "rule.combat.ranged", "item_ids": [841], "source_sequence": 426,
                "sound_id": 2693, "independent_name": "ARROWLAUNCH2",
                "event": "ordinary shortbow projectile release, not equip or target impact",
                "video": "1lHcLieBdLk", "recording_date": "20240521",
                "cue_start_seconds": [517.6946485260771, 520.1095238095238],
                "correlations": [0.994381682692452, 0.9770664608897152],
                "visual_event_bound_seconds": [517.217, 518.118],
                "rejected_name_only_alternatives": [2692, 2700, 2702],
                "repeat_count": 1,
                "source_relative_timing": "Game-triggered launch boundary; no embedded frame event in426. Preserve received queue delay rather than adding a guessed frame or projectile-impact offset.",
            },
            {
                "rule": "rule.combat.tutorial_rat", "npc_ids": [3313, 3314, 3315],
                "source_sequences": {"attack": 4933, "hit": 4934, "death": 4935},
                "sound_ids": {"attack": 710, "hit": 713, "death": 711},
                "video": "1lHcLieBdLk", "recording_date": "20240521",
                "attack_observations": [
                    {"seconds": 480.50535147392293, "correlation": 0.9880185670344562},
                    {"seconds": 482.90861678004535, "correlation": 0.9211709158031158},
                ],
                "hit_correlation": 0.9983217579894066,
                "death_correlation": 0.9978990332236555,
                "event": "Separate game-triggered attack, damage-reaction and death cues, not sounds on every movement frame",
            },
            {
                "rule": "rule.smelting.bronze", "item_ids": [436, 438, 2349],
                "source_sequence": 899, "sound_id": 2725,
                "video": "1lHcLieBdLk", "recording_date": "20240521",
                "cue_start_seconds": 400.07142857142856,
                "event": "Accepted copper/tin furnace operation begins; source chat says the ores are placed together in the furnace",
                "visual_event_bound_seconds": [399.732, 400.099],
                "independent_band_alignment": "All five disjoint bands align within one22050Hz sample; the6000-9000Hz band reaches0.7819773 and the next off-event peak is only0.0455 in magnitude.",
                "not_inferred_from_other_sequence": 3243,
                "frame_payloads_899_3243_identical": False,
                "source_relative_timing": "Game-triggered smelt-start cue; no embedded event in899 and no duplicated waveform-leading delay.",
            },
        ],
        "cooks_assistant_observation": {
            "quest": "quest.cooks_assistant",
            "normal_account_video": "ppHJckQj7Vk",
            "recording_date": "20170823",
            "quest_jingle": 152,
            "quest_cue_start_seconds": 681.5116553287982,
            "five_second_correlation": 0.9810391310274564,
            "quest_scroll_visible_at_seconds": [681.566, 683.0, 688.0],
            "levelup_after_scroll_dismissal": {
                "jingle": 33, "level": 4, "cue_start_seconds": 690.1516553287981,
                "correlation": 0.9822065977733833,
                "dialog_visible_at_seconds": [690.2, 691.5],
            },
            "later_actual_cooking_levelup": {
                "jingle": 34, "level": 5, "cue_start_seconds": 715.7616326530613,
                "correlation": 0.9207284175007333,
                "not_a_simultaneous_quest_reward": True,
            },
            "ordering_rule_observed": "Quest completion/152 precedes the reward-caused level-up dialog/33; the level-up is shown and sounded after the quest scroll is dismissed.",
            "native_queue_rule_unchanged": "No quest/skill rank or auxiliary priority parameter. Last accepted native jingle request wins; the observed ordering comes from when the game submits requests.",
            "excluded_counterexample": "fu46Z8ECdVg has a Quest Speedrunning results interface and audible34; it is not promoted to the normal-account quest ordering.",
            "not_154_by_novice_guess": True,
        },
        "current_scope_limit": "These observations identify exact current-cache audio payloads in dated source-game contexts. The historical recording dates are retained; they are not claimed to be captures of build240. Recent recordings were separately tested and do not silently replace that version caveat.",
        "still_unresolved": [
            {
                "rule": "quest.learning_the_ropes",
                "selector": "Actual current server-emitted quest jingle among152/153/154, another cue or no-request, and its submission order against any simultaneous skill-level jingle.",
                "attempts": "Whole current-named tutorial guide, whole post-2025 no-commentary tutorial, upload-filtered recent recordings and a bounded September10 stream interval; none provides a decisive quest-cue observation. Some recent stream portions were already on the mainland and are not falsely treated as tutorial completion evidence.",
            },
            {
                "rule": "rule.food.healing", "item_ids": [315, 2309],
                "selector": "Ordinary shrimps/bread selection of829 plus a game-triggered2393 versus sound-enabled12526, and the actual source action callback boundary.",
                "known": "Current12526 contains2393 atframe1, starts after the raw4-cycle frame0 length, and is otherwise motion/timing equivalent to829. Both exact food definitions have Eat but no selector parameters. The runtime has no direct2393/12526 constant-based item callback.",
                "attempts": "Exact item metadata, current native/CS2 audit, current eating-recording candidate and known tutorial/quest recordings; no literal315/2309 consumption observation decisively matches.",
            },
        ],
        "no_existing_audio_converted": True,
        "parent_pack_changed": False,
        "source_account_login": False,
        "owner_pack_approved": False,
        "running_browser_audio_accepted": False,
    }
    # Take the date from the actual retrieved metadata instead of a guide/title inference.
    normal = next(record for record in videos if record["id"] == "ppHJckQj7Vk")
    result["cooks_assistant_observation"]["recording_date"] = normal["upload_date"]
    write_json(Path("research/audio-source/selector-observations.json"), result)
    print("Recorded source2693/710/2725 and normal Cook's152/dialog-order observations; exact residuals retained.")


if __name__ == "__main__":
    main()
