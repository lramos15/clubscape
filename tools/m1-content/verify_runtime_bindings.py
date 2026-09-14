#!/usr/bin/env python3
"""Audit canonical source application, independent loot and money-preserving departure data."""

from collections import Counter
from copy import deepcopy
from fractions import Fraction
import json

from common import CONTENT, canonical, item_stack, load, sha, write
from runtime_application import ADDITIONAL_ITEMS, DIRECTORY, POTION_ITEMS, classify_residuals, source_applier
from state_oracles import Oracle, OracleRefusal


def require(value, message):
    if not value:
        raise ValueError(message)


def select_exclusive(pool, draw):
    if not 0 <= draw < pool["total_weight"]:
        raise ValueError("Loot draw is outside the declared uniform domain")
    for entry in pool["entries"]:
        if draw < entry["weight"]:
            return entry["items"]
        draw -= entry["weight"]
    raise ValueError("Declared loot weights do not cover their domain")


def loot_oracles(content, application):
    pools = content["npcs"]["npc.goblin.level_2"]["combat"]["mechanics"]["loot"]
    require(sha(canonical(pools[:3])) == application["approved_loot"]["primary_and_guaranteed_sha256"],
            "Approved supplement altered existing primary/guaranteed loot")
    supplement = pools[3]
    require(supplement == application["approved_loot"]["lowered_pool"], "Approved dose distribution changed")
    require(supplement["kind"] == "exclusive" and supplement["total_weight"] == 64, "Wrong independent supplement domain")
    require(supplement["entries"][0] == {"weight": 60, "items": []}, "Potion miss is not the declared60/64 outcome")
    require([entry["items"][0]["item"] for entry in supplement["entries"][1:]] == POTION_ITEMS,
            "Three-dose or another source dose is missing")
    primary = pools[1]["pools"][0]
    require(primary["kind"] == "exclusive" and primary["total_weight"] == 128, "Published primary table changed")
    require(sum(entry["weight"] for entry in primary["entries"]) == 128, "Primary weights were renormalized")
    counts = Counter()
    for primary_draw in range(128):
        expected_primary = deepcopy(select_exclusive(primary, primary_draw))
        by_primary = Counter()
        for potion_draw in range(64):
            actual_primary = select_exclusive(primary, primary_draw)
            require(actual_primary == expected_primary, "Potion draw changed the independent primary outcome")
            dose = select_exclusive(supplement, potion_draw)
            require(len(dose) <= 1, "More than one potion can be awarded on an event")
            if dose:
                require(dose[0]["minimum"] == dose[0]["maximum"] == 1, "Potion quantity is not one")
                counts[dose[0]["item"]] += 1
                by_primary[dose[0]["item"]] += 1
        require(by_primary == Counter({item: 1 for item in POTION_ITEMS}), "Potion distribution depends on primary draw")
    require(Fraction(sum(counts.values()), 128 * 64) == Fraction(1, 16), "Wrong potion event probability")
    require(all(Fraction(value, sum(counts.values())) == Fraction(1, 4) for value in counts.values()),
            "Dose probabilities are not equal conditional on the event")
    source = content["npcs"]["npc.goblin.level_2"]["source"]
    approved = [record for record in source if record["reference"] == "milestones/m1-goblin-loot-approval.json"]
    require(len(approved) == 1 and approved[0]["status"] == "approved_adaptation", "Approval provenance was lost or relabeled as source odds")
    require(content["mechanics"]["world_members"] is False, "Ordinary potion applicability became members-only")
    require(application["approved_loot"]["retained_tertiary_candidates"], "Full-target tertiary candidates were discarded")
    # Reference receipt invariants use the same spawn/life/loot-resolved identity
    # as EntityRuntime; enforcement in the production executor remains separate.
    state = {"spawn": "spawn.goblin.level_2.3246.3235.p0", "life": 3, "loot_resolved": False}
    def claim(spawn, life, credited):
        if spawn != state["spawn"] or life != state["life"] or not credited or state["loot_resolved"]:
            return []
        state["loot_resolved"] = True
        return select_exclusive(supplement, 62)
    require(claim(state["spawn"], 2, True) == [], "Old NPC life was eligible")
    require(claim(state["spawn"], 3, False) == [], "Uncredited kill was eligible")
    reward = claim(state["spawn"], 3, True)
    require(reward == [{"item": "item.energy_potion.three_dose", "minimum": 1, "maximum": 1}], "Correct current kill was not eligible")
    require(claim(state["spawn"], 3, True) == [], "Replayed NPC life duplicated loot")
    return {"joint_primary_potion_vectors": 128 * 64, "event_probability": "1/16",
            "conditional_dose_probabilities": {item: "1/4" for item in POTION_ITEMS},
            "receipt_invariant_vectors": 4, "production_replay_execution_claimed": False}


