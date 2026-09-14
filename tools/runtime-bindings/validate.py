#!/usr/bin/env python3
"""Check binding dispositions, exact source hashes and independent source oracles."""

import argparse
from collections import Counter, defaultdict
from copy import deepcopy
from datetime import datetime, timezone
import gzip
import hashlib
import json
from pathlib import Path
import re
import urllib.parse

from apply import at, canonical_hash, prepare
from build import index_content


ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "research/runtime-bindings"
CLASSES = {"known_fact", "source_supported_inference", "codebug_parent_fixed",
           "codebug_parent_required", "inactive_dependency", "owner_decision"}


def check(condition, message):
    if not condition:
        raise ValueError(message)


def load(name):
    return json.loads((OUT / name).read_text())


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def integer(value, maximum=2**32 - 1):
    return type(value) is int and 0 <= value <= maximum


def snapshots(sources, require_raw):
    for source in sources:
        check(re.fullmatch(r"[a-zA-Z0-9_.]+", source["id"]), "Unsafe source snapshot ID")
        check(re.fullmatch(r"[0-9a-f]{64}", source["sha256"]), "Malformed source hash")
        check(integer(source["bytes"]) and source["bytes"] > 0, "Empty source snapshot")
        check(source["retrieved_at"], "Missing source retrieval date")
        parsed = urllib.parse.urlparse(source["url"])
        check(parsed.scheme == "https" and parsed.hostname, "Non-HTTPS source reference")
        if source["kind"] == "wiki_revision":
            check(parsed.hostname == "oldschool.runescape.wiki", "Unexpected wiki source")
            check(urllib.parse.parse_qs(parsed.query).get("oldid") == [str(source["revision"])],
                  "Wiki URL/revision mismatch")
        elif source["kind"] == "public_code":
            check("/" + source["revision"] + "/" in parsed.path, "Unpinned code reference")
        path = OUT / source["local_raw"]
        check(path.resolve().is_relative_to(OUT / ".local/raw"), "Snapshot outside owned raw directory")
        if source["kind"] == "public_price_feed":
            data = gzip.decompress((OUT / "inputs/guide-prices.json.gz").read_bytes())
        elif path.exists():
            data = path.read_bytes()
        elif require_raw:
            raise ValueError("Restore exact source bytes first: " + source["id"])
        else:
            continue
        check(hashlib.sha256(data).hexdigest() == source["sha256"], "Source hash mismatch: " + source["id"])
        check(len(data) == source["bytes"], "Source byte-length mismatch: " + source["id"])


