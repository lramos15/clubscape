"""GameContent definitions from real cache records and the pinned behavior contracts."""

from decimal import Decimal, localcontext

from common import (
    ASSET_PREFIX, BINDINGS, all_of, bound, constant_chance, load, requirement,
    skill_chance, source_record, stack, unique_sources, unresolved,
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
        if raw["stackable"] not in (0, 1, 2):
            raise ValueError(f"Unsupported source stack mode for {identifier}: {raw['stackable']}")
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
                requirements = [requirement("skill.ranged" if identifier == "item.shortbow" else "skill.attack", basis="base")]
            elif identifier in ("item.shield.bronze_square", "item.shield.wooden"):
                requirements = [requirement("skill.defence", basis="base")]
            from mechanics import weapon
            equip = {
                "slot": slots[0], "occupied_slots": list(dict.fromkeys(slots)),
                "requirements": requirements, "bonuses": bonuses(params),
                "attack_speed_ticks": None, "attack_styles": [],
                "weapon": weapon(inputs, identifier) if speed else None,
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
            "source_id": number,
            "stackable": ({"source_mode": 2, "rule": unresolved(
                "Source opcode160/mode2 container-specific stacking/origin rules remain unbound. "
                "The source identity is retained; no ordinary M1 grant or acquisition is invented.",
                sources)} if raw["stackable"] == 2 else noted or raw["stackable"] == 1),
            "tradable": bool(base["tradeable"]), "base_value": base["cost"],
            "equipment": equip,
            "noted_variant": ids[note] if note in ids and not noted else None,
            "unnoted_variant": ids[raw["notedID"]] if noted else None,
            "healing": {"item.shrimps.cooked": 3, "item.bread": 5}.get(identifier),
            "weight": bound({"grams": 0 if noted else base["weight"],
                             "inventory": "none" if noted or base["weight"] == 0 else "per_unit",
                             "equipment": "none" if noted or base["weight"] == 0 else "per_unit"}, sources),
            "charges": None,
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
        if identifier.startswith("item.milk.bottomless_bucket"):
            definition["charges"] = {
                "kind": "charge.milk", "maximum": 10000,
                "empty_variant": "item.milk.bottomless_bucket.empty", "charged_variant": "item.milk.bottomless_bucket",
                "trade_with_charges": False,
                "source": [inputs.wiki("Bottomless milk bucket",
                                      "Source empty33091/full33089 and capacity10000 milk uses. "
                                      "Ordinary M1 acquires a bucket of milk; rare acquisition is not granted or required.")],
            }
            bindings[identifier]["scope_disposition"] = (
                "Retained full-target charged-container alternative. The ordinary M1 route uses item.milk.bucket; "
                "parent must select reachable acquisition/charge metadata before enabling rare-container delivery.")
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
            "clip": {"blocks_movement": bool(raw["interactType"]) and not raw["isHollow"],
                     "blocks_projectiles": raw["blocksProjectile"] and not raw["isHollow"],
                     "access_blocked_sides": raw["blockingMask"] & 15},
            "morph": None,
            "asset": inputs.asset("object", number), "source": inputs.definition_source("object", number),
        } for number, raw in sorted(inputs.collections["object"].items())
    }


