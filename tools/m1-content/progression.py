"""Lower every source graph edge to schema-2 authoritative facts, guards and effects."""

from collections import deque

from common import (
    all_of, always, counter_guard, has_items, item_stack, position, quest_at,
    region_id, restore_run, set_counter, source_record, tutorial_at, unique_sources,
)
from mechanics import completed_counter
from spawns import dialogue_id


LEARNING_STAGES = {name: "stage.learning_the_ropes." + name for name in ("not_started", "in_progress", "completed")}
RECIPE_RULES = {
    "rule.cooking.dough": "recipe.cooking.dough",
    "rule.smelting.bronze": "recipe.smelting.bronze",
    "rule.smithing.bronze_dagger": "recipe.smithing.bronze_dagger",
    "rule.cooking.shrimps": "recipe.cooking.shrimps.fire",
    "rule.cooking.bread": "recipe.cooking.bread.range",
}
CROSSINGS = {
    "transition.tutorial.starting_door": [(3098, 3107, 0)],
    "transition.tutorial.survival_gate": [(3089, 3091, 0), (3089, 3092, 0)],
    "transition.tutorial.chef_door": [(3078, 3084, 0)],
    "transition.tutorial.chef_exit": [(3072, 3090, 0)],
    "transition.tutorial.quest_door": [(3086, 3125, 0)],
    "transition.tutorial.mining_gate": [(3095, 9502, 0), (3095, 9503, 0)],
    "transition.tutorial.rat_pen_entry": [(3110, 9518, 0), (3110, 9519, 0)],
    "transition.tutorial.rat_pen_exit": [(3111, 9518, 0), (3111, 9519, 0)],
    "transition.tutorial.account_door": [(3125, 3124, 0)],
    "transition.tutorial.account_exit": [(3130, 3124, 0)],
    "transition.tutorial.chapel_door": [(3125, 3107, 0)],
    "transition.tutorial.chapel_exit": [(3122, 3102, 0)],
    "transition.tutorial.magic_entry": [(3140, 3087, 0)],
}


def event_guard(kind, **fields):
    return {"kind": "event", "condition": {"kind": kind, **fields}}


def within_points(points):
    return {"kind": "any", "guards": [{"kind": "within", "tile": {"x": x, "y": y, "plane": plane}, "distance": 0}
                                      for x, y, plane in points]}


def source_predicates(guard):
    if "all" in guard:
        return [predicate for child in guard["all"] for predicate in source_predicates(child)]
    return [guard]


def predicate_value(edge, path):
    return next((predicate["value"] for predicate in source_predicates(edge["guard"]) if predicate.get("path") == path), None)


def enter_stage(inputs, stage):
    original = next(state for state in inputs.rules["tutorial"]["states"] if state["id"] == stage)
    return [{"kind": "set_tutorial_stage", "stage": stage}] + [
        {"kind": "unlock_interface", "interface": interface.replace("ui.", "interface.", 1)}
        for interface in original["ui_unlock_refs"]]


def source_state_guard(inputs, guard):
    if "all" in guard:
        return all_of(*[source_state_guard(inputs, nested) for nested in guard["all"]])
    if "any" in guard:
        return {"kind": "any", "guards": [source_state_guard(inputs, nested) for nested in guard["any"]]}
    path, operation, value = guard["path"], guard["op"], guard["value"]
    if path.startswith("event."):
        return always()
    if path == "state.inventory.free_slots":
        if operation != "gte":
            raise ValueError("Source inventory capacity operator is not represented")
        return {"kind": "free_capacity", "container": "inventory", "slots": value}
    if path.startswith("state.inventory."):
        alias = path.removeprefix("state.inventory.")
        item = inputs.rules["vocabulary"]["guard_projection"]["inventory_count_aliases"][alias]
        return has_items([item_stack(item, value)])
    if path.startswith("state.equipment."):
        if path.endswith("ammo_quantity"):
            if value != 1 or operation != "gte":
                raise ValueError("Unexpected source ammo count guard")
            return {"kind": "equipped", "item": "item.arrow.bronze"}
        return {"kind": "equipped", "item": value}
    if path.startswith("state.tutorial."):
        return counter_guard("counter." + path.removeprefix("state."), value)
    if path == "state.bank.seed_claimed":
        return {"kind": "entitlement_claimed", "entitlement": "entitlement.tutorial.bank_coins"}
    if path == "state.quests.learning_the_ropes.status":
        return {"kind": "quest_stage", "quest": "quest.learning_the_ropes", "stage": LEARNING_STAGES[value]}
    if path == "state.account.mode" and value == "normal":
        return always()
    raise ValueError(f"Untranslated source state guard: {guard}")


