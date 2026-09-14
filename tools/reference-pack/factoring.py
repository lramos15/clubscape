"""Literal Section30.2 source families, distinct visual states and separate acceptance gates."""

from collections import defaultdict
import hashlib
import json

from catalogue import TUTORIAL_GROUPS, wiki, update, music_update
from components import ROOT, SOURCE, digest, load_gzip
from text_oracles import make_text_oracles
from native_hud import validate_records as validate_native_hud_records


FAMILY_DEFINITIONS = [
    ("entry.native", "Native title/login/loading/authentication/terms",
     "welcome login_form loading invalid_credentials terms",
     "entry.title entry.login entry.loading entry.authentication_failure entry.terms", []),
    ("entry.web", "Web-only owner-review compositions",
     "registration registration_rejection connecting capability_error runtime_error scope_feedback branding",
     "entry.registration entry.registration_rejected entry.connecting entry.capability_error entry.runtime_error entry.unavailable entry.branding", []),
    ("entry.reconnect", "Reconnect feedback in the source visual language",
     "connection_lost retrying returned_to_login", "entry.reconnect", [162]),
    ("hud.classic", "Complete stock Resizable - Classic frame",
     "full_frame minimap chat tabs source_locked source_highlighted unlocked run_active prayer_active",
     "hud.resizable_classic hud.minimap hud.chat hud.tabs_unlocks", [161, 162, 614]),
    ("hud.minimenu", "Source minimenu and item/spell selection",
     "default object npc weapon item_use spell_use cancel rejected_target", "hud.context_menu", []),
    ("tutorial.appearance", "Source appearance composition; separate penguin adaptation",
     "appearance_controls preview confirmation", "tutorial.appearance", []),
    ("tutorial.experience", "Three source experience choices",
     "brand_new returning experienced", "tutorial.experience", [929]),
    ("dialogue.flow", "Reusable dialogue, instruction, item-message and choice layouts",
     "speaker_chathead player_speaker instruction item_grant choices continue refusal", "ui.dialogue", [162, 219, 231, 614]),
    ("ui.inventory", "Inventory slots and value-dependent presentation",
     "empty occupied stack_count selected_item drag item_on_item full_rejection",
     "ui.inventory", [149]),
    ("ui.equipment", "Equipment slots and a distinct statistics panel",
     "slots stats weapon_swap bow_displaces_shield requirement_rejection", "ui.equipment", [84, 85]),
    ("ui.skills", "24 skills, tooltips and current level-up presentation",
     "grid_24 tooltip levelup_chat levelup_popup highlight", "ui.skills", [320]),
    ("ui.combat", "Combat-style controls",
     "melee_styles ranged_styles selected_style auto_retaliate target_feedback", "ui.combat", [593]),
    ("ui.prayer", "Prayer availability and activation",
     "unavailable available active quick_prayers depleted restored", "ui.prayer", [541]),
    ("ui.magic", "Spell filters, availability, selection and channel feedback",
     "level_filtered unfiltered available missing_runes selected_spell rejected_target teleport_channel",
     "ui.magic", [218]),
    ("ui.production", "Production selection and outcome presentation",
     "item_mixing cooking_success cooking_burn smelting anvil dagger_selection unavailable_recipe",
     "ui.smithing", []),
    ("ui.bank", "Bank controls, content and amount states",
     "open closed tabs search placeholders items notes amount selection rejected_amount",
     "ui.bank", [12, 15, 192, 213]),
    ("ui.shop", "Shop stock, buy/sell and quantity states",
     "stock buy sell amount insufficient_coins full_inventory out_of_stock",
     "ui.shop", [300, 301]),
    ("ui.quests", "Quest list, journal, ingredient projections and rewards",
     "not_started active complete ingredient_missing ingredient_carried ingredient_delivered reward_ropes reward_cooks",
     "ui.quests ui.quest_rewards", [119, 153, 399]),
    ("ui.settings", "Source display/control settings",
     "sidebar all_settings classic_selected", "ui.settings", []),
    ("ui.account_poll", "Account, logout and current poll composition",
     "account links logout poll_intro poll_choices poll_unavailable", "ui.account_poll_logout", [182, 558]),
    ("ui.death", "Death and recovery presentation families",
     "death office_dialogue kept_items grave fees recovery", "ui.death_recovery", [4, 669, 670, 671]),
    ("world.scenery", "Calibrated source scenes and complete instructor-route context",
     "tutorial_house tutorial_coast kitchen quest_house mine combat_cave bank chapel magic_area lumbridge_castle bridge farms_mill route_transition",
     "world.tutorial_house world.tutorial_coast world.lumbridge_arrival world.lumbridge_bridge world.cooks_route", []),
    ("world.activities", "Source action/animation families, not one screenshot per tick",
     "fishing chopping firemaking cooking mining smelting smithing melee ranged magic home_teleport",
     "", []),
    ("model.ordinary", "Unchanged tree and goblin source views/animation cycles",
     "tree_orientations goblin_idle goblin_walk", "model.tree model.goblin", []),
    ("model.penguin", "Original penguin candidate; owner-reviewed fitting",
     "idle walk equipment_fit_all_slots", "model.penguin", []),
    ("audio.music", "Five original source music tracks",
     "title tutorial_surface tutorial_dungeon lumbridge farms",
     "audio.music.title audio.music.tutorial_surface audio.music.tutorial_dungeon audio.music.lumbridge audio.music.farms", []),
    ("audio.effects", "Required activity/interaction/ambient sound identities and triggers",
     "gathering production combat interaction ambient",
     "audio.effects.gathering audio.effects.production audio.effects.combat audio.effects.interaction audio.effects.ambient", []),
    ("audio.jingles", "Quest and level-up jingle identities",
     "ropes_completion cooks_completion level_up precedence", "audio.jingles", []),
    ("audio.playback", "Source controls and controlled playback calibration",
     "volume mute single area shuffle playlists transition reconnect gesture",
     "audio.controls audio.transitions audio.reconnect", [239]),
]