def validate(document, original, inventory, sources, oracles, values):
    rows = document["resolutions"]
    source_ids = {s["id"] for s in sources}
    check(set(rows) == {r["path"] for r in inventory} and len(rows) == 112,
          "Every original112 path must have exactly one disposition")
    check(document["path_count"] == 112, "Wrong path count")
    check(document["counts"] == dict(Counter(r["classification"] for r in rows.values())),
          "Disposition counts disagree")
    originals = index_content(original)
    grouped = defaultdict(list)
    for path, row in rows.items():
        check(row["path"] == path and row["classification"] in CLASSES, "Malformed disposition")
        check(tuple(row["pointer"]) == originals[path][0], "Dotted/bracket path mapped incorrectly: " + path)
        check(canonical_hash(at(original, row["pointer"])) == row["original_binding_sha256"],
              "Input binding fingerprint mismatch: " + path)
        check(row["source_refs"] and set(row["source_refs"]) <= source_ids, "Missing source references")
        check(row["reason"] and row["acceptance_impact"], "Missing rationale/impact")
        check(row["source_observation_claimed"] is False, "Source observation was fabricated")
        blocked_class = row["classification"] in {"inactive_dependency", "owner_decision", "codebug_parent_required"}
        check(row["apply_to_ordinary_profile"] is not blocked_class,
              "An inactive/decision/type-gap record was promoted or a usable binding was hidden")
        if blocked_class:
            check(row["replacement"] is None and row["proposed_value"] is None, "Nonexecuting disposition has a fake bound value")
        else:
            replacement = row["replacement"]
            check(replacement["status"] == "bound" and replacement["value"] == row["proposed_value"],
                  "Bad typed replacement")
            check(replacement["source"], "Bound value has no provenance")
            if row["classification"] == "source_supported_inference":
                check(all(s["status"] == "inference" for s in replacement["source"]),
                      "Inference was promoted to verified-reference provenance")
            check(all(s["status"] != "approved_adaptation" for s in replacement["source"]), "Self-approved adaptation")
        grouped[row["rule_group"]].append(row)

        value = row["proposed_value"]
        if value is not None and row["value_type"] in {"u32", "Option<u32>"}:
            check(integer(value), "Invalid tick unit/value")
        if row["value_type"] == "TickDuration" and value is not None:
            check(value == {"kind": "fixed", "ticks": value["ticks"]} and integer(value["ticks"]) and value["ticks"] > 0,
                  "Invalid respawn duration")
        if row["value_type"] == "ProjectileTiming":
            check(set(value) == {"launch_delay_ticks", "base_flight_ticks", "ticks_per_tile", "rounding",
                                 "damage_on_launch", "recheck_target_on_impact"}, "Unexpected projectile fields")
            check(integer(value["base_flight_ticks"]) and value["base_flight_ticks"] > 0, "Fake zero projectile flight")
            ratio = value["ticks_per_tile"]
            check(integer(ratio["numerator"]) and integer(ratio["denominator"]) and ratio["denominator"] > 0,
                  "Invalid projectile ratio")
        if row["value_type"] == "BTreeMap<ItemId,u64>":
            check(set(value) == set(original["items"]), "Death values don't cover the represented item universe")
            check(all(integer(v, 2**64 - 1) for v in value.values()), "Invalid death value")
    for group, members in grouped.items():
        check(len({canonical_hash(r["proposed_value"]) for r in members}) == 1,
              "Repeated rule drift: " + group)

    candidate = prepare(original, document)
    changed = sum(r["apply_to_ordinary_profile"] for r in rows.values())
    check(changed == 104, "Unexpected binding-application count")
    for key in ["items", "npcs", "spawns", "recipes", "quests", "tutorial"]:
        check(len(candidate[key]) == len(original[key]), "Content was deleted to make closure appear complete")
    remaining = [p for p, r in rows.items() if not r["apply_to_ordinary_profile"]]
    check(len(remaining) == 8, "Remaining dispositions changed")
    check(oracles["status"] == "independent_source_requirements_not_game_execution", "Oracles became fake execution evidence")
    check(not document["gameplay_executed"] and not document["presentation_approved"], "Acceptance was self-approved")

    checked = Counter()
    projectile_types = candidate["mechanics"]["projectiles"]
    bow = projectile_types["projectile.bronze_arrow"]["timing"]["value"]
    magic = projectile_types["projectile.wind_strike"]["timing"]["value"]
    check(not bow["damage_on_launch"] and not magic["damage_on_launch"], "Projectile damage was applied at launch")
    check(bow["recheck_target_on_impact"] and magic["recheck_target_on_impact"], "Original target/life validation was removed")

    def impact(policy, distance):
        ratio = policy["ticks_per_tile"]
        numerator, denominator = distance * ratio["numerator"], ratio["denominator"]
        rounded = ((2 * numerator + denominator) // (2 * denominator)
                   if policy["rounding"] == "nearest_ties_up" else numerator // denominator)
        return policy["launch_delay_ticks"] + policy["base_flight_ticks"] + rounded

    for distance, bow_base, magic_base, bow_expected, magic_expected in oracles["projectile_tables"]["rows"]:
        check(1 + (distance + 3) // 6 == bow_base and 1 + (distance + 1) // 3 == magic_base,
              "Literal projectile table disagrees with source formula")
        check(impact(bow, distance) == bow_expected and impact(magic, distance) == magic_expected,
              "Typed projectile timing disagrees with source PvM table")
        checked["projectile_distances"] += 1
    hits = candidate["mechanics"]["combat_styles"]["style.magic.wind_strike"]["maximum_hit"]["value"]["hits"]
    for level, expected in oracles["wind_strike"]["rows"]:
        actual = hits[str(max(k for k in map(int, hits) if k <= level))]
        check(actual == expected, "Known Wind Strike hit table changed")
        checked["wind_strike_levels"] += 1
    shop = candidate["shops"]["shop.lumbridge.general_store"]
    pricing = shop["stock"][0]["mechanics"]["pricing"]
    check(pricing["overstock"]["value"] == "linear_to_clamp", "Wrong overstock policy")

    def price(value, base, current, field):
        rate = field["base_per_mille"] + field["change_per_stock"] * (base - current)
        rate = max(field["minimum_per_mille"], min(field["maximum_per_mille"], rate))
        return max(field["minimum_price"], value * rate // 1000)

    for value, base, current, buy, sell in oracles["shop_prices"]["rows"]:
        check(price(value, base, current, pricing["buy"]) == buy, "Buy-price oracle mismatch")
        check(price(value, base, current, pricing["sell"]) == sell, "Sell-price oracle mismatch")
        checked["shop_price_vectors"] += 1
    for case in oracles["shop_prices"]["bulk"]:
        x, e = case["given"], case["expect"]
        buying = "paid_by_player" in e
        field = pricing["buy" if buying else "sell"]
        direction = -1 if buying else 1
        total = sum(price(x["base_value"], x["base_stock"], x["current_stock"] + direction * i, field)
                    for i in range(x["quantity"]))
        check(total == e["paid_by_player" if buying else "paid_to_player"]
              and x["current_stock"] + direction * x["quantity"] == e["new_stock"], "Bulk-price oracle mismatch")
        checked["shop_bulk_vectors"] += 1
    for case in oracles["restock"]["cases"]:
        x, e = case["given"], case["expect"]
        if "interval" not in x:
            check(x["current_tick"] == x["last_processed_boundary"] and e["additional_restock_applications"] == 0,
                  "Restock replay oracle inconsistent")
        else:
            due = x["current_tick"] % x["interval"] == 0
            after = x["stock"] + (1 if x["stock"] < x["base_stock"] else -1) if due else x["stock"]
            check(due == e["restock_due"] and after == e["stock"], "World-phase restock oracle mismatch")
        checked["restock_vectors"] += 1
    for case in oracles["timing"]["cases"]:
        check(all(r["proposed_value"] == case["expect_ticks"] for r in grouped[case["group"]]), "Timing group disagrees with oracle")
        checked["timing_groups"] += 1
    queue = oracles["timing"]["queued_cooking"]
    x = queue["given"]
    check([x["menu_delay"] + x["first"] + i * x["repeat"] for i in range(x["items"])] == queue["expect_completion_offsets"],
          "Queued cooking became boosted singles")
    checked["cooking_queue_vectors"] += 1
    for case in oracles["npc_damage"]["cases"]:
        x = case["given"]
        npc = ("npc.tutorial_chicken" if "chicken" in case["id"] else
               "npc.goblin.level_2" if "goblin" in case["id"] else "npc.tutorial_rat")
        policy = candidate["npcs"][npc]["combat"]["mechanics"]["damage"]["value"]
        check(policy["cap_to_remaining_hitpoints"], "NPC damage no longer caps to actual remaining HP")
        amount = min(max(policy["successful_minimum"], x["raw_damage"]), x["player_hp"]) if x["accuracy_succeeded"] else 0
        if x.get("on_tutorial"):
            amount = min(amount, max(0, x["player_hp"] - 1))
        check(amount == case["expect_damage"], "NPC zero-inclusive damage oracle mismatch")
        checked["npc_damage_vectors"] += 1
    for case in oracles["ground_origins"]["cases"]:
        if case["origin"] == "tutorial":
            check(case["expect"]["maximum_lifetime_ticks"] == 50, "Tutorial drop lifetime changed")
            continue
        policy = candidate["mechanics"]["ground_policies"]["ground_policy." + case["origin"]]
        for field in ["public_after", "expires_after"]:
            check(policy[field]["value"] == case["expect"][field], "Ground origin timer mismatch")
        checked["ground_origin_vectors"] += 1
    provider = candidate["mechanics"]["value_providers"]["value_provider.osrs.death"]
    check(provider["method"] == "fixed_source_table", "Mixed source valuation mislabeled")
    prices = provider["values"]["value"]
    check(len(values["rows"]) == 116, "Incomplete per-item value audit")
    feed = json.loads(gzip.decompress((OUT / "inputs/guide-prices.json.gz").read_bytes()))
    feed_by_id = {p["id"]: p for p in feed}
    check(len(feed_by_id) == len(feed), "Duplicate source guide-price IDs")
    audit = {r["item"]: r for r in values["rows"]}
    for item_id, definition in original["items"].items():
        normal_id = definition["unnoted_variant"] or item_id
        normal = original["items"][normal_id]
        published = feed_by_id.get(normal["source_id"])
        high_alchemy = 3 * normal["base_value"] // 5
        if normal_id == "item.coins":
            expected = 1
        elif not normal["tradable"]:
            expected = normal["base_value"]
        elif published is not None:
            expected = max(published["price"], high_alchemy)
        else:
            expected = high_alchemy
        check(prices[item_id] == expected == audit[item_id]["effective_death_value"],
              "Per-item value differs from source audit: " + item_id)
        check(audit[item_id]["guide_price"] == (published["price"] if published else None),
              "Unpublished GE quote was fabricated")
        check(audit[item_id]["canonical_item"] == normal_id, "Wrong note alias")
    checked["per_item_value_audits"] = len(audit)
    for case in oracles["valuation"]["rows"]:
        check(prices[case["item"]] == case["expect"], "Death value oracle mismatch")
        if "not_wiki_transaction_price" in case:
            check(prices[case["item"]] != case["not_wiki_transaction_price"], "Used active traded price instead of guide")
        checked["valuation_vectors"] += 1
    ranking = oracles["valuation"]["ranking"]
    check(sorted(ranking["given"]["items"], key=lambda i: prices[i], reverse=True)[:3] == ranking["expect"],
          "Kept-item valuation ranking mismatch")
    checked["valuation_rankings"] += 1
    policies = candidate["mechanics"]["reconciliations"]["reconciliation.tutorial.departure"]["policies"]["value"]
    check({p["container"] for p in policies} == {"inventory", "equipment", "bank"}, "Incomplete noncurrency cleanup")
    check(all(p["kind"] == "remove_items" and "item.coins" not in p["items"] for p in policies),
          "Departure resets/reseeds currency")
    grant = candidate["mechanics"]["grants"]["grant.tutorial.departure.provisions"]
    check(len(grant["lines"]) == 18 and all(l["item"] != "item.coins" for l in grant["lines"]), "Wrong provision grant")
    check(grant["entitlement"] == "entitlement.tutorial.departure.provisions", "Missing provision replay guard")
    for case in oracles["departure"]["cases"]:
        containers = {
            "inventory": {"item.coins": case["given"]["inventory_coins"], "item.rune.air": 100, "item.hammer": 1},
            "equipment": {"item.arrow.bronze": 47, "item.shortbow": 1},
            "bank": {"item.coins": case["given"]["bank_coins"], "item.ore.copper": 28},
        }
        for policy in policies:
            for item_id in policy["items"]:
                containers[policy["container"]].pop(item_id, None)
        for line in grant["lines"]:
            containers["inventory"][line["item"]] = containers["inventory"].get(line["item"], 0) + line["quantity"]
        for location in ["bank_coins", "inventory_coins", "ground_coins"]:
            check(case["given"][location] == case["expect"][location], "Currency-conservation oracle contradicted")
        check(containers["bank"].get("item.coins", 0) == case["expect"]["bank_coins"]
              and containers["inventory"].get("item.coins", 0) == case["expect"]["inventory_coins"],
              "Typed departure effects failed coin conservation")
        check(containers["inventory"]["item.arrow.bronze"] == 25 and containers["equipment"] == {},
              "Departure arrow/equipment reconciliation mismatch")
        check(containers["inventory"]["item.rune.air"] == 25 and containers["inventory"]["item.rune.mind"] == 15
              and containers["bank"].get("item.ore.copper", 0) == 0 and containers["inventory"].get("item.hammer", 0) == 0,
              "Tutorial resources were duplicated or smuggled")
        check(sum(case["given"].values()) == case["expect"]["total_coins"], "Currency was minted or lost at departure")
        check(case["expect"]["provision_kinds"] == len(grant["lines"]), "Wrong provision count")
        checked["departure_currency_cases"] += 1
    repeat = candidate["mechanics"]["death"]["repeat"]["value"]
    check(repeat["old_unstackable_per_item_limit"] == 28 and repeat["keep_old_grave_location"], "Repeated death rules changed")
    for case in oracles["death"]["repeat_cases"]:
        x, e = case["given"], case["expect"]
        if "old_logs" in x:
            overflow = max(0, x["old_logs"] - 28)
            check(overflow == e["old_logs_to_office"]
                  and x["old_logs"] - overflow + x["incoming_logs"] == e["grave_logs"], "Wrong repeated-death order")
        if "old_copper" in x:
            check("item.ore.copper" in repeat["old_items_to_office"]
                  and e["old_copper_to_office"] == x["old_copper"] and e["grave_copper"] == x["incoming_copper"],
                  "Old resources not transferred before new")
        if "old_bread" in x:
            check("item.bread" in repeat["supply_items"] and e["old_bread_in_private_supply_pile"] == x["old_bread"],
                  "Old food did not leave the grave")
            check(e["new_bread_in_grave"] == (0 if x["supply_setting"] else x["incoming_bread"]),
                  "New food ignored the configured supply setting")
        if "new_bread" in x:
            check(e["new_bread_in_private_supply_pile"] == (x["new_bread"] if x["supply_setting"] else 0)
                  and e["new_bread_in_grave"] == (0 if x["supply_setting"] else x["new_bread"]),
                  "Source supply setting ignored")
        checked["repeat_death_cases"] += 1
    restore = candidate["mechanics"]["death"]["restoration"]["value"]
    check(restore["on_first_office_exit"] == {}, "Office exit causes an extra heal")
    for case in oracles["death"]["restoration_cases"]:
        x, e = case["given"], case["expect"]
        if x.get("operation") == "first_office_exit":
            actual = {"hp": x["current_hp"], "prayer": x["current_prayer"], "run_energy": x["run_energy"]}
        else:
            actual = {"hp": x["current_hp"], "prayer": x["current_prayer"], "run_energy": x["run_energy"]}
            maxima = {"hp": x["base_hp"], "prayer": x["base_prayer"], "run_energy": 10000}
            for vital, operation in restore["on_arrival"].items():
                field = "hp" if vital == "hitpoints" else vital
                if operation["kind"] == "to_base_maximum":
                    actual[field] = maxima[field]
                elif operation["kind"] == "set":
                    actual[field] = operation["amount"]
                elif operation["kind"] == "amount":
                    actual[field] = min(maxima[field], actual[field] + operation["amount"])
                else:
                    raise ValueError("Unknown restoration operation")
        check(actual == e, "Death restoration oracle mismatch")
        checked["death_restoration_cases"] += 1
    level_cases = oracles["death"]["level_up_conditional_cases"]
    for case in level_cases:
        expected = case["new_base"] if case["current"] == case["old_base"] else case["current"]
        check(expected == case["expect"], "Conditional current-vital oracle mismatch")
        checked["conditional_level_vectors"] += 1
    for strategy in ["preserve", "add_difference", "restore"]:
        def existing_strategy(case):
            if strategy == "preserve":
                return case["current"]
            if strategy == "add_difference":
                return case["current"] + case["new_base"] - case["old_base"]
            return case["new_base"]
        check(any(existing_strategy(c) != c["expect"] for c in level_cases), "Claimed enum gap is not real")
    checked["existing_vital_policies_disproved"] = 3
    for path, row in rows.items():
        if row["rule_group"] == "nondepleting_fishing_no_respawn":
            rule = at(original, row["pointer"][:-2])
            check(rule["depletion"]["numerator_at_level_1"] == 0
                  and rule["depletion"]["numerator_at_level_99"] == 0
                  and rule["depletion"]["domain"]["kind"] == "constant", "Inactive fishing proof does not hold")
            checked["inactive_fishing_proofs"] += 1
    check(candidate["initial_state"]["runtime"]["settings"]["death_auto_equip"] is True, "Missed explicit new-account default")
    check(candidate["initial_state"]["runtime"]["settings"]["death_supply_piles"] is True, "Supply default inference not applied consistently")
    checked["default_setting_bindings"] = 2
    return {
        "exact_paths": 112, "rule_groups": len(grouped), "classification_counts": document["counts"],
        "applyable_bindings": changed, "coupled_updates": len(document["coupled_updates"]),
        "unpromoted_dispositions": remaining, "oracle_checks": dict(checked),
        "numeric_and_edge_checks": sum(checked.values()),
        "outcome": "source_binding_contract_valid",
        "gameplay_executed": False, "presentation_approved": False,
    }


def mutation_tests(document, original, inventory, sources, oracles, values):
    mutations = [
        lambda d: d["resolutions"].pop(next(iter(d["resolutions"]))),
        lambda d: d["resolutions"]["mechanics.projectiles.projectile.wind_strike.timing"]["proposed_value"].update(base_flight_ticks=0),
        lambda d: d["resolutions"]["shops.shop.lumbridge.general_store.stock[0].mechanics.pricing.overstock"].update(
            proposed_value="base_price_above_base_stock"),
        lambda d: d["resolutions"]["npcs.npc.goblin.level_2.combat.mechanics.loot[3]"].update(
            apply_to_ordinary_profile=True, replacement={"kind": "guaranteed", "items": []}),
        lambda d: d["resolutions"]["mechanics.reconciliations.reconciliation.tutorial.departure.policies"]["proposed_value"][0]["items"].append("item.coins"),
        lambda d: d["resolutions"]["mechanics.death.ties"]["replacement"]["source"][0].update(status="verified_reference"),
        lambda d: d["resolutions"]["mechanics.death.office_overflow"].update(
            classification="known_fact", apply_to_ordinary_profile=True, proposed_value="delete_oldest"),
        lambda d: d.update(presentation_approved=True),
    ]
    for mutate in mutations:
        changed = deepcopy(document)
        mutate(changed)
        try:
            validate(changed, original, inventory, sources, oracles, values)
        except (ValueError, KeyError):
            continue
        raise ValueError("A deliberately invalid resolution was accepted")
    return len(mutations)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sources", action="store_true", help="Require/hash every restored raw source snapshot.")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--report", action="store_true")
    args = parser.parse_args()
    document = load("resolutions.json")
    sources = load("sources.json")["sources"]
    source_path = ROOT / document["inputs"]["unresolved_path"]
    content_path = ROOT / document["inputs"]["content_path"]
    check(sha(source_path) == document["inputs"]["unresolved_sha256"], "Original unresolved list changed")
    check(sha(content_path) == document["inputs"]["content_sha256"], "Original product content changed")
    check(sha(ROOT / "crates/game-types/src/mechanics.rs") == document["inputs"]["types_sha256"], "Parent types changed; refresh the disposition")
    original = json.loads(gzip.decompress(content_path.read_bytes()))
    inventory = json.loads(source_path.read_text())["bindings"]
    snapshots(sources, args.sources)
    oracles, values = load("oracles.json"), load("death-values.json")
    result = validate(document, original, inventory, sources, oracles, values)
    result["source_snapshots"] = len(sources)
    result["all_raw_source_hashes_checked"] = args.sources
    typecheck_path = OUT / "typecheck-result.json"
    if typecheck_path.exists():
        typed = json.loads(typecheck_path.read_text())
        expected_remaining = set(result["unpromoted_dispositions"])
        check(typed["strict_candidate_compile_passed"] and typed["mode"] == "runtime", "Strict typecheck did not pass")
        check(set(typed["remaining_unresolved_bindings"]) == expected_remaining, "Compiler found unclassified bindings")
        candidate = OUT / ".local/game-content.candidate.json"
        if candidate.exists():
            check(typed["candidate_sha256"] == sha(candidate), "Typecheck report describes a different candidate")
        result["strict_typecheck_report_sha256"] = sha(typecheck_path)
    if args.self_test:
        result["invalid_mutations_rejected"] = mutation_tests(document, original, inventory, sources, oracles, values)
    if args.report:
        files = ["resolutions.json", "sources.json", "oracles.json", "death-values.json", "profile-resolutions.json", "inputs/guide-prices.json.gz"]
        report = {
            "schema_version": 1, "validated_at": datetime.now(timezone.utc).isoformat(),
            "command": "python3 tools/runtime-bindings/validate.py"
                       + (" --sources" if args.sources else "") + (" --self-test" if args.self_test else "") + " --report",
            "input_sha256": {name: sha(OUT / name) for name in files},
            "tool_sha256": {name: sha(ROOT / "tools/runtime-bindings" / name)
                           for name in ["build.py", "sources.py", "apply.py", "validate.py", "typecheck/src/main.rs", "typecheck/Cargo.toml", "typecheck/Cargo.lock"]},
            "result": result,
        }
        (OUT / "validation-result.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