def source_effects(inputs, edge):
    effects = []
    for effect in edge["effects"]:
        operation = effect["op"]
        if operation == "grant":
            effects.append({"kind": "grant", "grant": effect["grant_ref"]})
        elif operation == "set":
            path, value = effect["path"], effect["value"]
            if path.startswith("state.tutorial."):
                effects.append(set_counter("counter." + path.removeprefix("state."), value))
            elif path == "state.quests.learning_the_ropes.status":
                if value != "completed":
                    effects.append({"kind": "set_quest_stage", "quest": "quest.learning_the_ropes", "stage": LEARNING_STAGES[value]})
            elif path != "state.appearance_confirmed":
                raise ValueError(f"Untranslated source effect {effect}")
        elif operation == "mark":
            if effect["key"] != "selected_experience":
                effects.append(set_counter("counter.tutorial." + effect["key"]))
        elif operation == "reward" and effect["reward_ref"] == "reward.learning_the_ropes":
            effects.append({"kind": "once", "entitlement": "entitlement.quest.learning_the_ropes",
                            "effects": [{"kind": "set_quest_stage", "quest": "quest.learning_the_ropes",
                                         "stage": LEARNING_STAGES["completed"]},
                                        {"kind": "add_quest_points", "amount": 1}, restore_run()]})
        elif operation == "reconcile_departure":
            # Travel completion owns the entitled container transaction before the completed event.
            continue
        else:
            raise ValueError(f"Untranslated source effect {effect}")
    return effects + [set_counter(completed_counter(edge["id"])), *enter_stage(inputs, edge["to_ref"])]


def build_dialogues(inputs):
    result = {}
    for npc, number in inputs.selection["npcs"].items():
        if npc in ("npc.tutorial_rat", "npc.tutorial_chicken", "npc.goblin.level_2", "npc.chicken", "npc.tutorial.fishing_spot"):
            continue
        identifier = dialogue_id(npc)
        result[identifier] = {
            "id": identifier, "nodes": [], "entry_nodes": [],
            "source": inputs.definition_source("npc", number) + [source_record(
                "research/journey-rules/tutorial.json#dialogue_policy",
                "Paraphrased source dialogue beats and actual choice guards/effects, not approved rendered strings or layout.",
                "inference", "source-contract-v1")],
        }
    return result