# States sharing a renderer still retain their exact state IDs and source data.
STAGE_VISUALS = [
    ("appearance", "tutorial.appearance.appearance_controls"),
    ("experience", "tutorial.experience.brand_new tutorial.experience.returning tutorial.experience.experienced"),
    ("guide_greeting guide_settings", "dialogue.flow.speaker_chathead dialogue.flow.continue"),
    ("settings_open", "ui.settings.sidebar hud.classic.source_highlighted"),
    ("starting_exit survival_exit chef_entry chef_exit quest_entry quest_ladder mining_exit combat_exit account_entry account_exit chapel_entry chapel_exit magic_entry",
     "world.scenery.route_transition dialogue.flow.instruction"),
    ("survival_greeting survival_tools chef_greeting mining_greeting mining_hammer melee_supply ranged_supply magic_supply",
     "dialogue.flow.speaker_chathead dialogue.flow.item_grant ui.inventory.occupied"),
    ("inventory_open", "ui.inventory.occupied hud.classic.source_highlighted"),
    ("catch_shrimp", "world.activities.fishing ui.inventory.occupied"),
    ("skills_open", "ui.skills.grid_24 ui.skills.tooltip hud.classic.source_highlighted"),
    ("cut_logs", "world.activities.chopping ui.inventory.occupied"),
    ("light_fire", "world.activities.firemaking ui.inventory.item_on_item"),
    ("cook_shrimp", "world.activities.cooking ui.production.cooking_success ui.production.cooking_burn"),
    ("make_dough", "ui.production.item_mixing ui.inventory.item_on_item"),
    ("bake_bread", "world.activities.cooking ui.production.cooking_success ui.production.cooking_burn"),
    ("run_toggle", "hud.classic.run_active"),
    ("quest_greeting quest_explanation", "dialogue.flow.speaker_chathead ui.quests.active"),
    ("journal_open", "ui.quests.not_started ui.quests.active ui.quests.complete hud.classic.source_highlighted"),
    ("mine_first mine_second", "world.activities.mining ui.inventory.occupied"),
    ("smelt_bronze", "world.activities.smelting ui.production.smelting"),
    ("anvil_open", "ui.production.anvil ui.production.unavailable_recipe"),
    ("smith_dagger", "world.activities.smithing ui.production.dagger_selection"),
    ("combat_greeting", "dialogue.flow.speaker_chathead"),
    ("equipment_open", "ui.equipment.slots hud.classic.source_highlighted"),
    ("equipment_stats_open", "ui.equipment.stats"),
    ("equip_dagger equip_melee", "ui.equipment.weapon_swap ui.equipment.slots"),
    ("combat_open", "ui.combat.melee_styles ui.combat.auto_retaliate hud.classic.source_highlighted"),
    ("enter_rat_pen leave_rat_pen", "dialogue.flow.instruction world.scenery.combat_cave"),
    ("melee_rat", "world.activities.melee ui.combat.target_feedback"),
    ("equip_ranged", "ui.equipment.bow_displaces_shield ui.equipment.slots ui.combat.ranged_styles"),
    ("ranged_rat", "world.activities.ranged ui.combat.target_feedback"),
    ("bank_open", "ui.bank.open ui.bank.items ui.bank.amount"),
    ("bank_close", "ui.bank.closed"),
    ("poll_inspect", "ui.account_poll.poll_intro ui.account_poll.poll_choices ui.account_poll.poll_unavailable"),
    ("account_greeting account_explanation", "dialogue.flow.speaker_chathead ui.account_poll.links ui.account_poll.logout"),
    ("account_open", "ui.account_poll.account hud.classic.source_highlighted"),
    ("prayer_greeting prayer_explanation", "dialogue.flow.speaker_chathead ui.prayer.available ui.prayer.active ui.prayer.restored"),
    ("prayer_open", "ui.prayer.unavailable ui.prayer.available hud.classic.source_highlighted"),
    ("magic_greeting", "dialogue.flow.speaker_chathead"),
    ("magic_open", "ui.magic.level_filtered ui.magic.unfiltered ui.magic.missing_runes hud.classic.source_highlighted"),
    ("wind_strike", "ui.magic.selected_spell world.activities.magic ui.quests.reward_ropes"),
    ("departure_offer", "ui.quests.reward_ropes dialogue.flow.choices"),
    ("departure_confirmation", "dialogue.flow.choices dialogue.flow.continue"),
    ("home_teleport teleport_channel", "ui.magic.teleport_channel world.activities.home_teleport"),
    ("mainland", "world.scenery.lumbridge_castle dialogue.flow.instruction"),
]

