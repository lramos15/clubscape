"""Safe existing-schema projections of the complete source graphs.

Unrepresentable edges stay in the binding ledger, not a client stage-advance shortcut.
"""

from common import (
    all_of, always, has_items, item_stack, never, quest_at, region_id, source_record,
    stack, tutorial_at, unique_sources,
)
from spawns import dialogue_id


LEARNING_STAGES = {
    "not_started": "stage.learning_the_ropes.not_started",
    "in_progress": "stage.learning_the_ropes.in_progress",
    "completed": "stage.learning_the_ropes.completed",
}
BASE_ACTIONS = ["walk", "interact", "select_dialogue", "open_interface", "close_interface",
                "cancel_activity", "request_logout"]
RECIPE_RULES = {
    "rule.cooking.dough": "recipe.cooking.dough",
    "rule.smelting.bronze": "recipe.smelting.bronze",
    "rule.smithing.bronze_dagger": "recipe.smithing.bronze_dagger",
}
SIMPLE_GRANTS = {
    "grant.tutorial.net", "grant.tutorial.survival_tools", "grant.tutorial.pickaxe", "grant.tutorial.hammer",
}


def flag(name, value=1):
    return {"kind": "flag", "name": name, "equals": value}


def set_flag(name, value=1):
    return {"kind": "set_flag", "name": name, "value": value}


def completed_flag(edge):
    return edge.removeprefix("transition.") + ".completed"


def enter_stage(inputs, stage):
    definition = next(s for s in inputs.rules["tutorial"]["states"] if s["id"] == stage)
    return [{"kind": "set_tutorial_stage", "stage": stage}] + [
        {"kind": "unlock_interface", "interface": identifier.replace("ui.", "interface.", 1)}
        for identifier in definition["ui_unlock_refs"]]


def source_predicates(guard):
    if "all" in guard:
        result = []
        for nested in guard["all"]:
            result.extend(source_predicates(nested))
        return result
    return [guard]


def predicate_value(edge, path):
    return next((predicate.get("value") for predicate in source_predicates(edge["guard"])
                 if predicate.get("path") == path), None)


def translated_effects(inputs, edge, flags):
    effects, gaps = [], []
    grants = {grant["id"]: grant for grant in inputs.rules["tutorial"]["grant_definitions"]}
    for source in edge["effects"]:
        operation = source["op"]
        if operation == "grant":
            identifier = source["grant_ref"]
            if identifier not in SIMPLE_GRANTS:
                gaps.append("partial_and_missing_grants")
                continue
            grant = grants[identifier]
            effect = {"kind": "give_items", "items": [stack(value) for value in grant["items"]]}
            if identifier == "grant.tutorial.hammer":
                effect = {"kind": "conditional", "guard": {"kind": "not", "guard": has_items([item_stack("item.hammer")])},
                          "effects": [effect]}
            effects.append(effect)
        elif operation == "set":
            path, value = source["path"], source["value"]
            if path == "state.quests.learning_the_ropes.status":
                effects.append({"kind": "set_quest_stage", "quest": "quest.learning_the_ropes",
                                "stage": LEARNING_STAGES[value]})
            elif path.startswith("state.tutorial."):
                name = path.removeprefix("state.")
                flags[name] = 0
                effects.append(set_flag(name, int(value)))
            elif path == "state.appearance_confirmed":
                flags["tutorial.appearance_confirmed"] = 0
                effects.append(set_flag("tutorial.appearance_confirmed"))
            else:
                gaps.append("unrepresented_effect:" + path)
        elif operation == "mark" and "from_path" not in source:
            name = "tutorial." + source["key"]
            flags[name] = 0
            effects.append(set_flag(name))
        elif operation == "reward":
            gaps.append("quest_reward_run_energy")
        elif operation == "reconcile_departure":
            gaps.append("departure_reconciliation")
        else:
            gaps.append("authoritative_events")
    name = completed_flag(edge["id"])
    flags[name] = 0
    effects.extend([set_flag(name), *enter_stage(inputs, edge["to_ref"])])
    return effects, sorted(set(gaps))


