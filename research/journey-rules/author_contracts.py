#!/usr/bin/env python3
"""Author factual source contracts, not executable game content or acceptance results."""

import hashlib
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parent


def write(name, value):
    (ROOT / name).write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n")


def basis(*sources, classification="verified_reference", assumptions=()):
    result = {
        "classification": classification,
        "source_refs": [s if s.startswith("source.") else f"source.wiki.{s}" for s in sources],
    }
    if assumptions:
        result["assumption_refs"] = [f"assumption.{a}" for a in assumptions]
    return result


def pred(path, op, value):
    return {"path": path, "op": op, "value": value}


def eq(path, value):
    return pred(path, "eq", value)


def ge(path, value):
    return pred(path, "gte", value)


def one_of(path, values):
    return pred(path, "in", values)


def all_of(*conditions):
    return {"all": list(conditions)}


def any_of(*conditions):
    return {"any": list(conditions)}


def effect(op, **fields):
    return {"op": op, **fields}


def set_value(path, value):
    return effect("set", path=path, value=value)


def item(item_id, quantity=1):
    return {"item_ref": "item." + item_id, "quantity": quantity}


def local_sources():
    manifest = json.loads((ROOT / "sources.json").read_text())
    existing = {s["id"] for s in manifest["sources"]}
    for name, relative in (("prompt", "../../prompt.md"), ("milestone", "../../spec/milestone-01.md")):
        if f"source.project.{name}" not in existing:
            data = (ROOT / relative).read_bytes()
            manifest["sources"].append({
                "id": f"source.project.{name}",
                "kind": "repository_file",
                "path": relative,
                "revision": "cab0699",
                "retrieved_at": manifest["retrieved_at"],
                "sha256": hashlib.sha256(data).hexdigest(),
                "bytes": len(data),
                "role": "Project requirements, not evidence of OSRS execution.",
            })
    write("sources.json", manifest)


def vocabulary():
    events = {
        "appearance.confirmed": ["appearance"],
        "experience.selected": ["choice"],
        "dialogue.completed": ["npc_ref", "topic"],
        "ui.opened": ["ui_ref"],
        "ui.closed": ["ui_ref"],
        "world.transitioned": ["from_ref", "to_ref", "link_ref"],
        "gather.succeeded": ["rule_ref", "item_ref", "quantity"],
        "fire.lit": ["rule_ref", "object_ref", "position"],
        "cook.resolved": ["rule_ref", "input_item_ref", "output_item_ref", "outcome"],
        "craft.succeeded": ["rule_ref", "output_item_ref", "quantity"],
        "equipment.changed": ["slot_ref", "item_ref"],
        "combat.kill_credited": ["npc_ref", "method", "kill_instance"],
        "spell.resolved": ["spell_ref", "npc_ref", "outcome"],
        "teleport.started": ["spell_ref"],
        "teleport.interrupted": ["spell_ref", "reason"],
        "teleport.completed": ["spell_ref", "destination_ref"],
        "setting.changed": ["setting", "value"],
        "object.inspected": ["object_ref"],
        "quest.accepted": ["quest_ref", "npc_ref"],
        "quest.declined": ["quest_ref", "npc_ref"],
        "quest.delivered": ["quest_ref", "npc_ref", "delivered"],
        "quest.reward_requested": ["quest_ref"],
        "player.died": ["death_instance", "items_lost"],
        "death.topic_completed": ["topic"],
        "death.exit_requested": [],
        "death.portal_crossed": [],
        "grave.reclaimed": ["death_instance", "items"],
        "grave.expired": ["death_instance"],
        "inventory.pickup_requested": ["ground_instance", "quantity"],
        "inventory.drop_requested": ["item_ref", "quantity"],
        "equipment.equip_requested": ["item_ref"],
        "equipment.unequip_requested": ["slot_ref"],
        "movement.requested": ["destination"],
        "activity.requested": ["rule_ref", "target"],
        "activity.roll": ["rule_ref", "rng"],
        "bank.deposit_requested": ["item_ref", "quantity"],
        "bank.withdraw_requested": ["item_ref", "quantity", "noted"],
        "shop.buy_requested": ["item_ref", "quantity", "stock_version"],
        "shop.sell_requested": ["item_ref", "quantity", "stock_version"],
        "prayer.toggled": ["prayer_ref", "enabled"],
        "altar.prayed": ["object_ref"],
        "food.eaten": ["item_ref"],
        "mill.hopper_filled": ["item_ref"],
        "mill.controls_operated": ["object_ref"],
        "mill.flour_collected": ["object_ref"],
        "session.disconnected": [],
        "session.reconnected": [],
        "server.restarted": [],
    }
    uis = ["appearance", "experience", "settings", "inventory", "skills", "quests",
           "equipment", "equipment_stats", "combat", "bank", "poll", "account",
           "logout", "account_links", "prayer", "magic", "smithing", "cooking",
           "shop", "quest_reward", "items_kept_on_death", "grave", "death_retrieval",
           "music", "emotes", "adventure_paths"]
    places = {
        "tutorial.start_house": ("Starting house", "learning_the_ropes"),
        "tutorial.survival": ("Survival Expert pond and trees", "learning_the_ropes"),
        "tutorial.kitchen": ("Master Chef kitchen", "learning_the_ropes"),
        "tutorial.quest_house": ("Quest Guide house", "learning_the_ropes"),
        "tutorial.mine": ("Mining section of Tutorial Island dungeon", "learning_the_ropes"),
        "tutorial.rat_cave": ("Combat section, including distinct inside/outside pen", "tutorial_rat"),
        "tutorial.bank": ("Tutorial Island bank booth and poll booth", "learning_the_ropes"),
        "tutorial.account_room": ("Account Guide room east of bank", "learning_the_ropes"),
        "tutorial.chapel": ("Brother Brace chapel", "learning_the_ropes"),
        "tutorial.magic_house": ("Magic Instructor house and chicken pen", "learning_the_ropes"),
        "lumbridge.castle_arrival": ("Castle-area arrival, not an exact spawn tile", "tutorial_island"),
        "lumbridge.jon_arrival": ("Outside Sheared Ram, near Adventurer Jon", "tutorial_island"),
        "lumbridge.kitchen": ("Castle ground-floor kitchen", "cooks_assistant"),
        "lumbridge.cellar": ("Castle cellar reached through kitchen trapdoor", "cooks_assistant"),
        "lumbridge.bank": ("Castle second floor UK / third floor US", "lumbridge_castle"),
        "lumbridge.general_store": ("General store north of castle", "lumbridge_general_store"),
        "lumbridge.east_swamp_mine": ("East Lumbridge Swamp mine", "copper_rocks"),
        "lumbridge.west_coop": ("Fred's chicken coop, north-west of castle", "cooks_assistant"),
        "lumbridge.north_cows": ("Dairy cow field north of Fred's coop", "cooks_assistant"),
        "lumbridge.east_cows": ("Dairy cow field across River Lum, near Gillie", "cooks_transcript"),
        "lumbridge.wheat": ("Wheat field near Mill Lane Mill", "cooks_assistant"),
        "lumbridge.mill_ground": ("Mill Lane Mill flour bin floor", "mill_lane_mill"),
        "lumbridge.mill_middle": ("Mill Lane Mill millstone floor", "mill_lane_mill"),
        "lumbridge.mill_top": ("Mill Lane Mill hopper and controls floor", "mill_lane_mill"),
        "lumbridge.east_bridge_goblins": ("Level-2 goblins east of River Lum", "goblin"),
        "lumbridge.death_entrance": ("Death's Office entrance in church graveyard", "deaths_office"),
        "death.office": ("Death's Office; separate map, not relocated into Lumbridge", "deaths_office"),
    }
    npcs = ["gielinor_guide", "survival_expert", "master_chef", "quest_guide",
            "mining_instructor", "combat_instructor", "account_guide", "brother_brace",
            "magic_instructor", "ironman_tutor", "adventurer_jon", "lumbridge_guide",
            "cook", "death", "millie_miller", "gillie_groats", "shopkeeper",
            "tutorial_rat", "tutorial_chicken", "goblin.level_2", "dairy_cow"]
    objects = ["tutorial.start_door", "tutorial.survival_gate", "tutorial.chef_entry",
               "tutorial.chef_exit", "tutorial.quest_entry", "tutorial.quest_ladder",
               "tutorial.mine_gate", "tutorial.rat_gate", "tutorial.combat_ladder",
               "tutorial.bank_booth", "tutorial.poll_booth", "tutorial.account_entry",
               "tutorial.account_exit", "tutorial.chapel_entry", "tutorial.chapel_exit",
               "tutorial.magic_entry", "tutorial.fishing_spot", "tree.normal", "fire.normal",
               "range.tutorial", "range.lumbridge", "furnace.tutorial", "anvil.tutorial",
               "rock.copper", "rock.tin", "altar.tutorial", "altar.lumbridge",
               "mill.hopper", "mill.controls", "mill.flour_bin", "mill.ladder_lower",
               "mill.ladder_upper", "lumbridge.kitchen_trapdoor", "lumbridge.castle_stairs",
               "lumbridge.bank_booth", "lumbridge.bridge", "death.portal", "water_source",
               "wheat", "egg_spawn", "pot_spawn", "bucket_spawn"]
    write("vocabulary.json", {
        "schema_version": 1, "id": "contract.journey.vocabulary",
        "scope": "Semantic IDs only; external content extraction must bind each source variant and map link.",
        "events": [{"id": "event." + k, "required_fields": v} for k, v in events.items()],
        "event_authority": {
            "requests": "Client intents require ownership, reachability, UI unlock, stage and capacity checks.",
            "results": "Only shared authoritative systems emit succeeded/resolved/changed/credited events after their real operation commits.",
            "identity": "Every event has actor_id, durable operation_id, session_epoch and server_tick. A received result name is never a permission to mint items or XP.",
            "xp": "Tutorial and quest listeners must not repeat XP already awarded by an activity.",
            "basis": basis("source.project.prompt", classification="engineering_contract"),
        },
        "guard_language": {
            "all": "Every nested predicate must match.", "any": "At least one must match.",
            "predicate": {"path": "state.* or event.*; dotted scalar lookup", "op": ["eq", "ne", "gte", "lte", "in"], "value": "JSON literal"},
            "phase": "Guards inspect authoritative state after the input event's shared-system mutation, before transition effects.",
            "missing_path": "Unknown/missing does not satisfy a predicate, including ne.",
            "unmatched_event": "No stage change; may still be a legitimate shared action if independently unlocked.",
            "order": "At most one transition per graph per operation. Never chain through a second stage on one event.",
        },
        "guard_projection": {
            "state.levels.<skill>": "Current usable level from the named skill record; base level from XP is used specifically for tutorial caps.",
            "state.inventory.free_slots": "28 minus occupied backpack slots after resolving compatible stacks.",
            "inventory_count_aliases": {
                "tin_ore": "item.ore.tin", "copper_ore": "item.ore.copper",
                "bronze_bar": "item.bar.bronze", "hammer": "item.hammer",
                "flour": "item.flour.pot", "water_bucket": "item.water.bucket",
                "empty_bucket": "item.bucket", "empty_pot": "item.pot", "grain": "item.grain",
                "air_runes": "item.rune.air", "mind_runes": "item.rune.mind",
            },
            "inventory_alias_scope": "state.inventory.<alias> counts actual carried unnoted units, not banked/equipped/ground items.",
            "state.tools.usable_pickaxe": "A source-level-eligible pickaxe in inventory OR weapon slot.",
            "state.tools.usable_axe": "A source-level-eligible axe in inventory OR weapon slot.",
            "state.tools.small_net_in_inventory": "At least one actual item.fishing_net.small in inventory.",
            "state.tools.tinderbox_in_inventory": "At least one actual item.tinderbox in inventory.",
            "state.equipment.<slot>": "Item semantic ID occupying the named slot, or null; ammo_quantity is the equipped stack count.",
            "state.quests.<quest_name>": "Persisted record indexed by quest.<quest_name>.",
            "state.rewards.<quest_name>": "Whether once-only entitlement quest.<quest_name> exists in the durable reward ledger.",
            "state.location": "mill_ground/mill_middle/mill_top are projections of the three mapped Mill Lane Mill location IDs, not player assertions.",
            "event.lesson_action_unlocked": "Authoritative action_unlocks plus tutorial restriction predicates in tutorial.json, never a client boolean.",
            "event.reachable_and_other_validation_facts": "Computed by shared authority from source geometry, items, recipes, stock and clocks. Fixture facts test these boundaries but are not protocol fields a player can assert.",
            "completed_transition_set": "Persist every traversed graph transition ID atomically; used for sticky access permissions and reconstruction, never client-supplied.",
        },
        "effect_operations": {
            "set": "Set a durable field in the same transaction as the triggering event.",
            "unlock": "Enable a real UI/action; does not perform that action.",
            "grant": "Apply referenced grant's capacity/ownership policy atomically.",
            "reward": "Claim referenced per-character entitlement once, atomically with its quest state.",
            "mark": "Persist evidence of a real authoritative action, not a client checkbox.",
            "reconcile_departure": "Apply the explicitly provisional departure rule, not a generic tutorial skip.",
            "project_journal": "Recompute text state from quest bits and current held items.",
        },
        "uis": [{"id": "ui." + k} for k in uis],
        "slots": [{"id": "slot." + k} for k in ["head", "cape", "neck", "weapon", "body", "shield", "legs", "hands", "feet", "ring", "ammo"]],
        "locations": [{"id": "location." + k, "description": v[0], "position": None,
                       "binding_status": "external_source_map_required", "basis": basis(v[1])}
                      for k, v in places.items()],
        "npcs": [{"id": "npc." + k, "source_numeric_id": None, "binding_status": "external_source_variant_required"} for k in npcs],
        "objects": [{"id": "object." + k, "source_numeric_id": None, "binding_status": "external_source_placement_required"} for k in objects],
        "spells": [{"id": "spell.wind_strike"}, {"id": "spell.lumbridge_home_teleport"}],
        "prayers": [{"id": "prayer.thick_skin"}],
        "experience_choices": [{"id": "choice.experience." + k} for k in ["brand_new", "returning", "experienced"]],
    })


