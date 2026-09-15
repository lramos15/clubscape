"""Source-owned content-4 gameplay UI bindings; no outcome inference from client state."""
from copy import deepcopy, copy

from common import ROOT, BINDINGS, ASSET_PREFIX, load, bound, canonical, sha, source_record, unique_sources, always, tutorial_at

CONTAINER_ITEMS = {"item.vial": 229, "item.vial.noted": 230, "item.beer_glass": 1919, "item.beer_glass.noted": 1920}
UI_INTERFACES = {"interface.level_up", "interface.read_book", "interface.newcomer_map", "interface.grouping"}


def legacy_content(content):
    """Remove only this versioned extension so the retained v3 behavior fingerprint remains a gate."""
    result = deepcopy(content)
    if result.get("ui") is None:
        return result
    result.pop("ui")
    result["schema_version"] = 3
    for item in CONTAINER_ITEMS:
        del result["items"][item]
    for interface in UI_INTERFACES:
        del result["interfaces"][interface]
    provider = result["mechanics"]["value_providers"]["value_provider.osrs.death"]
    for item in CONTAINER_ITEMS:
        del provider["values"]["value"][item]
    provider["values"]["source"] = [record for record in provider["values"]["source"]
                                    if record["reference"] != "research/m1-bindings/ui-item-definitions.json.gz"]
    provider["revision"] = provider["revision"].split(".ui4.", 1)[0]
    return result