def build_tutorial(inputs, dialogues, spawns, flags):
    data = inputs.rules["tutorial"]
    stage_index = {value["id"]: index for index, value in enumerate(data["states"])}
    caps = next(r for r in inputs.rules["activities"]["rules"] if r["id"] == "rule.tutorial.xp_cap")
    stages = {}
    for source in data["states"]:
        stage = source["id"]
        island = stage != "stage.tutorial.mainland"
        allowed = list(BASE_ACTIONS)
        if stage_index[stage] >= stage_index["stage.tutorial.inventory_open"]:
            allowed += ["drop", "take_ground_item", "use_item", "move_inventory", "eat"]
        if stage_index[stage] >= stage_index["stage.tutorial.make_dough"]:
            allowed += ["produce"]
        if stage_index[stage] >= stage_index["stage.tutorial.equip_dagger"]:
            allowed += ["equip", "unequip", "set_combat_style"]
        if stage_index[stage] >= stage_index["stage.tutorial.bank_open"]:
            allowed += ["bank_deposit", "bank_withdraw"]
        if stage_index[stage] >= stage_index["stage.tutorial.prayer_explanation"]:
            allowed += ["set_prayer"]
        if stage_index[stage] >= stage_index["stage.tutorial.wind_strike"]:
            allowed += ["cast"]
        if not island:
            allowed += ["shop_buy", "shop_sell"]
        xp_caps = {}
        if island:
            xp_caps = {f"skill.{name}": 0 for name in (
                "agility", "herblore", "thieving", "crafting", "fletching", "slayer", "farming",
                "runecraft", "hunter", "construction", "sailing")}
            xp_caps["skill.hitpoints"] = 11540
            xp_caps.update({skill: caps["cap_below_next_level_tenths"] for skill in caps["skill_refs"]})
        stages[stage] = {
            "id": stage, "instruction": source["instruction"], "allowed_actions": allowed,
            "xp_caps_tenths": xp_caps,
            "xp_stop_levels": {skill: caps["level_cap"] for skill in caps["skill_refs"]} if island else {},
            "nonfatal_combat": island, "transitions": [],
            "source": unique_sources(inputs.basis(source["basis"]) + inputs.basis(caps["basis"])),
        }
    bindings = []
    for edge in data["transitions"]:
        record = {"id": edge["id"], "from": edge["from_ref"], "to": edge["to_ref"],
                  "source_event": edge["event_ref"], "source_guard": edge["guard"],
                  "source_effects": edge["effects"], "runtime_hooks": [], "gaps": []}
        effects, effect_gaps = translated_effects(inputs, edge, flags)
        record["gaps"].extend(effect_gaps)
        event = edge["event_ref"]
        if event == "event.dialogue.completed":
            npc, topic = predicate_value(edge, "event.npc_ref"), predicate_value(edge, "event.topic")
            if npc not in inputs.selection["npcs"]:
                record["gaps"].append("npc_origin")
            else:
                guard = tutorial_at(edge["from_ref"])
                # Initial single/batch grants enforce the source free-slot precondition atomically.
                # Partial/top-up policies are not equivalent and are deliberately blocked above.
                for predicate in source_predicates(edge["guard"]):
                    path = predicate.get("path")
                    if path in ("event.npc_ref", "event.topic", "state.inventory.free_slots"):
                        continue
                    if path == "state.account.mode" and predicate["value"] == "normal":
                        continue
                    if "any" in predicate and edge["id"] == "transition.tutorial.hammer":
                        continue
                    record["gaps"].append("guard:" + str(path))
                if record["gaps"]:
                    guard = all_of(guard, never())
                node = {
                    "id": edge["id"], "text": stages[edge["from_ref"]]["instruction"],
                    "guard": tutorial_at(edge["from_ref"]),
                    "choices": [{"id": topic, "text": topic.replace("_", " ").capitalize(),
                                 "guard": guard, "effects": effects, "next_node": None}],
                }
                target = dialogue_id(npc)
                dialogues[target]["nodes"].append(node)
                dialogues[target]["entry_nodes"].append(node["id"])
                record["runtime_hooks"].append({"kind": "dialogue_choice", "dialogue": target,
                                                "node": node["id"], "choice": topic})
        elif event == "event.ui.opened":
            interface = predicate_value(edge, "event.ui_ref").replace("ui.", "interface.", 1)
            if edge["id"] == "transition.tutorial.bank":
                record["gaps"].append("bank_first_open_entitlement")
            if not record["gaps"]:
                transition = {"event": "interface_opened", "target": interface,
                              "guard": tutorial_at(edge["from_ref"]), "effects": effects}
                stages[edge["from_ref"]]["transitions"].append(transition)
                record["runtime_hooks"].append({"kind": "tutorial_transition", "stage": edge["from_ref"],
                                                "index": len(stages[edge["from_ref"]]["transitions"]) - 1})
        elif event == "event.equipment.changed":
            required = [predicate["value"] for predicate in source_predicates(edge["guard"])
                        if predicate.get("path", "").startswith("state.equipment.") and
                        predicate["op"] == "eq"]
            guard = all_of(tutorial_at(edge["from_ref"]), *[{"kind": "equipped", "item": item} for item in required])
            stages[edge["from_ref"]]["transitions"].append(
                {"event": "equipped", "target": None, "guard": guard, "effects": effects})
            record["runtime_hooks"].append({"kind": "tutorial_transition", "stage": edge["from_ref"],
                                            "index": len(stages[edge["from_ref"]]["transitions"]) - 1})
        elif event == "event.craft.succeeded":
            recipe = RECIPE_RULES.get(predicate_value(edge, "event.rule_ref"))
            if recipe:
                stages[edge["from_ref"]]["transitions"].append(
                    {"event": "produced", "target": recipe,
                     "guard": tutorial_at(edge["from_ref"]), "effects": effects})
                record["runtime_hooks"].append({"kind": "tutorial_transition", "stage": edge["from_ref"],
                                                "index": len(stages[edge["from_ref"]]["transitions"]) - 1})
            else:
                record["gaps"].append("conditional_recipes")
        elif event == "event.world.transitioned":
            record["gaps"].append("dynamic_travel_and_crossing")
        elif event == "event.gather.succeeded":
            record["gaps"].append("source_chance_and_timing")
        elif event in ("event.cook.resolved", "event.fire.lit"):
            record["gaps"].append("conditional_recipes")
        elif event in ("event.combat.kill_credited", "event.spell.resolved"):
            record["gaps"].extend(["authoritative_events", "npc_combat"])
        else:
            record["gaps"].append("authoritative_events")
        record["gaps"] = sorted(set(record["gaps"]))
        record["status"] = "blocked" if record["gaps"] else "projected_not_executed"
        bindings.append(record)
    add_recovery_dialogue(inputs, dialogues, flags)
    return stages, bindings