STATE_TEXT_SECTIONS = [
    ("appearance experience", None, [None]),
    ("guide_greeting settings_open", None, [None]),
    ("guide_settings starting_exit", None, ["After opening settings menu"]),
    ("survival_greeting", None, ["Talking to the Survival Expert"]),
    ("inventory_open", None, ["After opening inventory"]),
    ("catch_shrimp", None, ["Before fishing", "After clicking on a fishing spot", "Before opening skills menu"]),
    ("skills_open", None, ["Before opening skills menu", "After opening skills menu"]),
    ("survival_tools", None, ["After fishing"]),
    ("cut_logs", None, ["Before cutting a tree", "After clicking on a tree"]),
    ("light_fire", None, ["Before lighting a fire", "After using tinderbox on logs"]),
    ("cook_shrimp", None, ["Before cooking the shrimp", "After using shrimp on fire", "Cooking more shrimp"]),
    ("survival_exit", None, ["After opening the gate"]),
    ("chef_entry", None, ["Opening the Master Chef's door"]),
    ("chef_greeting", None, ["Talking to the Master Chef"]),
    ("make_dough", None, ["Making the dough"]),
    ("bake_bread", None, ["Cooking the dough", "Cooking more dough"]),
    ("chef_exit", None, ["Opening the exit door"]),
    ("run_toggle", None, ["After toggling the run button"]),
    ("quest_entry", None, ["Opening the Quest Guide's door"]),
    ("quest_greeting", None, ["Talking to the Quest Guide"]),
    ("journal_open", None, ["Opening the quest journal"]),
    ("quest_explanation", None, ["Talking to the Quest Guide after viewing the quest journal"]),
    ("quest_ladder", "Mining Instructor", ["After entering the caves"]),
    ("mining_greeting", None, ["Talking to the Mining Instructor"]),
    ("mine_first", None, ["Mining a rock", "Before obtaining both ores"]),
    ("mine_second", None, ["Mining a rock", "After obtaining both ores"]),
    ("smelt_bronze", None, ["Smelting bronze"]),
    ("mining_hammer", None, ["Talking to the Mining Instructor after smelting a bronze bar"]),
    ("anvil_open", None, ["Before making the dagger", "Opening the smithing menu"]),
    ("smith_dagger", None, ["After smithing a bronze dagger", "Attempting to make other bronze items"]),
    ("mining_exit", None, ["Opening the gate"]),
    ("combat_greeting", None, ["Talking to the Combat Instructor"]),
    ("equipment_open", None, ["Opening the worn inventory tab"]),
    ("equipment_stats_open", None, ["Opening the equipment stats menu"]),
    ("equip_dagger", None, ["After equipping a bronze dagger"]),
    ("melee_supply", None, ["Talking to the Combat Instructor after equipping a bronze dagger"]),
    ("equip_melee", None, ["After equipping a bronze sword and wooden shield"]),
    ("combat_open", None, ["Opening the combat interface"]),
    ("enter_rat_pen", None, ["Entering the rat cage"]),
    ("melee_rat", None, ["Attacking a giant rat", "After killing a giant rat", "Attempting to attack a 2nd giant rat with melee"]),
    ("leave_rat_pen ranged_supply", None, ["Talking to the combat instructor after killing the first giant rat"]),
    ("equip_ranged", None, ["Talking to the combat instructor before killing the second giant rat"]),
    ("ranged_rat", None, ["After killing a second giant rat", "Attempting to attack a 3rd giant rat with ranged"]),
    ("combat_exit", None, ["Climbing the exit ladder"]),
    ("bank_open bank_close", None, ["Opening the bank"]),
    ("poll_inspect", None, ["Accessing the poll booth"]),
    ("account_entry", None, ["Entering the Account Guide's room"]),
    ("account_greeting", None, ["Talking to the Account Guide"]),
    ("account_open", None, ["Opening the Account Management menu"]),
    ("account_explanation", None, ["Talking to the Account Guide after opening the Account Management menu"]),
    ("account_exit", None, ["Talking to the Account Guide again", "Attempting to leave through the bank exit"]),
    ("chapel_entry prayer_greeting", None, ["Talking to Brother Brace"]),
    ("prayer_open", None, ["Opening the prayer menu"]),
    ("prayer_explanation", None, ["Talking to Brother Brace after opening the prayer menu"]),
    ("chapel_exit", None, ["After exiting the chapel"]),
    ("magic_entry magic_greeting", None, ["Talking to the Magic Instructor"]),
    ("magic_open", None, ["Opening the magic interface"]),
    ("magic_supply", None, ["Talking to the Magic Instructor after opening the magic interface"]),
    ("wind_strike", None, ["After casting air strike on a chicken"]),
    ("departure_offer departure_confirmation home_teleport teleport_channel", "After casting wind strike", [None]),
    ("mainland", "After casting wind strike", ["After teleporting to Lumbridge"]),
]