def decisions():
    entries = [
        ("fresh_containers", False, "Pre-dialogue inventory/equipment are empty; seed the 25-coin bank entitlement exactly once before its first visible opening. Clothing is appearance, not equipment. Use the documented base 400 bank spaces provisionally; exact fresh-account identity-provider/security bonus mapping is not observed.",
         "The transcript calls the net the first given item; the quest guide explicitly shows 25 bank coins at first opening, but neither is a fresh-account dump. Lazy versus creation-time bank seeding is unobservable before access.",
         ["tutorial_transcript", "learning_the_ropes"], "Replace defaults from an authorized fresh-account state capture; keep already acknowledged ownership changes."),
        ("tutorial_xp_boundary", False, "If base level is already 3, award zero tutorial XP. Otherwise award normal tenths, capped below the level-4 threshold (2759 tenths). HP awards are always zero on the island.",
         "Sources specify level 3, not an exact XP total. This deliberately does not invent a hard cap of 174.0 XP; a final ordinary action can pass 174.0 without reaching level 4.",
         ["tutorial_island", "experience", "copper_rocks"], "Measure boundary actions; change this policy without changing prior earned XP or tutorial stage."),
        ("missing_tutorial_tools", False, "The owning instructor returns one missing net/axe/tinderbox/pickaxe/dagger as specified in recovery handlers, only after its original lesson unlocked it and while still on Tutorial Island.",
         "Hammer, Chef ingredients, sword/shield, bow/arrows and rune replacements are explicit. Other lost-tool branches are not in the transcript. Prefer re-gathering/re-smithing a lost dagger; instructor replacement is the last recoverability fallback.",
         ["tutorial_transcript", "tutorial_island"], "Replace only missing-tool handlers after a source capture; never blanket-grant a starter kit to advance a stage."),
        ("multi_item_grant_capacity", False, "Undocumented multi-item instructor grants are all-or-none; explicit sword/shield and bow/arrows grants remain sequential and partial.",
         "Transcript explicitly permits a sword or bow alone in one free slot. It does not describe every survival/Chef/rune overflow branch.",
         ["tutorial_transcript", "inventory"], "Observe individual overflow cases. Preserve the explicit partial branches regardless."),
        ("departure_reconciliation", True, "On successful authorized Home Teleport, replace tutorial-carried/equipped possessions with the 18-kind standard kit in inventory, clear tutorial bank contents, then set bank coins to 25. Preserve skills, quest point and completed tutorial flags. Commit once.",
         "Sources say gathered items are lost and list standard provisions, but do not prove equipment placement, bank-deposit retention, withdrawn coins, duplicate tools or exact reconciliation. This is a reversible provisional normalization, NOT an observed inventory dump.",
         ["learning_the_ropes", "tutorial_island"], "Capture departure after deposits/drops/withdrawal and with equipped bow; swap reconciliation policy and migrate tutorial-owned provenance, not ordinary mainland possessions."),
        ("departure_missing_dialogue", False, "Keep the documented mainland confirmation and normal-account/Ironman-information choices; an omitted brand-new dialogue span is explanatory, not a grant or skill gate.",
         "The current transcript explicitly marks missing text, but guide and remaining transcript agree on Home Teleport and arrival. No full dialogue reproduction is claimed.",
         ["tutorial_transcript", "learning_the_ropes"], "Replace the paraphrased missing span from approved reference dialogue; retain actual spell completion."),
        ("quest_tracking_start", False, "Track onboarding internally from appearance; Learning the Ropes journal status changes to in_progress on first Quest Guide dialogue, and to completed on the first valid chicken Wind Strike resolution.",
         "Quest details name the Quest Guide as start. Exact hidden varp/journal initialization is not provided; the conversion to a tracked quest and the completion condition are explicit.",
         ["learning_the_ropes", "tutorial_transcript"], "Bind hidden progress/journal values when extracted or captured; do not use remembered historical varp numbers."),
        ("bread_first_attempt", False, "Use the ordinary shared range-cooking roll. A resolved bread attempt may advance, including a burn; never boost success secretly.",
         "The current transcript shows first bread success and later burned bread allowing exit. The older guide says bread cannot burn before an obsolete music step. Whether current first-bread success is guaranteed remains unverified.",
         ["tutorial_transcript", "tutorial_island", "bread"], "If a current source demonstrates a first-attempt guarantee, add that explicit source-defined override and change only its fixture."),
        ("dough_containers", False, "Mixing flour and water leaves an empty pot and bucket plus dough: net +1 inventory slot. Reject before mutation if that extra slot cannot fit.",
         "Recipe pages list the dough output but omit container byproducts. Returning these containers is an inference from ordinary Cooking container behavior, not a verified recipe-output capture.",
         ["bread_dough", "pot_of_flour", "pot", "bucket"], "Capture mixing with 27 and 28 slots used; adjust byproducts without changing the flour/water ingredient requirements."),
        ("run_rounding", False, "Store run energy in 0..10000 units. Apply the published post-2025 drain expression then floor its final result once; regenerate floor(Agility/10)+15 units on eligible non-running ticks.",
         "Energy page is under construction and flags exact drain rounding/tick boundaries as needing confirmation. The old pre-2025 drain formula is not acceptable.",
         ["energy"], "Resolve rounding from a dated controlled source run; keep weight and agility inputs explicit."),
        ("fire_success", False, "Use the published crowdsourced skilling parameters low=64, high=512 for normal logs, with the shared success function. No guaranteed instant lighting.",
         "Firemaking explicitly describes this as a belief and carries a citation-needed warning. It is not labeled verified here.",
         ["firemaking", "skilling_success_rate"], "Replace parameters with source-server evidence; the rest of the fire lifecycle and 40 XP remain independently usable."),
        ("environment_timers", False, "Use normal-tree respawn 60..100 ticks and fire lifetime 100..199 ticks, uniformly. Firelighting retries consume 4 ticks. First gathering-roll phase is one full cycle after action acceptance.",
         "Tree prose cites 36-60 seconds but its infobox differs. Fire developer quote gives 60-119 seconds, without a tick distribution. Action-length table gives steady cadence, not every initial/retry phase. These chosen integer schedules are inferences, not measured boundaries.",
         ["tree", "fire", "action_lengths"], "Bind captured initial/retry phases and distributions; do not change XP or guaranteed tree depletion."),
        ("negative_combat_roll", False, "Clamp maximum accuracy/defence rolls to at least zero before inclusive random rolls. This affects Tutorial Island rats' -100 defence bonuses.",
         "Rat stats explicitly contain -100 defence, but the wiki's simplified positive-roll hit-chance formula cannot directly handle negative maxima. The clamp is a labeled implementation assumption.",
         ["tutorial_rat", "melee_dps"], "Resolve negative-roll handling from a trusted current combat calculation/source test; positive-roll vectors remain valid."),
        ("combat_xp_thirds", False, "For the published 1.33-XP-per-damage terms, use floor(40*credited_damage/3) tenths per hit. Cap credited damage to HP actually removed. Do not round each point separately.",
         "Combat Options publishes 1.33 and Experience documents tenths/flooring, but they do not distinguish exact 4/3 from decimal 1.33 at three damage or every overkill case.",
         ["combat_options", "combat_spells", "experience"], "Capture 1/2/3-damage and overkill XP. Retain exact fixed-point storage and source XP family selection."),
        ("shop_overstock", False, "Use the Shop article's linear stock-delta pricing for overstock as well as understock, with explicit bounds; use integer base item values, not rounded displayed prices as inputs.",
         "The wiki calculator's overstock buy branch divides instead of subtracting and can disagree with the Shop article's worked example. Understock/default starter prices do not require resolving this conflict.",
         ["shop", "shop_module", "lumbridge_general_store"], "Capture a known value>=100 item at surplus stock; replace only overstock pricing. Do not report it as verified."),
        ("goblin_loot_variant", True, "Choose the ordinary unarmed level-2 Lumbridge goblin, primary 128-weight table 1. In the F2P profile, member-only table results become no primary item; tertiary rolls stay separately represented. The newly listed common energy-potion drops have no usable weights and remain an explicit unresolved supplement, not a made-up probability.",
         "Current table's numeric weights sum to 128 before three unweighted energy-potion rows. Some level-2 variants use table 2. Exact selected spawn/table and conditional potion logic must be bound; guaranteed bones and listed weights are useful independently.",
         ["goblin"], "Verify chosen source spawn and the potion-drop condition. Never substitute a guaranteed coin reward or pretend bones-only is the full drop table."),
        ("death_tie_break", False, "For equal source-provided per-unit kept-on-death values, use original carried/equipment slot order as a stable tie-breaker; protect individual units, not an entire arbitrary stack.",
         "The source defines three valuable items and value ranking, not every equal-value stack/slot tie. The fixture values are synthetic reference inputs, not GE observations.",
         ["items_kept_on_death", "stackable_items"], "Bind the exact tie/stack behavior from Items Kept on Death source output. Do not use total stack price as a shortcut."),
        ("mill_ownership_overflow", False, "Track the ordinary mill hopper and shared-across-windmills flour count per character. Reject grain loading/processing when it would exceed the 30-flour capacity; do not lose grain silently.",
         "The flour source states sharing between mills, not players. Hopper capacity one is explicit; account ownership, overflow and disconnect timing are not fully documented.",
         ["mill_lane_mill", "flour_bin", "hopper_transcript"], "Capture two-character ownership and full-bin controls. Keep the three-floor route and material conservation."),
        ("journal_inventory_scope", False, "Journal 'held' means currently carried, unnoted ingredients; delivered takes precedence. Banked/ground/lost items do not satisfy turn-in guards.",
         "Journal differentiates found and delivered, but does not explicitly describe its bank visibility or what it displays after losing a previously found ingredient. Actual turn-ins must own usable items.",
         ["cooks_journal", "cooks_transcript"], "Adjust only journal projection if source uses broader ownership; never allow remote bank consumption."),
        ("initial_ui_defaults", False, "Run starts off, Logout remains usable, ordinary music/emote controls are not mandatory progress tasks. Other lesson tabs unlock at their documented step.",
         "No fresh-account UI/settings dump exists. The current detailed transcript lacks old music/emote/friends lessons; treating their absence as a modern variant choice is stronger than reintroducing obsolete mandatory tasks.",
         ["tutorial_transcript", "tutorial_island", "energy"], "Bind actual default settings and optional-control visibility to current client captures; do not remove required documented unlocks."),
    ]
    write("decisions.json", {
        "schema_version": 1, "id": "contract.journey.decisions",
        "profile": {
            "id": "profile.osrs.current_normal_f2p",
            "selected": "Post-2026-07 normal F2P journey; reference facts frozen by sources.json, not by a client jar's age.",
            "historical_identity": "osrs-240-cache-2695 from first-slice-sources.json is historical, not a retrieval prerequisite.",
            "runtime_binding": None,
            "runtime_binding_owner": "Independent source-runtime/cache/asset worker and Director",
            "required_features": ["24 skills including Sailing", "Learning the Ropes tracking and 1 QP", "post-2025 run energy", "post-2026-01 tutorial combat cap", "2026-07 grave inventory restoration"],
            "material_choice": "Prefer a matching current runtime/cache. If only an older client is usable, keep these behavior contracts and explicitly record missing client UI/data as a version mismatch; an old client does not silently authorize historical tutorial mechanics or 23 skills.",
            "basis": basis("skills", "learning_the_ropes", "tutorial_island", "grave", "energy"),
        },
        "evidence_classes": {
            "verified_reference": "Explicit fact in a retrieved, pinned source; NOT an observed live-server result and NOT automatic wiki authority.",
            "inference": "A stated derivation/translation of source facts; not verified behavior.",
            "assumption": "A reversible provisional choice needed to make a bounded contract actionable.",
            "engineering_contract": "Project-required authority/persistence/transaction behavior, not a claim about OSRS internals.",
            "adaptation": "A deliberate departure requiring the relevant approval. None are approved by these documents.",
        },
        "variant_resolutions": [
            {"id": "decision.tutorial.modern_flow", "choice": "Current Learning the Ropes transcript, not remembered pre-quest Tutorial Island.",
             "basis": basis("learning_the_ropes", "tutorial_transcript", "tutorial_island", classification="inference"),
             "deltas": ["experience has its own screen", "quest tracker + 1 QP", "run instruction retained", "no mandatory music/emote/friends/ignore lesson in current transcript", "burnt shrimp advances", "bank precedes poll", "5 initial rune pairs, 50 initial arrows versus standardized departure kit", "chicken cast completes quest before authorized Home Teleport"]},
            {"id": "decision.mining.ore_order", "choice": "Tin is the prompted first ore; either order can satisfy possession of both after real mining.",
             "basis": basis("tutorial_transcript", classification="inference"),
             "reason": "Mining messages substitute tin/copper symmetrically. Never grant the second ore or assume clicking a rock succeeded."},
            {"id": "decision.magic.cast_not_kill", "choice": "A valid resolved Wind Strike on the tutorial chicken is required; a chicken kill is not. A splash counts provisionally.",
             "basis": basis("tutorial_transcript", "learning_the_ropes", "combat_spells", classification="inference"),
             "reason": "Transcript says after casting, not after damaging/killing. Splash completion is inferred, not independently captured."},
            {"id": "decision.cooks.no_coin_reward", "choice": "1 QP + 300 Cooking XP + range permission only; no invented money, ingredients or cake-making step.",
             "basis": basis("cooks_assistant", "cooks_transcript", "cooks_journal")},
            {"id": "decision.death.first_item_loss", "choice": "First item-losing death, not necessarily first zero-HP event, triggers the forced Death tutorial. All three optional-looking topics must be covered before exit.",
             "basis": basis("death_dialogue", "grave")},
        ],
        "assumptions": [{"id": "assumption." + key, "critical_for_exact_fidelity": critical,
                         "chosen_policy": policy, "reason": reason,
                         "basis": basis(*sources, classification="assumption"),
                         "reversal": reversal, "approval": "not_owner_approved; not an adaptation approval"}
                        for key, critical, policy, reason, sources, reversal in entries],
        "external_bindings": [
            {"id": "binding.world.source_layout", "required": "Actual reachable tiles, floors, collision/LOS flags, gates/ladders/stairs, region closure and initial/arrival cameras; wiki markers are not certified placements.", "value": None},
            {"id": "binding.tutorial.source_progress", "required": "Numeric tutorial progress/varbits and interface IDs if source client wiring needs them. Semantic stages do not depend on guessed numbers.", "value": None},
            {"id": "binding.combat.projectile_timing", "required": "Current attack-animation launch/hit/projectile travel offsets and range/LOS mapping; do not replace with zero-delay hits.", "value": None},
            {"id": "binding.death.valuation", "required": "Pinned effective per-unit GE/alchemy values for the chosen content. Fixture values are inputs, not a current market snapshot.", "value": None},
        ],
        "acceptance": {
            "implementation_executed": False, "journey_executed": False,
            "presentation_approved": False, "milestone_accepted": False,
            "note": "This task supplies bounded source contracts and validates their internal consistency only. Separate Section 30 gates remain.",
        },
    })


def initial_state():
    skills = ["attack", "strength", "defence", "ranged", "prayer", "magic", "runecraft",
              "construction", "hitpoints", "agility", "herblore", "thieving", "crafting",
              "fletching", "slayer", "hunter", "mining", "smithing", "fishing", "cooking",
              "firemaking", "woodcutting", "farming", "sailing"]
    write("initial-state.json", {
        "schema_version": 1, "id": "contract.journey.initial_state",
        "profile_ref": "profile.osrs.current_normal_f2p",
        "account": {"mode": "normal", "membership": False, "appearance_confirmed": False,
                    "experience_choice_ref": None,
                    "basis": basis("learning_the_ropes", "tutorial_transcript")},
        "skills": [{"id": "skill." + s, "xp_tenths": 11540 if s == "hitpoints" else 0,
                    "base_level": 10 if s == "hitpoints" else 1, "current_level": 10 if s == "hitpoints" else 1,
                    "basis": basis("skills", "hitpoints")} for s in skills],
        "derived": {"total_level": 33, "basis": basis("skills", "hitpoints", classification="inference")},
        "inventory": {"slot_count": 28, "items": [],
                      "basis": basis("inventory", "tutorial_transcript", classification="assumption", assumptions=["fresh_containers"])},
        "equipment": {"slots": {f"slot.{s}": None for s in ["head", "cape", "neck", "weapon", "body", "shield", "legs", "hands", "feet", "ring", "ammo"]},
                      "appearance_is_not_equipment": True,
                      "basis": basis("equipment", "learning_the_ropes", classification="assumption", assumptions=["fresh_containers"])},
        "bank": {"base_capacity": 400, "items": [], "seed_entitlement_ref": "grant.tutorial.bank_coins",
                 "seed_claimed": False, "first_visible_contents": [item("coins", 25)],
                 "security_bonus_slots": 0,
                 "basis": basis("bank", "learning_the_ropes", classification="assumption", assumptions=["fresh_containers"]),
                 "note": "No Jagex-account/Authenticator/PIN bonus is silently awarded to a ClubScape account. Base 400 is source-backed; identity-provider bonus mapping is outside these mechanics."},
        "tutorial": {"stage_ref": "stage.tutorial.appearance", "source_numeric_progress": None,
                     "departed": False, "melee_kill": False, "ranged_kill": False, "chicken_cast": False,
                     "departure_authorized": False, "grant_ledger": [], "reward_ledger": [],
                     "basis": basis("tutorial_transcript", classification="inference")},
        "quests": [
            {"id": "quest.learning_the_ropes", "status": "not_started", "reward_claimed": False,
             "basis": basis("learning_the_ropes", classification="assumption", assumptions=["quest_tracking_start"])},
            {"id": "quest.cooks_assistant", "status": "not_started", "delivered": {"milk": False, "flour": False, "egg": False}, "reward_claimed": False,
             "basis": basis("cooks_transcript", "cooks_journal")},
        ],
        "quest_points": 0,
        "vitals": {"hitpoints": 10, "prayer_points": 1, "run_energy_units": 10000, "run_enabled": False,
                   "basis": basis("hitpoints", "prayer", "energy", classification="assumption", assumptions=["initial_ui_defaults"])},
        "world": {"location_ref": "location.tutorial.start_house", "tile": None, "camera": None,
                  "binding_ref": "binding.world.source_layout", "basis": basis("learning_the_ropes")},
        "ui": {"visible_lesson_ref": "ui.appearance", "initially_available_refs": ["ui.appearance", "ui.logout"],
               "unlock_policy": "tutorial.json stage.ui_unlock_refs; never treat unavailable tabs as successful opens",
               "basis": basis("tutorial_transcript", classification="assumption", assumptions=["initial_ui_defaults"])},
        "death": {"tutorial_seen": False, "topics": {"fees": False, "timer": False, "kept_items": False},
                  "grave": None, "office_items": [], "food_supply_pile_setting": None,
                  "basis": basis("death_dialogue", "grave"),
                  "note": "Food-supply-pile default is not observed. Both source settings are supported in activities; tests select the setting explicitly."},
        "mill": {"hopper": None, "flour_units": 0,
                 "basis": basis("mill_lane_mill", classification="assumption", assumptions=["mill_ownership_overflow"])},
        "experience_branches": [
            {"choice_ref": "choice.experience.brand_new", "source_option": "I'm brand new! This is my first time here.", "adventure_paths_eligible": True,
             "arrival_ref": "location.lumbridge.jon_arrival", "arrival_npc_ref": "npc.adventurer_jon"},
            {"choice_ref": "choice.experience.returning", "source_option": "I've played in the past, but not recently.", "adventure_paths_eligible": True,
             "arrival_ref": "location.lumbridge.jon_arrival", "arrival_npc_ref": "npc.adventurer_jon"},
            {"choice_ref": "choice.experience.experienced", "source_option": "I'm an experienced player.", "adventure_paths_eligible": False,
             "arrival_ref": "location.lumbridge.castle_arrival", "arrival_npc_ref": "npc.lumbridge_guide"},
        ],
        "experience_branch_basis": basis("tutorial_transcript", "tutorial_island", "adventure_paths"),
        "experience_branch_scope": "All choices require the same complete tutorial. Jon's introduction is triggered for eligible arrivals; starting Adventure Paths is optional, never a prerequisite or a silent extra reward. The source has A/B path variants, so do not choose a random rewards program.",
        "persistence": {"preserve": ["skills", "inventory order", "equipment", "bank", "tutorial stage/flags/unlocks", "experience choice", "quests/delivered bits", "grant/reward ledgers", "mill counts", "death tutorial topics/grave state", "world position"],
                        "acknowledged_operations": "Persist atomically before acknowledgement. Reconnect/server restart never reseeds normal-account defaults.",
                        "basis": basis("source.project.prompt", classification="engineering_contract")},
    })