def add_recovery_dialogue(inputs, dialogues, flags):
    entries = (
        ("npc.survival_expert", "item.fishing_net.small", "transition.tutorial.net"),
        ("npc.survival_expert", "item.axe.bronze", "transition.tutorial.survival_tools"),
        ("npc.survival_expert", "item.tinderbox", "transition.tutorial.survival_tools"),
        ("npc.mining_instructor", "item.pickaxe.bronze", "transition.tutorial.pickaxe"),
        ("npc.mining_instructor", "item.hammer", "transition.tutorial.hammer"),
    )
    for npc, item, original in entries:
        original_flag = completed_flag(original)
        flags[original_flag] = 0
        missing = all_of(
            flag(original_flag), {"kind": "not", "guard": tutorial_at("stage.tutorial.mainland")},
            {"kind": "not", "guard": has_items([item_stack(item)])},
            {"kind": "not", "guard": {"kind": "equipped", "item": item}},
        )
        node_id = "recovery." + item
        node = {"id": node_id, "text": "Replace a missing, previously unlocked tutorial tool.",
                "guard": missing, "choices": [{"id": "replace", "text": "Replace the missing tool",
                "guard": missing, "effects": [{"kind": "give_items", "items": [item_stack(item)]}],
                "next_node": None}]}
        dialogue = dialogues[dialogue_id(npc)]
        dialogue["nodes"].append(node)
        dialogue["entry_nodes"].append(node_id)
        dialogue["source"] = unique_sources(dialogue["source"] + [source_record(
            "research/journey-rules/tutorial.json#recovery.tutorial.tools",
            "Missing-only replacement, after the real original grant; atomic inventory capacity failure leaves "
            "stage and possessions unchanged. This source-contract recovery fallback remains an inference.",
            "inference", "source-contract-v1")])