SOURCE_REVIEW_INPUTS = {
    "input.classic_frame_calibration": {
        "family_ids": ["hud.classic", "dialogue.flow"],
        "case_ids": ["case.hud.resizable_classic", "case.ui.dialogue"],
        "literal_basis": "30.2: baseline Resizable - Classic stock layout, exact settings and reference HUD captures.",
        "status": "missing_native_reference",
        "available": "Authorized source commit db103ba supplies16 original complete1920x1080 Classic frame/panel/"
                     "attachment-family fixtures, native161/CS2/layout readbacks, source font/dialogue geometry "
                     "and hash-bound script/input contracts. Public images and pinned text oracles remain complementary.",
        "missing": "The complete native HUD collection/calibration must be present and hash-valid; no separate "
                   "authenticated micro-state or per-speaker screenshot is required.",
        "not_required": "No authenticated account, no per-micro-transition capture and no implemented ClubScape screenshot.",
        "evidence_ids": [],
    },
    "input.required_effect_bindings": {
        "family_ids": ["audio.effects"],
        "case_ids": ["case.audio.effects.production", "case.audio.effects.combat"],
        "literal_basis": "30.2 explicitly requires required sound IDs and source-defined playback triggers/timing.",
        "status": "unresolved_source_identity",
        "available": "All258 FLACs and current cache sequence/ambient events, plus named independent candidates.",
        "missing": "Current ordinary shortbow, goblin/rat, eating and bronze-smelting cue bindings/event boundaries. "
                   "These cannot be inferred from a candidate that merely plays a plausible sound. Smelting2725 remains a candidate.",
        "not_required": "No live-account proof for already identified native frame events; those use recorded cycle offsets. "
                        "Device latency/gain and observed gameplay synchronization are later candidate checks.",
        "evidence_ids": [],
    },
    "input.quest_jingle_binding": {
        "family_ids": ["audio.jingles"],
        "case_ids": ["case.audio.jingles"],
        "literal_basis": "30.2: source sound IDs and triggers across the complete required journey.",
        "status": "unresolved_source_identity",
        "available": "Pinned jingle152/153/154 PCM. Rechecked exact wiki revisions:154 usually accompanies Beginner/Easy "
                     "quests;152 Master;153 Intermediate/Expert. This narrows the candidate, not the actual quest binding.",
        "missing": "Exact Learning the Ropes/Cook's Assistant cue selection and quest/level-up precedence. "
                   "A wiki 'usually' statement is not an observed or defined per-quest selector.",
        "not_required": "No extra rendered quest-scroll screenshots; existing current/source-compatible scroll families suffice.",
        "evidence_ids": [],
    },
}