def build_tutorial(inputs, world, content, bindings):
    original = inputs.rules["tutorial"]
    stages, records = {}, []
    ordered = [state["id"] for state in original["states"]]
    cap = inputs.activity_rules["rule.tutorial.xp_cap"]
    noncombat_interactions = sorted(
        f"interact:{spawn['id']}:{interaction['name']}"
        for spawn in content["spawns"].values() for interaction in spawn["interactions"]
        if interaction["action"]["kind"] != "attack")
    rat_attacks = sorted(f"interact:{spawn['id']}:Attack" for spawn in content["spawns"].values()
                         if spawn["kind"].get("npc") == "npc.tutorial_rat")
    for state in original["states"]:
        identifier = state["id"]
        island = identifier != "stage.tutorial.mainland"
        actions = ["walk", "dialogue", "select_dialogue", "open_interface", "close_interface", "cancel_activity", "request_logout"]
        actions += noncombat_interactions if island else ["interact"]
        if identifier in ("stage.tutorial.melee_rat", "stage.tutorial.ranged_rat"):
            actions += rat_attacks
        if identifier == "stage.tutorial.appearance":
            actions.append("confirm_appearance")
        if identifier == "stage.tutorial.experience":
            actions.append("select_experience")
        if ordered.index(identifier) >= ordered.index("stage.tutorial.inventory_open"):
            actions += ["drop", "take_ground_item", "use_item", "move_inventory", "eat"]
        if ordered.index(identifier) >= ordered.index("stage.tutorial.catch_shrimp"):
            actions += ["gather"]
        if ordered.index(identifier) >= ordered.index("stage.tutorial.light_fire"):
            actions += ["produce", "produce_at", "interact_with"]
        if ordered.index(identifier) >= ordered.index("stage.tutorial.equip_dagger"):
            actions += ["equip", "unequip", "set_combat_style", "combat_style"]
        if ordered.index(identifier) >= ordered.index("stage.tutorial.bank_open"):
            actions += ["bank_deposit", "bank_withdraw", "bank"]
        if ordered.index(identifier) >= ordered.index("stage.tutorial.prayer_explanation"):
            actions += ["set_prayer", "prayer"]
        if ordered.index(identifier) >= ordered.index("stage.tutorial.wind_strike"):
            actions += ["cast"]
        actions += ["set_setting"]
        if not island:
            actions += ["shop_buy", "shop_sell", "shop", "reclaim"]
        ceilings = {}
        if island:
            ceilings = {skill: 0 for skill in content["skills"] if skill not in cap["skill_refs"]}
            ceilings["skill.hitpoints"] = 11540
            ceilings.update({skill: cap["cap_below_next_level_tenths"] for skill in cap["skill_refs"]})
        stages[identifier] = {
            "id": identifier, "instruction": state["instruction"], "allowed_actions": actions,
            "xp_caps_tenths": ceilings,
            "xp_stop_levels": {skill: 3 for skill in cap["skill_refs"]} if island else {},
            "nonfatal_combat": island, "transitions": [],
            "source": unique_sources(inputs.basis(state["basis"]) + inputs.basis(cap["basis"])),
        }
    npc_spawns = {}
    for spawn in content["spawns"].values():
        if spawn["kind"]["kind"] == "npc":
            npc_spawns.setdefault(spawn["kind"]["npc"], []).append(spawn["id"])
    target_rules = {}
    for spawn in content["spawns"].values():
        for action in spawn["interactions"]:
            if action["action"]["kind"] == "gather":
                method = action["action"]["rule"]["mechanics"]["method"].replace("action.", "rule.", 1)
                target_rules.setdefault(method, []).append(spawn["id"])
    pen = rat_pen(world, content)
    bindings["rat_pen_cells"] = [list(point) for point in sorted(pen)]
    for edge in original["transitions"]:
        event = edge["event_ref"]
        base = all_of(tutorial_at(edge["from_ref"]), source_state_guard(inputs, edge["guard"]))
        if edge["id"] == "transition.tutorial.quest_intro":
            base = all_of(base, {"kind": "quest_stage", "quest": "quest.learning_the_ropes",
                                 "stage": LEARNING_STAGES["not_started"]})
        if edge["id"] == "transition.tutorial.arrive":
            destinations = content["mechanics"]["travels"]["travel.tutorial.departure"]["destination"]["value"]["branches"]
            base = all_of(base, {"kind": "any", "guards": [
                all_of({"kind": "experience", "experience": experience},
                       {"kind": "within", "tile": destination["tile"], "distance": 0})
                for experience, destination in destinations.items()]})
        effects = source_effects(inputs, edge)
        hooks = []
        def add(kind, target=None, guard=None):
            index = len(stages[edge["from_ref"]]["transitions"])
            stages[edge["from_ref"]]["transitions"].append(
                {"event": kind, "target": target, "guard": all_of(base, guard or always()), "effects": effects})
            hooks.append({"kind": "tutorial_transition", "stage": edge["from_ref"], "index": index})
        if event == "event.appearance.confirmed":
            add("appearance_confirmed")
        elif event == "event.experience.selected":
            for experience in content["mechanics"]["experiences"]:
                add("experience_selected", experience, {"kind": "experience", "experience": experience})
        elif event == "event.dialogue.completed":
            npc, topic = predicate_value(edge, "event.npc_ref"), predicate_value(edge, "event.topic")
            if not npc_spawns.get(npc):
                raise ValueError(f"Required tutorial NPC has no source-bound spawn: {npc}")
            for speaker in npc_spawns[npc]:
                add("dialogue_selected", speaker, event_guard("dialogue_choice", speaker=speaker, choice=topic))
            node = {
                "id": edge["id"], "text": stages[edge["from_ref"]]["instruction"],
                "guard": base, "choices": [{"id": topic, "text": topic.replace("_", " ").capitalize(),
                                          "guard": base, "effects": [], "next_node": None}],
            }
            dialogue = content["dialogues"][dialogue_id(npc)]
            dialogue["nodes"].append(node)
            dialogue["entry_nodes"].append(node["id"])
        elif event == "event.ui.opened":
            interface = predicate_value(edge, "event.ui_ref").replace("ui.", "interface.", 1)
            if interface == "interface.bank":
                for spawn in content["spawns"].values():
                    if any(action["action"]["kind"] == "open_bank" for action in spawn["interactions"]):
                        add("interface_presented", interface, event_guard(
                            "interface", interface=interface, context={"kind": "bank", "banker": spawn["id"]}))
            else:
                add("interface_opened", interface)
        elif event == "event.ui.closed":
            add("interface_closed", predicate_value(edge, "event.ui_ref").replace("ui.", "interface.", 1))
        elif event == "event.equipment.changed":
            add("equipped")
        elif event == "event.gather.succeeded":
            rules = predicate_value(edge, "event.rule_ref")
            rules = [rules] if isinstance(rules, str) else rules
            for rule in rules:
                for spawn in target_rules[rule]:
                    add("gathered", spawn)
        elif event in ("event.craft.succeeded", "event.cook.resolved"):
            rule = predicate_value(edge, "event.rule_ref")
            recipe = RECIPE_RULES[rule]
            definition = content["recipes"][recipe]
            outcomes = [("success", definition["outputs"])]
            if event == "event.cook.resolved":
                outcomes.append(("failure", definition["failed_outputs"]))
            condition = {"kind": "any", "guards": [
                event_guard("production", recipe=recipe, method=definition["mechanics"]["method"],
                            facility=None, outcome=outcome, output=items[0]["item"] if items else None)
                for outcome, items in outcomes]}
            add("production_resolved", recipe, condition)
        elif event == "event.fire.lit":
            add("temporary_object_created", "temporary_object.fire.normal")
        elif event == "event.setting.changed":
            add("setting_changed", guard=event_guard("setting", setting={"setting": "run", "enabled": True}))
        elif event == "event.object.inspected":
            for spawn in content["spawns"].values():
                if spawn["kind"].get("object") == "object.tutorial.poll_booth":
                    add("inspected", spawn["id"], event_guard("inspection", target=spawn["id"], explanation="tutorial_poll_explanation"))
        elif event == "event.combat.kill_credited":
            method = predicate_value(edge, "event.method")
            condition = event_guard("kill", npc="npc.tutorial_rat", method=method, credited=True)
            if method == "ranged":
                condition = all_of(condition, {"kind": "not", "guard": within_points(sorted(pen))})
            add("npc_killed", guard=condition)
        elif event == "event.spell.resolved":
            condition = {"kind": "any", "guards": [
                event_guard("spell", spell="spell.wind_strike", target=spawn, outcomes=["hit", "splash"])
                for spawn in npc_spawns["npc.tutorial_chicken"]]}
            add("spell_resolved", "spell.wind_strike", all_of(
                condition, {"kind": "quest_stage", "quest": "quest.learning_the_ropes", "stage": LEARNING_STAGES["in_progress"]},
                {"kind": "not", "guard": {"kind": "entitlement_claimed", "entitlement": "entitlement.quest.learning_the_ropes"}}))
        elif event.startswith("event.teleport."):
            phase = event.removeprefix("event.teleport.")
            add("teleport", "travel.tutorial.departure", event_guard("teleport", travel="travel.tutorial.departure", phase=phase))
        elif event == "event.world.transitioned":
            link = predicate_value(edge, "event.link_ref")
            transit = {"object.tutorial.quest_ladder": "travel.tutorial_quest_ladder.forward",
                       "object.tutorial.combat_ladder": "travel.tutorial_combat_ladder.forward"}.get(link)
            if transit:
                add("teleport", transit, event_guard("teleport", travel=transit, phase="completed"))
            else:
                points = CROSSINGS[edge["id"]]
                related = [group for group in bindings["doors"] if group["status"] == "bound_inference" and any(
                    content["spawns"][spawn]["kind"].get("object") == link for spawn in group["spawns"])]
                door_guards = [counter_guard(group["counter"]) for group in related]
                add("moved", guard=all_of(within_points(points), *door_guards))
        else:
            raise ValueError(f"No schema-2 lowering for {edge['id']}: {event}")
        records.append({
            "id": edge["id"], "from": edge["from_ref"], "to": edge["to_ref"],
            "source_event": event, "source_guard": edge["guard"], "source_effects": edge["effects"],
            "runtime_hooks": hooks, "gaps": [], "status": "bound_not_gameplay_verified",
            "coordinate_basis": "Source door/area crossing coordinates are preserved explicit inferences, not observations."
                                if event == "event.world.transitioned" else None,
        })
    add_recovery(inputs, content)
    return stages, records


