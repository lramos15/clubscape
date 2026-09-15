"""Exact, source-scoped UI extension audit; does not waive asset publication or presentation gates."""
from common import CONTENT, BINDINGS, ROOT, canonical, load, sha, write
from ui4 import CONTAINER_ITEMS, legacy_content
from verify_assets import behavior_projection


def require(value, message):
    if not value:
        raise ValueError(message)


def verify_ui4(content):
    application = load(BINDINGS / "application-result.json")
    require(content["schema_version"] == 4 and content["ui"]["version"] == 1, "UI content/version mismatch")
    legacy = legacy_content(content)
    require(sha(canonical(behavior_projection(legacy))) == application["runtime3"]["content_behavior_sha256"],
            "UI4 changed an existing v3 gameplay, geometry or progression checkpoint")
    prices = {row["id"]: row["price"] for row in load(ROOT / "research/runtime-bindings/inputs/guide-prices.json.gz")}
    provider = content["mechanics"]["value_providers"]["value_provider.osrs.death"]
    rows = load(BINDINGS / "ui-bindings.json")["death_value_extension"]["rows"]
    require({row["item"] for row in rows} == set(CONTAINER_ITEMS), "Unexpected source container valuation extension")
    require(provider["revision"] == legacy["mechanics"]["value_providers"]["value_provider.osrs.death"]["revision"]
            + ".ui4." + sha(canonical(rows))[:16], "UI4 valuation revision is not its actual table identity")
    for row in rows:
        item = content["items"][row["item"]]
        base = content["items"][item["unnoted_variant"] or item["id"]]
        require(item["source_id"] == CONTAINER_ITEMS[item["id"]], "Container source identity was substituted")
        require(item["asset"] == f"asset.source.osrs.cache2695.item.{item['source_id']}", "Required original asset became null or a substitute")
        require(row["source_item_id"] == base["source_id"] and row["guide_price"] == prices[base["source_id"]]
                and row["high_alchemy"] == base["base_value"] * 3 // 5, "Container price is not the pinned guide/high-alchemy policy")
        require(row["effective_death_value"] == max(row["guide_price"], row["high_alchemy"])
                == provider["values"]["value"][item["id"]], "Container death valuation drift")
    ui = content["ui"]
    require(ui["quest_rewards"]["quest.cooks_assistant"]["quest_points"] == 1
            and ui["quest_rewards"]["quest.cooks_assistant"]["xp"] == [{"skill": "skill.cooking", "amount_tenths": 3000}]
            and ui["quest_rewards"]["quest.cooks_assistant"]["items"] == [], "Cook reward display minted coins or changed the actual reward")
    require(ui["quest_rewards"]["quest.learning_the_ropes"]["quest_points"] == 1
            and ui["quest_rewards"]["quest.learning_the_ropes"]["xp"] == []
            and ui["quest_rewards"]["quest.learning_the_ropes"]["items"] == [], "Learning the Ropes reward changed")
    facts = load(ROOT / "research/interface-contracts/sources.json")
    doses = ["one_dose", "two_dose", "three_dose", "four_dose"]
    for index, dose in enumerate(doses):
        drink = next(action["action"] for action in ui["item_actions"][f"item.energy_potion.{dose}"] if action["id"] == "drink")
        require(drink["replacement"] == ("item.vial" if index == 0 else f"item.energy_potion.{doses[index-1]}"),
                "Drink does not use the exact original next-dose identity")
        require(drink["restore"] == {"run_energy": {"kind": "amount", "amount": facts["energy_restore_units"]}}
                and drink["delay_ticks"]["value"] == facts["drink_cooldown_ticks"], "Drink changed source restoration or independent phase")
    require(ui["coffer"]["value"]["eligible_items"] == [], "An ordinary M1 coffer sacrifice was invented")
    require(ui["direct_production"] == ["recipe.cooks.milk"], "One-click source milking was replaced by a dummy menu")
    report = {
        "schema_version": 1, "ui_version": 1, "source_scope_passed": True,
        "retained_v3_behavior_sha256": application["runtime3"]["content_behavior_sha256"],
        "ui_sha256": sha(canonical(ui)), "container_values": rows,
        "tutorial_states": len(content["tutorial"]), "cooks_states": len(content["quests"]["quest.cooks_assistant"]["journal"]),
        "asset_publication_claimed": False, "gameplay_acceptance": False, "presentation_acceptance": False,
    }
    write(BINDINGS / "ui-validation.json", report, pretty=True)
    return report


if __name__ == "__main__":
    verify_ui4(load(CONTENT / "game-content.json.gz"))