ACCEPTANCE_OBLIGATIONS = [
    {
        "id": "acceptance.tutorial_progression",
        "stage": "candidate_behavior_and_source_fidelity",
        "scope": "All71 states, legitimate guards/rewards/recovery, actual dynamic text/values and source-defined UI "
                 "visibility/highlighting. Reuse the factored source layouts; do not demand71 independent source screenshots.",
        "proof_needed": "Authoritative event trace and candidate captures at distinct visual-family/signature changes; "
                        "test every state projection against the pinned source oracles.",
    },
    {
        "id": "acceptance.arrival_state",
        "stage": "candidate_behavior_and_source_fidelity",
        "scope": "Experience-specific departure possessions, arrival location, optional Adventure Paths and persistence.",
        "proof_needed": "Validate the retained source branch rules and resolve their explicitly provisional container policy. "
                        "Use the EXISTING recorded Lumbridge fixture camera for matched presentation tests; do not "
                        "mislabel it as an observed first-login/arrival camera or force both gameplay branches to that camera.",
    },
    {
        "id": "acceptance.dynamic_layout",
        "stage": "candidate_visual_fidelity",
        "scope": "Text length/wrapping, counters, item quantities, selection, disabled/hidden controls and scene entities.",
        "proof_needed": "Zero glyph/advance/value errors and full-panel source-checked coverage. A dynamic field is "
                        "parameterized, not exempted. Native script-resolved fields must be recorded before comparison.",
    },
    {
        "id": "acceptance.source_ambiguities",
        "stage": "candidate_source_fidelity",
        "scope": "Unrecorded transcript lines, optional tutorial control visibility and existing labeled behavior assumptions.",
        "proof_needed": "Retain uncertainty; do not invent lines, numeric progress or hidden controls. They are not new "
                        "visual-layout families or justification for a live-account prerequisite to agreeing the shared frames.",
    },
    {
        "id": "acceptance.audio_playback",
        "stage": "candidate_audio_fidelity",
        "scope": "Audible playback, exact bound event synchronization, gains, fades, loops, region changes, gestures and reconnect.",
        "proof_needed": "Compare against native PCM/event/calibration inputs once bound; test source-event phase and "
                        "browser scheduling/device latency separately. File decoding is not audible acceptance.",
    },
    {
        "id": "acceptance.mac_and_resize",
        "stage": "candidate_platform_performance",
        "scope": "Actual game resizing and M-series Mac Chrome/Edge visual/audio/performance.",
        "proof_needed": "Owner-run hardware/browser versions and measurements after implementation. Existing native "
                        "geometry/static anchors and gallery checks are agreement/development inputs, not fabricated Mac results.",
    },
    {
        "id": "acceptance.owner",
        "stage": "separate_owner_checkpoints",
        "scope": "Reference-pack approval followed later by candidate presentation acceptance.",
        "proof_needed": "Owner review of the completed hash-bound input pack, numeric policy, resize range, penguin/fitting "
                        "and web-only proposals. Neither ready_for_owner_review nor source-tool tests grant approval.",
    },
]


COMPARISON_FACTORIZATION = {
    "source_capture_count_rule": "One representative source image/component family may support many state projections. "
                                 "Every distinct layout, control mode, enabled/disabled/highlight signature and outcome "
                                 "remains explicitly required; data factoring does not truncate the71-state journey.",
    "lane_1_whole_reference_state": "At least one candidate fixture per distinct family/style state reproduces the "
                                   "actual representative source input's declared state and uses full-panel comparison. "
                                   "Do not invent a value or camera and call it the photographed source state.",
    "lane_2_dynamic_state_projection": "Keep the same frozen source frame/font/sprite/layout. Bind dialogue strings, "
                                       "quantities, choices, filters, statuses and unlock flags from pinned source "
                                       "transcripts/contracts. Compare glyph masks/advances, source styling and values separately.",
    "pixel_accounting": "100% of each unchanged panel is checked, never masked. A partition may use source pixels, "
                        "source background+native glyphs, source background+native item sprites or source widget primitives. "
                        "Every partition has actual input IDs; union is the complete panel and intersections are empty.",
    "dynamic_background": "A text/item rectangle includes its SOURCE background. Do not erase old lettering by cloning "
                          "nearby pixels or excluding an entire text box. If the appropriate background/native widget "
                          "geometry is unavailable, require the native-panel input rather than declaring that rectangle passed.",
    "scene_accounting": "Compare complete unchanged scene regions with existing original scene fixtures at their exact "
                        "recorded camera/lighting/texture settings. Unknown public cameras stay unknown; no whole "
                        "scenery/creature masks or source-frame relabeling.",
    "native_resolved_values": "Source widget files record static inputs, not every client-script result. Resolve dynamic "
                              "widget size/font/color/visibility from actual native fixture metadata before pixel acceptance; "
                              "do not choose a font/scale merely because it fits a candidate.",
    "numeric": {
        "unchecked_panel_pixel_fraction": 0,
        "overlapping_partition_pixel_fraction": 0,
        "text_codepoint_mismatches": 0,
        "glyph_advance_error_px": 0,
        "glyph_mask_different_pixels": 0,
        "dynamic_value_mismatches": 0,
        "source_colour_run_mismatches": 0,
        "strikethrough_flag_mismatches": 0,
        "missing_distinct_visual_variants": 0,
        "missing_tutorial_states": 0,
        "removed_or_hidden_required_controls": 0,
    },
}


def family_variant_set():
    return {f"{identifier}.{variant}" for identifier, _, variants, _, _ in FAMILY_DEFINITIONS
            for variant in variants.split()}


def stage_visual_map():
    result = {}
    for states, variants in STAGE_VISUALS:
        for state in states.split():
            if state in result:
                raise ValueError(f"Duplicate distinct-state binding: {state}")
            result[state] = variants.split()
    if any(set(variants) - family_variant_set() for variants in result.values()):
        raise ValueError("Unknown authored visual variant")
    return result


def text_selectors():
    result = {}
    for states, chapter, sections in STATE_TEXT_SECTIONS:
        for state in states.split():
            if state in result:
                raise ValueError(f"Duplicate text selector: {state}")
            result[state] = {"chapter_override": chapter, "sections": sections}
    return result


def review_ready(requirements):
    return bool(requirements) and all(row["status"] == "available" and row["evidence_ids"] for row in requirements)