def rat_pen(world, content):
    available = {position(cell["tile"]) for region in content["regions"].values() for cell in region["cells"]}
    start = (3105, 9514, 0)
    frontier, visited = deque([start]), {start}
    while frontier:
        point = frontier.popleft()
        for dx, dy in ((0, 1), (1, 0), (0, -1), (-1, 0)):
            other = point[0] + dx, point[1] + dy, point[2]
            if other in available and other not in visited and world.can_step(point, other):
                visited.add(other)
                frontier.append(other)
    if not 20 <= len(visited) <= 500 or (3112, 9518, 0) in visited:
        raise ValueError("Source rat pen did not resolve to a bounded closed-geometry component")
    return visited


def add_recovery(inputs, content):
    def append(npc, key, guard, effects, text):
        dialogue = content["dialogues"][dialogue_id(npc)]
        dialogue["nodes"].append({"id": key, "text": text, "guard": guard, "choices": [
            {"id": key, "text": text, "guard": guard, "effects": effects, "next_node": None}]})
        dialogue["entry_nodes"].append(key)
    island = {"kind": "not", "guard": tutorial_at("stage.tutorial.mainland")}
    for original in inputs.rules["tutorial"]["grant_definitions"]:
        if original["npc_ref"] is None:
            continue
        grant = original["id"]
        suffix = {"grant.tutorial.net": "net", "grant.tutorial.survival_tools": "survival_tools",
                  "grant.tutorial.chef_ingredients": "chef_ingredients", "grant.tutorial.pickaxe": "pickaxe",
                  "grant.tutorial.hammer": "hammer", "grant.tutorial.melee_gear": "melee_gear",
                  "grant.tutorial.ranged_gear": "ranged_gear", "grant.tutorial.runes": "runes"}[grant]
        guard = all_of(island, counter_guard(completed_counter("transition.tutorial." + suffix)))
        if suffix == "chef_ingredients":
            guard = all_of(guard, {"kind": "any", "guards": [
                tutorial_at("stage.tutorial.make_dough"), tutorial_at("stage.tutorial.bake_bread")]},
                {"kind": "not", "guard": has_items([item_stack("item.bread.dough")])},
                {"kind": "not", "guard": has_items([item_stack("item.bread")])})
        if suffix in ("melee_gear", "ranged_gear"):
            kill = "melee_kill" if suffix == "melee_gear" else "ranged_kill"
            guard = all_of(guard, counter_guard("counter.tutorial." + kill, False))
        if suffix == "runes":
            guard = all_of(guard, {"kind": "not", "guard": has_items([
                item_stack("item.rune.air"), item_stack("item.rune.mind")])})
        entitlement = "entitlement." + grant.removeprefix("grant.")
        claimed = {"kind": "entitlement_claimed", "entitlement": entitlement}
        append(original["npc_ref"], "finish." + suffix, all_of(guard, {"kind": "not", "guard": claimed}),
               [{"kind": "grant", "grant": grant}], "Finish the original source supply without reclaiming satisfied lines.")
        append(original["npc_ref"], "replace." + suffix, all_of(guard, claimed),
               [{"kind": "grant", "grant": grant + ".recovery"}], "Replace only missing eligible supplies.")