def tutorial():
    states, transitions = [], []

    def stage(name, area, instruction, unlock=(), evidence=None):
        states.append({
            "id": f"stage.tutorial.{name}", "location_ref": "location." + area,
            "source_numeric_progress": None, "instruction": instruction,
            "ui_unlock_refs": ["ui." + u for u in unlock],
            "basis": evidence or basis("tutorial_transcript"),
        })

    def edge(name, start, end, event, *guards, effects=(), evidence=None):
        transitions.append({
            "id": "transition.tutorial." + name, "from_ref": "stage.tutorial." + start,
            "to_ref": "stage.tutorial." + end, "event_ref": "event." + event,
            "guard": all_of(*guards), "effects": list(effects),
            "basis": evidence or basis("tutorial_transcript"),
        })

    def talk(name, start, end, npc, topic, *guards, effects=(), evidence=None):
        edge(name, start, end, "dialogue.completed", eq("event.npc_ref", "npc." + npc),
             eq("event.topic", topic), *guards, effects=effects, evidence=evidence)

    def opened(name, start, end, ui):
        edge(name, start, end, "ui.opened", eq("event.ui_ref", "ui." + ui))

    def travel(name, start, end, to, obj):
        edge(name, start, end, "world.transitioned", eq("event.to_ref", "location." + to),
             eq("event.link_ref", "object." + obj))

    definitions = [
        ("appearance", "tutorial.start_house", "Confirm actual character appearance.", ["appearance"]),
        ("experience", "tutorial.start_house", "Select one of the three source experience choices.", ["experience"]),
        ("guide_greeting", "tutorial.start_house", "Speak to the Gielinor Guide; introducing settings is a real dialogue.", []),
        ("settings_open", "tutorial.start_house", "Open Settings, not merely click the guide again.", ["settings"]),
        ("guide_settings", "tutorial.start_house", "Hear guide's explanation and permission to use the door.", []),
        ("starting_exit", "tutorial.start_house", "Walk to and pass the indicated eastern door.", []),
        ("survival_greeting", "tutorial.survival", "Receive a small fishing net from the Survival Expert.", []),
        ("inventory_open", "tutorial.survival", "Open inventory to view the net.", ["inventory"]),
        ("catch_shrimp", "tutorial.survival", "Actually catch raw shrimps using the net.", []),
        ("skills_open", "tutorial.survival", "Open Skills after the catch.", ["skills"]),
        ("survival_tools", "tutorial.survival", "Receive bronze axe and tinderbox after the skill explanation.", []),
        ("cut_logs", "tutorial.survival", "Actually chop a normal tree using the bronze axe.", []),
        ("light_fire", "tutorial.survival", "Use tinderbox/logs to successfully create a real fire.", []),
        ("cook_shrimp", "tutorial.survival", "Cook raw shrimps on a fire. Source accepts a resolved burn as well as success.", []),
        ("survival_exit", "tutorial.survival", "Use the western gate after the cooking attempt.", []),
        ("chef_entry", "tutorial.kitchen", "Reach and enter the Chef's kitchen via its real door.", []),
        ("chef_greeting", "tutorial.kitchen", "Receive flour and water from the Master Chef.", []),
        ("make_dough", "tutorial.kitchen", "Combine pot of flour and bucket of water into dough.", ["cooking"]),
        ("bake_bread", "tutorial.kitchen", "Use the range for a real bread cooking attempt.", []),
        ("chef_exit", "tutorial.kitchen", "Leave through the north-west door.", []),
        ("run_toggle", "tutorial.kitchen", "Use the run orb; running still consumes real energy.", []),
        ("quest_entry", "tutorial.quest_house", "Follow the path and enter the Quest Guide house.", []),
        ("quest_greeting", "tutorial.quest_house", "Talk to the Quest Guide and start tracked Learning the Ropes.", []),
        ("journal_open", "tutorial.quest_house", "Open Quest List/journal.", ["quests"]),
        ("quest_explanation", "tutorial.quest_house", "Hear the explanation of not-started, active and complete quests.", []),
        ("quest_ladder", "tutorial.quest_house", "Descend the actual ladder to the dungeon.", []),
        ("mining_greeting", "tutorial.mine", "Receive bronze pickaxe and mining instruction.", []),
        ("mine_first", "tutorial.mine", "Mine an actual tin or copper ore. Tin is prompted first.", []),
        ("mine_second", "tutorial.mine", "Mine until currently holding one of each ore.", []),
        ("smelt_bronze", "tutorial.mine", "Use the furnace; consume tin+copper and make bronze.", []),
        ("mining_hammer", "tutorial.mine", "Talk to the instructor for hammer and smithing explanation.", []),
        ("anvil_open", "tutorial.mine", "Open the smithing interface at an actual anvil.", ["smithing"]),
        ("smith_dagger", "tutorial.mine", "Select and actually smith a bronze dagger.", []),
        ("mining_exit", "tutorial.mine", "Pass the mining-to-combat gate.", []),
        ("combat_greeting", "tutorial.rat_cave", "Talk to Vannaka about wielding equipment.", []),
        ("equipment_open", "tutorial.rat_cave", "Open Worn Equipment.", ["equipment"]),
        ("equipment_stats_open", "tutorial.rat_cave", "Open the shield/helmet Equipment Stats button.", ["equipment_stats"]),
        ("equip_dagger", "tutorial.rat_cave", "Equip the dagger using real inventory/slot ownership.", []),
        ("melee_supply", "tutorial.rat_cave", "Receive sword/shield, with explicit partial grants if space is limited.", []),
        ("equip_melee", "tutorial.rat_cave", "Equip bronze sword and wooden shield; replace dagger.", []),
        ("combat_open", "tutorial.rat_cave", "Open Combat Options; preserve source styles and XP selection.", ["combat"]),
        ("enter_rat_pen", "tutorial.rat_cave", "Pass through the pen gate with instructor permission.", []),
        ("melee_rat", "tutorial.rat_cave", "Kill one tutorial rat using actual melee combat; tutorial cannot kill the player.", []),
        ("leave_rat_pen", "tutorial.rat_cave", "Leave the pen and return to Vannaka.", []),
        ("ranged_supply", "tutorial.rat_cave", "Receive shortbow and 50 bronze arrows; one free slot yields bow alone.", []),
        ("equip_ranged", "tutorial.rat_cave", "Equip shortbow and ammunition; a bow must displace weapon AND shield.", []),
        ("ranged_rat", "tutorial.rat_cave", "Kill a second rat with ranged from outside the pen.", []),
        ("combat_exit", "tutorial.rat_cave", "Climb the eastern ladder after both combat lessons.", []),
        ("bank_open", "tutorial.bank", "Open the bank booth; first bank already has 25 coins.", ["bank"]),
        ("bank_close", "tutorial.bank", "Use real banking if desired, then close it.", []),
        ("poll_inspect", "tutorial.bank", "Inspect the poll booth and explanation; do not fabricate a vote.", ["poll"]),
        ("account_entry", "tutorial.bank", "Pass the eastern door only after bank and poll inspection.", []),
        ("account_greeting", "tutorial.account_room", "Talk to Account Guide.", []),
        ("account_open", "tutorial.account_room", "Open Account Management.", ["account"]),
        ("account_explanation", "tutorial.account_room", "Hear membership/worlds/bonds/inbox/security explanation; demonstrated tabs actually open.", ["logout", "account_links"]),
        ("account_exit", "tutorial.account_room", "Pass the eastern exit.", []),
        ("chapel_entry", "tutorial.chapel", "Walk to and enter the chapel.", []),
        ("prayer_greeting", "tutorial.chapel", "Talk to Brother Brace.", []),
        ("prayer_open", "tutorial.chapel", "Open Prayer.", ["prayer"]),
        ("prayer_explanation", "tutorial.chapel", "Hear activation, draining, altar recharge and burying bones. Actually praying/burying is optional.", []),
        ("chapel_exit", "tutorial.chapel", "Leave the chapel by its southern door.", []),
        ("magic_entry", "tutorial.magic_house", "Follow the real path to the Magic Instructor; Ironman information is optional for normal accounts.", []),
        ("magic_greeting", "tutorial.magic_house", "Talk to Terrova.", []),
        ("magic_open", "tutorial.magic_house", "Open spellbook; initial list is filtered by available Magic level.", ["magic"]),
        ("magic_supply", "tutorial.magic_house", "Receive five air and five mind runes.", []),
        ("wind_strike", "tutorial.magic_house", "Select Wind Strike and target a tutorial chicken; actually cast once.", []),
        ("departure_offer", "tutorial.magic_house", "Learning the Ropes is complete and pays 1 QP; still on island until authorized departure.", ["quest_reward"]),
        ("departure_confirmation", "tutorial.magic_house", "Confirm mainland travel and normal-account status, or ask about Ironman and return later.", []),
        ("home_teleport", "tutorial.magic_house", "Cast real Lumbridge Home Teleport; no runes, no XP, not an instant stage command.", []),
        ("teleport_channel", "tutorial.magic_house", "Complete default 24-tick Home Teleport or retry after interruption.", []),
        ("mainland", "lumbridge.castle_arrival", "Arrive on the selected experience branch with reconciled possessions; tutorial is permanently complete.", []),
    ]
    for name, area, instruction, unlocks in definitions:
        evidence = None
        if name == "bake_bread":
            evidence = basis("tutorial_transcript", "bread", classification="assumption", assumptions=["bread_first_attempt"])
        if name == "run_toggle":
            evidence = basis("tutorial_transcript", classification="inference")
        stage(name, area, instruction, unlocks, evidence)
    states[-1]["location_alternative_refs"] = ["location.lumbridge.castle_arrival", "location.lumbridge.jon_arrival"]

    edge("appearance", "appearance", "experience", "appearance.confirmed",
         eq("event.validated", True), effects=[set_value("state.appearance_confirmed", True)])
    edge("experience", "experience", "guide_greeting", "experience.selected",
         one_of("event.choice", ["brand_new", "returning", "experienced"]),
         effects=[effect("mark", key="selected_experience", from_path="event.choice")])
    talk("guide_intro", "guide_greeting", "settings_open", "gielinor_guide", "greeting_and_settings")
    opened("settings", "settings_open", "guide_settings", "settings")
    talk("guide_exit_permission", "guide_settings", "starting_exit", "gielinor_guide", "settings_and_next_instructor")
    travel("starting_door", "starting_exit", "survival_greeting", "tutorial.survival", "tutorial.start_door")
    talk("net", "survival_greeting", "inventory_open", "survival_expert", "fishing_intro", ge("state.inventory.free_slots", 1),
         effects=[effect("grant", grant_ref="grant.tutorial.net")])
    opened("inventory", "inventory_open", "catch_shrimp", "inventory")
    edge("shrimp", "catch_shrimp", "skills_open", "gather.succeeded", eq("event.rule_ref", "rule.fishing.shrimps"), eq("event.item_ref", "item.shrimps.raw"),
         effects=[effect("mark", key="caught_shrimp")])
    opened("skills", "skills_open", "survival_tools", "skills")
    talk("survival_tools", "survival_tools", "cut_logs", "survival_expert", "woodcutting_firemaking_intro", ge("state.inventory.free_slots", 2),
         effects=[effect("grant", grant_ref="grant.tutorial.survival_tools")],
         evidence=basis("tutorial_transcript", classification="assumption", assumptions=["multi_item_grant_capacity"]))
    edge("logs", "cut_logs", "light_fire", "gather.succeeded", eq("event.rule_ref", "rule.woodcutting.normal"), eq("event.item_ref", "item.logs.normal"))
    edge("fire", "light_fire", "cook_shrimp", "fire.lit", eq("event.rule_ref", "rule.firemaking.normal"))
    edge("shrimp_cooked", "cook_shrimp", "survival_exit", "cook.resolved", eq("event.rule_ref", "rule.cooking.shrimps"),
         one_of("event.outcome", ["cooked", "burnt"]), eq("event.facility", "fire"),
         effects=[effect("mark", key="shrimp_attempt")])
    travel("survival_gate", "survival_exit", "chef_entry", "tutorial.kitchen", "tutorial.survival_gate")
    travel("chef_door", "chef_entry", "chef_greeting", "tutorial.kitchen", "tutorial.chef_entry")
    talk("chef_ingredients", "chef_greeting", "make_dough", "master_chef", "bread_intro", ge("state.inventory.free_slots", 2),
         effects=[effect("grant", grant_ref="grant.tutorial.chef_ingredients")])
    edge("dough", "make_dough", "bake_bread", "craft.succeeded", eq("event.rule_ref", "rule.cooking.dough"), eq("event.output_item_ref", "item.bread.dough"))
    edge("bread", "bake_bread", "chef_exit", "cook.resolved", eq("event.rule_ref", "rule.cooking.bread"), one_of("event.outcome", ["cooked", "burnt"]),
         eq("event.facility", "range"), evidence=basis("tutorial_transcript", "bread", classification="assumption", assumptions=["bread_first_attempt"]))
    travel("chef_exit", "chef_exit", "run_toggle", "tutorial.kitchen", "tutorial.chef_exit")
    edge("run", "run_toggle", "quest_entry", "setting.changed", eq("event.setting", "run_enabled"), eq("event.value", True),
         evidence=basis("tutorial_transcript", classification="inference"))
    travel("quest_door", "quest_entry", "quest_greeting", "tutorial.quest_house", "tutorial.quest_entry")
    talk("quest_intro", "quest_greeting", "journal_open", "quest_guide", "quest_intro",
         effects=[set_value("state.quests.learning_the_ropes.status", "in_progress")],
         evidence=basis("learning_the_ropes", "tutorial_transcript", classification="assumption", assumptions=["quest_tracking_start"]))
    opened("journal", "journal_open", "quest_explanation", "quests")
    talk("quest_explanation", "quest_explanation", "quest_ladder", "quest_guide", "quest_journal_explanation")
    travel("quest_ladder", "quest_ladder", "mining_greeting", "tutorial.mine", "tutorial.quest_ladder")
    talk("pickaxe", "mining_greeting", "mine_first", "mining_instructor", "mining_intro", ge("state.inventory.free_slots", 1),
         effects=[effect("grant", grant_ref="grant.tutorial.pickaxe")])
    edge("first_ore", "mine_first", "mine_second", "gather.succeeded", one_of("event.rule_ref", ["rule.mining.copper", "rule.mining.tin"]),
         evidence=basis("tutorial_transcript", classification="inference"))
    edge("both_ores", "mine_second", "smelt_bronze", "gather.succeeded", one_of("event.rule_ref", ["rule.mining.copper", "rule.mining.tin"]),
         ge("state.inventory.tin_ore", 1), ge("state.inventory.copper_ore", 1),
         evidence=basis("tutorial_transcript", classification="inference"))
    edge("bronze_bar", "smelt_bronze", "mining_hammer", "craft.succeeded", eq("event.rule_ref", "rule.smelting.bronze"), eq("event.output_item_ref", "item.bar.bronze"))
    talk("hammer", "mining_hammer", "anvil_open", "mining_instructor", "smithing_intro",
         any_of(ge("state.inventory.hammer", 1), ge("state.inventory.free_slots", 1)),
         effects=[effect("grant", grant_ref="grant.tutorial.hammer")])
    opened("smithing_menu", "anvil_open", "smith_dagger", "smithing")
    edge("dagger", "smith_dagger", "mining_exit", "craft.succeeded", eq("event.rule_ref", "rule.smithing.bronze_dagger"), eq("event.output_item_ref", "item.dagger.bronze"))
    travel("mining_gate", "mining_exit", "combat_greeting", "tutorial.rat_cave", "tutorial.mine_gate")
    talk("combat_intro", "combat_greeting", "equipment_open", "combat_instructor", "equipment_intro")
    opened("equipment", "equipment_open", "equipment_stats_open", "equipment")
    opened("equipment_stats", "equipment_stats_open", "equip_dagger", "equipment_stats")
    edge("dagger_equipped", "equip_dagger", "melee_supply", "equipment.changed", eq("state.equipment.weapon", "item.dagger.bronze"))
    talk("melee_gear", "melee_supply", "equip_melee", "combat_instructor", "melee_gear", ge("state.inventory.free_slots", 1),
         effects=[effect("grant", grant_ref="grant.tutorial.melee_gear")])
    edge("melee_equipped", "equip_melee", "combat_open", "equipment.changed",
         eq("state.equipment.weapon", "item.sword.bronze"), eq("state.equipment.shield", "item.shield.wooden"))
    opened("combat_options", "combat_open", "enter_rat_pen", "combat")
    travel("rat_pen_entry", "enter_rat_pen", "melee_rat", "tutorial.rat_cave", "tutorial.rat_gate")
    transitions[-1]["guard"]["all"].append(eq("event.side", "inside"))
    edge("melee_kill", "melee_rat", "leave_rat_pen", "combat.kill_credited", eq("event.npc_ref", "npc.tutorial_rat"),
         eq("event.method", "melee"), effects=[set_value("state.tutorial.melee_kill", True)])
    travel("rat_pen_exit", "leave_rat_pen", "ranged_supply", "tutorial.rat_cave", "tutorial.rat_gate")
    transitions[-1]["guard"]["all"].append(eq("event.side", "outside"))
    talk("ranged_gear", "ranged_supply", "equip_ranged", "combat_instructor", "ranged_intro", ge("state.inventory.free_slots", 1),
         effects=[effect("grant", grant_ref="grant.tutorial.ranged_gear")])
    edge("ranged_equipped", "equip_ranged", "ranged_rat", "equipment.changed",
         eq("state.equipment.weapon", "item.shortbow"), eq("state.equipment.ammo", "item.arrow.bronze"), ge("state.equipment.ammo_quantity", 1))
    edge("ranged_kill", "ranged_rat", "combat_exit", "combat.kill_credited", eq("event.npc_ref", "npc.tutorial_rat"),
         eq("event.method", "ranged"), eq("event.player_outside_pen", True),
         effects=[set_value("state.tutorial.ranged_kill", True)])
    travel("combat_ladder", "combat_exit", "bank_open", "tutorial.bank", "tutorial.combat_ladder")
    edge("bank", "bank_open", "bank_close", "ui.opened", eq("event.ui_ref", "ui.bank"),
         eq("state.bank.seed_claimed", True), effects=[effect("mark", key="bank_opened")])
    edge("bank_closed", "bank_close", "poll_inspect", "ui.closed", eq("event.ui_ref", "ui.bank"))
    edge("poll", "poll_inspect", "account_entry", "object.inspected", eq("event.object_ref", "object.tutorial.poll_booth"),
         eq("event.explanation_completed", True))
    travel("account_door", "account_entry", "account_greeting", "tutorial.account_room", "tutorial.account_entry")
    talk("account_intro", "account_greeting", "account_open", "account_guide", "account_intro")
    opened("account", "account_open", "account_explanation", "account")
    talk("account_explanation", "account_explanation", "account_exit", "account_guide", "membership_worlds_bonds_security")
    travel("account_exit", "account_exit", "chapel_entry", "tutorial.chapel", "tutorial.account_exit")
    travel("chapel_door", "chapel_entry", "prayer_greeting", "tutorial.chapel", "tutorial.chapel_entry")
    talk("prayer_intro", "prayer_greeting", "prayer_open", "brother_brace", "prayer_intro")
    opened("prayer", "prayer_open", "prayer_explanation", "prayer")
    talk("prayer_explanation", "prayer_explanation", "chapel_exit", "brother_brace", "prayer_activation_points_bones")
    travel("chapel_exit", "chapel_exit", "magic_entry", "tutorial.magic_house", "tutorial.chapel_exit")
    travel("magic_entry", "magic_entry", "magic_greeting", "tutorial.magic_house", "tutorial.magic_entry")
    talk("magic_intro", "magic_greeting", "magic_open", "magic_instructor", "magic_intro")
    opened("magic", "magic_open", "magic_supply", "magic")
    talk("runes", "magic_supply", "wind_strike", "magic_instructor", "wind_strike_intro",
         ge("state.inventory.free_slots", 2), effects=[effect("grant", grant_ref="grant.tutorial.runes")])
    edge("chicken_cast", "wind_strike", "departure_offer", "spell.resolved",
         eq("event.spell_ref", "spell.wind_strike"), eq("event.npc_ref", "npc.tutorial_chicken"),
         one_of("event.outcome", ["hit", "splash"]),
         effects=[set_value("state.tutorial.chicken_cast", True), set_value("state.quests.learning_the_ropes.status", "completed"),
                  effect("reward", reward_ref="reward.learning_the_ropes")],
         evidence=basis("tutorial_transcript", "combat_spells", classification="inference"))
    talk("departure_offer", "departure_offer", "departure_confirmation", "magic_instructor", "offer_mainland")
    talk("decline_departure", "departure_confirmation", "departure_offer", "magic_instructor", "stay_on_island")
    talk("ironman_information", "departure_confirmation", "departure_offer", "magic_instructor", "ask_ironman_tutor")
    talk("confirm_normal_departure", "departure_confirmation", "home_teleport", "magic_instructor", "confirm_normal_mainland",
         eq("state.account.mode", "normal"), effects=[set_value("state.tutorial.departure_authorized", True)],
         evidence=basis("tutorial_transcript", classification="assumption", assumptions=["departure_missing_dialogue"]))
    edge("start_home_teleport", "home_teleport", "teleport_channel", "teleport.started",
         eq("event.spell_ref", "spell.lumbridge_home_teleport"), eq("state.tutorial.departure_authorized", True))
    edge("interrupt_home_teleport", "teleport_channel", "home_teleport", "teleport.interrupted", eq("event.spell_ref", "spell.lumbridge_home_teleport"))
    edge("arrive", "teleport_channel", "mainland", "teleport.completed",
         eq("event.spell_ref", "spell.lumbridge_home_teleport"), eq("state.tutorial.departure_authorized", True),
         eq("state.quests.learning_the_ropes.status", "completed"), eq("event.destination_matches_experience", True),
         effects=[effect("reconcile_departure", rule_ref="rule.tutorial.departure"),
                  set_value("state.tutorial.departed", True)],
         evidence=basis("tutorial_transcript", "learning_the_ropes", classification="assumption", assumptions=["departure_reconciliation"]))

    def grant(name, npc, items, policy, sources=("tutorial_transcript",), assumptions=()):
        return {"id": "grant.tutorial." + name, "npc_ref": "npc." + npc if npc else None,
                "items": items, "policy": policy,
                "basis": basis(*sources, classification="assumption" if assumptions else "verified_reference", assumptions=assumptions)}

    grants = [
        grant("net", "survival_expert", [item("fishing_net.small")], "one_initial; missing-only replacement per recovery policy"),
        grant("survival_tools", "survival_expert", [item("axe.bronze"), item("tinderbox")], "one_initial_each; all-or-none unless both missing replacements fit", assumptions=("multi_item_grant_capacity",)),
        grant("chef_ingredients", "master_chef", [item("flour.pot"), item("water.bucket")], "missing_each_only while dough/bread absent and lesson not exited; never duplicate the present ingredient"),
        grant("pickaxe", "mining_instructor", [item("pickaxe.bronze")], "one_initial; missing-only replacement per recovery policy"),
        grant("hammer", "mining_instructor", [item("hammer")], "one_if_missing_in_inventory_and_free_slot; repeat dialogue supported after dagger"),
        grant("melee_gear", "combat_instructor", [item("sword.bronze"), item("shield.wooden")], "missing_each_over_inventory_and_equipment; sword_first; 0 slots=none,1 slot=first_missing,2 slots=both_missing"),
        grant("ranged_gear", "combat_instructor", [item("shortbow"), item("arrow.bronze", 50)], "missing_bow_first; top_up_total_inventory_plus_equipped_arrows_to_50; 1 free slot with neither grants bow_only; existing arrow stack needs no free slot"),
        grant("runes", "magic_instructor", [item("rune.air", 5), item("rune.mind", 5)], "initial_5_each; replacement_if_cannot_pay_one_cast; provisional_top_up_each_to_5_not_add_5",
              assumptions=("multi_item_grant_capacity",)),
        grant("bank_coins", None, [item("coins", 25)], "bank_only; entitlement_once_before_first_visible_open; no_reseed_on_close/reopen",
              sources=("learning_the_ropes",), assumptions=("fresh_containers",)),
    ]
    recovery = [
        {"id": "recovery.tutorial.tools", "event_ref": "event.dialogue.completed",
         "scope": "after original grant unlock, before departure, owning instructor reachable",
         "missing_items": [item("fishing_net.small"), item("axe.bronze"), item("tinderbox"), item("pickaxe.bronze")],
         "policy": "Return only each missing unlocked tool. Full inventory: explicit refusal, unchanged stage; nearby dropped tools may be picked up first.",
         "basis": basis("tutorial_transcript", classification="assumption", assumptions=["missing_tutorial_tools"])},
        {"id": "recovery.tutorial.ingredients", "event_ref": "event.dialogue.completed", "grant_refs": ["grant.tutorial.chef_ingredients"],
         "scope": "Master Chef lesson, no dough/bread; even if stage is bake_bread and dough was lost",
         "policy": "Replenish only missing flour/water; actually remake dough and bake. Losing inputs never completes the lesson.", "basis": basis("tutorial_transcript")},
        {"id": "recovery.tutorial.ore_bar_dagger", "rule_refs": ["rule.mining.tin", "rule.mining.copper", "rule.smelting.bronze", "rule.smithing.bronze_dagger"],
         "policy": "Unlocked earlier activities remain usable on their reachable tutorial objects until departure; a lost ore/bar/dagger must be mined/smelted/smithed again. Reopen smithing UI if needed. Hammer replacement does not mint a bar. Last-resort instructor dagger replacement is provisional, not used by ordinary journey fixtures.",
         "basis": basis("tutorial_transcript", classification="assumption", assumptions=["missing_tutorial_tools"])},
        {"id": "recovery.tutorial.combat", "event_ref": "event.dialogue.completed", "grant_refs": ["grant.tutorial.melee_gear", "grant.tutorial.ranged_gear"],
         "scope": "after original respective lesson grant, before corresponding required kill",
         "policy": "Count equipped items as owned; fill only missing gear and arrows up to 50; never grant after every attack. One-slot partial grants persist across retries.",
         "basis": basis("tutorial_transcript")},
        {"id": "recovery.tutorial.runes", "event_ref": "event.dialogue.completed", "grant_refs": ["grant.tutorial.runes"],
         "scope": "after rune lesson, before departure",
         "policy": "If air>=1 and mind>=1, instructor refuses more. Otherwise top up each to 5 if resulting stacks fit. Exact asymmetric depleted-rune count is not in source; do not label the top-up as verified.",
         "basis": basis("tutorial_transcript", classification="assumption", assumptions=["multi_item_grant_capacity"])},
    ]
    write("tutorial.json", {
        "schema_version": 1, "id": "contract.journey.tutorial", "quest_ref": "quest.learning_the_ropes",
        "profile_ref": "profile.osrs.current_normal_f2p", "start_ref": "stage.tutorial.appearance",
        "terminal_refs": ["stage.tutorial.mainland"], "states": states, "transitions": transitions,
        "grant_definitions": grants, "recovery_handlers": recovery,
        "rule_refs": ["rule.tutorial.restrictions", "rule.tutorial.xp_cap", "rule.tutorial.departure", "rule.magic.home_teleport"],
        "action_unlocks": [
            {"at_stage_ref": "stage.tutorial.catch_shrimp", "rule_refs": ["rule.fishing.shrimps"], "sticky_until_departure": True},
            {"at_stage_ref": "stage.tutorial.cut_logs", "rule_refs": ["rule.woodcutting.normal"], "sticky_until_departure": True},
            {"at_stage_ref": "stage.tutorial.light_fire", "rule_refs": ["rule.firemaking.normal"], "sticky_until_departure": True},
            {"at_stage_ref": "stage.tutorial.cook_shrimp", "rule_refs": ["rule.cooking.shrimps"], "sticky_until_departure": True},
            {"at_stage_ref": "stage.tutorial.make_dough", "rule_refs": ["rule.cooking.dough"], "sticky_until_departure": True},
            {"at_stage_ref": "stage.tutorial.bake_bread", "rule_refs": ["rule.cooking.bread"], "sticky_until_departure": True},
            {"at_stage_ref": "stage.tutorial.mine_first", "rule_refs": ["rule.mining.tin", "rule.mining.copper"], "sticky_until_departure": True},
            {"at_stage_ref": "stage.tutorial.smelt_bronze", "rule_refs": ["rule.smelting.bronze"], "sticky_until_departure": True},
            {"at_stage_ref": "stage.tutorial.anvil_open", "rule_refs": ["rule.smithing.bronze_dagger"], "sticky_until_departure": True},
            {"at_stage_ref": "stage.tutorial.equip_dagger", "rule_refs": ["rule.equipment.equip", "rule.equipment.unequip"], "sticky_until_departure": True},
            {"at_stage_ref": "stage.tutorial.melee_rat", "rule_refs": ["rule.combat.melee"], "sticky_until_departure": False,
             "only_target_ref": "npc.tutorial_rat", "until_flag": "state.tutorial.melee_kill"},
            {"at_stage_ref": "stage.tutorial.ranged_rat", "rule_refs": ["rule.combat.ranged"], "sticky_until_departure": False,
             "only_target_ref": "npc.tutorial_rat", "until_flag": "state.tutorial.ranged_kill", "must_be_outside_pen": True},
            {"at_stage_ref": "stage.tutorial.prayer_explanation", "rule_refs": ["rule.prayer.thick_skin"], "sticky_until_departure": True},
            {"at_stage_ref": "stage.tutorial.wind_strike", "rule_refs": ["rule.magic.wind_strike"], "sticky_until_departure": True,
             "allowed_target_refs": ["npc.tutorial_chicken", "npc.tutorial_rat"], "chicken_until_flag": "state.tutorial.chicken_cast"},
            {"at_stage_ref": "stage.tutorial.home_teleport", "rule_refs": ["rule.magic.home_teleport"], "sticky_until_departure": True},
        ],
        "action_unlock_basis": basis("tutorial_transcript", "tutorial_island", classification="inference"),
        "action_unlock_limits": "Sticky unlocks do not override source area/object restrictions, level caps, equipment requirements or rat/chicken repeat-action guards. Returning to an earlier workshop uses real travel. Mainland removes tutorial-specific locks, not ordinary level/quest requirements.",
        "exit_permissions": [
            {"object_ref": "object.tutorial.start_door", "requires_completed_transition_ref": "transition.tutorial.guide_exit_permission"},
            {"object_ref": "object.tutorial.survival_gate", "requires_completed_transition_ref": "transition.tutorial.shrimp_cooked"},
            {"object_ref": "object.tutorial.chef_exit", "requires_completed_transition_ref": "transition.tutorial.bread"},
            {"object_ref": "object.tutorial.quest_ladder", "requires_completed_transition_ref": "transition.tutorial.quest_explanation"},
            {"object_ref": "object.tutorial.mine_gate", "requires_completed_transition_ref": "transition.tutorial.dagger"},
            {"object_ref": "object.tutorial.combat_ladder", "requires_completed_transition_ref": "transition.tutorial.ranged_kill"},
            {"object_ref": "object.tutorial.account_entry", "requires_completed_transition_ref": "transition.tutorial.poll"},
            {"object_ref": "object.tutorial.account_exit", "requires_completed_transition_ref": "transition.tutorial.account_explanation"},
            {"object_ref": "object.tutorial.chapel_exit", "requires_completed_transition_ref": "transition.tutorial.prayer_explanation"},
        ],
        "rat_pen_permission": {"object_ref": "object.tutorial.rat_gate", "entry_stage_refs": ["stage.tutorial.enter_rat_pen", "stage.tutorial.melee_rat"],
                               "exit_allowed": True, "entry_denied_after_ranged_supply": True, "basis": basis("tutorial_transcript")},
        "source_progress_policy": "All numeric values are intentionally null. No source numeric stage/varp was established by these public transcripts.",
        "entry_guards": [
            "Entry to the graph requires a new persisted normal account, not a test seed.",
            "Each noninitial state requires its immediately preceding transition or a persisted acknowledged checkpoint; stage numbers are never accepted as player commands.",
            "Result events require the shared system's actual item/tool/level/range/LOS/timing checks. A graph match alone is insufficient.",
            "UI unlocks on state entry persist. Refresh/reconnect projects the same state and does not run grants again.",
        ],
        "dialogue_policy": "Topics are semantic/paraphrased beats, not purported exact source strings. Required dialogue must actually be completed; recap loops and rejected earlier actions do not progress. Presentation text, timing, highlighting, chatheads and audio still need the approved reference pack.",
        "bank_seed_hook": {"before_event_ref": "event.ui.opened", "ui_ref": "ui.bank", "grant_ref": "grant.tutorial.bank_coins",
                           "guard": all_of(eq("state.bank.seed_claimed", False), eq("state.tutorial.departed", False)),
                           "effects": [set_value("state.bank.seed_claimed", True)],
                           "note": "Seeding, first bank opening and stage acknowledgement are one transaction."},
        "optional_controls": {"chosen_graph_tasks": ["run toggle demonstration"], "not_reintroduced_as_mandatory": ["music selection", "emote", "friends list", "ignore list"],
                              "run_gate_caveat": "The transcript describes a run prompt and the after-toggle prompt, but not a refusal when walking to the next door. Requiring the demonstrated toggle is a labeled graph inference, not a captured source door lock.",
                              "basis": basis("tutorial_transcript", "tutorial_island", classification="inference")},
        "dialogue_fact_parameters": {
            "poll": {"membership_required_to_vote": True, "skill_total_required_to_vote": 300, "support_percent": 70},
            "account": {"free_by_default": True, "typical_world_capacity": 2000,
                        "topics_in_order": ["membership", "worlds_and_logout_switcher", "bonds", "inbox", "name_changer", "bank_pin_and_two_factor", "support_links"]},
            "basis": basis("tutorial_transcript"),
        },
        "journey_evidence": "Structural reachability is not a passed journey. All live/headless acceptance runs must emit the full real event trace.",
    })


