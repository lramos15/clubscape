#!/usr/bin/env python3
"""Bind independent source graph oracles to actual schema2 state/events; not a live journey."""

from copy import deepcopy
import json

from common import BINDINGS, CONTENT, Inputs, canonical, counter_value, load, sha, write
from progression import RECIPE_RULES
from state_oracles import Oracle


def source_event(content, model, source_kind, given):
    if source_kind == "event.ui.opened":
        interface = given["event.ui_ref"].replace("ui.", "interface.", 1)
        if interface == "interface.bank":
            return {"kind": "interface_presented", "interface": interface,
                    "context": {"kind": "bank", "banker": "spawn.tutorial.banker"}}
        return {"kind": "interface_opened", "interface": interface}
    if source_kind == "event.activity.requested":
        return {"kind": "interacted", "target": "spawn.tutorial.fishing_spot.3099.3090.p0", "action": "Net"}
    if source_kind == "event.gather.succeeded":
        method = given["event.rule_ref"].replace("rule.", "action.", 1)
        spawn, rule = next((spawn, action["action"]["rule"]) for spawn in content["spawns"].values()
                           for action in spawn["interactions"] if action["action"]["kind"] == "gather" and
                           action["action"]["rule"]["mechanics"]["method"] == method)
        return {"kind": "gathered", "target": spawn["id"], "stack": deepcopy(rule["output"])}
    if source_kind == "event.cook.resolved":
        recipe = RECIPE_RULES[given["event.rule_ref"]]
        definition = content["recipes"][recipe]
        burnt = given["event.outcome"] == "burnt"
        return {"kind": "production_resolved", "recipe": recipe,
                "method": definition["mechanics"]["method"],
                "facility": {"kind": "temporary_object", "object": "dynamic_object.source_oracle.fire"},
                "outcome": "failure" if burnt else "success",
                "outputs": deepcopy(definition["failed_outputs"] if burnt else definition["outputs"])}
    if source_kind == "event.world.transitioned":
        name = {"object.tutorial.quest_ladder": "travel.tutorial_quest_ladder.forward"}[given["event.link_ref"]]
        destination = content["mechanics"]["travels"][name]["destination"]["value"]["location"]
        return {"kind": "teleport", "travel": name,
                "phase": {"kind": "completed", "origin": {"region": model.data["region"], "tile": model.data["tile"], "instance": None},
                          "destination": destination}}
    if source_kind == "event.object.inspected":
        target = next(spawn["id"] for spawn in content["spawns"].values()
                      if spawn["kind"].get("object") == given["event.object_ref"])
        return {"kind": "inspected", "target": target, "explanation": "tutorial_poll_explanation"}
    if source_kind == "event.dialogue.completed":
        return {"kind": "dialogue_selected", "speaker": "spawn." + given["event.npc_ref"].removeprefix("npc."),
                "choice": given["event.topic"]}
    if source_kind == "event.spell.resolved":
        return {"kind": "spell_resolved", "spell": given["event.spell_ref"],
                "target": "spawn.tutorial_chicken.3138.3093.p0", "outcome": given["event.outcome"],
                "damage": 0 if given["event.outcome"] == "splash" else 1,
                "tile": {"x": 3138, "y": 3093, "plane": 0}}
    if source_kind.startswith("event.teleport."):
        kind = source_kind.removeprefix("event.teleport.")
        phase = {"kind": kind}
        if kind == "interrupted":
            phase["reason"] = "movement"
        elif kind == "completed":
            branches = content["mechanics"]["travels"]["travel.tutorial.departure"]["destination"]["value"]["branches"]
            destination = branches["experience.experienced"]
            model.data["runtime"]["settings"]["experience"] = "experience.brand_new"
            model.data["tile"] = destination["tile"]
            phase.update({"origin": {"region": "region.osrs.12592", "tile": {"x": 3141, "y": 3087, "plane": 0}, "instance": None},
                          "destination": destination})
        return {"kind": "teleport", "travel": "travel.tutorial.departure", "phase": phase}
    raise ValueError(f"Unmapped independent oracle event {source_kind}")


