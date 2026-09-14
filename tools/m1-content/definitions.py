"""GameContent definitions from real cache records and the pinned behavior contracts."""

from decimal import Decimal, localcontext

from common import (
    ASSET_PREFIX, BINDINGS, load, source_record,
    stack, unique_sources,
)


SKILL_ORDER = (
    "attack", "defence", "strength", "hitpoints", "ranged", "prayer", "magic",
    "cooking", "woodcutting", "fletching", "fishing", "firemaking", "crafting",
    "smithing", "mining", "herblore", "agility", "thieving", "slayer", "farming",
    "runecraft", "hunter", "construction", "sailing",
)
WEAR_SLOTS = {0: "head", 1: "cape", 2: "neck", 3: "weapon", 4: "body", 5: "shield",
              7: "legs", 9: "hands", 10: "feet", 12: "ring", 13: "ammo"}
STYLE_NAMES = {
    "item.pickaxe.bronze": ["melee.stab.accurate", "melee.stab.aggressive", "melee.crush.aggressive", "melee.stab.defensive"],
    "item.axe.bronze": ["melee.slash.accurate", "melee.slash.aggressive", "melee.crush.aggressive", "melee.slash.defensive"],
    "item.dagger.bronze": ["melee.stab.accurate", "melee.stab.aggressive", "melee.slash.aggressive", "melee.stab.defensive"],
    "item.sword.bronze": ["melee.stab.accurate", "melee.stab.aggressive", "melee.slash.aggressive", "melee.stab.defensive"],
    "item.shortbow": ["ranged.accurate", "ranged.rapid", "ranged.longrange"],
    "item.spear.bronze": ["melee.stab.controlled", "melee.slash.controlled", "melee.crush.controlled", "melee.stab.defensive"],
}
INTERFACES = {
    "appearance": ("PLAYER_DESIGN",),
    "experience": ("TUTORIAL_PLAYER_EXPERIENCE",),
    "settings": ("SETTINGS_SIDE", "SETTINGS"),
    "inventory": ("INVENTORY",),
    "skills": ("STATS",),
    "quests": ("QUESTLIST", "QUESTJOURNAL"),
    "equipment": ("WORNITEMS",),
    "equipment_stats": ("EQUIPMENT", "EQUIPMENT_SIDE"),
    "combat": ("COMBAT_INTERFACE",),
    "bank": ("BANKMAIN", "BANKSIDE"),
    "poll": ("BALLOT",),
    "account": ("ACCOUNT_SUMMARY_SIDEPANEL",),
    "logout": ("LOGOUT",),
    "account_links": ("ACCOUNT",),
    "prayer": ("PRAYERBOOK",),
    "magic": ("MAGIC_SPELLBOOK",),
    "smithing": ("SMITHING",),
    "cooking": ("SKILLMULTI",),
    "shop": ("SHOPMAIN", "SHOPSIDE"),
    "quest_reward": ("QUESTSCROLL",),
    "items_kept_on_death": ("DEATHKEEP",),
    "grave": ("GRAVESTONE_RETRIEVAL",),
    "death_retrieval": ("DEATH_OFFICE",),
    "music": ("MUSIC",),
    "emotes": ("EMOTE",),
    "adventure_paths": ("ADVENTUREPATH", "ADVENTUREPATH_SIDE", "ADVENTUREPATH_REWARD"),
}


def bonuses(params):
    order = ("stab", "slash", "crush", "magic", "ranged")
    return {
        "attack": {name: params.get(str(index), 0) for index, name in enumerate(order)},
        "defence": {name: params.get(str(index + 5), 0) for index, name in enumerate(order)},
        "melee_strength": params.get("10", 0),
        "ranged_strength": params.get("12", 0),
        "magic_damage_percent": 0,
        "prayer": params.get("11", 0),
    }


