#!/usr/bin/env python3
"""Validate source-contract structure and literal oracles, never run the game."""

import argparse
import copy
from fractions import Fraction
import hashlib
import itertools
import json
import math
from pathlib import Path
import re
import urllib.parse


ROOT = Path(__file__).resolve().parent
FILES = [
    "sources.json", "vocabulary.json", "decisions.json", "initial-state.json",
    "tutorial.json", "activities.json", "cooks-assistant.json", "expected-scenarios.json",
]
ID = re.compile(r"^[a-z][a-z0-9_]*(?:\.[a-z][a-z0-9_]*)+$")
KINDS = {"source", "contract", "profile", "choice", "event", "ui", "slot", "location",
         "npc", "object", "spell", "prayer", "skill", "quest", "item", "stage",
         "transition", "rule", "formula", "grant", "reward", "recovery", "decision",
         "assumption", "binding", "route", "graph", "loot", "scenario", "fixture_input"}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def walk(value):
    if isinstance(value, dict):
        yield value
        for child in value.values():
            yield from walk(child)
    elif isinstance(value, list):
        for child in value:
            yield from walk(child)


def predicates(condition):
    if "all" in condition or "any" in condition:
        for child in condition.get("all", condition.get("any", [])):
            yield from predicates(child)
    else:
        yield condition


def matches(condition, values):
    if "all" in condition:
        return all(matches(c, values) for c in condition["all"])
    if "any" in condition:
        return any(matches(c, values) for c in condition["any"])
    path, op, expected = condition["path"], condition["op"], condition["value"]
    if path not in values:
        return False
    actual = values[path]
    if op in ("eq", "ne"):
        equal = type(actual) is type(expected) and actual == expected
        return equal if op == "eq" else not equal
    if op == "in":
        return any(type(actual) is type(v) and actual == v for v in expected)
    require(op in ("gte", "lte"), f"Unsupported guard operation: {op}")
    require(type(actual) in (int, float) and type(expected) in (int, float),
            f"Non-numeric ordering guard: {path}")
    return actual >= expected if op == "gte" else actual <= expected


