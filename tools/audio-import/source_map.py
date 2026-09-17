"""Source event relationships, deliberately separate from live playback acceptance."""

import gzip
import json
from pathlib import Path


def read_collection(kind):
    path = Path(f"assets/source/osrs/cache2695/collections/{kind}.json.gz")
    return {value["id"] if "id" in value else int(key.rsplit(".", 1)[1]): value
            for key, value in json.loads(gzip.decompress(path.read_bytes())).items()}


def sequence_events(sequence):
    result = []
    lengths = sequence.get("frameLengths")
    for key, sounds in sequence["frameSounds"].items():
        frame = int(key)
        if lengths is not None and not 0 <= frame < len(lengths):
            raise ValueError("Source sound frame is outside its original sequence")
        offset = sum(lengths[:frame]) if lengths is not None else None
        for sound in sounds:
            result.append({
                "sequence_id": sequence["id"],
                "frame": frame,
                "source_frame_start_cycle_sum": offset,
                "nominal_frame_start_ms": offset * 20 if offset is not None else None,
                "wall_clock_enqueue_offset_verified": False,
                **sound,
            })
    return sorted(result, key=lambda value: (value["frame"], value["id"]))


def effect_bounds(definition):
    instruments = []
    for index, instrument in enumerate(definition["field1008"]):
        if instrument is not None:
            instruments.append({
                "instrument": index,
                "duration_ms": instrument["field1176"],
                "offset_ms": instrument["field1188"],
                "echo_delay_ms": instrument["field1187"],
                "echo_decay_percent": instrument["field1184"],
                "oscillator_volumes": instrument["field1180"],
                "oscillator_pitch_offsets": instrument["field1177"],
                "oscillator_delays_ms": instrument["field1179"],
            })
    duration_ms = max((value["duration_ms"] + value["offset_ms"] for value in instruments), default=0)
    return {
        "expected_frames": duration_ms * 22050 // 1000,
        "source_duration_ms": duration_ms,
        "loop_start_ms": definition["field1006"],
        "loop_end_ms": definition["field1009"],
        "instruments": instruments,
        "offsets_baked": True,
        "queue_delay_trim_applied": False,
        "offset_note": "The full original waveform includes instrument offsets. Do not add these offsets again when using the exported waveform. Runtime packet delay and optional source queue-delay trimming are separate and not observed here.",
    }