def departure_oracles(content, source_oracles):
    effects = content["mechanics"]["travels"]["travel.tutorial.departure"]["completion_effects"]
    require(effects == [
        {"kind": "reconcile_containers", "reconciliation": "reconciliation.tutorial.departure"},
        {"kind": "grant", "grant": "grant.tutorial.departure.provisions"}],
        "Departure omitted or reordered its coupled cleanup/grant")
    policies = content["mechanics"]["reconciliations"]["reconciliation.tutorial.departure"]["policies"]["value"]
    require(all(policy["kind"] == "remove_items" and "item.coins" not in policy["items"] for policy in policies),
            "Currency was placed in the cleanup set")
    grant = content["mechanics"]["grants"]["grant.tutorial.departure.provisions"]
    require(len(grant["lines"]) == 18 and all(line["item"] != "item.coins" for line in grant["lines"]),
            "Departure has a second coin reward or wrong provision kit")
    results = []
    for case in source_oracles["departure"]["cases"]:
        model = Oracle(content)
        model.data["tutorial_stage"] = "stage.tutorial.teleport_channel"
        x, expected = case["given"], case["expect"]
        for location, container in (("inventory_coins", "inventory"), ("bank_coins", "bank")):
            if x[location]:
                model.give("item.coins", x[location], container)
        model.data["ground_coins"] = x["ground_coins"]
        model.give("item.rune.air", 100)
        model.give("item.hammer", 1)
        model.give("item.ore.copper", 28, "bank")
        model.data["equipment"]["slot.weapon"] = item_stack("item.shortbow")
        model.data["equipment"]["slot.ammo"] = item_stack("item.arrow.bronze", 47)
        before_coins = model.count("item.coins") + model.count("item.coins", "bank") + model.data["ground_coins"]
        model.effects(effects)
        actual = {"inventory_coins": model.count("item.coins"), "bank_coins": model.count("item.coins", "bank"),
                  "ground_coins": model.data["ground_coins"]}
        require(all(actual[key] == expected[key] for key in actual), f"Currency moved/reset in {case['id']}")
        require(sum(actual.values()) == before_coins == expected["total_coins"], "Departure minted/lost money")
        require(model.count("item.arrow.bronze", "inventory_and_equipment") == 25 and
                model.count("item.rune.air", "inventory_and_equipment") == 25 and
                model.count("item.rune.mind", "inventory_and_equipment") == 15, "Provision totals are wrong")
        require(model.count("item.hammer") == 0 and model.count("item.ore.copper", "bank") == 0, "Tutorial noncurrency was smuggled")
        require(model.grant_claimed("entitlement.tutorial.departure") and
                model.grant_claimed("entitlement.tutorial.departure.provisions"), "Coupled claims were not committed")
        stable = deepcopy(model.data)
        model.effects(effects)
        require(model.data == stable, "Replayed completion reset legitimate possessions")
        results.append({"id": case["id"], "passed": True, "coins_before": before_coins, "coins_after": sum(actual.values())})
    full = Oracle(content)
    full.give("item.energy_potion.three_dose", 28)
    before = deepcopy(full.data)
    try:
        full.effects(effects)
    except OracleRefusal:
        pass
    else:
        raise ValueError("Expected full-capacity grant refusal was not surfaced")
    require(full.data == before, "Cleanup was acknowledged without the coupled grant on failure")
    return {"currency_vectors": results, "replay_cases": len(results), "atomic_capacity_rollback": True}