def formula_result(name, x):
    # Deliberately no eval, game import, network request or implementation oracle.
    def clamp(n, low, high):
        return min(high, max(low, n))

    if name == "formula.skilling.success":
        return clamp(1 + (x["low"] * (99 - x["level"]) + x["high"] * (x["level"] - 1) + 49) // 98, 0, 256)
    if name == "formula.xp.level_threshold":
        return 10 * (sum(math.floor(n + 300 * 2 ** (n / 7)) for n in range(1, x["level"])) // 4)
    if name == "formula.run.drain":
        base = math.floor(60 + 67 * clamp(x["weight_kg"], 0, 64) / 64)
        return base * (300 - x["agility"]) // 300
    if name == "formula.run.recovery":
        return x["agility"] // 10 + 15
    if name == "formula.combat.effective_level":
        return x["current_level"] * x["prayer_numerator"] // x["prayer_denominator"] + x["style_bonus"] + 8
    if name == "formula.combat.maximum_hit":
        return (x["effective_strength"] * (x["equipment_strength"] + 64) + 320) // 640
    if name == "formula.combat.accuracy":
        a, d = x["attack_roll_max"], x["defence_roll_max"]
        require(a >= 0 and d >= 0, "Positive-roll oracle cannot certify the negative-roll assumption")
        fraction = 1 - Fraction(d + 2, 2 * (a + 1)) if a > d else Fraction(a, 2 * (d + 1))
        return {"numerator": fraction.numerator, "denominator": fraction.denominator}
    if name == "formula.combat.magic_attack":
        return (x["magic"] * x["prayer_numerator"] // x["prayer_denominator"] + 8) * (x["equipment_magic_attack"] + 64)
    if name == "formula.combat.magic_defence_npc":
        return (x["magic"] + 9) * (x["equipment_magic_defence"] + 64)
    if name == "formula.combat.hp_xp":
        return 0 if x["on_tutorial"] else 40 * x["credited_damage"] // 3
    if name in ("formula.shop.buy", "formula.shop.sell"):
        difference = x["base_stock"] - x["current_stock"]
        buying = name.endswith(".buy")
        rate = clamp((1300 if buying else 400) + 30 * difference, 300 if buying else 100, 6300 if buying else 1400)
        return max(1 if buying else 0, x["base_value"] * rate // 1000)
    if name == "formula.death.grave_fee":
        return min(500000, sum(0 if v < 100000 else 1000 if v < 1000000 else 10000 if v < 10000000 else 100000 for v in x["values"]))
    if name == "formula.death.office_fee":
        return sum(0 if v < 100000 else v * 5 // 100 for v in x["values"])
    raise ValueError(f"No independent arithmetic check for {name}")


def inspect_graph(graph):
    states = {s["id"] for s in graph["states"]}
    edges = graph["transitions"]
    require(graph["start_ref"] in states, f"Invalid graph start: {graph['id']}")
    require(set(graph["terminal_refs"]) <= states, f"Invalid graph terminal: {graph['id']}")
    forward = {s: set() for s in states}
    backward = {s: set() for s in states}
    for edge in edges:
        require(edge["from_ref"] in states and edge["to_ref"] in states,
                f"Dangling graph transition: {edge['id']}")
        forward[edge["from_ref"]].add(edge["to_ref"])
        backward[edge["to_ref"]].add(edge["from_ref"])

    def closure(seeds, neighbors):
        seen = set(seeds)
        pending = list(seen)
        while pending:
            for neighbor in neighbors[pending.pop()]:
                if neighbor not in seen:
                    seen.add(neighbor)
                    pending.append(neighbor)
        return seen

    require(closure([graph["start_ref"]], forward) == states, f"Unreachable graph states: {graph['id']}")
    require(closure(graph["terminal_refs"], backward) == states, f"States cannot reach terminal: {graph['id']}")


def validate(documents, snapshots=False):
    registry = {}
    for filename, doc in documents.items():
        require(doc["schema_version"] == 1, f"Unsupported schema: {filename}")
        for obj in walk(doc):
            if "id" in obj:
                identifier = obj["id"]
                require(ID.fullmatch(identifier), f"Invalid kind-first ID: {identifier}")
                require(identifier not in registry, f"Duplicate ID: {identifier}")
                registry[identifier] = obj

    def references(value):
        if isinstance(value, dict):
            for key, child in value.items():
                if key.endswith("_ref"):
                    require(child is None or isinstance(child, str), f"Non-scalar reference: {key}")
                    require(child is None or child in registry, f"Dangling reference {key}: {child}")
                elif key.endswith("_refs"):
                    require(isinstance(child, list), f"Non-array references: {key}")
                    for ref in child:
                        require(ref in registry, f"Dangling reference {key}: {ref}")
                if key not in ("id", "path", "from_path"):
                    references(child)
        elif isinstance(value, list):
            for child in value:
                references(child)
        elif isinstance(value, str) and ID.fullmatch(value) and value.split(".")[0] in KINDS:
            require(value in registry, f"Unresolved embedded semantic ID: {value}")

    for doc in documents.values():
        references(doc)
        for obj in walk(doc):
            if "basis" in obj:
                evidence = obj["basis"]
                require(evidence["classification"] in documents["decisions.json"]["evidence_classes"],
                        "Invalid evidence class")
                require(evidence["source_refs"], "Evidence needs at least one source")
                if evidence["classification"] == "verified_reference":
                    require(not evidence.get("assumption_refs"), "An assumption cannot be labeled verified")
            if "guard" in obj:
                for predicate in predicates(obj["guard"]):
                    require(set(predicate) == {"path", "op", "value"}, f"Invalid predicate: {predicate}")
                    require(predicate["path"].startswith(("state.", "event.")), "Invalid guard namespace")
                    require(predicate["op"] in ("eq", "ne", "gte", "lte", "in"), "Invalid guard operation")
            if "xp_tenths" in obj:
                require(type(obj["xp_tenths"]) is int and 0 <= obj["xp_tenths"] <= 2000000000,
                        "XP must be nonnegative integer tenths within source cap")

    for source in documents["sources.json"]["sources"]:
        if source["kind"] == "wiki_revision":
            require(type(source["revision"]) is int, "Wiki revision must be numeric")
            url = urllib.parse.urlparse(source["url"])
            require(url.scheme == "https" and url.hostname == "oldschool.runescape.wiki",
                    f"Unexpected source host: {source['id']}")
            require(urllib.parse.parse_qs(url.query).get("oldid") == [str(source["revision"])],
                    f"Revision/link mismatch: {source['id']}")
            require(re.fullmatch(r"[0-9a-f]{64}", source["sha256_utf8_wikitext"]), "Invalid source hash")
            require(source["revision_timestamp"] and source["bytes_utf8_wikitext"] > 0, "Missing source identity")
            if snapshots:
                path = ROOT / ".local" / "snapshots" / (source["id"].removeprefix("source.wiki.") + ".wikitext")
                data = path.read_bytes()
                require(hashlib.sha256(data).hexdigest() == source["sha256_utf8_wikitext"], f"Source hash mismatch: {source['id']}")
                require(len(data) == source["bytes_utf8_wikitext"], f"Source length mismatch: {source['id']}")
        elif source["kind"] == "repository_file":
            data = (ROOT / source["path"]).read_bytes()
            require(hashlib.sha256(data).hexdigest() == source["sha256"], f"Project authority file changed: {source['id']}")
        else:
            raise ValueError(f"Unknown source kind: {source['kind']}")

    graphs = [documents["tutorial.json"], documents["cooks-assistant.json"], documents["activities.json"]["death_graph"]]
    for graph in graphs:
        inspect_graph(graph)
    require("event.advance_tutorial" not in registry, "Scripted stage-skip event is forbidden")
    initial = documents["initial-state.json"]
    require(len(initial["skills"]) == 24, "Modern profile requires 24 skills")
    require(sum(s["base_level"] for s in initial["skills"]) == 33, "Initial total level must be 33")
    for skill in initial["skills"]:
        require(skill["xp_tenths"] == (11540 if skill["id"] == "skill.hitpoints" else 0),
                "Boosted initial skill state")
    required_families = {"movement", "inventory", "equipment", "mining", "woodcutting", "fishing",
                         "firemaking", "cooking", "smelting", "smithing", "melee", "ranged", "magic",
                         "banking", "shop", "death", "quest", "ingredient", "persistence", "prayer"}
    families = {r["family"] for d in documents.values() for r in d.get("rules", [])}
    require(required_families <= families, f"Missing rule families: {required_families - families}")
    rewards = documents["cooks-assistant.json"]["rewards"]
    require(len({r["claim_key"] for r in rewards}) == len(rewards), "Duplicate reward entitlement key")
    for reward in rewards:
        require(any(p["path"].startswith("state.rewards.") and p["op"] == "eq" and p["value"] is False
                    for p in predicates(reward["guard"])), f"Reward lacks once-only condition: {reward['id']}")
        claims = [t for g in graphs for t in g["transitions"]
                  if any(e.get("reward_ref") == reward["id"] for e in t["effects"])]
        require(len(claims) == 1, f"Reward has more/less than one completion claim site: {reward['id']}")
    cook_reward = registry["reward.cooks_assistant"]
    require(cook_reward["quest_points"] == 1 and cook_reward["xp_awards"] == [{"skill_ref": "skill.cooking", "xp_tenths": 3000}]
            and cook_reward["items"] == [], "Cook reward contradicts pinned source")
    require(registry["reward.learning_the_ropes"]["quest_points"] == 1, "Missing modern tutorial QP")
    departure = registry["rule.tutorial.departure"]
    require(len(departure["kit"]) == 18, "Departure kit must have 18 distinct kinds")
    kit = {entry["item_ref"]: entry["quantity"] for entry in departure["kit"]}
    require(kit["item.arrow.bronze"] == 25 and kit["item.rune.air"] == 25 and kit["item.rune.mind"] == 15,
            "Do not confuse training grants with departure provisions")
    require("item.hammer" not in kit and departure["bank"] == [{"item_ref": "item.coins", "quantity": 25}],
            "Departure source provisions mismatch")
    require(sum(row["weight"] for row in registry["loot.goblin.level_2_table_1"]["primary"]) == 128,
            "Goblin primary weights must sum to 128 without inventing common-potion weights")
    require(registry["loot.goblin.level_2_table_1"]["unresolved_supplement"]["weight"] is None,
            "Unknown potion chance must not be fabricated")

    fixtures = documents["expected-scenarios.json"]
    require(fixtures["status"] == "independently_authored_requirements_not_implementation_execution",
            "Fixtures are not execution evidence")
    for case in fixtures["formula_cases"]:
        require(formula_result(case["formula_ref"], case["input"]) == case["expect"],
                f"Arithmetic oracle mismatch: {case['id']}")
    for case in fixtures["guard_cases"]:
        values = {**registry[case["input_profile_ref"]]["values"], **case["given"]}
        target = registry[case.get("rule_ref", case.get("reward_ref"))]
        missing = {p["path"] for p in predicates(target["guard"])} - values.keys()
        require(not missing, f"Fixture has unknown rather than deliberately invalid inputs: {case['id']}: {missing}")
        require(matches(target["guard"], values) is case["expect_allowed"], f"Guard oracle mismatch: {case['id']}")
    for case in fixtures["graph_cases"]:
        graph = registry[case["graph_ref"]]
        matching = [t for t in graph["transitions"] if t["from_ref"] == case["from_ref"]
                    and t["event_ref"] == case["event_ref"] and matches(t["guard"], case["given"])]
        require(len(matching) <= 1, f"Ambiguous graph fixture: {case['id']}")
        actual_transition = matching[0]["id"] if matching else None
        actual_stage = matching[0]["to_ref"] if matching else case["from_ref"]
        require(actual_transition == case["expect_transition_ref"] and actual_stage == case["expect_stage_ref"],
                f"Graph fixture mismatch: {case['id']}")
    for case in fixtures["outcome_cases"]:
        expect = case["expect"]
        if expect.get("xp_awards") and "rule_ref" in case:
            rule = registry[case["rule_ref"]]
            expected_awards = rule.get("xp_awards", rule.get("base_xp_awards"))
            require(expect["xp_awards"] == expected_awards, f"XP fixture contradicts rule: {case['id']}")
        if "used_slots" in expect:
            require(0 <= expect["used_slots"] <= 28, f"Inventory fixture exceeds capacity: {case['id']}")
    orders = fixtures["quest_delivery_orders"]["orders"]
    require({tuple(o) for o in orders} == set(itertools.permutations(["milk", "flour", "egg"])),
            "Partial delivery fixtures must include all six orders")
    for order in orders:
        current = "stage.cooks.delivered.none"
        delivered = set()
        for ingredient in order:
            delivered.add(ingredient)
            matching = [t for t in documents["cooks-assistant.json"]["transitions"]
                        if t["from_ref"] == current and t["event_ref"] == "event.quest.delivered"
                        and matches(t["guard"], {"event.quest_ref": "quest.cooks_assistant",
                                                 "event.npc_ref": "npc.cook", "event.delivered": [ingredient],
                                                 "event.owned_items_consumed": True})]
            require(len(matching) == 1, f"Partial delivery order cannot progress: {order}")
            current = matching[0]["to_ref"]
            require({k for k, v in registry[current]["delivered"].items() if v} == delivered,
                    "Partial delivery bits contradict transition")
        require(current == "stage.cooks.delivered.milk_flour_egg", "Partial deliveries fail to reach reward readiness")
    exact = registry["scenario.outcome.cooks_exact_reward"]
    require(exact["expect"]["cooking_xp_tenths"] - exact["given"]["cooking_xp_tenths"] == 3000
            and exact["expect"]["quest_points"] - exact["given"]["quest_points"] == 1
            and exact["expect"]["items"] == [], "Cook reward fixture contradicts source")
    require(registry["scenario.outcome.departure_no_resource_smuggling"]["expect"]["standard_kit_kind_count"] == len(kit),
            "Departure fixture differs from provisional policy")
    require(all(j["arrival_ref"] in {b["arrival_ref"] for b in initial["experience_branches"]}
                for j in fixtures["journey_requirements"] if "choice_ref" in j), "Arrival branch fixture mismatch")
    branches = {b["choice_ref"]: b for b in initial["experience_branches"]}
    for journey in fixtures["journey_requirements"]:
        if "choice_ref" in journey:
            require(journey["arrival_ref"] == branches[journey["choice_ref"]]["arrival_ref"], "Experience-specific arrival mismatch")
    acceptance = documents["decisions.json"]["acceptance"]
    require(not any(acceptance[k] for k in ("implementation_executed", "journey_executed", "presentation_approved", "milestone_accepted")),
            "Source validation must not self-approve milestone or presentation")
    return {
        "ids": len(registry), "source_snapshots": len(documents["sources.json"]["sources"]),
        "wiki_snapshots": sum(s["kind"] == "wiki_revision" for s in documents["sources.json"]["sources"]),
        "tutorial_states": len(graphs[0]["states"]), "tutorial_transitions": len(graphs[0]["transitions"]),
        "cooks_states": len(graphs[1]["states"]), "cooks_transitions": len(graphs[1]["transitions"]),
        "death_states": len(graphs[2]["states"]), "death_transitions": len(graphs[2]["transitions"]),
        "activity_rules": len(documents["activities.json"]["rules"]), "quest_rules": len(documents["cooks-assistant.json"]["rules"]),
        "rule_families": len(families), "formula_vectors": len(fixtures["formula_cases"]),
        "guard_vectors": len(fixtures["guard_cases"]), "graph_vectors": len(fixtures["graph_cases"]),
        "outcome_requirements": len(fixtures["outcome_cases"]), "delivery_orders": len(orders),
        "checkpoint_requirements": len(fixtures["checkpoint_requirements"]),
        "journey_requirements": len(fixtures["journey_requirements"]),
        "assumptions": len(documents["decisions.json"]["assumptions"]),
        "critical_assumption_ids": [a["id"] for a in documents["decisions.json"]["assumptions"] if a["critical_for_exact_fidelity"]],
        "snapshots_hash_checked": snapshots, "gameplay_executed": False,
    }


def self_test(documents):
    mutations = [
        lambda d: d["tutorial.json"]["transitions"][0].update(to_ref="stage.tutorial.nonexistent"),
        lambda d: d["tutorial.json"]["transitions"].pop(0),
        lambda d: d["cooks-assistant.json"]["rewards"][0].update(claim_key="quest.cooks_assistant"),
        lambda d: d["expected-scenarios.json"]["formula_cases"][0].update(expect=100),
        lambda d: d["expected-scenarios.json"]["graph_cases"][0].update(expect_stage_ref="stage.tutorial.settings_open"),
        lambda d: d["decisions.json"]["acceptance"].update(milestone_accepted=True),
    ]
    for mutate in mutations:
        altered = copy.deepcopy(documents)
        mutate(altered)
        try:
            validate(altered)
        except ValueError:
            continue
        raise ValueError("Validator accepted a deliberately corrupted contract")
    return len(mutations)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--snapshots", action="store_true", help="Also hash local raw wiki snapshots acquired by fetch_sources.py.")
    parser.add_argument("--self-test", action="store_true", help="Reject in-memory dangling edges, unreachable states, duplicated rewards and incorrect oracles.")
    parser.add_argument("--report", action="store_true", help="Save a hash-bound source-only validation report beside the contracts.")
    args = parser.parse_args()
    documents = {name: json.loads((ROOT / name).read_text()) for name in FILES}
    result = validate(documents, args.snapshots)
    if args.self_test:
        result["invalid_mutations_rejected"] = self_test(documents)
    if args.report:
        from datetime import datetime, timezone
        report = {
            "schema_version": 1,
            "validation_kind": "source_contract_consistency_only",
            "validated_at": datetime.now(timezone.utc).isoformat(),
            "command": "python3 research/journey-rules/validate.py"
                       + (" --snapshots" if args.snapshots else "")
                       + (" --self-test" if args.self_test else "") + " --report",
            "input_sha256": {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest()
                             for name in FILES + ["author_contracts.py", "validate.py", "fetch_sources.py"]},
            "result": result,
            "not_proven": ["game implementation", "executed starter journey", "source-client binding",
                           "visual/audio fidelity", "RuneLite compatibility", "browser performance", "owner approval"],
        }
        (ROOT / "validation-result.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