def build_dialogues(inputs):
    dialogues = {}
    for npc, number in inputs.selection["npcs"].items():
        if npc in ("npc.tutorial_rat", "npc.tutorial_chicken", "npc.goblin.level_2",
                   "npc.chicken", "npc.tutorial.fishing_spot"):
            continue
        identifier = dialogue_id(npc)
        dialogues[identifier] = {
            "id": identifier, "nodes": [], "entry_nodes": [],
            "source": inputs.definition_source("npc", number) + [source_record(
                "research/journey-rules/tutorial.json#dialogue_policy",
                "Semantic/paraphrased source dialogue beats only, not approved source strings, chatheads, "
                "control layout or presentation. No progression is attached to arbitrary Continue clicks.",
                "inference", "source-contract-v1")],
        }
    return dialogues


def build_cooks(inputs, dialogues, flags):
    contract = inputs.rules["cooks-assistant"]
    dialogue = dialogues["dialogue.cook"]
    states = {state["id"]: state for state in contract["states"]}
    items = {"milk": "item.milk.bucket", "flour": "item.flour.pot", "egg": "item.egg"}
    bindings = []
    for state_id, state in states.items():
        delivered = state.get("delivered", {})
        description = "Cook's Assistant: " + state["status"].replace("_", " ")
        if delivered:
            description += "; delivered " + (", ".join(name for name, yes in delivered.items() if yes) or "none")
        dialogue["nodes"].append({"id": state_id, "text": description,
                                  "guard": quest_at(state_id), "choices": []})
        dialogue["entry_nodes"].append(state_id)
    nodes = {node["id"]: node for node in dialogue["nodes"]}
    for edge in contract["transitions"]:
        guard, effects, gaps = quest_at(edge["from_ref"]), [], []
        if edge["event_ref"] == "event.quest.accepted":
            guard = all_of(guard, tutorial_at("stage.tutorial.mainland"))
            text = "Agree to help the Cook"
        elif edge["event_ref"] == "event.quest.declined":
            text = "Decline to help"
        elif edge["event_ref"] == "event.quest.delivered":
            before = states[edge["from_ref"]]["delivered"]
            after = states[edge["to_ref"]]["delivered"]
            new = [name for name in items if after[name] and not before[name]]
            carried = [item_stack(items[name]) for name in new]
            still_missing = [name for name in items if not after[name]]
            guard = all_of(guard, has_items(carried), *[
                {"kind": "not", "guard": has_items([item_stack(items[name])])} for name in still_missing])
            effects.append({"kind": "take_items", "items": carried})
            text = "Hand over " + ", ".join(new)
        else:
            flags["reward.cooks_assistant"] = 0
            guard = all_of(guard, flag("reward.cooks_assistant", 0), never())
            effects.extend([{"kind": "add_quest_points", "amount": 1},
                            {"kind": "award_xp", "rewards": [{"skill": "skill.cooking", "amount_tenths": 3000}]},
                            set_flag("reward.cooks_assistant")])
            gaps = ["quest_reward_run_energy"]
            text = "Thank the Cook and receive the quest reward"
        if edge["from_ref"] != edge["to_ref"]:
            effects.append({"kind": "set_quest_stage", "quest": "quest.cooks_assistant", "stage": edge["to_ref"]})
        choice = {"id": edge["id"], "text": text, "guard": guard, "effects": effects, "next_node": None}
        nodes[edge["from_ref"]]["choices"].append(choice)
        bindings.append({
            "id": edge["id"], "from": edge["from_ref"], "to": edge["to_ref"],
            "source_event": edge["event_ref"],
            "runtime_hooks": [{"kind": "dialogue_choice", "dialogue": "dialogue.cook",
                               "node": edge["from_ref"], "choice": edge["id"]}],
            "status": "blocked" if gaps else "projected_not_executed", "gaps": gaps,
            "basis": edge["basis"],
        })
    dialogue["source"] = unique_sources(inputs.basis(contract["journal"]["basis"]) + [source_record(
        "research/journey-rules/cooks-assistant.json#dialogue_routes",
        "All ten states and 22 edges retained. All carried, still-required ingredients are atomically delivered "
        "without requiring self-gathering or post-start acquisition. Completion is blocked until its run-energy "
        "effect can accompany the once-only 1 QP, 300 Cooking XP and range permission.",
        "inference", "source-contract-v1")])
    quest = {
        "id": "quest.cooks_assistant", "name": "Cook's Assistant",
        "initial_stage": contract["start_ref"], "completed_stage": "stage.cooks.completed",
        "journal": {state: nodes[state]["text"] for state in states}, "transitions": [],
        "source": dialogue["source"],
    }
    return quest, bindings