def build_items(inputs):
    result, bindings, excluded = {}, {}, {}
    source_items = {entry["id"]: entry for entry in inputs.rules["activities"]["items"]}
    ids = dict(inputs.item_ids)
    for number, identifier in list(ids.items()):
        definition = inputs.collections["item"][number]
        if definition["notedID"] >= 0:
            other = inputs.collections["item"][definition["notedID"]]
            if other["notedTemplate"] >= 0:
                ids[other["id"]] = identifier + ".noted"
    for number, identifier in sorted(ids.items()):
        raw = inputs.collections["item"][number]
        if raw["stackable"] not in (0, 1):
            excluded[identifier] = {
                "source_id": number, "gap": "conditional_stackability",
                "source_stackable_mode": raw["stackable"],
                "reason": "A source stackability mode other than 0/1 is not coerced to a boolean.",
            }
            continue
        noted = raw["notedTemplate"] >= 0
        base = inputs.collections["item"][raw["notedID"]] if noted else raw
        name = base["name"]
        params = base["params"] or {}
        equip = None
        if not noted and identifier != "item.goblin_mail" and any(
                operation in ("Wear", "Wield") for operation in base["interfaceOptions"]):
            slots = [f"slot.{WEAR_SLOTS[pos]}" for pos in
                     (base["wearPos1"], base["wearPos2"], base["wearPos3"]) if pos in WEAR_SLOTS]
            if not slots:
                raise ValueError(f"Equipable source item has no bound equipment position: {number}")
            speed = params.get("14") if slots[0] == "slot.weapon" else None
            requirements = []
            if slots[0] == "slot.weapon":
                requirements = [{"skill": "skill.ranged" if identifier == "item.shortbow" else "skill.attack", "level": 1}]
            elif identifier in ("item.shield.bronze_square", "item.shield.wooden"):
                requirements = [{"skill": "skill.defence", "level": 1}]
            equip = {
                "slot": slots[0], "occupied_slots": list(dict.fromkeys(slots)),
                "requirements": requirements, "bonuses": bonuses(params),
                "attack_speed_ticks": speed, "attack_styles": STYLE_NAMES.get(identifier, []) if speed else [],
            }
        reference = source_items.get(identifier, {})
        sources = unique_sources(inputs.definition_source("item", number) +
                                 inputs.basis(reference.get("basis")))
        if equip and identifier in STYLE_NAMES:
            sources.append(source_record(
                "research/journey-rules/activities.json#rule.equipment.equip",
                "Source combat-style semantic labels; per-style cadence/XP/ammunition remain in behavior bindings.",
                "inference", "source-contract-v1"))
        note = base["notedID"] if not noted else -1
        definition = {
            "id": identifier, "name": name + (" (noted)" if noted else ""),
            "source_id": number, "stackable": noted or raw["stackable"] == 1,
            "tradable": bool(base["tradeable"]), "base_value": base["cost"],
            "equipment": equip,
            "noted_variant": ids[note] if note in ids and not noted else None,
            "unnoted_variant": ids[raw["notedID"]] if noted else None,
            "healing": {"item.shrimps.cooked": 3, "item.bread": 5}.get(identifier),
            "asset": inputs.asset("item", number), "source": unique_sources(sources),
        }
        result[identifier] = definition
        models = sorted({base[field] for field in (
            "inventoryModel", "maleModel0", "maleModel1", "maleModel2", "femaleModel0",
            "femaleModel1", "femaleModel2", "maleHeadModel", "maleHeadModel2",
            "femaleHeadModel", "femaleHeadModel2") if base[field] >= 0})
        bindings[identifier] = {
            "source_id": number, "source_asset": f"{ASSET_PREFIX}item.{number}",
            "asset_available": bool(definition["asset"]), "models": models,
            "missing_model_ids": [model for model in models if not inputs.asset("model", model)],
            "native_weight_grams": base["weight"], "source_members": base["members"],
            "source_options": base["interfaceOptions"],
            "note_template": raw["notedTemplate"], "source_params": params,
            "interpretation": "Notes use the source template/base composition. Item models, recolors, "
                              "retextures, icon framing and wear attachments remain in the original definition.",
        }
        if identifier == "item.goblin_mail":
            bindings[identifier]["equipment_disposition"] = "Source wear-position metadata is not permission for a normal player to wear goblin mail."
    return result, bindings, excluded