def build_npcs(inputs):
    npcs, bindings = {}, {}
    for identifier, number in sorted(inputs.selection["npcs"].items()):
        raw = inputs.collections["npc"][number]
        source = inputs.definition_source("npc", number)
        from mechanics import npc_combat
        combat = npc_combat(inputs, identifier, raw)
        npcs[identifier] = {
            "id": identifier, "name": raw["name"], "source_id": number, "size": raw["size"],
            "combat": combat,
            "navigation": {"kind": "mobile", "wander_radius": 0,
                           "step_ticks": unresolved("Source wandering radius/step cadence is not observed; "
                                                    "zero radius is the recorded anchor-only bound, not free movement.", source),
                           "clip": "movement_and_actors"},
            "morph": None, "asset": inputs.asset("npc", number), "source": source,
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
            "combat_binding_status": "typed_source_mechanics" if combat else "noncombatant_or_outside_working_combat_scope",
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
        contextual = name in ("appearance", "experience", "equipment_stats", "bank", "poll", "smithing", "cooking",
                              "shop", "quest_reward", "items_kept_on_death", "grave", "death_retrieval")
        values[identifier] = {"id": identifier, "name": name.replace("_", " ").title(),
                              "access": "contextual" if contextual else "tab", "source_ids": groups, "source": source}
        bindings[identifier] = {
            "symbols": list(symbols), "source_groups": groups,
            "assets": [inputs.asset("interface", number) for number in groups if inputs.asset("interface", number)],
            "missing_source_groups": [number for number in groups if not inputs.asset("interface", number)],
            "presentation_status": "not_approved",
        }
    return values, bindings


def recipe(rule, identifier, name, objects, inputs, timing, guard, chance=None, lifecycle=None, tools=None):
    from mechanics import cadence
    source = inputs.rule_source(rule["id"])
    return {
        "id": identifier, "name": name,
        "inputs": [stack(item) for item in rule["consumed"]],
        "outputs": [stack(item) for item in rule.get("produced", [])],
        "failed_outputs": [stack(item) for item in rule.get("failure_produced", [])],
        "tools": tools if tools is not None else [tool["item_ref"] for tool in rule.get("tools", [])],
        "requirements": [requirement(rule["skill_ref"], rule["required_level"])] if "skill_ref" in rule else [],
        "xp": [{"skill": reward["skill_ref"], "amount_tenths": reward["xp_tenths"]} for reward in rule["xp_awards"]],
        "ticks": None,
        "success": chance or constant_chance(),
        "target_objects": objects,
        "mechanics": {"method": rule["id"].replace("rule.", "action.", 1), "guard": guard,
                      "chance_skill": rule.get("skill_ref") if chance else None,
                      "cadence": cadence(source, **timing), "tool_ownership": "inventory_and_equipment",
                      "failed_xp": [], "success_effects": [], "failure_effects": [],
                      "lifecycle": lifecycle or {"kind": "inventory_conversion"}},
        "source": source,
    }


def build_recipes(inputs):
    rules = {rule["id"]: rule for file in ("activities", "cooks-assistant") for rule in inputs.rules[file]["rules"]}
    recipes = {}
    from spawns import stage_from
    for rule_id, identifier, name, objects, timing, first in (
        ("rule.cooking.dough", "recipe.cooking.dough", "Make bread dough", [], {"single": 1, "first": 1, "repeat": 1}, "stage.tutorial.make_dough"),
        ("rule.smelting.bronze", "recipe.smelting.bronze", "Smelt a bronze bar", ["object.furnace.tutorial"],
         {"single": 6, "first": 4, "repeat": 5}, "stage.tutorial.smelt_bronze"),
        ("rule.smithing.bronze_dagger", "recipe.smithing.bronze_dagger", "Smith a bronze dagger", ["object.anvil.tutorial"],
         {"single": 5, "first": 5, "repeat": 5, "menu": 1}, "stage.tutorial.anvil_open"),
        ("rule.cooks.milk", "recipe.cooks.milk", "Milk a dairy cow", ["object.dairy_cow", "object.dairy_cow.east"],
         {"single": 3, "first": 3, "repeat": 8}, "stage.tutorial.mainland"),
        ("rule.cooks.milk", "recipe.cooks.milk.use_bucket", "Use a bucket on a dairy cow", ["object.dairy_cow", "object.dairy_cow.east"],
         {"single": 4, "first": 4, "repeat": 8}, "stage.tutorial.mainland"),
    ):
        recipes[identifier] = recipe(rules[rule_id], identifier, name, objects, inputs, timing,
                                     stage_from(inputs, first, include_mainland=True))
    for product, first in (("shrimps", "stage.tutorial.cook_shrimp"), ("bread", "stage.tutorial.bake_bread")):
        rule = rules["rule.cooking." + product]
        for facility, objects in (("fire", ["object.fire.normal"]), ("range", ["object.range.tutorial"]),
                                  ("lumbridge_range", ["object.range.lumbridge"])):
            if product == "bread" and facility == "fire":
                continue
            identifier = "recipe.cooking." + product + "." + facility
            roll = rule["roll"]["lumbridge_range"] if facility == "lumbridge_range" else rule["roll"]
            guard = stage_from(inputs, first, include_mainland=True)
            if facility == "lumbridge_range":
                guard = all_of(guard, {"kind": "quest_stage", "quest": "quest.cooks_assistant", "stage": "stage.cooks.completed"})
            timing = rule["timing"]
            recipes[identifier] = recipe(
                rule, identifier, "Cook " + product + " on " + facility.replace("_", " "), objects, inputs,
                {"single": timing["single_ticks"], "first": timing["make_x_first_ticks"],
                 "repeat": timing["make_x_repeat_ticks"]}, guard, skill_chance(roll["low"], roll["high"]))
    fire = rules["rule.firemaking.normal"]
    recipes["recipe.firemaking.normal"] = recipe(
        fire, "recipe.firemaking.normal", "Light logs", [], inputs, {"single": 4, "first": 4, "repeat": 4},
        stage_from(inputs, "stage.tutorial.light_fire", include_mainland=True),
        skill_chance(fire["roll"]["low"], fire["roll"]["high"]),
        {"kind": "firemaking", "ground_input": "item.logs.normal", "fire": "temporary_object.fire.normal",
         "step_priority": ["west", "east", "south", "north"], "retain_ground_input_on_failure": True},
        tools=["item.tinderbox"])
    return recipes


def build_shop(inputs, items):
    from mechanics import shop_line
    facts = load(BINDINGS / "wiki-facts.json")
    by_name = {definition["name"]: definition for definition in items.values() if not definition["unnoted_variant"]}
    stock = []
    for line in facts["shop_stock"]:
        definition = by_name[line["name"]]
        value = definition["base_value"]
        stock.append({
            "item": definition["id"], "base_stock": line["stock"], "restock_ticks": line["restock_ticks"],
            "buy_price": max(1, value * 1300 // 1000), "sell_price": value * 400 // 1000,
            "mechanics": shop_line(inputs, line["restock_ticks"]),
        })
    return {
        "shop.lumbridge.general_store": {
            "id": "shop.lumbridge.general_store", "name": "Lumbridge General Store", "currency": "item.coins",
            "stock": stock, "accepts_general_items": True,
            "unstocked": {"kind": "accept", "maximum_lines": inputs.profile["shop_unstocked_line_capacity_candidate"]["value"],
                          "rule": shop_line(inputs, 100), "base_stock": 0},
            "source": [
                inputs.wiki("Lumbridge General Store", "All 15 actual stock lines and source restock ticks; prices below are at base stock."),
                inputs.wiki("General store", "Player-sold unstocked items destock one per minute:100 source600ms ticks. "
                            "The bounded unstocked line-count candidate is recorded separately, not inferred from the timer."),
                source_record("research/journey-rules/activities.json#formula.shop.buy",
                              "Typed per-unit stock-sensitive pricing; base-stock prices agree with the source. "
                              "Exact restock phase and overstock conflict remain explicitly unresolved.",
                              "inference", "source-contract-v1"),
            ],
        }
    }


def equipment_slots(inputs):
    return [record["id"] for record in inputs.rules["vocabulary"]["slots"]]