def finish_dialogues(inputs, dialogues):
    for identifier, dialogue in dialogues.items():
        if not dialogue["nodes"]:
            npc = "npc." + identifier.removeprefix("dialogue.")
            name = inputs.collections["npc"][inputs.selection["npcs"][npc]]["name"]
            dialogue["nodes"] = [{"id": "information", "text": name + ": source dialogue binding.",
                                  "guard": always(), "choices": []}]
            dialogue["entry_nodes"] = ["information"]
    death = dialogues["dialogue.death"]
    death["nodes"] = [{
        "id": "first_item_losing_death", "text": "Death's Office: fees, timers and kept items must all be explained.",
        "guard": always(),
        "choices": [{"id": topic, "text": text, "guard": never(), "effects": [], "next_node": None}
                    for topic, text in (("fees", "Explain recovery fees"), ("timer", "Explain the grave timer"),
                                        ("kept_items", "Explain items kept on death"))],
    }]
    death["entry_nodes"] = ["first_item_losing_death"]
    death["source"] = unique_sources(death["source"] + [source_record(
        "research/journey-rules/activities.json#graph.death.first_office",
        "The four-state/six-edge source death graph is bound separately. Never mark it complete using "
        "an unrelated quest, tutorial stage, empty-inventory death or three unguarded flags.",
        "inference", "source-contract-v1")])


def build_initial_state(inputs, flags):
    source = inputs.rules["initial-state"]
    point = inputs.selection["initial_tile"]
    flags.update({"tutorial.departed": 0, "tutorial.departure_authorized": 0,
                  "tutorial.melee_kill": 0, "tutorial.ranged_kill": 0, "tutorial.chicken_cast": 0,
                  "tutorial.bank_seed_claimed": 0, "reward.learning_the_ropes": 0,
                  "reward.cooks_assistant": 0})
    return {
        "region": region_id(point["x"], point["y"]), "tile": point,
        "inventory": {"slots": [None] * 28}, "equipment": {},
        "bank": {"capacity": source["bank"]["base_capacity"], "slots": []},
        "skills": {s["id"]: {"xp_tenths": s["xp_tenths"], "current_level": s["current_level"]} for s in source["skills"]},
        "hitpoints": source["vitals"]["hitpoints"], "prayer_points": source["vitals"]["prayer_points"],
        "run_energy": source["vitals"]["run_energy_units"], "tutorial_stage": source["tutorial"]["stage_ref"],
        "quest_points": 0,
        "quests": {
            "quest.learning_the_ropes": {"stage": LEARNING_STAGES["not_started"], "flags": {}},
            "quest.cooks_assistant": {"stage": "stage.cooks.not_started", "flags": {}},
        },
        "flags": dict(sorted(flags.items())),
        "interfaces": [item.replace("ui.", "interface.", 1) for item in source["ui"]["initially_available_refs"]],
        "source": unique_sources(
            inputs.basis(source["inventory"]["basis"]) + inputs.basis(source["bank"]["basis"]) +
            inputs.basis(source["vitals"]["basis"]) + [
                source_record("research/m1-bindings/selection.json#initial_tile",
                              inputs.selection["initial_tile_note"], "inference", "m1-bindings-v1"),
                source_record("research/journey-rules/decisions.json#assumption.fresh_containers",
                              "Fresh containers and departure alternatives remain provisional. The 25-coin "
                              "first-open bank entitlement is NOT preseeded, repeated on open, granted in "
                              "inventory or silently reconciled at departure.",
                              "inference", "source-contract-v1"),
            ]),
    }


def learning_quest(inputs):
    return {
        "id": "quest.learning_the_ropes", "name": "Learning the Ropes",
        "initial_stage": LEARNING_STAGES["not_started"], "completed_stage": LEARNING_STAGES["completed"],
        "journal": {value: "Learning the Ropes: " + key.replace("_", " ") for key, value in LEARNING_STAGES.items()},
        "transitions": [],
        "source": [inputs.wiki("Learning the Ropes", "1 QP is earned by the valid tutorial chicken Wind Strike, "
                               "before departure and without requiring a kill. Its authoritative spell event and "
                               "run-energy reward cannot be replaced with a generic hit/kill or dialogue advance.")],
    }