def apply_ui(inputs, content, bindings):
    # Apply only after the audited legacy source substitutions/geometry fingerprints.
    from definitions import build_items
    additions = copy(inputs)
    additions.item_ids = {229: "item.vial", 1919: "item.beer_glass"}
    extra_items, extra_bindings, _ = build_items(additions)
    for identifier in extra_items:
        if identifier in content["items"]:
            raise ValueError(f"UI replacement identity already exists: {identifier}")
        # The reference is known even before publication; a missing image/model is not a null icon.
        extra_items[identifier]["asset"] = f"{ASSET_PREFIX}item.{extra_items[identifier]['source_id']}"
    content["items"].update(extra_items)
    bindings["items"].update(extra_bindings)
    prices = {row["id"]: row["price"] for row in load(ROOT / "research/runtime-bindings/inputs/guide-prices.json.gz")}
    provider = content["mechanics"]["value_providers"]["value_provider.osrs.death"]
    value_rows = []
    for identifier, item in sorted(extra_items.items()):
        normal = content["items"][item["unnoted_variant"] or identifier]
        guide = prices.get(normal["source_id"])
        if not normal["tradable"] or guide is None:
            raise ValueError(f"Required empty-container guide/tradability evidence is missing: {identifier}")
        alchemy = normal["base_value"] * 3 // 5
        value = max(guide, alchemy)
        provider["values"]["value"][identifier] = value
        value_rows.append({
            "item": identifier, "canonical_item": normal["id"], "source_item_id": normal["source_id"],
            "guide_price": guide, "high_alchemy": alchemy, "effective_death_value": value,
            "method": "maximum_guide_and_high_alchemy",
            "policy": "research/runtime-bindings/death-values.json",
        })
    provider["revision"] += ".ui4." + sha(canonical(value_rows))[:16]
    provider["values"]["source"] = unique_sources(provider["values"]["source"] + [source_record(
        "research/m1-bindings/ui-item-definitions.json.gz",
        "Original229/230/1919/1920 extend the same pinned guide/high-alchemy policy, with notes resolving to base definitions. "
        "Vial guide3/HA1 ->3; beer glass guide22/HA1 ->22. No shop-price or wikiPrice substitution.",
        "inference", provider["revision"])])
    facts = load(ROOT / "research/interface-contracts/sources.json")
    native = load(ROOT / "research/interface-contracts/native-ui.json")
    sources = facts["sources"]

    def source(page, notes, status="verified_reference"):
        row = sources[page]
        return [source_record(row["url"], notes, status, str(row["revision"]))]

    native_source = [source_record("research/interface-contracts/native-ui.json",
        "Pinned original interface/style metadata and approved semantic-state references; no invented varp281 state.",
        "verified_reference", native["sha256"])]
    def interface(identifier, symbol, contextual=True):
        group = inputs.code_sources["interface_group_ids"][symbol]
        content["interfaces"][identifier] = {
            "id": identifier, "name": symbol.replace("_", " ").title(),
            "access": "contextual" if contextual else "tab", "source_ids": [group],
            "source": native_source,
        }
    interface("interface.level_up", "LEVELUP_DISPLAY")
    interface("interface.read_book", "BOOK")
    interface("interface.newcomer_map", "AIDE_MAP")
    interface("interface.grouping", "GROUPING", False)
    bank_source = source("Bank", "Source native bank organization and non-spendable placeholders.")
    potion_source = source("Energy_potion", "Source15% run energy per dose, preserving original1-4 dose identities.")
    potion_source += source("Potions", "Separate three-tick drink timer; ordinary potion consumption does not use food attack delay.")
    mainland = tutorial_at("stage.tutorial.mainland")
    item_actions = {}
    def action(item, name, operation, source_rows, guard=None):
        if item not in content["items"]:
            raise ValueError(f"Required UI source item missing: {item}")
        item_actions.setdefault(item, []).append({
            "id": name.lower(), "label": name, "guard": deepcopy(guard or mainland),
            "action": operation, "source": unique_sources(source_rows),
        })
    doses = ["one_dose", "two_dose", "three_dose", "four_dose"]
    for index, name in enumerate(doses):
        identifier = f"item.energy_potion.{name}"
        replacement = "item.vial" if index == 0 else f"item.energy_potion.{doses[index-1]}"
        action(identifier, "Drink", {
            "kind": "drink", "replacement": replacement, "cooldown": "action.ui.potion_drink",
            "delay_ticks": bound(facts["drink_cooldown_ticks"], potion_source),
            "restore": {"run_energy": {"kind": "amount", "amount": facts["energy_restore_units"]}}, "skills": [],
        }, potion_source)
        action(identifier, "Empty", {"kind": "empty", "replacement": "item.vial"}, potion_source)
    beer_source = source("Beer", "Source beer effects; selected glass only. Base Strength boost and current Attack drain are qualified source formula bindings.")
    beer_source += source("Potions", "A separate drink admission group, not the food/attack timer.", "inference")
    action("item.beer", "Drink", {
        "kind": "drink", "replacement": "item.beer_glass", "cooldown": "action.ui.potion_drink",
        "delay_ticks": bound(3, beer_source), "restore": {"hitpoints": {"kind": "amount", "amount": 1}},
        "skills": [
            {"skill": "skill.strength", "basis": "base", "percent": {"numerator": 2, "denominator": 100}, "additive": 1, "drain": False},
            {"skill": "skill.attack", "basis": "current", "percent": {"numerator": 6, "denominator": 100}, "additive": 1, "drain": True},
        ],
    }, beer_source)
    for item, recipe in [("item.bones", "recipe.prayer.bones.ordinary"), ("item.bones.tutorial", "recipe.prayer.bones.tutorial")]:
        definition = content["recipes"][recipe]
        action(item, "Bury", {"kind": "consume_recipe", "recipe": recipe}, definition["source"], definition["mechanics"]["guard"])
    for item, replacement in [("item.water.bucket", "item.bucket"), ("item.milk.bucket", "item.bucket"), ("item.flour.pot", "item.pot")]:
        action(item, "Empty", {"kind": "empty", "replacement": replacement},
               inputs.definition_source("item", content["items"][item]["source_id"]))
    action("item.security_book", "Read", {
        "kind": "read", "interface": "interface.read_book", "title": "Account Security",
        "pages": facts["security_book_pages"], "map_asset": None, "native_map": False,
    }, source("Transcript:Security_book", "Original source book transcript; attribution retained in interface-contracts/sources.json."))
    action("item.newcomer_map", "Read", {
        "kind": "read", "interface": "interface.newcomer_map", "title": "Newcomer map",
        "pages": [], "map_asset": None, "native_map": True,
    }, source("Newcomer_map", "Original map/tutor-toggle behavior combined with pinned AIDE_MAP native children; layout linkage is qualified inference.", "inference") + native_source)
    for identifier in ("item.goblin_book", "item.goblin_champion_scroll", "item.milk.bottomless_bucket"):
        if identifier in content["items"]:
            action(identifier, "Read" if "book" in identifier or "scroll" in identifier else "Empty",
                {"kind": "unavailable", "reason": "This retained members/rare alternative is outside the ordinary normal-F2P M1 route."},
                inputs.definition_source("item", content["items"][identifier]["source_id"]))
    rewards = {
        "quest.cooks_assistant": {
            "entitlement": "entitlement.quest.cooks_assistant", "interface": "interface.quest_reward",
            "title": content["quests"]["quest.cooks_assistant"]["name"],
            "lines": ["1 Quest Point", "300 Cooking XP", "Access to the Cook's range"],
            "items": [], "xp": [{"skill": "skill.cooking", "amount_tenths": 3000}], "quest_points": 1,
            "source": inputs.basis(next(row for row in inputs.rules["cooks-assistant"]["rewards"]
                                       if row["id"] == "reward.cooks_assistant")["basis"]),
        },
        "quest.learning_the_ropes": {
            "entitlement": "entitlement.quest.learning_the_ropes", "interface": "interface.quest_reward",
            "title": content["quests"]["quest.learning_the_ropes"]["name"],
            "lines": ["1 Quest Point"], "items": [], "xp": [], "quest_points": 1,
            "source": inputs.basis(next(row for row in inputs.rules["cooks-assistant"]["rewards"]
                                       if row["id"] == "reward.learning_the_ropes")["basis"]),
        },
    }
    state_bindings = {row["state_id"]: row for row in native["tutorial_states"]}
    rules, overlays = {}, {}
    for stage in content["tutorial"]:
        declared = set(state_bindings[stage]["declared_controls"])
        rules[stage] = []
        overlay = {"stage.tutorial.appearance": "interface.appearance", "stage.tutorial.experience": "interface.experience"}.get(stage)
        overlays[stage] = overlay
        for identifier, definition in sorted(content["interfaces"].items()):
            contextual = definition["access"] == "contextual"
            unavailable = "Grouping/clan systems are outside the approved M1 gameplay scope." if identifier == "interface.grouping" else None
            rules[stage].append({
                "interface": identifier, "visibility": "enabled" if not contextual or identifier == overlay else "hidden",
                "highlighted": ("hud.classic.source_highlighted" in state_bindings[stage]["visual_variant_ids"]
                                and identifier.replace("interface.", "ui.", 1) in declared),
                "guard": always(), "unavailable_reason": unavailable,
                "source": native_source + inputs.basis(next(row for row in inputs.rules["tutorial"]["states"] if row["id"] == stage)["basis"]),
            })
    production = {}
    direct_production = {"recipe.cooks.milk"}
    for recipe in content["recipes"].values():
        if recipe["id"] in direct_production:
            continue
        method = recipe.get("mechanics", {}).get("method", "")
        production[recipe["id"]] = "interface.smithing" if method.startswith("action.smithing") else "interface.cooking"
    # Native category rows preserve source option order, not a label guessed from a combat bonus.
    categories = {row["columnValues"][0][0]: row["columnValues"][1] for row in native["combat_categories"]}
    names = {}
    for item, definition in content["items"].items():
        equipment = definition.get("equipment")
        if not equipment or not equipment.get("weapon"):
            continue
        source_id = str(definition["source_id"])
        category = native["weapon_categories"].get(source_id)
        if category is None:
            raise ValueError(f"No original UI combat category for {item}")
        row = categories[category]
        labels = [row[index + 1] for index in range(0, len(row), 4)]
        styles = equipment["weapon"]["styles"]
        if len(labels) != len(styles):
            raise ValueError(f"Source style/UI arity mismatch for {item}")
        names[item] = dict(zip(styles, labels))
    unarmed = content["mechanics"]["player_combat"]["unarmed"]["value"]["styles"]
    row = categories[0]
    unarmed_names = dict(zip(unarmed, [row[index + 1] for index in range(0, len(row), 4)]))
    abilities = {}
    for name in content["mechanics"]["prayers"]:
        abilities[name] = name.removeprefix("prayer.").replace("_", " ").title()
    for name in content["mechanics"]["spells"]:
        abilities[name] = {"spell.wind_strike": "Wind Strike", "spell.lumbridge_home_teleport": "Lumbridge Home Teleport"}.get(name, name.removeprefix("spell.").replace("_", " ").title())
    exchange = {id: prices[item["source_id"]] for id, item in content["items"].items() if item["source_id"] in prices}
    # No ordinary M1 item clears the source individual-price threshold; retained rare alternatives are not enabled.
    eligible = [id for id, price in exchange.items() if price >= facts["coffer"]["minimum_exchange_value"]
                and content["items"][id]["tradable"] and not id.startswith(("item.milk.bottomless_bucket", "item.ensouled_goblin_head"))]
    if eligible:
        raise ValueError(f"New coffer candidates require source eligibility qualification, not a guessed allowance: {eligible}")
    coffer_source = source("Death%27s_Coffer", "Source individual GE threshold,105% credit and2147483647 cap. No coin deposit and no invented sacrifice eligibility.")
    ui_source = native_source + source("Tutorial_Island", "Current normal Tutorial Island keeps public chat restricted until mainland.")
    content["ui"] = {
        "version": 1, "production_interfaces": production, "direct_production": sorted(direct_production), "quest_rewards": rewards,
        "level_up": {"interface": "interface.level_up", "title": "Congratulations, you have just advanced a {skill} level!",
                     "line": "Your {skill} level is now {level}.", "source": native_source},
        "stage_interfaces": rules, "stage_overlays": overlays,
        "equipment_stats_interface": "interface.equipment_stats", "death_preview_interface": "interface.items_kept_on_death",
        "ability_names": abilities, "weapon_style_names": names, "unarmed_style_names": unarmed_names,
        "item_actions": item_actions,
        "bank": {"maximum_tabs": facts["bank_maximum_extra_tabs"], "initial_insert": False, "initial_placeholders": False,
                 "unavailable_containers": [
                     {"id": identifier, "label": label,
                      "reason": "This source container is absent from the declared normal-F2P M1 item/feature universe.",
                      "source": bank_source}
                     for identifier, label in [("rune_pouch", "Rune pouch"), ("looting_bag", "Looting bag"), ("potion_storage", "Potion storage")]
                 ],
                 "source": bank_source + [source_record("research/interface-contracts/sources.json", "Explicit native bank preference migration defaults; not observed per-account settings.", "inference", "ui-contract-v1")]},
        "coffer": bound({"eligible_items": [], "exchange_values": exchange, "minimum_value": facts["coffer"]["minimum_exchange_value"],
                         "credit": {"numerator": 105, "denominator": 100}, "maximum_balance": 2147483647, "source": coffer_source}, coffer_source),
        "chat": {"guard": mainland, "maximum_bytes": 80, "radius": 15, "messages_per_window": 5, "window_ticks": 8,
                 "source": ui_source + [source_record("research/interface-contracts/sources.json",
                    "Bounded normal public CP1252 transport.15-square player-info locality is qualified protocol-source inference; admission window is infrastructure, not an action cooldown.", "inference", "ui-contract-v1")]},
        "appearance_base": bound({"asset": "asset.source.osrs.cache2695.npc.2063", "source_npc": 2063,
                                  "adaptation": "milestones/approvals/m1-reference-pack-v1.3.0.json"},
                                 [source_record("milestones/approvals/m1-reference-pack-v1.3.0.json",
                                  "Owner-approved original penguin base2063/model21547 at75/128; no new kits/colours.", "approved_adaptation", "1.3.0")]),
        "source": ui_source,
    }
    content["schema_version"] = 4
    bindings["ui"] = {"schema_version": 1, "capability": "game.ui.v1", "source_facts": "research/interface-contracts/sources.json",
                      "semantic_states": len(rules), "introduced_containers": ["item.vial", "item.beer_glass"],
                      "death_value_extension": {"revision": provider["revision"], "rows": value_rows},
                      "scope": "Required consumption replacements only; no new acquisition family, source geometry or reward change."}
    from runtime_application import classify_residuals, RESOLUTIONS
    bindings["application"]["residuals"] = classify_residuals(content, load(RESOLUTIONS))
    return content