def verify():
    content = load(CONTENT / "game-content.json.gz")
    application = load(DIRECTORY / "application-result.json")
    resolutions = load(DIRECTORY / "resolutions.json")
    context = load(DIRECTORY / "application-context.json")
    module = source_applier()
    require(sha((CONTENT / "game-content.json.gz").read_bytes()) == application["content_compressed_sha256"],
            "Application report is stale")
    require(application["resolutions_sha256"] == sha((DIRECTORY / "resolutions.json").read_bytes()) and
            application["application_context_sha256"] == sha((DIRECTORY / "application-context.json").read_bytes()),
            "Source application input hashes changed")
    require(application["bound_path_count"] == 104 and application["coupled_update_count"] == 7, "Incomplete source application")
    checked = Counter()
    for path, record in resolutions["resolutions"].items():
        if not record["apply_to_ordinary_profile"]:
            continue
        current = deepcopy(module.at(content, record["pointer"]))
        if path.endswith("maximum_hit") and record["classification"] == "codebug_parent_fixed":
            require(current["status"] == "bound" and current["value"] == record["proposed_value"], "Parent fixed source table was changed")
            require(module.canonical_hash(current) == context["accepted_current_targets"][path]["sha256"],
                    "Already-bound parent provenance changed without exact reconciliation")
        else:
            if record["rule_group"] == "death_value_snapshot":
                for item in ADDITIONAL_ITEMS:
                    require(current["value"].pop(item) == 123, "New potion death price is not the fixed guide value")
                current["source"] = record["replacement"]["source"]
            elif record["rule_group"] == "repeat_death":
                current["value"]["supply_items"].remove("item.energy_potion.three_dose")
                current["value"]["source"] = record["replacement"]["value"]["source"]
            require(current == record["replacement"], "Canonical source replacement drift: " + path)
        if record["classification"] == "source_supported_inference":
            require(all(source["status"] == "inference" for source in module.at(content, record["pointer"])["source"]),
                    "A source inference was promoted to observation or approval")
        checked[record["classification"]] += 1
    for change in resolutions["coupled_updates"]:
        require(module.at(content, change["pointer"]) == change["value"], "Coupled update missing or changed: " + change["id"])
    residuals = classify_residuals(content, resolutions)
    require(residuals == application["residuals"], "Inactive/active reachability classification is stale")
    provider = content["mechanics"]["value_providers"]["value_provider.osrs.death"]
    require(provider["method"] == "fixed_source_table" and len(provider["values"]["value"]) == len(content["items"]),
            "Fixed source death table is incomplete or mislabeled")
    audit = load(DIRECTORY / "death-values.json")
    for row in audit["rows"]:
        require(provider["values"]["value"][row["item"]] == row["effective_death_value"], "Original fixed guide value changed")
    source_oracles = load(DIRECTORY / "oracles.json")
    report = {
        "schema_version": 1, "task": "m1-apply-runtime-bindings", "result": "passed",
        "content_compressed_sha256": application["content_compressed_sha256"],
        "source_bindings_checked": dict(checked), "source_bindings_consumed": sum(checked.values()),
        "coupled_updates_checked": len(resolutions["coupled_updates"]),
        "death_values_checked": len(provider["values"]["value"]),
        "loot": loot_oracles(content, application), "departure": departure_oracles(content, source_oracles),
        "residuals": residuals,
        "original_source_numeric_oracles": "research/runtime-bindings/application-source-validation.json",
        "remaining_selector_hooks": application["remaining_selector_hooks"],
        "gameplay_executed": False, "presentation_approved": False,
    }
    write(DIRECTORY / "application-validation.json", report, pretty=True)
    print(json.dumps({"source_bindings_consumed": report["source_bindings_consumed"],
                      "coupled_updates": report["coupled_updates_checked"],
                      "loot_joint_vectors": report["loot"]["joint_primary_potion_vectors"],
                      "currency_vectors": len(report["departure"]["currency_vectors"]),
                      "active_residuals": residuals["active_or_conditionally_active_count"],
                      "inactive_full_target_dependencies": residuals["inactive_full_target_count"]}))
    return report


if __name__ == "__main__":
    verify()