def assess_source_requirements(originals, public, native_hud_inputs=()):
    requirements = [{"id": identifier, **definition} for identifier, definition in SOURCE_REVIEW_INPUTS.items()]
    by_id = {row["id"]: row for row in requirements}
    if native_hud_inputs:
        validate_native_hud_records(native_hud_inputs)
        row = by_id["input.classic_frame_calibration"]
        row["status"] = "available"
        row["evidence_ids"] = [entry["id"] for entry in native_hud_inputs]
        row["demonstrated_scope"] = "Original complete frame/panels, NPC dialogue231, source-font/native-coordinate "
        row["demonstrated_scope"] += "background calibration and6 actual attachment families; synthetic fixture "
        row["demonstrated_scope"] += "text is not source dialogue and these are not71 authenticated progression captures."
    source_map = json.loads((ROOT / "research/audio-source/source-map.json").read_text())
    actions = {row["journey_rule_id"]: row for row in source_map["actions"]}
    required = ("rule.combat.ranged", "rule.goblin.level_2", "rule.combat.tutorial_rat",
                "rule.food.healing", "rule.smelting.bronze")
    remaining = [
        identifier for identifier in required
        if not actions[identifier]["identified_sound_ids"]
        or not (actions[identifier]["source_frame_events"] or actions[identifier].get("source_binding_verified", False))
    ]
    by_id["input.required_effect_bindings"]["unbound_rule_ids"] = remaining
    if not remaining:
        row = by_id["input.required_effect_bindings"]
        row["status"] = "available"
        row["evidence_ids"] = list(required)
    return requirements