def build_cooks(inputs, content):
    contract = inputs.rules["cooks-assistant"]
    dialogue = content["dialogues"]["dialogue.cook"]
    states = {state["id"]: state for state in contract["states"]}
    items = {"milk": "item.milk.bucket", "flour": "item.flour.pot", "egg": "item.egg"}
    nodes, bindings = {}, []
    for identifier, state in states.items():
        delivered = [name for name, yes in state.get("delivered", {}).items() if yes]
        node = {"id": identifier, "text": "Cook's Assistant: " + state["status"].replace("_", " ") +
                (("; delivered " + ", ".join(delivered)) if delivered else ""),
                "guard": quest_at(identifier), "choices": []}
        dialogue["nodes"].append(node)
        dialogue["entry_nodes"].append(identifier)
        nodes[identifier] = node
    for edge in contract["transitions"]:
        guard, effects = quest_at(edge["from_ref"]), []
        if edge["event_ref"] == "event.quest.accepted":
            guard = all_of(guard, tutorial_at("stage.tutorial.mainland"))
            text = "Agree to help the Cook"
        elif edge["event_ref"] == "event.quest.declined":
            text = "Decline to help"
        elif edge["event_ref"] == "event.quest.delivered":
            before, after = states[edge["from_ref"]]["delivered"], states[edge["to_ref"]]["delivered"]
            newly = [name for name in items if after[name] and not before[name]]
            carried = [item_stack(items[name]) for name in newly]
            guard = all_of(guard, has_items(carried), *[
                {"kind": "not", "guard": has_items([item_stack(items[name])])} for name in items if not after[name]])
            effects.append({"kind": "take_items", "items": carried})
            text = "Hand over " + ", ".join(newly)
        else:
            guard = all_of(guard, {"kind": "not", "guard": {"kind": "entitlement_claimed", "entitlement": "entitlement.quest.cooks_assistant"}})
            effects = [{"kind": "once", "entitlement": "entitlement.quest.cooks_assistant", "effects": [
                {"kind": "set_quest_stage", "quest": "quest.cooks_assistant", "stage": "stage.cooks.completed"},
                {"kind": "award_xp", "rewards": [{"skill": "skill.cooking", "amount_tenths": 3000}]},
                {"kind": "add_quest_points", "amount": 1}, restore_run()]}]
            text = "Thank the Cook"
        if edge["from_ref"] != edge["to_ref"] and edge["to_ref"] != "stage.cooks.completed":
            effects.append({"kind": "set_quest_stage", "quest": "quest.cooks_assistant", "stage": edge["to_ref"]})
        nodes[edge["from_ref"]]["choices"].append({
            "id": edge["id"], "text": text, "guard": guard, "effects": effects, "next_node": None,
        })
        bindings.append({"id": edge["id"], "from": edge["from_ref"], "to": edge["to_ref"], "source_event": edge["event_ref"],
                         "runtime_hooks": [{"kind": "dialogue_choice", "dialogue": "dialogue.cook",
                                            "node": edge["from_ref"], "choice": edge["id"]}],
                         "status": "bound_not_gameplay_verified", "gaps": [], "basis": edge["basis"]})
    dialogue["source"] = unique_sources(inputs.basis(contract["journal"]["basis"]) + [source_record(
        "research/journey-rules/cooks-assistant.json",
        "All ten states and22 edges; ordinary milk/flour/egg subsets can be precollected and delivered in any order. "
        "Exactly-once reward includes source run-energy restoration. Charged milk remains a retained, "
        "parent-scoped rare alternative, not a new starter grant/acquisition.",
        "inference", "source-contract-v1")])
    return {"id": "quest.cooks_assistant", "name": "Cook's Assistant", "initial_stage": contract["start_ref"],
            "completed_stage": "stage.cooks.completed", "journal": {identifier: nodes[identifier]["text"] for identifier in nodes},
            "transitions": [], "source": dialogue["source"]}, bindings