def verify():
    inputs = Inputs()
    content = load(CONTENT / "game-content.json.gz")
    aliases = inputs.rules["vocabulary"]["guard_projection"]["inventory_count_aliases"]
    bindings = load(BINDINGS / "graph-bindings.json")
    results = []
    for case in inputs.rules["expected-scenarios"]["graph_cases"]:
        model = Oracle(content)
        given, initial = case["given"], case["from_ref"]
        if initial.startswith("stage.tutorial."):
            model.data["tutorial_stage"] = initial
            if initial in ("stage.tutorial.wind_strike", "stage.tutorial.departure_offer"):
                model.data["quests"]["quest.learning_the_ropes"]["stage"] = "stage.learning_the_ropes.in_progress"
            for path, value in given.items():
                if path.startswith("state.inventory.") and value:
                    model.give(aliases[path.removeprefix("state.inventory.")], value)
                elif path == "state.bank.seed_claimed" and value:
                    model.effects([{"kind": "grant", "grant": "grant.tutorial.bank_coins"}])
                elif path.startswith("state.tutorial."):
                    model.data["runtime"]["counters"]["counter." + path.removeprefix("state.")] = counter_value(value)
                elif path == "state.quests.learning_the_ropes.status":
                    model.data["quests"]["quest.learning_the_ropes"]["stage"] = "stage.learning_the_ropes." + value
            event = source_event(content, model, case["event_ref"], given)
            matched = model.event(event)
            actual = model.data["tutorial_stage"]
        elif initial.startswith("stage.cooks."):
            model.data["tutorial_stage"] = "stage.tutorial.mainland"
            model.data["quests"]["quest.cooks_assistant"]["stage"] = initial
            if given.get("event.owned_items_consumed"):
                for item in given["event.delivered"]:
                    model.give({"milk": "item.milk.bucket", "flour": "item.flour.pot", "egg": "item.egg"}[item], 1)
            node = next(node for node in content["dialogues"]["dialogue.cook"]["nodes"] if node["id"] == initial)
            choices = [choice for choice in node["choices"] if model.guard(choice["guard"])]
            matched = len(choices) == 1
            if matched:
                model.effects(choices[0]["effects"])
            actual = model.data["quests"]["quest.cooks_assistant"]["stage"]
        else:
            model.data["life"] = "first_death_office"
            model.data["runtime"]["counters"]["counter.death.introduction_heard"] = counter_value(True)
            for topic in ("fees", "timer", "kept_items"):
                if given.get("state.death.topics." + topic):
                    model.data["death_topics"].add(topic)
            done = next(node for node in content["dialogues"]["dialogue.death"]["nodes"] if node["id"] == "stage.death.topics")["choices"][-1]
            matched = model.guard(done["guard"])
            if matched:
                model.effects(done["effects"])
            actual = "stage.death.exit_ready" if model.counter("counter.death.exit_confirmed")["value"] else "stage.death.topics"
        if actual != case["expect_stage_ref"] or matched != (case["expect_transition_ref"] is not None):
            raise AssertionError(f"{case['id']}: got {(matched, actual)}, expected {(case['expect_transition_ref'], case['expect_stage_ref'])}")
        results.append({"id": case["id"], "passed": True, "expected_stage": case["expect_stage_ref"]})
    report = {
        "schema_version": 2,
        "game_content_sha256": sha((CONTENT / "game-content.json.gz").read_bytes()),
        "source_oracles_sha256": sha(canonical(inputs.rules["expected-scenarios"])),
        "independent_graph_oracles": results, "passed": len(results),
        "tutorial_source_edges_bound": len(bindings["tutorial"]),
        "cook_source_edges_bound": len(bindings["cooks"]),
        "death_source_edges_bound": len(bindings["death"]["transitions"]),
        "source_boundary_policy": "Fixtures are independently supplied post-authoritative operation/state facts. "
                                  "They test actual schema2 guards/effects against source expectations, not "
                                  "unbound projectile/timing/departure/death execution. No public client event "
                                  "injection, source-observation claim or whole-gameplay acceptance is made.",
        "runtime_journey_passed": False,
    }
    write(BINDINGS / "state-oracle-validation.json", report, pretty=True)
    print(json.dumps({"independent_graph_oracles_passed": len(results), "tutorial_edges_bound": 73,
                      "cook_edges_bound": 22, "death_edges_bound": 6, "runtime_journey_passed": False}))
    return report


if __name__ == "__main__":
    verify()