def activities():
    item_rows = [
        ("pickaxe.bronze", "Bronze pickaxe", False, "weapon", "bronze_pickaxe", 1),
        ("axe.bronze", "Bronze axe", False, "weapon", "bronze_axe", 16),
        ("tinderbox", "Tinderbox", False, None, "tinderbox", 1),
        ("fishing_net.small", "Small fishing net", False, None, "small_fishing_net", 5),
        ("shrimps.raw", "Raw shrimps", False, None, "raw_shrimps", 5),
        ("shrimps.cooked", "Shrimps", False, None, "shrimps", 5),
        ("shrimps.burnt", "Burnt shrimp", False, None, "shrimps", None),
        ("logs.normal", "Logs", False, None, "tree", None),
        ("ashes", "Ashes", False, None, "fire", None),
        ("flour.pot", "Pot of flour", False, None, "pot_of_flour", None),
        ("water.bucket", "Bucket of water", False, None, "bread_dough", None),
        ("bread.dough", "Bread dough", False, None, "bread_dough", 4),
        ("bread", "Bread", False, None, "bread", 12),
        ("bread.burnt", "Burnt bread", False, None, "bread", None),
        ("ore.tin", "Tin ore", False, None, "tin_rocks", None),
        ("ore.copper", "Copper ore", False, None, "copper_rocks", None),
        ("bar.bronze", "Bronze bar", False, None, "bronze_bar", 8),
        ("hammer", "Hammer", False, None, "hammer", 1),
        ("dagger.bronze", "Bronze dagger (unpoisoned)", False, "weapon", "bronze_dagger", 10),
        ("sword.bronze", "Bronze sword", False, "weapon", "bronze_sword", 26),
        ("shield.wooden", "Wooden shield", False, "shield", "wooden_shield", 20),
        ("shortbow", "Shortbow", False, "weapon", "shortbow", 50),
        ("arrow.bronze", "Bronze arrow (unpoisoned)", True, "ammo", "bronze_arrow", 1),
        ("rune.air", "Air rune", True, None, "inventory", None),
        ("rune.mind", "Mind rune", True, None, "inventory", None),
        ("rune.water", "Water rune", True, None, "inventory", None),
        ("rune.earth", "Earth rune", True, None, "inventory", None),
        ("rune.body", "Body rune", True, None, "inventory", None),
        ("bucket", "Bucket", False, None, "bucket", 2),
        ("pot", "Pot", False, None, "pot", 1),
        ("coins", "Coins", True, None, "inventory", 1),
        ("egg", "Egg", False, None, "egg", None),
        ("milk.bucket", "Bucket of milk", False, None, "bucket_of_milk", 6),
        ("milk.bottomless_bucket", "Bottomless bucket of milk", False, None, "cooks_assistant", None),
        ("grain", "Grain", False, None, "grain", None),
        ("bones.tutorial", "Bones (Tutorial Island), normal not 2020 variant", False, None, "tutorial_bones", 1),
        ("bones", "Bones", False, None, "bones", None),
        ("shield.bronze_square", "Bronze sq shield", False, "shield", "goblin", None),
        ("spear.bronze", "Bronze spear", False, "weapon", "goblin", None),
        ("bolts.bronze", "Bronze bolts", True, "ammo", "goblin", None),
        ("goblin_book", "Goblin book", False, None, "goblin", None),
        ("goblin_mail", "Goblin mail", False, None, "goblin", None),
        ("chefs_hat", "Chef's hat", False, "head", "goblin", None),
        ("beer", "Beer", False, None, "goblin", None),
        ("brass_necklace", "Brass necklace", False, "neck", "goblin", None),
        ("talisman.air", "Air talisman", False, None, "goblin", None),
        ("energy_potion.one_dose", "Energy potion(1)", False, None, "goblin", None),
        ("energy_potion.two_dose", "Energy potion(2)", False, None, "goblin", None),
        ("energy_potion.four_dose", "Energy potion(4)", False, None, "goblin", None),
        ("clue.beginner", "Clue scroll (beginner), exact variant externally bound", False, None, "goblin", None),
        ("clue.easy", "Clue scroll (easy), exact variant externally bound", False, None, "goblin", None),
        ("ensouled_goblin_head", "Ensouled goblin head", False, None, "goblin", None),
        ("goblin_champion_scroll", "Goblin champion scroll", False, None, "goblin", None),
    ]
    items = []
    for key, name, stackable, slot, source, value in item_rows:
        record = {"id": "item." + key, "source_name": name, "stackable": stackable,
                  "slot_ref": "slot." + slot if slot else None, "base_item_value_coins": value,
                  "runtime_numeric_id": None, "basis": basis(source),
                  "binding_note": "Bind exact unnoted source definition from the chosen runtime; do not conflate visually similar variants."}
        if source == "goblin":
            record["basis"] = basis(source, classification="inference")
            record["binding_note"] = "Required drop-result identity. Stack/equip hints are not item-definition evidence; bind full definition before enabling its equipment/consumption behavior."
        if key == "shortbow":
            record["occupies_slot_refs"] = ["slot.weapon", "slot.shield"]
        items.append(record)
    equipment_stats = {
        "pickaxe.bronze": ([4, -2, 2, 0, 0], [0, 1, 0, 0, 0], 5, 0, 5, 1),
        "axe.bronze": ([-2, 4, 2, 0, 0], [0, 1, 0, 0, 0], 5, 0, 5, 1),
        "dagger.bronze": ([4, 2, -4, 1, 0], [0, 0, 0, 1, 0], 3, 0, 4, 1),
        "sword.bronze": ([4, 3, -2, 0, 0], [0, 2, 1, 0, 0], 5, 0, 4, 1),
        "shield.wooden": ([0, 0, 0, 0, 0], [4, 5, 3, 1, 4], 0, 0, None, None),
        "shortbow": ([0, 0, 0, 0, 8], [0, 0, 0, 0, 0], 0, 0, 4, 7),
        "arrow.bronze": ([0, 0, 0, 0, 0], [0, 0, 0, 0, 0], 0, 7, None, None),
    }
    for record in items:
        key = record["id"].removeprefix("item.")
        if key in equipment_stats:
            attack, defence, strength, ranged_strength, ticks, reach = equipment_stats[key]
            record["combat"] = {
                "bonus_order": ["stab", "slash", "crush", "magic", "ranged"],
                "attack": attack, "defence": defence, "melee_strength": strength,
                "ranged_strength": ranged_strength, "magic_damage_percent": 0, "prayer_bonus": 0,
                "attack_cycle_ticks": ticks, "base_attack_range_tiles": reach,
            }
    rules = []

    def rule(name, family, event, guard, facts, evidence, **fields):
        rules.append({"id": "rule." + name, "family": family, "event_ref": "event." + event,
                      "guard": guard, "contract": facts, "basis": evidence, **fields})

    rule("time.tick", "time", "activity.requested", all_of(),
         ["Use a 600 ms authoritative simulation tick; don't speed time up for a small population.",
          "Server actions become eligible at the next processing tick; local tab/context-menu rendering is not itself a server action.",
          "The ordering of shared operations is deterministic and persisted, not a claim that this document has captured every OSRS PID tie."],
         basis("game_tick"), tick_ms=600)
    rule("movement.path", "movement", "movement.requested",
         all_of(eq("event.destination_in_supported_source_area", True)),
         ["Use real mapped tiles/floors, object footprints, walls and corner blockers.",
          "BFS neighbor order is W,E,S,N,SW,SE,NW,NE; shortest path first within a 128x128 grid centered as specified by source.",
          "Requested object south-west tile must lie in the 101x101 candidate area; unreachable fallback searches 21x21 around it, path length <100, minimizing squared destination distance then path length; tie iteration west then south.",
          "Retain at most 25 path corners. Recheck actual walk edges as the environment changes.",
          "Moving toward an unreachable interactable may stop nearby; it does not cause the interaction's success event.",
          "NPC/player targets may repath each tick; static unreachable object/tile targets stop. Diagonal corner clipping and attacks through closed blockers are forbidden."],
         basis("pathfinding"), neighbor_order=["W", "E", "S", "N", "SW", "SE", "NW", "NE"],
         binding_refs=["binding.world.source_layout"])
    rule("movement.step", "movement", "movement.requested",
         all_of(eq("event.same_plane", True), eq("event.destination_walkable", True), eq("event.crossed_edges_clear", True),
                eq("event.diagonal_cardinal_edges_clear", True)),
         ["Walking consumes one adjacent legal step per tick. Running consumes up to two legal adjacent steps, checking BOTH edges and intermediate tile.",
          "A ladder/stair changes plane or dungeon coordinate via its source link only after approach; a plane change is never inferred from x/y proximity.",
          "A closed tutorial gate additionally needs its tutorial permission even if a client predicts the open object.",
          "Projectile/line-of-sight masks differ from walk masks: the rat fence can block walking while permitting an allowed shot."],
         basis("pathfinding", "game_tick"), walk_tiles_per_tick=1, run_tiles_per_tick=2,
         binding_refs=["binding.world.source_layout"])
    rule("movement.energy", "movement", "setting.changed", all_of(),
         ["Maximum 10000 units; UI floors units/100. Switching run on requires >=100 units. A one-step walk does not spend run energy even with run enabled.",
          "On two-step running ticks apply current post-2025 weight/agility drain. Inventory and equipped weight count, clamped 0..64 kg.",
          "When not running and eligible, recover energy. No offline recovery. At exhaustion run switches off.",
          "Death and quest completion restore run energy. No unconditional infinite running in the tutorial."],
         basis("energy", classification="assumption", assumptions=["run_rounding"]),
         formula_refs=["formula.run.drain", "formula.run.recovery"], maximum_units=10000)
    rule("inventory.capacity", "inventory", "inventory.pickup_requested",
         all_of(ge("event.quantity", 1), eq("event.owned_or_pickable", True), eq("event.reachable", True), eq("event.result_fits", True)),
         ["28 backpack slots. Each ordinary unstackable unit consumes one slot; an existing compatible stack consumes no additional slot.",
          "An equipped object is not also in inventory. Note and unnoted forms have distinct source identities.",
          "No negative quantity, stack overflow, or silent replacement of another item.",
          "If a one-item pickup cannot fit, keep the item on the ground and make no inventory or XP mutation.",
          "Take stackable quantities only up to the source maximum; overflow is a rejected transaction rather than integer wrap."],
         basis("inventory", "stackable_items", "tutorial_transcript"), slots=28, maximum_stack=2147483647)
    rule("inventory.drop", "inventory", "inventory.drop_requested",
         all_of(ge("event.quantity", 1), eq("event.owns_quantity", True)),
         ["Move ownership from inventory to a ground instance, not a second copy. Picking back up does not earn XP.",
          "Tutorial drops despawn after 30 seconds (50 ticks), before other players could see them. Tutorial drop trading cannot work.",
          "Mainland ordinary drop visibility/expiry requires the bound ground-item policy; do not use a grave as an ordinary dropped-item bag."],
         basis("tutorial_island", "inventory"), tutorial_despawn_ticks=50,
         mainland_ordinary_drop_expiry_ticks=None)
    rule("equipment.equip", "equipment", "equipment.equip_requested",
         all_of(eq("event.item_owned_in_inventory", True), eq("event.level_requirements_met", True),
                eq("event.equipment_action_unlocked", True), eq("event.result_fits", True)),
         ["Move the item to its source slot and return replaced equipment to inventory atomically.",
          "Shortbow occupies weapon+shield. Equipping it with both occupied and a full inventory fails; freeing one extra slot permits the swap.",
          "Ammunition is usable only when equipped in ammo; arrows in the backpack do not fire.",
          "Equipping another weapon into a two-handed setup releases the weapon; equipping a shield unequips the bow.",
          "Tools may be used from backpack or equipped; wielding a bronze pickaxe is not required to mine.",
          "On Tutorial Island equipping is denied before Equipment Stats instruction, not merely before receiving a weapon."],
         basis("equipment", "tutorial_transcript", "bronze_pickaxe"))
    rule("equipment.unequip", "equipment", "equipment.unequip_requested",
         all_of(eq("event.slot_owned", True), eq("event.result_fits", True)),
         ["With no free space/compatible stack, leave equipment unchanged and show full-inventory feedback.",
          "Unequipping ammunition merges a compatible existing inventory stack rather than requiring a new slot."],
         basis("equipment", "tutorial_transcript", "inventory"))
    rule("xp.level", "xp", "activity.roll", all_of(),
         ["XP is integer tenths, never floats in durable state. Skill level is computed from cumulative thresholds, not number of actions.",
          "Global XP cap is 2000000000 tenths; later actions still function without adding XP.",
          "Displayed XP is floor(xp_tenths/10); displayed drops are the change in displayed total, so fractional awards accumulate.",
          "Use the source cumulative level formula, not a constant 100 XP per level."],
         basis("experience"), formula_refs=["formula.xp.level_threshold"],
         thresholds_tenths={"1": 0, "2": 830, "3": 1740, "4": 2760, "5": 3880, "10": 11540},
         maximum_xp_tenths=2000000000)
    trainable = ["attack", "strength", "defence", "ranged", "prayer", "magic", "mining", "smithing", "fishing", "cooking", "firemaking", "woodcutting"]
    rule("tutorial.xp_cap", "tutorial", "activity.roll", all_of(eq("state.tutorial.departed", False)),
         ["Only the listed source skills can train on the island, only to level 3. Hitpoints XP remains 11540 regardless of damage dealt.",
          "Choose XP boundary policy explicitly; don't replace it with source-claimed 174.0 XP.",
          "Finishing Learning the Ropes before departure does NOT lift the cap. Successful mainland teleport does.",
          "Changing account experience choice, reconnecting or casting at an off-island target cannot bypass the cap."],
         basis("tutorial_island", "copper_rocks", classification="assumption", assumptions=["tutorial_xp_boundary"]),
         skill_refs=["skill." + s for s in trainable], level_cap=3, hp_xp_award_tenths=0,
         cap_below_next_level_tenths=2759)
    rule("tutorial.restrictions", "tutorial", "activity.requested", all_of(eq("state.tutorial.departed", False)),
         ["Deny early fishing/chopping/mining/smelting/smithing/equipping until the corresponding shared lesson action unlocks.",
          "Reject oak chopping (requires 15) and smithing anything except a bronze dagger even if a character somehow has materials.",
          "Starting-house, survival, Chef, Quest ladder, mine gate, combat ladder, bank/account and chapel exits are guarded by their actual lesson graph.",
          "Only one credited melee rat kill, then one credited ranged rat kill advances. Further melee after first kill is blocked; further ranged after second kill is blocked.",
          "After ranged gear is granted the rat pen cannot be re-entered. Arrows must be fired from outside; actually moving to the gate is not a ranged hit.",
          "A chicken cannot be attacked by melee/ranged, and only the first Wind Strike on a chicken is allowed. Magic on rats after spell unlock is source-described optional training, still capped.",
          "Public chat, trading, poll voting and Node crossing by a normal account are unavailable. Poll inspection and account-information dialogue are still mandatory.",
          "Damage to the player is nonfatal: HP may reach 1, never 0. This protection ends on departure, not at the tutorial quest reward."],
         basis("tutorial_transcript", "tutorial_island"), tutorial_minimum_hp=1)
    for ore in ("copper", "tin"):
        rule("mining." + ore, "mining", "activity.requested",
             all_of(ge("state.levels.mining", 1), eq("state.tools.usable_pickaxe", True),
                    ge("state.inventory.free_slots", 1), eq("event.target_ready", True), eq("event.reachable", True),
                    eq("event.lesson_action_unlocked", True)),
             ["A success consumes the rock's available ore, adds one unnoted ore and awards XP once. A failed roll adds nothing and keeps trying while valid.",
              "A competing depletion before this actor's successful commit gives no ore or XP. Revalidate the source rock generation.",
              "Bronze pickaxe speed is 8 ticks, not a guaranteed ore every tick. Mining level affects success, pickaxe tier affects cadence.",
              "A full inventory stops gathering with explicit feedback, without depleting the rock or adding XP.",
              "For mainland copper choose the existing East Lumbridge Swamp mine and legitimate travel/bank route, not a rock placed beside the tutorial exit."],
             basis(ore + "_rocks", "bronze_pickaxe", "mining"),
             skill_ref="skill.mining", required_level=1,
             produced=[item("ore." + ore)], consumed=[],
             xp_awards=[{"skill_ref": "skill.mining", "xp_tenths": 175}],
             roll={"formula_ref": "formula.skilling.success", "low": 100, "high": 350, "denominator": 256},
             timing={"cycle_ticks": 8, "resource_respawn_ticks": 4, "first_roll_basis_ref": "assumption.environment_timers"},
             depletion="one_success", object_ref="object.rock." + ore)
    rule("woodcutting.normal", "woodcutting", "activity.requested",
         all_of(ge("state.levels.woodcutting", 1), eq("state.tools.usable_axe", True), ge("state.inventory.free_slots", 1),
                eq("event.target_ready", True), eq("event.reachable", True), eq("event.lesson_action_unlocked", True)),
         ["Use bronze axe from inventory/equipment. Each successful roll produces one log and 25 XP; an ordinary tree always becomes a stump after one log.",
          "No forestry timer/prolonged multi-log behavior is substituted for an ordinary tree.",
          "Failed rolls neither produce logs nor XP; inventory fullness, walking away or another player's depletion stops/revalidates the action."],
         basis("tree", "action_lengths"), skill_ref="skill.woodcutting", required_level=1,
         produced=[item("logs.normal")], consumed=[], xp_awards=[{"skill_ref": "skill.woodcutting", "xp_tenths": 250}],
         roll={"formula_ref": "formula.skilling.success", "low": 64, "high": 200, "denominator": 256},
         timing={"cycle_ticks": 4, "respawn_ticks_inclusive": [60, 100], "phase_assumption_ref": "assumption.environment_timers"},
         object_ref="object.tree.normal")
    rule("fishing.shrimps", "fishing", "activity.requested",
         all_of(ge("state.levels.fishing", 1), eq("state.tools.small_net_in_inventory", True), ge("state.inventory.free_slots", 1),
                eq("event.correct_fishing_spot", True), eq("event.reachable", True), eq("event.lesson_action_unlocked", True)),
         ["One net, no bait; a successful shrimp roll awards one raw shrimp and 10 Fishing XP.",
          "A failed 6-tick resource roll awards nothing. Do not interpret the fishing animation as a catch.",
          "At level 15+ small-net fishing rolls anchovies first (low24/high128), then shrimp if that fails; do not keep the tutorial's shrimp-only filter on mainland fishing forever.",
          "Moving fishing spots must remain source-bound entities; invalid/relocated spots stop or repath the action."],
         basis("raw_shrimps", "action_lengths"), skill_ref="skill.fishing", required_level=1,
         produced=[item("shrimps.raw")], consumed=[], xp_awards=[{"skill_ref": "skill.fishing", "xp_tenths": 100}],
         roll={"formula_ref": "formula.skilling.success", "low": 48, "high": 256, "denominator": 256, "domain": "Fishing 1..14; tutorial cap<=3"},
         timing={"cycle_ticks": 6, "phase_assumption_ref": "assumption.environment_timers"},
         object_ref="object.tutorial.fishing_spot")
    rule("firemaking.normal", "firemaking", "activity.requested",
         all_of(ge("state.levels.firemaking", 1), eq("state.tools.tinderbox_in_inventory", True),
                eq("event.owns_log", True), eq("event.valid_fire_tile", True), eq("event.lesson_action_unlocked", True)),
         ["Using tinderbox/logs places that owned log on the ground and attempts ignition. Failure is not destruction: the log remains to retry/pick up.",
          "Success replaces the log with a real fire and grants 40 Firemaking XP exactly once.",
          "Fire blocks another fire at that position. If two actors compete, only the successful owner burns their log; others' logs remain.",
          "After ignition try stepping W,E,S,N in that order; if all four blocked remain on the fire tile. Never teleport through a wall.",
          "When the fire expires it becomes ashes. Cooking requires it still exists at operation time; obtain another log/fire if it expires.",
          "Normal-log ignition probability is a crowdsourced assumption, not an asserted verified 100% success."],
         basis("firemaking", "fire", classification="assumption", assumptions=["fire_success", "environment_timers"]),
         skill_ref="skill.firemaking", required_level=1, consumed=[item("logs.normal")],
         produced_object_ref="object.fire.normal", expired_item_ref="item.ashes",
         xp_awards=[{"skill_ref": "skill.firemaking", "xp_tenths": 400}],
         roll={"formula_ref": "formula.skilling.success", "low": 64, "high": 512, "denominator": 256},
         timing={"attempt_cycle_ticks": 4, "lifetime_ticks_inclusive": [100, 199]},
         step_priority=["W", "E", "S", "N"])
    rule("cooking.dough", "cooking", "activity.requested",
         all_of(ge("state.inventory.flour", 1), ge("state.inventory.water_bucket", 1),
                eq("event.result_fits", True), eq("event.lesson_action_unlocked", True)),
         ["Use flour on bucket of water or vice versa. Dough is a material conversion, not bread and not quest delivery.",
          "No Cooking XP for mixing dough. Provisionally return empty pot and bucket, requiring one net additional slot.",
          "Full inventory must not consume both ingredients and silently drop the dough."],
         basis("bread_dough", classification="assumption", assumptions=["dough_containers"]),
         consumed=[item("flour.pot"), item("water.bucket")],
         produced=[item("bread.dough"), item("pot"), item("bucket")],
         xp_awards=[], timing={"single_conversion_ticks": 1}, deterministic=True)
    for name, raw, cooked, burnt, xp, low, high, lum_low, lum_high in [
        ("shrimps", "shrimps.raw", "shrimps.cooked", "shrimps.burnt", 300, 128, 512, 138, 532),
        ("bread", "bread.dough", "bread", "bread.burnt", 400, 118, 492, 128, 512),
    ]:
        rule("cooking." + name, "cooking", "activity.requested",
             all_of(ge("state.levels.cooking", 1), eq("event.owns_raw_input", True),
                    eq("event.facility_permits_recipe", True), eq("event.reachable", True),
                    eq("event.lesson_action_unlocked", True)),
             ["Consume one raw input; success produces the cooked variant and the listed XP, burn produces its burnt variant and zero XP.",
              "Bread requires a range. Shrimp can use a fire or range; its published ordinary fire/range success parameters are equal.",
              "The Lumbridge Cook-o-matic range requires Cook's Assistant completion and uses its separate lower-burn parameters.",
              "A disappearing fire/invalid range at action time cancels before consumption; walking interrupts the queue.",
              "Tutorial progress listens to real cooking outcomes; do not replace normal recipes with an 'advance tutorial' command."],
             basis("shrimps" if name == "shrimps" else "bread", "cooking", "action_lengths"),
             skill_ref="skill.cooking", required_level=1,
             consumed=[item(raw)], produced=[item(cooked)], failure_produced=[item(burnt)],
             xp_awards=[{"skill_ref": "skill.cooking", "xp_tenths": xp}], failure_xp_tenths=0,
             roll={"formula_ref": "formula.skilling.success", "low": low, "high": high, "denominator": 256,
                   "lumbridge_range": {"low": lum_low, "high": lum_high, "requires_quest_ref": "quest.cooks_assistant"}},
             timing={"make_x_first_ticks": 3 if name == "shrimps" else None, "make_x_repeat_ticks": 4,
                     "single_ticks": 1 if name == "bread" else None,
                     "note": "Source recipe/action table distinguishes single and Make-X. Null first-phase parameters are not an instant-action instruction."})
    rule("smelting.bronze", "smelting", "activity.requested",
         all_of(ge("state.levels.smithing", 1), ge("state.inventory.tin_ore", 1), ge("state.inventory.copper_ore", 1),
                eq("event.at_furnace", True), eq("event.reachable", True), eq("event.lesson_action_unlocked", True)),
         ["Consumes one tin and one copper to produce one bronze bar. This is deterministic: bronze smelting never uses iron's failure chance.",
          "Smithing XP is 6.2, not 6.25. A full inventory can still smelt because two slots become one.",
          "Anvil use cannot substitute for furnace use. Missing either ore leaves all items and XP unchanged."],
         basis("bronze_bar", "smithing"), skill_ref="skill.smithing", required_level=1,
         consumed=[item("ore.tin"), item("ore.copper")], produced=[item("bar.bronze")],
         xp_awards=[{"skill_ref": "skill.smithing", "xp_tenths": 62}], deterministic=True,
         timing={"single_ticks": 6, "make_x_first_ticks": 4, "make_x_repeat_ticks": 5},
         object_ref="object.furnace.tutorial")
    rule("smithing.bronze_dagger", "smithing", "activity.requested",
         all_of(ge("state.levels.smithing", 1), ge("state.inventory.bronze_bar", 1), ge("state.inventory.hammer", 1),
                eq("event.at_anvil", True), eq("event.reachable", True), eq("event.lesson_action_unlocked", True)),
         ["The smithing menu must select a dagger; consume one bar, keep hammer, produce one dagger and 12.5 Smithing XP.",
          "Requires a real anvil. No random failure chance. A full inventory can convert bar to dagger in place.",
          "On Tutorial Island other bronze products remain forbidden despite shared-system recipes existing elsewhere."],
         basis("bronze_dagger", "smithing", "tutorial_transcript"), skill_ref="skill.smithing", required_level=1,
         consumed=[item("bar.bronze")], produced=[item("dagger.bronze")], tools=[item("hammer")],
         xp_awards=[{"skill_ref": "skill.smithing", "xp_tenths": 125}], deterministic=True,
         timing={"cycle_ticks": 5, "menu_close_ticks": 1}, object_ref="object.anvil.tutorial")
    rule("combat.melee", "melee", "activity.requested",
         all_of(eq("event.attackable_npc", True), eq("event.melee_reach_and_edges_clear", True),
                eq("event.attack_cooldown_ready", True), eq("event.lesson_action_unlocked", True)),
         ["Use the selected source weapon category, attack style, bonuses, effective levels, opposed accuracy rolls and damage roll; not guaranteed damage each tick.",
          "Player successful damage roll 0 becomes 1; misses remain 0. Cap actual damage at target HP and record actual hitpoints removed.",
          "Dagger/sword/unarmed attack cycle 4 ticks; axe/pickaxe 5. Movement, eating and switching targets interact with the shared action queue, never grant a kill directly.",
          "Accurate trains Attack, aggressive Strength, defensive Defence, 4 XP per damage. Only weapon categories that offer controlled may split XP.",
          "Normal-account mainland damage also awards HP XP. Tutorial suppresses HP XP and clamps incoming lethal damage to keep the player at 1 HP."],
         basis("melee_dps", "combat_options", "action_lengths"),
         formula_refs=["formula.combat.effective_level", "formula.combat.maximum_hit", "formula.combat.accuracy", "formula.combat.damage"],
         xp_per_damage_tenths=40, hp_xp_formula_ref="formula.combat.hp_xp")
    rule("combat.ranged", "ranged", "activity.requested",
         all_of(eq("state.equipment.weapon", "item.shortbow"), eq("state.equipment.ammo", "item.arrow.bronze"),
                ge("state.equipment.ammo_quantity", 1), eq("event.line_of_sight", True),
                eq("event.within_attack_range", True), eq("event.attack_cooldown_ready", True),
                eq("event.lesson_action_unlocked", True)),
         ["Shortbow requires equipped compatible arrows. Consume one ammo unit at an accepted shot, including a miss; a rejected request consumes none.",
          "Accurate grants +3 effective Ranged and uses 4 ticks; rapid has no style level boost and uses 3 ticks; longrange uses 4 ticks and range 9 rather than 7.",
          "Accurate/rapid give 4 Ranged XP per damage. Longrange gives 2 Ranged +2 Defence XP per damage; neither bypasses tutorial caps.",
          "Outside-pen shots require true projectile LOS even though melee traversal is blocked.",
          "Without an Ava device, each shot removes one equipped arrow: 20% are lost, otherwise a ground arrow can be recovered. Arrow loss is independent of accuracy.",
          "Projectile launch/impact requires source bindings. There is no assumption that every arrow reappears or that every shot is immediately applied."],
         basis("shortbow", "bronze_arrow", "arrows", "combat_options", "ranged_dps"),
         formula_refs=["formula.combat.effective_level", "formula.combat.maximum_hit", "formula.combat.accuracy"],
         attack_range_tiles={"accurate": 7, "rapid": 7, "longrange": 9},
         cycle_ticks={"accurate": 4, "rapid": 3, "longrange": 4},
         ammo_per_shot=1, hp_xp_formula_ref="formula.combat.hp_xp",
         projectile_binding_ref="binding.combat.projectile_timing", ammunition_break_probability={"numerator": 1, "denominator": 5})
    rule("magic.wind_strike", "magic", "activity.requested",
         all_of(ge("state.levels.magic", 1), ge("state.inventory.air_runes", 1), ge("state.inventory.mind_runes", 1),
                eq("event.attackable_npc", True), eq("event.line_of_sight", True), eq("event.within_attack_range", True),
                eq("event.attack_cooldown_ready", True), eq("event.lesson_action_unlocked", True)),
         ["A valid cast consumes 1 air +1 mind rune and awards 5.5 base Magic XP even on a splash. Reject early/invalid/no-rune casts without consumption or XP.",
          "Normal casting adds 2 Magic XP per HP damage. HP XP also applies on mainland only. Defensive casting adds 1 Defence and the source 1.33 Magic term per damage; it is not selected implicitly.",
          "At Magic 1..4 base max hit=2; 5..8=4; 9..12=6; >=13=8. This post-2024 elemental scaling must not be replaced with permanent max hit 2.",
          "NPC Magic defence uses Magic level +9, not Defence level. Apply elemental weakness only if the selected source variant actually has one.",
          "A valid tutorial chicken cast, not an arbitrary magic animation, emits the tutorial completion event. Don't require a chicken kill."],
         basis("wind_strike", "combat_spells", "magic", "magic_dps"), spell_ref="spell.wind_strike",
         consumed=[item("rune.air"), item("rune.mind")], base_xp_awards=[{"skill_ref": "skill.magic", "xp_tenths": 55}],
         damage_magic_xp_tenths=20, attack_range_tiles=10, cycle_ticks=5,
         base_max_hit_levels=[{"minimum_level": 1, "max_hit": 2}, {"minimum_level": 5, "max_hit": 4}, {"minimum_level": 9, "max_hit": 6}, {"minimum_level": 13, "max_hit": 8}],
         formula_refs=["formula.combat.magic_attack", "formula.combat.magic_defence_npc", "formula.combat.accuracy"],
         hp_xp_formula_ref="formula.combat.hp_xp", projectile_binding_ref="binding.combat.projectile_timing")
    rule("magic.home_teleport", "magic", "activity.requested",
         all_of(eq("event.not_in_combat", True), eq("event.home_teleport_ready", True),
                eq("event.destination_authorized", True), eq("event.spell_unlocked", True)),
         ["No rune cost, no Magic XP, no Magic level requirement. Default animation takes 24 ticks; cooldown is 3000 ticks.",
          "Combat prevents starting and interrupts the channel. Movement/another interrupting action cancels before transport; provisionally begin cooldown only on completed transport.",
          "Tutorial permission is granted only by completed departure dialogue after the chicken-cast quest completion.",
          "Departure item reconciliation happens only once at actual successful transport, never on selecting the spell icon or opening a confirmation dialogue.",
          "On tutorial departure select the experience-dependent arrival rather than assuming the ordinary Home Teleport destination. The ordinary spell map marks center 3221,3218 radius 2; it is not an exact tutorial spawn proof."],
         basis("home_teleport", "tutorial_transcript", "tutorial_island"),
         spell_ref="spell.lumbridge_home_teleport", channel_ticks=24, cooldown_ticks=3000,
         rune_cost=[], xp_awards=[], interrupted_cooldown_policy="provisional_no_cooldown_until_transport_completes",
         cooldown_start_basis=basis("home_teleport", classification="inference"),
         binding_ref="binding.world.source_layout")
    rule("prayer.thick_skin", "prayer", "prayer.toggled",
         all_of(ge("state.levels.prayer", 1), eq("event.prayer_ui_unlocked", True),
                eq("event.prayer_points_positive", True)),
         ["Thick Skin is available at Prayer 1 and gives +5% Defence. The next prayer requires level 4 and is not tutorial-unlockable.",
          "Prayer toggles are applied on the next game tick; use a fractional drain accumulator, not one full point each click.",
          "Without prayer bonus it drains one point per 36 seconds (60 ticks). Zero prayer points disables active prayers.",
          "Merely opening Prayer for Brother Brace is sufficient for the lesson; activation/burying are genuine optional shared actions, not forced stages."],
         basis("prayer", "action_lengths", "tutorial_transcript"), prayer_ref="prayer.thick_skin",
         defence_multiplier={"numerator": 105, "denominator": 100}, zero_bonus_drain_ticks_per_point=60)
    rule("prayer.altar", "prayer", "altar.prayed",
         all_of(eq("event.reachable", True), eq("event.is_prayer_altar", True)),
         ["Recharge to the actual base Prayer level; no XP or item consumption. At full points give already-full feedback without increasing points.",
          "An altar restores prayer points, not skill XP and not the tutorial progression marker."],
         basis("prayer", "tutorial_transcript"), xp_awards=[])
    rule("prayer.bones", "prayer", "activity.requested",
         all_of(eq("event.owns_unnoted_bones", True), eq("event.action_cooldown_ready", True)),
         ["Bury one owned bone; ordinary Bones award 4.5 Prayer XP and take 2 ticks per bone.",
          "Tutorial rats drop a distinct Bones (Tutorial Island) item, not a normal-bones numeric alias. Provisionally award the ordinary 4.5 XP; the variant article omits XP, so this numeric match is an inference, not verified variant data.",
          "Tutorial cap still applies; bones are optional and dropping/recovering them never awards XP."],
         basis("bones", "tutorial_bones", "action_lengths"), cycle_ticks=2,
         ordinary_xp_tenths=45, tutorial_variant_xp_tenths=45,
         tutorial_variant_basis=basis("bones", "tutorial_bones", classification="inference"))
    rule("food.healing", "food", "food.eaten",
         all_of(eq("event.owns_edible_item", True), eq("event.eat_cooldown_ready", True)),
         ["Consume one food; shrimp restore 3 HP and bread 5, capped at base HP. Cooked food can be eaten even when no healing is needed.",
          "Burnt variants are not edible. Food is not an XP reward.",
          "Ordinary shrimp/bread add a 3-tick penalty to the next attack and prevent another ordinary food consumption for 3 ticks; no free spam-heal path."],
         basis("shrimps", "bread", "food"), heals=[{"item_ref": "item.shrimps.cooked", "hp": 3}, {"item_ref": "item.bread", "hp": 5}],
         eat_delay_ticks=3, attack_delay_ticks=3)
    rule("hitpoints.regeneration", "food", "activity.roll", all_of(),
         ["Ordinary player regeneration is one HP per minute (100 ticks), capped at current base maximum. Do not restore full HP each combat start or reconnect."],
         basis("hitpoints", "game_tick"), ticks_per_hp=100)
    for direction in ("deposit", "withdraw"):
        rule("bank." + direction, "banking", "bank." + direction + "_requested",
             all_of(eq("state.ui.bank_open", True), eq("event.bank_access_authorized", True),
                    eq("event.reachable_bank_session", True), ge("event.quantity", 1)),
             ["Move items between the same persistent bank and inventory; all normal source banks access the same account, not location-specific balances.",
              "Banking unstackable items groups compatible definitions into a bank stack. Notes may be deposited as their unnoted counterpart.",
              "Support 1/5/10/last-X/X/All and withdrawals All-but-one. Unnoted withdrawals fill only remaining inventory capacity; an existing compatible stack can receive more.",
              "Deposit/withdraw requests transfer only available capacity/quantity and never lose the remainder. Zero/negative/spoofed quantities are rejected.",
              "A bank-open flag without a valid source bank interaction session is not remote banking permission.",
              "The first tutorial bank has 25 coins and ordinary deposit/withdraw controls. Closing or reopening it does not recreate withdrawn coins.",
              "Use the castle upper bank for the journey; the Recipe for Disaster cellar bank is not unlocked by Cook's Assistant."],
             basis("bank", "tutorial_transcript", "learning_the_ropes", "lumbridge_castle"),
             base_capacity=400, partial_transfers=True)
    rule("shop.lumbridge", "shop", "shop.buy_requested",
         all_of(eq("event.reachable_shop_session", True), eq("event.item_in_stock", True), eq("event.result_fits", True), eq("event.can_pay", True)),
         ["Use finite shared normal-account stock per world, not a personal infinite vendor.",
          "Reprice per unit within buy/sell 1/5/10/50 batches. Reduce stock and inventory coins in the same transaction; no overdrafts or stale-price duplication.",
          "Stop a batch at capacity/coins/stock exhaustion, preserving untransferred stock and money.",
          "For a stocked row, one item replenishes/destocks toward base stock each restock interval. Restock values are ticks per pinned template documentation.",
          "Base prices for pot and bucket are 1 and 2 coins. Tinderbox/hammer base prices are 1. Tools do not become free quest rewards.",
          "Only the listed required interactive rows are contracted here; other visible shop stock must remain accurately presented and explicitly unavailable if outside M1, not invented/hidden."],
         basis("lumbridge_general_store", "store_line_docs", "shop"),
         shop_sell_multiplier_per_mille=1300, shop_buy_multiplier_per_mille=400, delta_per_mille=30,
         required_stock=[
             {"item_ref": "item.pot", "base_stock": 5, "restock_ticks": 10, "base_buy_price": 1, "base_item_value": 1},
             {"item_ref": "item.bucket", "base_stock": 3, "restock_ticks": 10, "base_buy_price": 2, "base_item_value": 2},
             {"item_ref": "item.tinderbox", "base_stock": 2, "restock_ticks": 100, "base_buy_price": 1, "base_item_value": 1},
             {"item_ref": "item.hammer", "base_stock": 5, "restock_ticks": 100, "base_buy_price": 1, "base_item_value": 1},
         ], formula_refs=["formula.shop.buy", "formula.shop.sell"])
    rule("shop.sell", "shop", "shop.sell_requested",
         all_of(eq("event.reachable_shop_session", True), eq("event.owns_quantity", True), eq("event.tradeable_item", True)),
         ["General store accepts tradeable source items; nontradeable/quest-restricted variants require an explicit refusal.",
          "Stock increases one per sold unit and prices are recalculated. The source minimum purchase rate is 10% of base item value; integer flooring means cheap items can sell for zero.",
          "A coins result must fit its stack bound. Selling is not deletion followed by an unguarded grant.",
          "Overstock buy pricing has an explicit source conflict; the selected linear model is provisional."],
         basis("shop", "shop_module", classification="assumption", assumptions=["shop_overstock"]),
         formula_refs=["formula.shop.buy", "formula.shop.sell"])
    rule("goblin.level_2", "combat_npc", "activity.requested", all_of(eq("event.bound_variant", "unarmed_level_2_table_1")),
         ["Use a real unarmed level-2 Lumbridge goblin with 5 HP, not a renamed/tutorial rat or a level-5/armed variant.",
          "It is nonaggressive initially, retaliates when attacked, uses crush on a 4-tick cycle, max hit 1, and respawns in 35 ticks.",
          "NPC death consumes that life once, creates source drops once and emits one credited kill; reconnect cannot duplicate a kill/drop.",
          "Numerically similar spawn variants still require source binding to table 1; numeric NPC IDs are intentionally not selected without the other worker's map extraction."],
         basis("goblin", "monster_infobox_docs"), npc_ref="npc.goblin.level_2",
         stats={"hp": 5, "attack": 1, "strength": 1, "defence": 1, "magic": 1, "ranged": 1,
                "attack_crush_bonus": -21, "melee_strength_bonus": -15,
                "defence_stab": -15, "defence_slash": -15, "defence_crush": -15, "defence_magic": -15,
                "defence_light": -15, "defence_standard": -15, "defence_heavy": -15},
         max_hit=1, attack_cycle_ticks=4, respawn_ticks=35,
         loot_table_ref="loot.goblin.level_2_table_1")
    rule("combat.tutorial_rat", "combat_npc", "activity.requested", all_of(eq("event.is_tutorial_rat", True)),
         ["Tutorial rat is size 2, 3 HP, combat level 3, max hit 1, nonaggressive, 4-tick stab.",
          "Attack/Strength/Defence/Magic/Ranged are 1. Stab/slash/crush and all ranged defence bonuses are -100; Magic defence bonus is 0.",
          "On death drop exactly one distinct tutorial-bones item. Do not choose the unused or alternate-2020 source variants.",
          "Kill credit is not necessarily last hit: the source describes a player doing 2/3 HP getting credit when another finishes. Provisionally give credit to greatest credited damage in that life, breaking equal-damage ties by earliest contribution."],
         basis("tutorial_rat", "tutorial_island"), npc_ref="npc.tutorial_rat", hp=3, size_tiles=2, cycle_ticks=4,
         guaranteed_drops=[item("bones.tutorial")], negative_roll_assumption_ref="assumption.negative_combat_roll",
         credit_policy={"selection": "greatest_credited_damage", "tie": "earliest_contribution", "basis": basis("tutorial_island", classification="inference")})
    rule("death.retention", "death", "player.died", all_of(eq("state.tutorial.departed", True)),
         ["Unsafe non-PvP, non-instance normal-account death is the M1 domain. No skull and no Protect Item keeps three most valuable carried/equipped items by effective GE/alchemy value.",
          "Banked items, XP, quest state, delivered ingredients and tutorial completion are never death drops.",
          "Protect Item adds one retained item; a skull normally removes the base three. Neither is silently activated for a starter account.",
          "Rank by source-provided per-unit effective value, not equipment-first or inventory-first value. Equal-value handling is explicitly provisional.",
          "Preserve original inventory/equipment layout metadata for July-2026 grave restoration. A grave is owner-private and world-independent.",
          "The source first Death tutorial triggers when items are actually lost and the tutorial has not been seen, not for an empty-inventory safe death."],
         basis("items_kept_on_death", "grave", "death_dialogue", classification="assumption", assumptions=["death_tie_break"]),
         kept_items_unskulled=3, protect_item_extra=1, valuation_binding_ref="binding.death.valuation")
    rule("death.first_office", "death", "player.died",
         all_of(eq("state.death.tutorial_seen", False), eq("event.items_lost", True), eq("state.tutorial.departed", True)),
         ["Transfer to Death's Office for the forced tutorial; retained items remain owned, lost items remain recoverable.",
          "The introduction explains gravestones; the player must complete fees, timer and kept-items-interface topics before saying done and leaving via portal.",
          "Early exit is refused. The grave waits without countdown throughout this first tutorial, even if the player disconnects.",
          "On permitted portal departure, normal Lumbridge respawn and 1500 active grave ticks begin. No automatic retrieval or free replacement inventory is granted."],
         basis("death_dialogue", "grave"), graph_ref="graph.death.first_office", grave_timer_ticks=1500,
         required_topics=["fees", "timer", "kept_items"])
    rule("death.grave_timer", "death", "activity.roll", all_of(eq("state.death.grave_exists", True)),
         ["Countdown starts after respawning/first-Office portal exit, with 15 minutes (1500 ticks).",
          "Pause while logged out, more than 10 seconds idle, or the grave UI is open. Bank UI does not pause it. Persist active-time remaining, not wall-clock expiry during logout.",
          "Owner can check/loot from up to 7 tiles with line of sight, on any world.",
          "At expiry, transfer remaining items to Death's Office storage once. Early remote collection via Death also transfers them and uses Office fees."],
         basis("grave", "death_fees"), duration_ticks=1500, idle_pause_after_ms=10000, interaction_range_tiles=7)
    rule("death.reclaim", "death", "grave.reclaimed",
         all_of(eq("event.owner_matches", True), eq("event.line_of_sight", True), eq("event.within_seven_tiles", True),
                eq("event.can_pay_recovery_fee", True)),
         ["Grave recovery below 100000 value is free; exact fee bands are recorded separately. Starter resources need no invented flat death fee.",
          "Transfer only items that fit, leaving the rest recoverable. Restore old layout when possible; honor the source auto-equip setting rather than blindly equipping all loot.",
          "Charge for the items actually reclaimed once, not the whole grave repeatedly; a second identical recovery cannot duplicate items or fees.",
          "Fees come from Death's coffer then bank per the pinned fees page; current starter fee=0 does not depend on unresolved payment-order differences in old dialogue."],
         basis("grave", "death_fees"), fee_formula_ref="formula.death.grave_fee")
    rule("death.office_reclaim", "death", "grave.reclaimed",
         all_of(eq("event.in_deaths_office", True), eq("event.owner_matches", True), eq("event.can_pay_recovery_fee", True)),
         ["Expired items are retained indefinitely in a 120-entry Office store, not dropped publicly after 15 minutes.",
          "Below 100000 per-item value is free; otherwise 5% for a normal account. Early remote grave collection uses the same higher Office rate.",
          "Inventory capacity and exact-once ownership apply. Office has a source 120-entry limit; do not promise infinite storage across repeated deaths.",
          "Exact excess-entry deletion order at the 120-slot boundary is not established in these references."],
         basis("deaths_office", "death_fees"), storage_slots=120, fee_formula_ref="formula.death.office_fee",
         overflow_eviction_order=None)
    rule("death.repeat", "death", "player.died", all_of(eq("state.death.grave_exists", True)),
         ["Newly lost items normally join the old grave at its original location; timer refreshes if contents change.",
          "Before adding new items, excess ordinary unstackables over 28 per item type in the old grave go to Death; then add incoming items.",
          "Old bones/ores and the listed restricted resources go to Office before incoming resources arrive.",
          "Old cooked food/potions become supply piles beneath the old grave and last one hour, instead of staying there.",
          "If the source setting enables supply piles, new food/potions drop at death position instead of entering the grave. Support both settings; don't silently choose a fresh-account default.",
          "Entrana/Wilderness/PvP exceptions are outside this normal Lumbridge domain and cannot be claimed implemented by this rule."],
         basis("grave"), grave_slots=120, supply_pile_lifetime_ticks=6000)
    kit = [item("axe.bronze"), item("pickaxe.bronze"), item("tinderbox"), item("fishing_net.small"),
           item("shrimps.cooked"), item("dagger.bronze"), item("sword.bronze"), item("shield.wooden"),
           item("shortbow"), item("arrow.bronze", 25), item("rune.air", 25), item("rune.mind", 15),
           item("bucket"), item("pot"), item("bread"), item("rune.water", 6), item("rune.earth", 4), item("rune.body", 2)]
    rule("tutorial.departure", "tutorial", "teleport.completed",
         all_of(eq("state.tutorial.departure_authorized", True), eq("state.quests.learning_the_ropes.status", "completed"),
                eq("state.tutorial.departed", False), eq("event.destination_matches_experience", True)),
         ["The listed 18-kind kit is source-backed as standard provisions, NOT a source-observed pre-dialogue inventory.",
          "Tutorial-gathered resources and extra runes do not transfer as unlimited mainland stock.",
          "Provisional reconciliation clears tutorial inventory/equipment/bank and places this kit in inventory with exactly 25 bank coins. No hammer is listed in the source kit.",
          "One 'shrimps' item represents the source couple of shrimp; it is not two inventory items.",
          "Keep all earned skill XP, Learning the Ropes' 1 QP and completion. Do not reset skills to an invented fixed departure level.",
          "Replaying teleport completion, reconnecting or re-reading reward UI must never reapply reconciliation to mainland possessions."],
         basis("learning_the_ropes", "tutorial_island", classification="assumption", assumptions=["departure_reconciliation"]),
         kit=kit, bank=[item("coins", 25)], ledger_key="tutorial_departure_v1")
    rule("persistence.acknowledged_state", "persistence", "session.reconnected", all_of(),
         ["Acknowledge only committed state; inventory/XP/progression/reward and operation-id ledger are atomic.",
          "Duplicate operation ids replay their result, not their side effects. A new authenticated session epoch invalidates old commands without wiping character state.",
          "Rebuild current UI and hint from persisted stage; do not replay on-entry item grants or rewind to a previous instructor.",
          "Do not auto-finish queued gathering/combat/teleport work on reconnect. Restore acknowledged results, cancel/revalidate unfinished work with real source guards.",
          "The same invariant applies to process restart, bank transfer, partial quest delivery, death topics, mill counts, quest reward and departure reconciliation."],
         basis("source.project.prompt", classification="engineering_contract"))

    formulas = [
        {"id": "formula.skilling.success", "inputs": ["level", "low", "high"], "output": "integer success count of 256 equally likely rolls",
         "expression": "clamp(1 + floor((low*(99-level) + high*(level-1) + 49)/98), 0, 256)",
         "domain": "level 1..99, no invisible boosts; success if integer rng in [0,255] is below output",
         "basis": basis("skilling_success_rate")},
        {"id": "formula.xp.level_threshold", "inputs": ["level"], "output": "xp_tenths",
         "expression": "10 * floor(sum(floor(n + 300*2**(n/7)) for n in 1..level-1)/4)",
         "domain": "source base levels 1..99", "basis": basis("experience")},
        {"id": "formula.run.drain", "inputs": ["agility", "weight_kg"], "output": "run energy units per two-step tick",
         "expression": "floor(floor(60 + 67*clamp(weight_kg,0,64)/64) * (300-agility)/300)",
         "basis": basis("energy", classification="assumption", assumptions=["run_rounding"])},
        {"id": "formula.run.recovery", "inputs": ["agility"], "output": "run energy units per eligible tick",
         "expression": "floor(agility/10)+15", "basis": basis("energy")},
        {"id": "formula.combat.effective_level", "inputs": ["current_level", "prayer_multiplier", "style_bonus"],
         "expression": "floor(current_level*prayer_multiplier)+style_bonus+8",
         "domain": "M1 player without void/boosting gear; NPCs use source level+9. Style bonus=3 for corresponding accurate/aggressive/defensive, 1 controlled, 0 otherwise; ranged accurate=3.",
         "basis": basis("melee_dps", "ranged_dps", "maximum_ranged_hit")},
        {"id": "formula.combat.maximum_hit", "inputs": ["effective_strength", "equipment_strength"],
         "expression": "floor((effective_strength*(equipment_strength+64)+320)/640)",
         "domain": "Player melee/ranged without extra gear modifiers; use melee or ranged strength as appropriate.",
         "basis": basis("melee_dps", "maximum_ranged_hit")},
        {"id": "formula.combat.accuracy", "inputs": ["attack_roll_max", "defence_roll_max"],
         "expression": "1-(D+2)/(2*(A+1)) if A>D else A/(2*(D+1))",
         "domain": "A,D>=0; exact probability as a rational. Inclusive random integer attack in[0,A] beats defence in[0,D]. Equal rolls miss.",
         "negative_input_policy_ref": "assumption.negative_combat_roll",
         "basis": basis("melee_dps", "ranged_dps", "magic")},
        {"id": "formula.combat.damage", "inputs": ["accuracy_succeeded", "raw_uniform_damage", "target_hp"],
         "expression": "0 if not accuracy_succeeded else min(target_hp,max(1,raw_uniform_damage))",
         "domain": "Player attack with positive max hit, ordinary M1 NPC; raw_uniform_damage in [0,max_hit]. NPC outgoing rolls are not established by this player-DPS formula.",
         "basis": basis("melee_dps", "ranged_dps", "magic_dps")},
        {"id": "formula.combat.hp_xp", "inputs": ["credited_damage", "on_tutorial"],
         "expression": "0 if on_tutorial else floor(40*credited_damage/3)",
         "output": "xp_tenths", "basis": basis("combat_options", "experience", classification="assumption", assumptions=["combat_xp_thirds"])},
        {"id": "formula.combat.magic_attack", "inputs": ["magic", "prayer_multiplier", "equipment_magic_attack"],
         "expression": "(floor(magic*prayer_multiplier)+8)*(equipment_magic_attack+64)",
         "domain": "Manual Wind Strike, no void/special damage equipment", "basis": basis("magic", "magic_dps")},
        {"id": "formula.combat.magic_defence_npc", "inputs": ["magic", "equipment_magic_defence"],
         "expression": "(magic+9)*(equipment_magic_defence+64)", "basis": basis("magic")},
        {"id": "formula.shop.buy", "inputs": ["base_value", "base_stock", "current_stock"],
         "expression": "max(1,floor(base_value*clamp(1300+30*(base_stock-current_stock),300,6300)/1000))",
         "domain": "Lumbridge General Store, per item before reducing stock; overstock branch is provisional linear article interpretation.",
         "basis": basis("shop", "lumbridge_general_store", "shop_module", classification="assumption", assumptions=["shop_overstock"])},
        {"id": "formula.shop.sell", "inputs": ["base_value", "base_stock", "current_stock"],
         "expression": "floor(base_value*clamp(400+30*(base_stock-current_stock),100,1400)/1000)",
         "domain": "Lumbridge General Store, per item before increasing stock; standard unstocked items have base_stock=0.",
         "basis": basis("shop", "lumbridge_general_store", "shop_module", classification="inference")},
        {"id": "formula.death.grave_fee", "inputs": ["reclaimed_effective_item_values"],
         "expression": "min(500000,sum(0 if v<100000 else 1000 if v<1000000 else 10000 if v<10000000 else 100000 for v in values))",
         "domain": "Normal account no discount. Values exactly at a threshold enter the higher band.",
         "basis": basis("death_fees")},
        {"id": "formula.death.office_fee", "inputs": ["reclaimed_effective_item_values"],
         "expression": "sum(0 if v<100000 else floor(v*5/100) for v in values)",
         "domain": "Normal account no discount; rounding for non-multiple-of-20 values follows integer-currency inference.",
         "basis": basis("death_fees", classification="inference")},
    ]
    primary = [
        ("shield.bronze_square", 1, 3, False), ("spear.bronze", 1, 4, True),
        ("rune.body", 7, 5, False), ("rune.water", 6, 6, False), ("rune.earth", 4, 3, False), ("bolts.bronze", 8, 3, False),
        ("coins", 5, 28, False), ("coins", 9, 3, False), ("coins", 15, 3, False), ("coins", 20, 2, False), ("coins", 1, 1, False),
        (None, 0, 38, False), ("hammer", 1, 15, False), ("goblin_book", 1, 2, True),
        ("goblin_mail", 1, 5, False), ("chefs_hat", 1, 3, False), ("beer", 1, 2, False), ("brass_necklace", 1, 1, False),
        ("talisman.air", 1, 1, False),
    ]
    death_states = [
        {"id": "stage.death.introduction", "instruction": "Death explains the first item loss.", "basis": basis("death_dialogue")},
        {"id": "stage.death.topics", "instruction": "Complete fees, timer, and kept-items topics, any order.", "basis": basis("death_dialogue")},
        {"id": "stage.death.exit_ready", "instruction": "All topics heard and done confirmed; use portal.", "basis": basis("death_dialogue")},
        {"id": "stage.death.respawned", "instruction": "Back in Lumbridge; active grave timer starts.", "basis": basis("death_dialogue")},
    ]
    death_edges = [
        {"id": "transition.death.introduction", "from_ref": "stage.death.introduction", "to_ref": "stage.death.topics",
         "event_ref": "event.dialogue.completed", "guard": all_of(eq("event.npc_ref", "npc.death"), eq("event.topic", "first_item_loss_intro")), "effects": []},
    ]
    for topic in ("fees", "timer", "kept_items"):
        death_edges.append({"id": "transition.death." + topic, "from_ref": "stage.death.topics", "to_ref": "stage.death.topics",
                            "event_ref": "event.death.topic_completed", "guard": all_of(eq("event.topic", topic)),
                            "effects": [set_value("state.death.topics." + topic, True)]})
    death_edges += [
        {"id": "transition.death.done", "from_ref": "stage.death.topics", "to_ref": "stage.death.exit_ready",
         "event_ref": "event.death.exit_requested",
         "guard": all_of(*(eq("state.death.topics." + t, True) for t in ("fees", "timer", "kept_items"))),
         "effects": [set_value("state.death.tutorial_seen", True)]},
        {"id": "transition.death.portal", "from_ref": "stage.death.exit_ready", "to_ref": "stage.death.respawned",
         "event_ref": "event.death.portal_crossed", "guard": all_of(eq("event.authorized_portal", True)),
         "effects": [set_value("state.death.grave_ticks_remaining", 1500)]},
    ]
    write("activities.json", {
        "schema_version": 1, "id": "contract.journey.activities", "profile_ref": "profile.osrs.current_normal_f2p",
        "scope": "Required starter activities and their shared invariants, not full-skill or all-item implementation coverage.",
        "items": items, "rules": rules, "formulas": formulas,
        "global_rule_refs": ["rule.time.tick", "rule.inventory.capacity", "rule.xp.level", "rule.tutorial.xp_cap", "rule.tutorial.restrictions", "rule.persistence.acknowledged_state"],
        "statistical_test_policy": "Fixture RNG values are deterministic test inputs at the real roll boundary. They are not a production RNG override, boosted chance, or server-execution claim.",
        "loot_tables": [{
            "id": "loot.goblin.level_2_table_1", "guaranteed": [item("bones")], "primary_denominator": 128,
            "primary": [{"item_ref": "item." + k if k else None, "quantity": quantity, "weight": weight, "members_only": members}
                        for k, quantity, weight, members in primary],
            "tertiary": [
                {"item_ref": "item.clue.beginner", "numerator": 1, "denominator": 64, "requires": "source clue ownership/eligibility rules"},
                {"item_ref": "item.clue.easy", "numerator": 1, "denominator": 128, "requires": "source members/eligibility rules; not established for this F2P profile"},
                {"item_ref": "item.ensouled_goblin_head", "numerator": 1, "denominator": 35, "requires": "members"},
                {"item_ref": "item.goblin_champion_scroll", "numerator": 1, "denominator": 5000, "requires": "members/source champion-scroll eligibility"},
            ],
            "unresolved_supplement": {"item_refs": ["item.energy_potion.one_dose", "item.energy_potion.two_dose", "item.energy_potion.four_dose"], "weight": None,
                                      "reason": "Source lists Common but numeric table already sums to 128; no probability or trigger is invented."},
            "basis": basis("goblin", classification="assumption", assumptions=["goblin_loot_variant"]),
        }],
        "death_graph": {"id": "graph.death.first_office", "start_ref": "stage.death.introduction",
                        "terminal_refs": ["stage.death.respawned"], "states": death_states,
                        "transitions": death_edges, "basis": basis("death_dialogue")},
        "map_markers": [
            {"location_ref": "location.lumbridge.east_swamp_mine", "source_tiles": [[3230, 3147], [3228, 3144], [3229, 3148], [3229, 3145], [3230, 3145]],
             "classification": "wiki_object_location_rows_not_extracted_collision", "basis": basis("copper_rocks")},
            {"location_ref": "location.lumbridge.death_entrance", "source_marker": [3238, 3194], "basis": basis("deaths_office")},
            {"location_ref": "location.lumbridge.castle_arrival", "normal_respawn_marker": {"center": [3221, 3219], "radius": 3},
             "tutorial_arrival_exact_tile": None, "basis": basis("respawn", "tutorial_island")},
        ],
        "null_parameter_policy": "Null is an explicit unverified source-binding requirement, never zero/no delay/no drop. Independently verified rule parts can be implemented; exact-family fidelity cannot be claimed until bound.",
    })