def finish_dialogues(inputs, content):
    for identifier, dialogue in content["dialogues"].items():
        if not dialogue["nodes"]:
            npc = "npc." + identifier.removeprefix("dialogue.")
            name = inputs.collections["npc"][inputs.selection["npcs"][npc]]["name"]
            dialogue["nodes"] = [{"id": "information", "text": name + ": source information.",
                                  "guard": always(), "choices": []}]
            dialogue["entry_nodes"] = ["information"]
    source = inputs.rule_source("rule.death.first_office")
    first_death = {"kind": "life", "phase": "first_death_office"}
    intro_guard = all_of(first_death, counter_guard("counter.death.introduction_heard", False))
    topics_guard = all_of(first_death, counter_guard("counter.death.introduction_heard"))
    death = content["dialogues"]["dialogue.death"]
    death["nodes"] = [
        {"id": "ordinary_office_information", "text": "Death explains owned-item retrieval, the grave and Office fees.",
         "guard": {"kind": "life", "phase": "alive"}, "choices": []},
        {"id": "stage.death.introduction", "text": "Death explains your first item loss and the grave.",
         "guard": intro_guard, "choices": [{"id": "first_item_loss_intro", "text": "Continue",
          "guard": intro_guard, "effects": [set_counter("counter.death.introduction_heard")],
          "next_node": "stage.death.topics"}]},
        {"id": "stage.death.topics", "text": "Ask about fees, active grave time and items kept on death.",
         "guard": topics_guard, "choices": [
             {"id": topic, "text": topic.replace("_", " "), "guard": topics_guard,
              "effects": [{"kind": "complete_death_topic", "topic": topic}], "next_node": "stage.death.topics"}
             for topic in ("fees", "timer", "kept_items")] + [
             {"id": "done", "text": "I understand. Leave through the portal.",
              "guard": all_of(topics_guard, {"kind": "death_topics", "topics": ["fees", "timer", "kept_items"]}),
              "effects": [set_counter("counter.death.exit_confirmed")], "next_node": None}]},
    ]
    death["entry_nodes"] = ["ordinary_office_information", "stage.death.introduction", "stage.death.topics"]
    death["source"] = source