def make_factoring(cases, originals, public, pages, native_hud_inputs=()):
    text_oracles = make_text_oracles(pages)
    tutorial = json.loads((ROOT / "research/journey-rules/tutorial.json").read_text())
    initial = json.loads((ROOT / "research/journey-rules/initial-state.json").read_text())
    case_map = {case["id"]: case for case in cases}
    visual_map = stage_visual_map()
    selectors = text_selectors()
    expected = {state["id"].removeprefix("stage.tutorial.") for state in tutorial["states"]}
    if set(visual_map) != expected or set(selectors) != expected:
        raise ValueError(f"Unfactored tutorial states: {set(visual_map) ^ expected}")
    phases = []
    phase_by_state = {}
    phase_scenery = {
        "Customization": "tutorial_house", "Gielinor Guide": "tutorial_house",
        "Survival Expert": "tutorial_coast", "Master Chef": "kitchen", "Quest Guide": "quest_house",
        "Mining Instructor": "mine", "Combat Instructor": "combat_cave", "Banking tutorial": "bank",
        "Prayer Tutorial": "chapel", "Magic Instructor": "magic_area",
    }
    for index, (suffixes, section, titles, widgets, scene) in enumerate(TUTORIAL_GROUPS):
        identifier = f"phase.tutorial.{index:02}"
        states = ["stage.tutorial." + suffix for suffix in suffixes.split()]
        phase = {
            "id": identifier, "order": index, "source_section": section,
            "state_ids": states, "pixel_input_ids": [wiki(title) for title in titles],
            "widget_groups": widgets,
            "source_location_variant": "world.scenery." + phase_scenery[section],
            "scenery_input_id": f"original.scenes.tutorial-{scene}" if scene else None,
            "reuse_basis": "Instructor/entry phase shares actual source panel/portrait/scenery inputs; "
                          "its full semantic state list and all distinct visual variants remain.",
        }
        phases.append(phase)
        phase_by_state.update({state: phase for state in states})
    permanent = {"ui.settings", "ui.inventory", "ui.skills", "ui.quests", "ui.equipment",
                 "ui.combat", "ui.account", "ui.logout", "ui.prayer", "ui.magic"}
    signatures = {}
    bindings = []
    unlocked = set()
    for state in tutorial["states"]:
        suffix = state["id"].removeprefix("stage.tutorial.")
        controls = state.get("ui_unlock_refs", [])
        unlocked.update(set(controls) & permanent)
        signature_key = tuple(sorted(unlocked))
        signature_id = "hud.signature." + hashlib.sha256("|".join(signature_key).encode()).hexdigest()[:12]
        if signature_id not in signatures:
            signatures[signature_id] = {
                "id": signature_id, "expected_introduced_tabs": list(signature_key), "state_ids": [],
                "basis": "Named control introductions in the pinned current transcript/journey contract; "
                         "NOT observed numeric source progress or a declaration that every unmentioned control is hidden.",
                "unmentioned_controls": "unknown; never convert absence from this signature into hidden/disabled",
                "display_states_required": ["source_locked", "source_highlighted", "unlocked"],
                "pixel_family_id": "hud.classic",
            }
        signatures[signature_id]["state_ids"].append(state["id"])
        phase = phase_by_state[state["id"]]
        selector = selectors[suffix]
        chapter = selector["chapter_override"] or phase["source_section"]
        records = [
            record["id"] for record in text_oracles["records"]
            if record["source_page"] == "Transcript:Learning the Ropes" and record["section_path"]
            and record["section_path"][0] == chapter
            and any((len(record["section_path"]) == 1 if section is None else section in record["section_path"][1:])
                    for section in selector["sections"])
        ]
        if not records:
            raise ValueError(f"No independent source text for phase {phase['id']}")
        variant_ids = list(visual_map[suffix])
        if suffix != "mainland":
            variant_ids = sorted(set(variant_ids) | {phase["source_location_variant"]})
        selected_records = [record for record in text_oracles["records"] if record["id"] in records]
        if any(record["speaker"] == "Player" for record in selected_records):
            variant_ids = sorted(set(variant_ids) | {"dialogue.flow.player_speaker"})
        family_ids = sorted({variant.rsplit(".", 1)[0] for variant in variant_ids} | {"hud.classic"})
        binding = {
            "state_id": state["id"], "case_id": "case.tutorial." + suffix,
            "phase_id": phase["id"], "visual_variant_ids": variant_ids, "family_ids": family_ids,
            "hud_signature_id": signature_id, "declared_controls": controls,
            "source_text_record_ids": sorted(set(records)),
            "source_text_selector": {"chapter": chapter, "sections": selector["sections"]},
            "text_selection_rule": "Choose the exact record and permitted branch using the authoritative event/guard "
                                   "trace and source section/order. This phase corpus is NOT permission to display "
                                   "any arbitrary line from the instructor at every state.",
            "separate_source_screenshot_required": False,
            "distinct_layout_still_required": True,
        }
        bindings.append(binding)
        case = case_map[binding["case_id"]]
        case["reference_family_ids"] = family_ids
        case["distinct_visual_variant_ids"] = variant_ids
        case["phase_id"] = phase["id"]
        case["hud_signature_id"] = signature_id
        case["source_text_record_ids"] = binding["source_text_record_ids"]
        case["source_pixel_evidence"] = "shared_actual_family_inputs; no separate source-session capture claimed"
        case["evidence_status"] = "factored_reference_inputs"
        case["acceptance_obligation_ids"] = ["acceptance.tutorial_progression", "acceptance.dynamic_layout"]
        case["source_gap_refs"] = []
        case.pop("exact_stage_screenshot_available", None)
        case["comparison_procedure"][3] = (
            "Apply the factored full-panel pixel and dynamic-state lanes. Unknown public settings remain unknown; "
            "do not demand another source screenshot for data-only changes or mask those pixels."
        )
        if suffix == "mainland":
            case["acceptance_obligation_ids"].append("acceptance.arrival_state")
    all_visual_ids = set(family_variant_set())
    families = []
    for identifier, title, variants, direct_cases, widgets in FAMILY_DEFINITIONS:
        direct = {"case." + suffix for suffix in direct_cases.split()}
        related = direct | {binding["case_id"] for binding in bindings if identifier in binding["family_ids"]}
        inputs = set()
        for case_id in related:
            inputs.update(case_map[case_id]["input_ids"])
            case = case_map[case_id]
            case.setdefault("reference_family_ids", [])
            if identifier not in case["reference_family_ids"]:
                case["reference_family_ids"].append(identifier)
            case["reference_family_ids"].sort()
        if identifier == "world.activities":
            inputs.update(entry["id"] for entry in originals if entry["kind"] == "original-runtime-model")
        if not inputs:
            raise ValueError(f"Empty required source family: {identifier}")
        families.append({
            "id": identifier, "title": title, "required_visual_variants": variants.split(),
            "case_ids": sorted(related), "pixel_or_audio_input_ids": sorted(inputs),
            "representative_input_ids": list(dict.fromkeys(
                input_id for suffix in direct_cases.split()
                for input_id in case_map["case." + suffix]["input_ids"]
            )) if direct else sorted(input_id for input_id in inputs if not input_id.startswith("original.items.")),
            "current_widget_inputs": [digest(SOURCE / f"interfaces/{group}.json.gz") for group in widgets],
            "state_evidence": "Source pixels + source widget/assets + pinned dynamic text/value/branch oracles. "
                              "Reusable input does not imply every state was separately photographed.",
            "comparison_lanes": ["whole_representative_source_state", "full_coverage_dynamic_state_projection"],
            "required_panel_pixel_coverage": 1.0,
            "candidate_panel_pixel_coverage_measured": None,
        })
    for case in cases:
        case.setdefault("acceptance_obligation_ids", ["acceptance.dynamic_layout"])
        case["source_gap_refs"] = []
        if case["id"] == "case.world.lumbridge_arrival":
            case["acceptance_obligation_ids"].append("acceptance.arrival_state")
        if case["family"] == "audio":
            case["acceptance_obligation_ids"] = ["acceptance.audio_playback"]
    requirements = assess_source_requirements(originals, public, native_hud_inputs)
    for row in requirements:
        for case_id in row["case_ids"]:
            if row["status"] != "available":
                case_map[case_id]["source_gap_refs"].append(row["id"])
    for family in families:
        family["missing_input_requirement_ids"] = [row["id"] for row in requirements
                                                  if family["id"] in row["family_ids"] and row["status"] != "available"]
        family["agreement_input_status"] = ("awaiting_named_source_input" if family["missing_input_requirement_ids"]
                                           else "factored_inputs_available")
    values = {
        "source_contracts": [digest(ROOT / f"research/journey-rules/{name}.json") for name in
                             ("initial-state", "tutorial", "activities", "cooks-assistant")],
        "source_facts": {
            "skill_count": len(initial["skills"]), "inventory_slots": 28,
            "tutorial_bank_first_open_coins": 25, "initial_ranged_arrow_grant": 50,
            "initial_magic_grant_air": 5, "initial_magic_grant_mind": 5,
            "learning_the_ropes_quest_points": 1, "cooks_assistant_quest_points": 1,
            "cooks_assistant_cooking_xp_tenths": 3000,
            "poll_vote_skill_total": tutorial["dialogue_fact_parameters"]["poll"]["skill_total_required_to_vote"],
            "poll_support_percent": tutorial["dialogue_fact_parameters"]["poll"]["support_percent"],
        },
        "application": "Facts describe source grant/seed/reward definitions, not unconditional inventory assertions "
                       "at every state. Apply the pinned eligibility, partial-grant, loss, consumption and quest guards. "
                       "Current dynamic values come from independently checked authoritative transitions.",
        "quest_journal_distinct_states": {
            "statuses": ["not_started", "in_progress", "completed"],
            "ingredients": ["milk", "flour", "egg"],
            "per_ingredient_states": ["missing", "carried", "delivered"],
            "source": "Transcript:Cook's Assistant/Journal",
            "delivered_style": "source strikethrough; not omission of the ingredient row",
        },
        "arrival": {
            "reference_camera": "original.scenes.lumbridge-castle-plaza",
            "camera_role": "actual recorded controlled fixture; not a claimed initial-player camera",
            "experience_branches": initial["experience_branches"],
            "container_policy": "Keep existing provisional departure policy explicit until source-behavior validation. "
                                "It does not require another visual-layout family or fabricate a captured inventory.",
        },
        "optional_controls": tutorial["optional_controls"],
    }
    return {
        "schema_version": 1, "scope": "literal_30_2_reference_agreement_not_30_3_acceptance",
        "families": families, "phases": phases, "state_bindings": bindings,
        "hud_signatures": list(signatures.values()), "dynamic_value_oracles": values,
        "source_review_requirements": requirements,
        "acceptance_obligations": ACCEPTANCE_OBLIGATIONS,
        "comparison_factorization": COMPARISON_FACTORIZATION,
        "counts": {"families": len(families), "phases": len(phases), "tutorial_states": len(bindings),
                   "hud_signatures": len(signatures), "distinct_visual_variants": len(all_visual_ids)},
        "literal_audit": [
            {"old_predicate": "One matched authenticated/full screenshot for every one of71 semantic states.",
             "decision": "removed_invented_prerequisite",
             "literal_basis": "30.2 asks for Tutorial Island stages and interfaces; it does not specify71 separately "
                              "photographed micro-transitions. 30.3 separately requires complete legitimate gameplay evidence.",
             "replacement": "All71 bindings + all instructor phases + required distinct family variants + progressive "
                            "HUD signatures + independently pinned text/value/state oracles. No whole-panel masks."},
            {"old_predicate": "Captured initial/arrival camera and complete branch-specific container dump before agreeing references.",
             "decision": "moved_state_validation_not_camera_fabrication",
             "literal_basis": "Existing controlled Lumbridge source captures already provide a recorded comparison camera "
                              "and scene. Literal30.2 does not demand an authenticated screenshot for each arrival branch.",
             "replacement": "Keep source branches and uncertainty; compare presentation at recorded fixture settings and "
                            "validate legitimate branch state with the candidate. Never claim that fixture is the live initial camera."},
            {"old_predicate": "A missing source-relative sound selector can be inferred from whichever candidate sounds plausible.",
             "decision": "rejected_source_input_deficit_remains",
             "literal_basis": "30.2 expressly names sound IDs, source-defined triggers and timing.",
             "replacement": "Keep the narrow unbound cue/selector list; move ONLY scheduling/device/performance "
                            "verification to candidate acceptance. Known native frame events do not require another live session."},
            {"old_predicate": "Always reject ready_for_owner_review, even when every required input is later available.",
             "decision": "removed_hardcoded_false_gate",
             "literal_basis": "Reference readiness and owner approval are separate bounded checkpoints.",
             "replacement": "Compute readiness from actual source-review requirement satisfaction; leave every approval flag false."},
        ],
    }, text_oracles