def cooks_assistant():
    import itertools

    order = ["milk", "flour", "egg"]
    subsets = [tuple(x for x, bit in zip(order, bits) if bit)
               for bits in itertools.product([False, True], repeat=3)]

    def stage_id(ingredients):
        return "stage.cooks.delivered." + ("_".join(ingredients) if ingredients else "none")

    states = [{"id": "stage.cooks.not_started", "status": "not_started", "basis": basis("cooks_transcript", "cooks_journal")}]
    for ingredients in subsets:
        states.append({"id": stage_id(ingredients), "status": "in_progress",
                       "delivered": {i: i in ingredients for i in order},
                       "ready_for_thanks_dialogue": len(ingredients) == 3,
                       "basis": basis("cooks_transcript", "cooks_journal", classification="inference")})
    states.append({"id": "stage.cooks.completed", "status": "completed", "basis": basis("cooks_assistant", "cooks_journal")})
    edges = [{
        "id": "transition.cooks.accept", "from_ref": "stage.cooks.not_started", "to_ref": stage_id(()),
        "event_ref": "event.quest.accepted",
        "guard": all_of(eq("event.quest_ref", "quest.cooks_assistant"), eq("event.npc_ref", "npc.cook"), eq("state.tutorial.departed", True)),
        "effects": [set_value("state.quests.cooks_assistant.status", "in_progress"), effect("project_journal", quest_ref="quest.cooks_assistant")],
        "basis": basis("cooks_transcript"),
    }, {
        "id": "transition.cooks.decline", "from_ref": "stage.cooks.not_started", "to_ref": "stage.cooks.not_started",
        "event_ref": "event.quest.declined",
        "guard": all_of(eq("event.quest_ref", "quest.cooks_assistant"), eq("event.npc_ref", "npc.cook")),
        "effects": [], "basis": basis("cooks_transcript"),
    }]
    for before in subsets:
        for after in subsets:
            if not set(before) < set(after):
                continue
            added = [x for x in order if x in after and x not in before]
            edges.append({
                "id": "transition.cooks.deliver." + ("_".join(before) if before else "none") + "_to_" + "_".join(after),
                "from_ref": stage_id(before), "to_ref": stage_id(after),
                "event_ref": "event.quest.delivered",
                "guard": all_of(eq("event.quest_ref", "quest.cooks_assistant"), eq("event.npc_ref", "npc.cook"),
                               eq("event.delivered", added), eq("event.owned_items_consumed", True)),
                "effects": [set_value("state.quests.cooks_assistant.delivered." + x, True) for x in added]
                           + [effect("project_journal", quest_ref="quest.cooks_assistant")],
                "basis": basis("cooks_transcript", "cooks_journal", classification="inference"),
            })
    edges.append({
        "id": "transition.cooks.complete", "from_ref": stage_id(tuple(order)), "to_ref": "stage.cooks.completed",
        "event_ref": "event.dialogue.completed",
        "guard": all_of(eq("event.npc_ref", "npc.cook"), eq("event.topic", "all_ingredients_thanks"),
                       *(eq("state.quests.cooks_assistant.delivered." + x, True) for x in order),
                       eq("state.rewards.cooks_assistant", False)),
        "effects": [set_value("state.quests.cooks_assistant.status", "completed"), effect("reward", reward_ref="reward.cooks_assistant"),
                    effect("project_journal", quest_ref="quest.cooks_assistant")],
        "basis": basis("cooks_transcript", "cooks_assistant"),
    })
    ingredients = [
        {"name": "milk", "item_ref": "item.milk.bucket", "quantity": 1,
         "alternative": {"item_ref": "item.milk.bottomless_bucket", "consume_charges": 1, "retain_container": True, "requires_bound_item_definition": True}},
        {"name": "flour", "item_ref": "item.flour.pot", "quantity": 1},
        {"name": "egg", "item_ref": "item.egg", "quantity": 1},
    ]
    routes = [
        {"id": "route.cooks.pot_spawn", "location_refs": ["location.lumbridge.kitchen"], "object_ref": "object.pot_spawn",
         "steps": [{"event_ref": "event.inventory.pickup_requested", "item_ref": "item.pot", "quantity": 1}],
         "guard": all_of(eq("event.spawn_present", True), ge("state.inventory.free_slots", 1), eq("event.reachable", True)),
         "basis": basis("cooks_assistant", "pot")},
        {"id": "route.cooks.bucket_spawn", "location_refs": ["location.lumbridge.kitchen", "location.lumbridge.cellar"], "object_ref": "object.bucket_spawn",
         "steps": [{"event_ref": "event.world.transitioned", "link_ref": "object.lumbridge.kitchen_trapdoor", "to_ref": "location.lumbridge.cellar"},
                   {"event_ref": "event.inventory.pickup_requested", "item_ref": "item.bucket", "quantity": 1}],
         "guard": all_of(eq("event.spawn_present", True), ge("state.inventory.free_slots", 1), eq("event.reachable", True)),
         "basis": basis("cooks_assistant", "bucket")},
        {"id": "route.cooks.shop_containers", "location_refs": ["location.lumbridge.bank", "location.lumbridge.general_store"],
         "steps": [{"event_ref": "event.bank.withdraw_requested", "item_ref": "item.coins", "quantity": 3},
                   {"event_ref": "event.shop.buy_requested", "item_ref": "item.pot", "quantity": 1},
                   {"event_ref": "event.shop.buy_requested", "item_ref": "item.bucket", "quantity": 1}],
         "guard": all_of(eq("event.legitimate_bank_and_shop_access", True)),
         "rule_refs": ["rule.bank.withdraw", "rule.shop.lumbridge"],
         "basis": basis("cooks_assistant", "lumbridge_general_store"),
         "note": "3 coins buys both only at these source prices/stock. Starter bank money must actually be withdrawn; quest does not grant it."},
        {"id": "route.cooks.egg", "location_refs": ["location.lumbridge.kitchen", "location.lumbridge.west_coop"], "object_ref": "object.egg_spawn",
         "steps": [{"event_ref": "event.inventory.pickup_requested", "item_ref": "item.egg", "quantity": 1}],
         "guard": all_of(eq("event.spawn_present", True), ge("state.inventory.free_slots", 1), eq("event.reachable", True)),
         "basis": basis("cooks_assistant", "egg"),
         "note": "Pick up a real chicken-coop egg; killing a chicken is neither required nor a guaranteed egg-grant action."},
        {"id": "route.cooks.milk_west", "location_refs": ["location.lumbridge.west_coop", "location.lumbridge.north_cows"],
         "npc_ref": "npc.dairy_cow", "rule_ref": "rule.cooks.milk",
         "basis": basis("cooks_assistant")},
        {"id": "route.cooks.milk_east", "location_refs": ["location.lumbridge.kitchen", "location.lumbridge.east_cows"],
         "object_ref": "object.lumbridge.bridge", "npc_ref": "npc.dairy_cow", "rule_ref": "rule.cooks.milk",
         "optional_tutor_ref": "npc.gillie_groats", "basis": basis("cooks_transcript", "bucket_of_milk"),
         "note": "Keep the bridge/river travel relationship. Do not put Gillie beside the west-field cow."},
        {"id": "route.cooks.flour", "location_refs": ["location.lumbridge.wheat", "location.lumbridge.mill_ground", "location.lumbridge.mill_middle", "location.lumbridge.mill_top"],
         "steps": [
             {"event_ref": "event.gather.succeeded", "rule_ref": "rule.cooks.grain", "item_ref": "item.grain"},
             {"event_ref": "event.world.transitioned", "link_ref": "object.mill.ladder_lower", "to_ref": "location.lumbridge.mill_middle"},
             {"event_ref": "event.world.transitioned", "link_ref": "object.mill.ladder_upper", "to_ref": "location.lumbridge.mill_top"},
             {"event_ref": "event.mill.hopper_filled", "item_ref": "item.grain"},
             {"event_ref": "event.mill.controls_operated", "object_ref": "object.mill.controls"},
             {"event_ref": "event.world.transitioned", "link_ref": "object.mill.ladder_upper", "to_ref": "location.lumbridge.mill_middle"},
             {"event_ref": "event.world.transitioned", "link_ref": "object.mill.ladder_lower", "to_ref": "location.lumbridge.mill_ground"},
             {"event_ref": "event.mill.flour_collected", "object_ref": "object.mill.flour_bin"},
         ],
         "optional_tutor_ref": "npc.millie_miller", "basis": basis("cooks_assistant", "mill_lane_mill", "hopper", "flour_bin"),
         "note": "Flour bin on floor 0, millstones on 1, hopper/controls on 2. Millie's explanation is optional, using hopper then controls is not."},
    ]
    rules = [
        {"id": "rule.cooks.start", "family": "quest", "event_ref": "event.quest.accepted",
         "guard": all_of(eq("event.npc_ref", "npc.cook"), eq("event.reachable", True), eq("state.quests.cooks_assistant.status", "not_started")),
         "contract": ["No skill, combat, quest-point, or ingredient-acquisition prerequisite.",
                      "Start via Cook's problem dialogue and explicit Yes; No leaves the quest unstarted.",
                      "Already carrying all ingredients enables the special start dialogue followed by real transfer and completion; never require gathering after start.",
                      "Partial ingredients at initial acceptance do not count as delivered until actually transferred."],
         "basis": basis("cooks_transcript", "cooks_assistant")},
        {"id": "rule.cooks.deliver", "family": "quest", "event_ref": "event.quest.delivered",
         "guard": all_of(eq("state.quests.cooks_assistant.status", "in_progress"), eq("event.npc_ref", "npc.cook"), eq("event.reachable", True)),
         "contract": ["Cook accepts any currently carried, usable, unnoted requested ingredients not already delivered; partial deliveries in any order persist.",
                      "The shared turn-in transaction chooses ALL eligible missing held ingredients in source dialogue order milk, flour, egg, removes exactly one of each and sets those delivered bits.",
                      "It then emits event.quest.delivered with its actual new ingredient list. The event cannot be supplied as a client assertion.",
                      "Do not consume a second egg/milk/flour after its bit is true. Do not require proof of self-gathering: legitimately purchased/traded/pre-collected ingredients also satisfy the source quest.",
                      "Normal milk and flour deliveries consume the filled container item; no empty container return is shown by source. Bottomless milk consumes one charge and retains the container.",
                      "Banked or noted items cannot be silently withdrawn/unnoted by dialogue. A missing item gets a reminder, not a magical replacement.",
                      "A transfer acknowledged before disconnect remains delivered even if death/restart happens before reward dialogue."],
         "ingredients": ingredients, "basis": basis("cooks_transcript", "cooks_journal", "cooks_assistant")},
        {"id": "rule.cooks.milk", "family": "ingredient", "event_ref": "event.activity.requested",
         "guard": all_of(ge("state.inventory.empty_bucket", 1), eq("event.target_is_dairy_cow", True), eq("event.reachable", True)),
         "consumed": [item("bucket")], "produced": [item("milk.bucket")], "xp_awards": [],
         "timing": {"left_click_ticks": 3, "use_bucket_ticks": 4, "auto_repeat_ticks": 8},
         "contract": ["One existing bucket transforms in place; full inventory still permits this net-zero-slot conversion.",
                      "An ordinary cow attack target, bucket of water, missing bucket or unreachable cow produces no milk.",
                      "No quest-start requirement; milk can be obtained before speaking to Cook."],
         "basis": basis("bucket_of_milk", "cooks_assistant")},
        {"id": "rule.cooks.grain", "family": "ingredient", "event_ref": "event.activity.requested",
         "guard": all_of(eq("event.target_is_pickable_wheat", True), eq("event.reachable", True), ge("state.inventory.free_slots", 1)),
         "consumed": [], "produced": [item("grain")], "xp_awards": [],
         "contract": ["Pick real wheat to receive grain. No Farming XP, Farming level or quest-start requirement is imposed.",
                      "A full inventory doesn't remove the wheat or set a milled/quest-delivered flag."],
         "basis": basis("grain", "cooks_assistant")},
        {"id": "rule.cooks.hopper", "family": "ingredient", "event_ref": "event.mill.hopper_filled",
         "guard": all_of(eq("state.location", "mill_top"), eq("state.mill.hopper", None), ge("state.inventory.grain", 1),
                        pred("state.mill.flour_units", "lte", 29), eq("event.reachable", True)),
         "consumed": [item("grain")], "produced": [], "xp_awards": [],
         "effects": [set_value("state.mill.hopper", "grain")],
         "contract": ["Hopper accepts one grain. A second fill before processing is refused without consuming another grain.",
                      "Filling does not itself mill flour; controls must operate next. The 30-flour overflow policy is provisional."],
         "basis": basis("hopper_transcript", "mill_lane_mill", classification="assumption", assumptions=["mill_ownership_overflow"])},
        {"id": "rule.cooks.mill_controls", "family": "ingredient", "event_ref": "event.mill.controls_operated",
         "guard": all_of(eq("state.location", "mill_top"), eq("state.mill.hopper", "grain"),
                        pred("state.mill.flour_units", "lte", 29), eq("event.reachable", True)),
         "effects": [set_value("state.mill.hopper", None), effect("set", path="state.mill.flour_units", expression="previous+1")],
         "xp_awards": [],
         "contract": ["Convert hopper grain to one unit in the ground-floor flour bin, not directly into inventory.",
                      "Operating empty controls creates no flour; repeat/replayed operations cannot duplicate it.",
                      "Flour count persists across logout and is shared across ordinary windmills for that character."],
         "basis": basis("mill_lane_mill", "flour_bin", classification="assumption", assumptions=["mill_ownership_overflow"])},
        {"id": "rule.cooks.collect_flour", "family": "ingredient", "event_ref": "event.mill.flour_collected",
         "guard": all_of(eq("state.location", "mill_ground"), ge("state.mill.flour_units", 1), ge("state.inventory.empty_pot", 1), eq("event.reachable", True)),
         "consumed": [item("pot")], "produced": [item("flour.pot")], "xp_awards": [],
         "effects": [effect("set", path="state.mill.flour_units", expression="previous-1")],
         "contract": ["Replace one empty pot with flour and decrement flour count once. No free inventory slot needed for the replacement.",
                      "No pot, no flour or wrong floor means no mutation. A full flour bin is not a full inventory item until collected.",
                      "Losing the pot is recoverable through kitchen spawn or shop; the processed flour remains available."],
         "basis": basis("mill_lane_mill", "flour_bin", "pot_of_flour")},
        {"id": "rule.cooks.range_access", "family": "quest", "event_ref": "event.activity.requested",
         "guard": all_of(eq("state.quests.cooks_assistant.status", "completed"), eq("event.reachable", True)),
         "contract": ["Cook-o-matic 100 permission is granted only at completed quest, not at first partial delivery.",
                      "Before permission, range use refuses without consuming raw food/XP. After permission use the shared recipe's Lumbridge-specific burn parameters.",
                      "This permission does not unlock the Recipe for Disaster chest, invent extra rewards or give free cooking ingredients."],
         "object_ref": "object.range.lumbridge", "rule_refs": ["rule.cooking.shrimps", "rule.cooking.bread"],
         "basis": basis("cooks_assistant", "cooks_journal", "bread", "shrimps")},
    ]
    rewards = [
        {"id": "reward.learning_the_ropes", "quest_ref": "quest.learning_the_ropes", "claim_key": "quest.learning_the_ropes",
         "guard": all_of(eq("state.quests.learning_the_ropes.status", "completed"), eq("state.tutorial.chicken_cast", True),
                        eq("state.rewards.learning_the_ropes", False)),
         "quest_points": 1, "xp_awards": [], "items": [],
         "unlocks": ["mainland departure through authorized Home Teleport"],
         "sets": {"state.rewards.learning_the_ropes": True, "state.vitals.run_energy_units": 10000},
         "basis": basis("learning_the_ropes", "tutorial_transcript", "energy")},
        {"id": "reward.cooks_assistant", "quest_ref": "quest.cooks_assistant", "claim_key": "quest.cooks_assistant",
         "guard": all_of(eq("state.quests.cooks_assistant.status", "completed"),
                        *(eq("state.quests.cooks_assistant.delivered." + i, True) for i in order),
                        eq("state.rewards.cooks_assistant", False)),
         "quest_points": 1, "xp_awards": [{"skill_ref": "skill.cooking", "xp_tenths": 3000}], "items": [],
         "unlocks": ["Cook-o-matic 100 range permission"],
         "sets": {"state.rewards.cooks_assistant": True, "state.vitals.run_energy_units": 10000},
         "basis": basis("cooks_assistant", "cooks_journal", "energy")},
    ]
    write("cooks-assistant.json", {
        "schema_version": 1, "id": "contract.journey.cooks_assistant", "quest_ref": "quest.cooks_assistant",
        "profile_ref": "profile.osrs.current_normal_f2p", "start_ref": "stage.cooks.not_started",
        "terminal_refs": ["stage.cooks.completed"], "states": states, "transitions": edges,
        "ingredients": ingredients, "rules": rules, "routes": routes, "rewards": rewards,
        "delivery_order": order,
        "reward_atomicity": "Complete status, once-only claim ledger, 1 QP, 3000 Cooking XP tenths and range permission commit together before the completion acknowledgement. Transaction failure must leave the prior ready-for-thanks state, not half a reward.",
        "journal": {
            "not_started": "Locate Cook in Lumbridge Castle.",
            "in_progress": {
                "intro": "Help Cook make the Duke's birthday cake by bringing three ordinary ingredients.",
                "ingredient_projection_order": ["delivered -> crossed out", "currently held usable unnoted -> found, ready to give", "otherwise -> missing, route hint"],
                "milk_hint": "Bring an empty bucket to a dairy cow; source journal points east of Lumbridge.",
                "flour_hint": "Bring an empty pot to the mill north-west of Lumbridge.",
                "egg_hint": "A chicken coop at Groats' farm/west of the cattle field.",
            },
            "completed": "All ingredients delivered, quest complete, high-quality range available.",
            "basis": basis("cooks_journal", classification="assumption", assumptions=["journal_inventory_scope"]),
            "not_exact_dialogue_text": True,
        },
        "dialogue_routes": [
            {"topic": "whats_wrong", "opens": "explicit yes/no quest offer"},
            {"topic": "cake_or_unhappy_or_hat", "opens": "alternate introductions converge on same quest offer; not distinct quests"},
            {"topic": "where_flour_milk_eggs", "opens": "optional route reminders; pot/bucket-specific advice depends on currently held containers"},
            {"topic": "already_have_everything", "opens": "special premature-acquisition dialogue, then normal item-transfer and thanks events"},
            {"topic": "progress", "opens": "transfer eligible partial ingredients and list the undelivered remainder"},
            {"topic": "all_ingredients_thanks", "opens": "finish quest; source does not require baking the cake, attending the party or collecting a coin reward"},
            {"topic": "talk_after_complete", "opens": "no replay of ingredient consumption or rewards; later quest content remains separately scoped"},
        ],
        "map_markers": [
            {"location_ref": "location.lumbridge.kitchen", "source_marker": [3208, 3214]},
            {"location_ref": "location.lumbridge.west_coop", "source_marker": [3181, 3288]},
            {"location_ref": "location.lumbridge.north_cows", "source_marker": [3177, 3315]},
            {"location_ref": "location.lumbridge.wheat", "source_marker": [3163, 3289]},
            {"location_ref": "location.lumbridge.mill_top", "hopper_marker": [3166, 3307, 2]},
        ],
        "marker_basis": basis("cooks_assistant", "hopper"),
        "marker_warning": "Guide/source rows are navigation anchors, not certified collision/object-origin coordinates; bind placements and every stair/ladder from actual source data.",
        "source_numeric_progress": None,
        "journey_vs_quest_guard": "M1 acceptance must actually acquire the egg/milk/flour through legitimate mapped routes. The quest itself must NOT require post-start gathering or provenance flags absent from the source.",
    })


def main():
    local_sources()
    vocabulary()
    decisions()
    initial_state()
    tutorial()
    activities()
    cooks_assistant()
    print("Authored six rule contracts; no gameplay or acceptance tests executed.")


if __name__ == "__main__":
    main()