def build_initial_state(inputs, content):
    source = inputs.rules["initial-state"]
    point = inputs.selection["initial_tile"]
    return {
        "region": region_id(point["x"], point["y"]), "tile": point,
        "inventory": {"slots": [None] * 28}, "equipment": {},
        "bank": {"capacity": source["bank"]["base_capacity"], "slots": []},
        "skills": {skill["id"]: {"xp_tenths": skill["xp_tenths"], "current_level": skill["current_level"]} for skill in source["skills"]},
        "hitpoints": source["vitals"]["hitpoints"], "prayer_points": source["vitals"]["prayer_points"],
        "run_energy": source["vitals"]["run_energy_units"], "tutorial_stage": source["tutorial"]["stage_ref"], "quest_points": 0,
        "quests": {"quest.learning_the_ropes": {"stage": LEARNING_STAGES["not_started"], "flags": {}},
                   "quest.cooks_assistant": {"stage": "stage.cooks.not_started", "flags": {}}},
        "flags": {},
        "interfaces": [identifier.replace("ui.", "interface.", 1) for identifier in source["ui"]["initially_available_refs"]],
        "runtime": {
            "settings": dict(inputs.profile["initial_settings"]),
            "counters": {identifier: definition["initial"] for identifier, definition in content["mechanics"]["counters"].items()
                         if definition["scope"] == "character"},
        },
        "source": unique_sources(inputs.basis(source["inventory"]["basis"]) + inputs.basis(source["bank"]["basis"]) +
                                 inputs.basis(source["vitals"]["basis"]) + [
            source_record("research/m1-bindings/selection.json#initial_tile", inputs.selection["initial_tile_note"], "inference", "240/cache2695"),
            source_record("research/m1-bindings/profile-v2.json",
                          "Initial auto-retaliate/death settings are explicit provisional profile selections, "
                          "not source observations or approvals. Original unbound settings and the capture impact "
                          "are retained. No appearance, experience, tutorial, quest or bank entitlement is precompleted.",
                          "inference", "m1-profile-v2")]),
    }


def learning_quest(inputs):
    return {"id": "quest.learning_the_ropes", "name": "Learning the Ropes",
            "initial_stage": LEARNING_STAGES["not_started"], "completed_stage": LEARNING_STAGES["completed"],
            "journal": {value: "Learning the Ropes: " + key.replace("_", " ") for key, value in LEARNING_STAGES.items()},
            "transitions": [], "source": [inputs.wiki("Learning the Ropes",
                "Valid chicken Wind Strike (hit or source-inferred splash) awards1QP before departure, "
                "without requiring a kill. Its entitlement and source energy refill commit together.")]}