def build(inputs, request, available_ids):
    sequences = read_collection("sequence")
    for path in (inputs / "definitions/sequence").glob("*.json"):
        sequence = json.loads(path.read_text())
        sequences[sequence["id"]] = sequence
    events = [event for sequence in sequences.values() for event in sequence_events(sequence)]
    if any(event["id"] not in available_ids for event in events):
        raise ValueError("Source sequence references an unconverted sound")
    objects = read_collection("object")
    sounding = {
        object_id: value for object_id, value in objects.items()
        if value["ambientSoundId"] >= 0 or value.get("ambientSoundIds")
    }
    regions = {object_id: set() for object_id in sounding}
    for path in sorted(Path("assets/source/osrs/cache2695/world").glob("*.json.gz")):
        world = json.loads(gzip.decompress(path.read_bytes()))
        for placement in world["placements"]:
            if placement[0] in regions:
                regions[placement[0]].add(world["region_id"])
    ambient = []
    for object_id, definition in sounding.items():
        sound_ids = list(definition.get("ambientSoundIds") or [])
        if definition["ambientSoundId"] >= 0:
            sound_ids.append(definition["ambientSoundId"])
        if any(source_id not in available_ids for source_id in sound_ids):
            raise ValueError("Source object references an unconverted ambient sound")
        ambient.append({
            "object_id": object_id,
            "name": definition["name"],
            "region_ids_with_original_placements": sorted(regions[object_id]),
            "source_fields": {key: value for key, value in definition.items() if "sound" in key.lower()},
            "state_note": "Definition/morph candidate, not proof the object is active. Preserve native distance, retention, change-tick, visibility and fade fields; attenuation/camera/player state needs source observation.",
        })
    actions = [
        ("rule.mining.copper", [625], [3220], "Source frame event; not a success/XP-only sound."),
        ("rule.mining.tin", [625], [3220], "Same original bronze-pickaxe frame event."),
        ("rule.woodcutting.normal", [879], [2735, 2734], "Chop2735 is a source frame event. Falling2734 is a named source input; its depletion timing is not observed."),
        ("rule.fishing.shrimps", [621], [2603], "Actual net-fishing frame4 sound. Do not play a replacement splash on every catch."),
        ("rule.firemaking.normal", [733], [2597, 2596], "Tinder2597 occurs at source frames8/10. Fire2596 is named but ignition scheduling is not observed."),
        ("rule.cooking.shrimps", [897], [2577], "Named cooking sound; no embedded sound in the selected source cooking sequence."),
        ("rule.cooking.bread", [896], [2577], "Named cooking sound; success/burn timing needs observation."),
        ("rule.cooking.dough", [], [], "No verified source sound; do not invent one."),
        ("rule.smelting.bronze", [899], [], "No verified current sound binding. Candidate2725 is prepared separately, based only on an independent server implementation using a different animation."),
        ("rule.smithing.bronze_dagger", [898], [3790, 3791], "Four actual alternating source frame events:5,7,9,11."),
        ("rule.combat.melee", [386, 390, 422, 423], [2498], "Named attack-hit input only; weapon-specific launch/hit assignment is not observed. Independently named unarmed2564/2565/2566 inputs remain candidates, not sword replacements."),
        ("rule.combat.ranged", [426], [], "No verified bow-specific sound binding in the selected source sequence; do not substitute a melee sound."),
        ("rule.magic.wind_strike", [711, 658, 659, 660], [220, 221, 227], "Wind Strike cast/hit names from pinned independent symbols; source spot90/91/92 -> sequence658/659/660. Launch, impact and splash selection/offsets still need original-session observation."),
        ("rule.magic.home_teleport", [], [193, 194, 195, 196, 200], "Original named teleport components; precise channel/sequence/queue offsets not observed."),
        ("rule.inventory.drop", [], [2739], "Named source input; no generic click replacement."),
        ("rule.equipment.equip", [], [2238], "Independent default-equipment symbol; exact item-specific selection unverified."),
        ("rule.prayer.bones", [827], [2738], "Named original bones-bury sound."),
        ("rule.prayer.thick_skin", [], [2690, 2663, 2672], "Named activation/deactivation/depletion inputs; not an altar or prayer-level-up replacement."),
        ("rule.bank.deposit", [], [], "No blanket banking sound assignment verified."),
        ("rule.bank.withdraw", [], [], "No blanket banking sound assignment verified."),
        ("rule.shop.lumbridge", [], [], "No blanket shop sound assignment verified."),
        ("rule.food.healing", [829, 12526], [2393], "Current sound-enabled sequence12526 is byte-semantically the same motion as829 plus frame1 cue2393 (loops1/location5/weight100). Sequence829 itself remains silent. Do not add a duplicate game-trigger cue when12526 already emits it; ordinary-food server choice of829 versus12526 is not present in client metadata."),
        ("rule.goblin.level_2", [6182, 6183, 6184, 6185], [469, 471, 472], "Current cue identities469(unarmed attack),472(hit),471(death) are independently named and match two public ordinary-goblin recordings; these are not animation-embedded events. Recordings are historical, not a current server packet trace. Armed-weapon substitutions and exact server delay remain unbound."),
        ("rule.combat.tutorial_rat", [4933, 4934, 4935], [711, 713], "Source NPC3313/3314/3315 uses the493x rat-update family, not legacy138/139/141. Source cue713(hit) and711(death) match a public tutorial recording strongly; attack710 remains a named candidate without a decisive recording match. No embedded frame sounds occur in4933/4934/4935."),
        ("rule.death.first_office", [], [512], "Independent player-death symbol; player hit variants509/510/518..521 and jingles89/90 are prepared, but exact live selection is not observed."),
    ]
    rule_ids = {rule["id"] for rule in json.loads(Path("research/journey-rules/activities.json").read_text())["rules"]}
    for rule_id, _, _, _ in actions:
        if rule_id not in rule_ids:
            raise ValueError("Audio source map has an unknown journey rule")
    return {
        "schema_version": 1,
        "source_selection": request["source_selection"],
        "browser_presentation_approved": False,
        "timing_policy": "Frame-start sums retain native source cycles. Nominal20ms conversion is descriptive, not calibrated enqueue time; original >frameLength boundary behavior, packet delay, channel volume and source-session evidence remain separate.",
        "verified_source_silence_ids": request["verified_silent_sound_ids"],
        "source_silence_policy": request["source_silence_note"],
        "music_scopes": [
            {**track, "index": 6, "exact_region_boundary_and_transition_verified": False}
            for track in request["music"]
        ],
        "region_reference": {
            "tutorial_surface_seed_regions": [12336, 12592],
            "tutorial_dungeon_seed_region": 12436,
            "lumbridge_seed_region": 12850,
            "farms_mill_seed_region": 12595,
            "note": "Existing source extraction seeds, not certified music-switch boundaries or approved game fences.",
        },
        "actions": [{
            "journey_rule_id": rule, "source_sequence_ids": ids, "identified_sound_ids": sounds,
            "source_frame_events": [event for event in events if event["sequence_id"] in ids],
            "source_session_trigger_verified": False, "note": note,
        } for rule, ids, sounds, note in actions],
        "quest_and_gathering": [
            {"action": "Cook's Assistant: collect egg/items", "sound_ids": [2582], "basis": "RuneLite ITEM_PICKUP symbol; exact context timing not observed"},
            {"action": "Cook's Assistant: pick wheat", "sound_ids": [2581], "basis": "RuneLite PICK_PLANT_BLOOP symbol; exact context timing not observed"},
            {"action": "Cook's Assistant: milk cow", "sound_ids": [372], "source_sequence_ids": [2305], "basis": "Independent milk_cow symbol; no frame sound in selected original sequence"},
            {"action": "Cook's Assistant: mill flour", "sound_ids": [], "source_sequence_ids": [472], "basis": "Use original object ambience where present; no new standalone milling sound invented"},
            {"action": "Learning the Ropes / Cook's Assistant completion", "jingle_candidates": [152, 153, 154], "basis": "Pinned named source jingles; exact per-quest selection and XP/quest-jingle precedence require source observation"},
            {"action": "UI", "sound_ids": [2266], "basis": "Named UI_BOOP input, not permission to sound every click/tab/menu"},
        ],
        "additional_prepared_candidates": {"smelting": [2725], "unarmed": [2564, 2565, 2566], "player_hits": [509, 510, 511, 518, 519, 520, 521]},
        "binding_audit": "research/audio-source/bindings.json",
        "reference_only_candidates": {"ordinary_shortbow": [2692, 2693, 2702], "tutorial_rat_attack": [710]},
        "sequence_sound_events": sorted(events, key=lambda value: (value["sequence_id"], value["frame"], value["id"])),
        "ambient_objects": sorted(ambient, key=lambda value: value["object_id"]),
        "jingles": request["jingles"],
        "remaining_observations": [
            "Stock source-session region/playlist transitions and music repeat behavior; the exported tracks are one-pass with original release.",
            "Ordinary shortbow selection among named launch/draw cues2692/2693/2702; current tutorial-rat attack710 confirmation; current food selection of829/game-triggered2393 versus sound-enabled12526; current bronze-smelting2725 callback. None is inferred from a plausible sound.",
            "Event/packet delay, frame entry phase, positional attenuation, mute/volume defaults and fade behavior in the actual source session.",
            "Exact server MIDI_JINGLE payload (including a possible no-request outcome) for Learning the Ropes/Cook's Assistant and its emission order versus level-up jingles. Native client requests are last-wins, with no quest/skill ranking and an ignored auxiliary integer. Browser gesture/autoplay/reconnect behavior remains after pack approval.",
        ],
    }