def build_skills(inputs):
    rule = next(rule for rule in inputs.rules["activities"]["rules"] if rule["id"] == "rule.xp.level")
    thresholds = xp_thresholds()
    for level, expected in rule["thresholds_tenths"].items():
        if thresholds[int(level) - 1] != expected:
            raise ValueError(f"Source XP formula disagrees with pinned level {level}")
    return {
        f"skill.{name}": {
            "id": f"skill.{name}", "name": name.capitalize(), "source_id": index,
            "xp_thresholds_tenths": thresholds, "maximum_xp_tenths": rule["maximum_xp_tenths"],
            "source": inputs.basis(rule["basis"]) + [source_record(
                "research/m1-bindings/selection.json#baseline",
                "Normal current 24-skill ordering, including Sailing; this is not 24 full training methods.",
                "verified_reference", "240/cache2695")],
        } for index, name in enumerate(SKILL_ORDER)
    }


def xp_thresholds():
    points, result = 0, [0]
    with localcontext() as context:
        context.prec = 70
        for level in range(1, 99):
            points += int(Decimal(level) + 300 * (Decimal(2) ** (Decimal(level) / 7)))
            result.append((points // 4) * 10)
    return result


def build_objects(inputs):
    return {
        inputs.object_id(number): {
            "id": inputs.object_id(number),
            "name": raw["name"].strip() if raw["name"].strip() not in ("", "null") else f"Unnamed source object {number}",
            "source_id": number, "size_x": raw["sizeX"], "size_y": raw["sizeY"],
            "asset": inputs.asset("object", number), "source": inputs.definition_source("object", number),
        } for number, raw in sorted(inputs.collections["object"].items())
    }


def build_npcs(inputs):
    npcs, bindings = {}, {}
    for identifier, number in sorted(inputs.selection["npcs"].items()):
        raw = inputs.collections["npc"][number]
        source = inputs.definition_source("npc", number)
        combat = None
        if identifier in ("npc.tutorial_rat", "npc.tutorial_chicken", "npc.goblin.level_2", "npc.chicken"):
            source.append(source_record(
                "research/m1-bindings/contract-gaps.json#npc_combat",
                "Original stats/bonuses are bound separately. Combat remains unarmed in this projection: "
                "unverified tutorial respawn timing and weighted/conditional loot cannot be replaced with "
                "arbitrary timers, independent primary rolls or bones-only live combat.",
                "inference", "m1-bindings-v1"))
        npcs[identifier] = {
            "id": identifier, "name": raw["name"], "source_id": number, "size": raw["size"],
            "combat": combat, "asset": inputs.asset("npc", number), "source": source,
        }
        models = (raw["models"] or []) + (raw["chatheadModels"] or [])
        animations = {key: value for key, value in raw.items() if "Animation" in key and isinstance(value, int) and value >= 0}
        bindings[identifier] = {
            "source_id": number, "source_asset": f"{ASSET_PREFIX}npc.{number}",
            "asset_available": bool(inputs.asset("npc", number)), "models": models,
            "missing_model_ids": sorted({model for model in models if not inputs.asset("model", model)}),
            "animations": animations, "source_stats_order": ["attack", "defence", "strength", "hitpoints", "ranged", "magic"],
            "source_stats": raw["stats"], "source_combat_level": raw["combatLevel"],
            "source_params": raw["params"], "width_scale": raw["widthScale"],
            "height_scale": raw["heightScale"],
            "combat_binding_status": "blocked_npc_combat" if identifier in (
                "npc.tutorial_rat", "npc.tutorial_chicken", "npc.goblin.level_2", "npc.chicken") else "noncombatant",
        }
    return npcs, bindings


def build_interfaces(inputs):
    values, bindings = {}, {}
    for name, symbols in INTERFACES.items():
        identifier = f"interface.{name}"
        groups = [inputs.code_sources["interface_group_ids"][symbol] for symbol in symbols]
        source = [source_record(
            "research/m1-bindings/code-sources.json#interfaces",
            "Exact pinned RuneLite interface-group symbols. Logical binding only; no rendered widget/control acceptance.",
            "verified_reference" if groups else "inference", "ac79ed8bd8926bec7bf172aa291574b4d944b0e7")]
        if not groups:
            source.append(source_record(
                "research/journey-rules/tutorial.json#bank_seed_hook",
                "Poll-booth lesson widget group remains unbound; an empty source-ID list is not a claimed screen.",
                "inference", "source-contract-v1"))
        values[identifier] = {"id": identifier, "name": name.replace("_", " ").title(), "source_ids": groups, "source": source}
        bindings[identifier] = {
            "symbols": list(symbols), "source_groups": groups,
            "assets": [inputs.asset("interface", number) for number in groups if inputs.asset("interface", number)],
            "missing_source_groups": [number for number in groups if not inputs.asset("interface", number)],
            "presentation_status": "not_approved",
        }
    return values, bindings


def recipe(rule, identifier, name, objects, ticks, inputs):
    return {
        "id": identifier, "name": name,
        "inputs": [stack(item) for item in rule["consumed"]],
        "outputs": [stack(item) for item in rule["produced"]], "failed_outputs": [],
        "tools": [tool["item_ref"] for tool in rule.get("tools", [])],
        "requirements": [{"skill": rule["skill_ref"], "level": rule["required_level"]}] if "skill_ref" in rule else [],
        "xp": [{"skill": reward["skill_ref"], "amount_tenths": reward["xp_tenths"]} for reward in rule["xp_awards"]],
        "ticks": ticks,
        "success": {"numerator_at_level_1": 1, "numerator_at_level_99": 1, "denominator": 1},
        "target_objects": objects,
        "source": unique_sources(inputs.basis(rule["basis"]) + [source_record(
            "research/journey-rules/" + ("cooks-assistant.json" if "cooks." in rule["id"] else "activities.json") + "#" + rule["id"],
            "Deterministic source conversion, not a manufactured success roll. This duration is the named single-action mode; "
            "mode-specific Make-X timing and direct Produce authorization remain integration gates.",
            "inference", "source-contract-v1")]),
    }


def build_recipes(inputs):
    rules = {rule["id"]: rule for file in ("activities", "cooks-assistant") for rule in inputs.rules[file]["rules"]}
    recipes = {}
    for rule_id, identifier, name, objects, ticks in (
        ("rule.cooking.dough", "recipe.cooking.dough", "Make bread dough", [], 1),
        ("rule.smelting.bronze", "recipe.smelting.bronze", "Smelt a bronze bar", ["object.furnace.tutorial"], 6),
        ("rule.smithing.bronze_dagger", "recipe.smithing.bronze_dagger", "Smith a bronze dagger", ["object.anvil.tutorial"], 5),
        ("rule.cooks.milk", "recipe.cooks.milk", "Milk a dairy cow", ["object.dairy_cow", "object.dairy_cow.east"], 3),
        ("rule.cooks.milk", "recipe.cooks.milk.use_bucket", "Use a bucket on a dairy cow", ["object.dairy_cow", "object.dairy_cow.east"], 4),
    ):
        recipes[identifier] = recipe(rules[rule_id], identifier, name, objects, ticks, inputs)
    return recipes


def build_shop(inputs, items):
    facts = load(BINDINGS / "wiki-facts.json")
    by_name = {definition["name"]: definition for definition in items.values() if not definition["unnoted_variant"]}
    stock = []
    for line in facts["shop_stock"]:
        definition = by_name[line["name"]]
        value = definition["base_value"]
        stock.append({
            "item": definition["id"], "base_stock": line["stock"], "restock_ticks": line["restock_ticks"],
            "buy_price": max(1, value * 1300 // 1000), "sell_price": value * 400 // 1000,
        })
    return {
        "shop.lumbridge.general_store": {
            "id": "shop.lumbridge.general_store", "name": "Lumbridge General Store", "currency": "item.coins",
            "stock": stock, "accepts_general_items": True,
            "source": [
                inputs.wiki("Lumbridge General Store", "All 15 actual stock lines and source restock ticks; prices below are at base stock."),
                source_record("research/journey-rules/activities.json#formula.shop.buy",
                              "Shared ShopDefinition has fixed prices, so this base-stock projection is not enabled "
                              "for live trading until stock-sensitive buy/sell formulas are represented.",
                              "inference", "source-contract-v1"),
            ],
        }
    }


def equipment_slots(inputs):
    return [record["id"] for record in inputs.rules["vocabulary"]["slots"]]
