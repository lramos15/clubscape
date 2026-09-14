"""Schema-2 source mechanics; unresolved observations stay typed, local and non-executable."""

from copy import deepcopy

from common import (
    bound, counter_value, level_domain, requirement,
    source_record, tutorial_at, unique_sources, unresolved,
)


REGISTRIES = ("counters", "grants", "entitlements", "reconciliations", "object_transforms",
              "temporary_objects", "ground_policies", "instances", "travels", "experiences",
              "combat_styles", "spells", "projectiles", "prayers", "value_providers")
PROGRESS_COUNTERS = ("departed", "departure_authorized", "melee_kill", "ranged_kill",
                     "chicken_cast", "caught_shrimp", "shrimp_attempt", "bank_opened")


def create_counter(mechanics, identifier, initial, source, maximum=None, scope="character", source_variable=None):
    value_type = ({"type": "boolean"} if type(initial) is bool else
                  {"type": "integer", "minimum": 0, "maximum": maximum})
    definition = {"id": identifier, "scope": scope, "value_type": value_type,
                  "initial": counter_value(initial), "source_variable": source_variable, "source": source}
    if identifier in mechanics["counters"] and mechanics["counters"][identifier] != definition:
        raise ValueError(f"Conflicting source counter {identifier}")
    mechanics["counters"][identifier] = definition
    return identifier


def completed_counter(edge):
    return "counter." + edge.removeprefix("transition.") + ".completed"


def entitlement(identifier, purpose, source):
    return {"id": identifier, "purpose": purpose, "source": source}


def cadence(source, single=None, first=None, repeat=None, menu=0, note="Source phase timing is not recorded."):
    return {name: bound(value, source) if value is not None else unresolved(f"{name}: {note}", source)
            for name, value in (("single", single), ("first", first), ("repeat", repeat), ("menu_delay", menu))}


def base_mechanics(inputs):
    result = {name: {} for name in REGISTRIES}
    result.update({"world_members": inputs.profile["world_members"], "appearance": None, "run": None, "vitals": None, "death": None})
    source = inputs.basis(inputs.rules["initial-state"]["account"]["basis"])
    for name in PROGRESS_COUNTERS:
        create_counter(result, "counter.tutorial." + name, False, source)
    for edge in inputs.rules["tutorial"]["transitions"]:
        create_counter(result, completed_counter(edge["id"]), False, inputs.basis(edge["basis"]))
    mill_source = inputs.rule_source("rule.cooks.hopper")
    create_counter(result, "counter.mill.hopper_grain", False, mill_source)
    create_counter(result, "counter.mill.flour", 0, inputs.rule_source("rule.cooks.collect_flour"), maximum=30,
                   source_variable={"kind": "varbit", "id": 5325})
    create_counter(result, "counter.death.introduction_heard", False, inputs.rule_source("rule.death.first_office"))
    create_counter(result, "counter.death.exit_confirmed", False, inputs.rule_source("rule.death.first_office"))
    for value in inputs.rules["initial-state"]["experience_branches"]:
        name = value["choice_ref"].removeprefix("choice.")
        result["experiences"][name] = {
            "id": name, "name": value["source_option"],
            "selection_guard": tutorial_at("stage.tutorial.experience"), "source": source,
        }
    result["appearance"] = {
        "choices": {"body_type": inputs.profile["appearance"]["body_type_choices"]},
        "confirmation_guard": tutorial_at("stage.tutorial.appearance"),
        "source": [source_record(
            "research/m1-bindings/selection.json#source_appearance",
            "Source body-type selector 0/1, not a selected penguin model or approved equipment fit. "
            "Full source kit/color controls and presentation remain the source-pack integration surface.",
            "inference", "240/cache2695")],
    }
    build_grants(inputs, result)
    build_vitals(inputs, result)
    build_styles(inputs, result)
    build_ground_policies(inputs, result)
    return result


def build_grants(inputs, mechanics):
    for original in inputs.rules["tutorial"]["grant_definitions"]:
        identifier = original["id"]
        coin = identifier == "grant.tutorial.bank_coins"
        partial = identifier in ("grant.tutorial.melee_gear", "grant.tutorial.ranged_gear")
        source = inputs.basis(original["basis"])
        claim = "entitlement." + identifier.removeprefix("grant.")
        lines = []
        for stack in original["items"]:
            mode = "add"
            if identifier in ("grant.tutorial.chef_ingredients", "grant.tutorial.hammer", "grant.tutorial.melee_gear"):
                mode = "missing_only"
            elif identifier == "grant.tutorial.ranged_gear":
                mode = "top_up" if stack["item_ref"] == "item.arrow.bronze" else "missing_only"
            elif identifier == "grant.tutorial.runes":
                mode = "top_up"
            lines.append({"item": stack["item_ref"], "quantity": stack["quantity"], "mode": mode,
                          "ownership": "bank" if coin else "inventory_and_equipment"})
        mechanics["grants"][identifier] = {
            "id": identifier, "target": "bank" if coin else "inventory",
            "capacity": "ordered_partial" if partial else "atomic", "lines": lines,
            "entitlement": claim, "source": source,
        }
        mechanics["entitlements"][claim] = entitlement(claim, {"kind": "grant", "grant": identifier}, source)
        if not coin:
            recovery = deepcopy(mechanics["grants"][identifier])
            recovery["id"] = identifier + ".recovery"
            recovery["entitlement"] = None
            for line in recovery["lines"]:
                if line["mode"] == "add":
                    line["mode"] = "missing_only"
            recovery["source"] = unique_sources(source + inputs.assumption("assumption.missing_tutorial_tools"))
            mechanics["grants"][recovery["id"]] = recovery
    for name in ("learning_the_ropes", "cooks_assistant"):
        key = "entitlement.quest." + name
        reward = next(r for r in inputs.rules["cooks-assistant"]["rewards"] if r["quest_ref"] == "quest." + name)
        mechanics["entitlements"][key] = entitlement(key, {"kind": "atomic_reward"}, inputs.basis(reward["basis"]))
    source = inputs.rule_source("rule.tutorial.departure")
    key = "entitlement.tutorial.departure"
    reconciliation = "reconciliation.tutorial.departure"
    mechanics["entitlements"][key] = entitlement(key, {"kind": "reconciliation", "reconciliation": reconciliation}, source)
    mechanics["reconciliations"][reconciliation] = {
        "id": reconciliation, "entitlement": key,
        "policies": unresolved(
            "Exact departure inventory/equipment/bank reconciliation after deposits, drops and withdrawals "
            "is not observed or owner-selected. The 18-kind provisions and all alternatives are preserved "
            "in policy-bindings.json; do not normalize ordinary possessions or silently restore bank coins.",
            source + inputs.assumption("assumption.departure_reconciliation")),
        "source": source,
    }


def build_vitals(inputs, mechanics):
    source = inputs.rule_source("rule.movement.energy")
    mechanics["run"] = {
        "agility": "skill.agility", "levels": level_domain(), "activation_minimum": 100,
        "disable_on_exhaustion": True,
        "drain": bound({"base": 60, "weight_scale": 67, "weight_minimum_grams": 0,
                        "weight_maximum_grams": 64000, "agility_scale": 300,
                        "floor_weight_term_before_agility": True, "rounding": "floor"}, source),
        "regeneration": bound({"skill_divisor": 10, "additive_units": 15,
                               "pauses": ["offline", "running"]}, source),
        "source": source,
    }
    hp_source = inputs.rule_source("rule.hitpoints.regeneration")
    food_source = inputs.rule_source("rule.food.healing")
    mechanics["vitals"] = {
        "hitpoints_skill": "skill.hitpoints", "prayer_skill": "skill.prayer",
        "regeneration": [{"vital": "hitpoints", "interval_ticks": bound(100, hp_source),
                          "amount": 1, "pauses": ["offline"], "idle_after_milliseconds": None}],
        "level_up": unresolved("Source current-vital adjustment on a base-level gain is not established by the pinned rules.",
                               inputs.rule_source("rule.xp.level")),
        "food_delay_ticks": bound(3, food_source), "food_attack_delay_ticks": bound(3, food_source),
        "source": unique_sources(hp_source + food_source),
    }
    source = inputs.rule_source("rule.prayer.thick_skin")
    mechanics["prayers"]["prayer.thick_skin"] = {
        "id": "prayer.thick_skin", "interface": "interface.prayer",
        "requirements": [requirement("skill.prayer", basis="base")],
        "modifiers": [{"skill": "skill.defence", "multiplier": {"numerator": 105, "denominator": 100},
                       "rounding": "floor"}],
        "drain": bound({"points_per_tick": {"numerator": 1, "denominator": 60},
                        "bonus_offset": 30, "bonus_divisor": 30}, source),
        "exclusive_with": [], "source": source,
    }


def effective(skill, style=0):
    return {"skill": skill, "basis": "current", "style_bonus": style,
            "constant_bonus": 8, "prayer_before_style": True}


def damage_xp(skill, numerator, denominator=1):
    return {"skill": skill, "tenths_per_damage": {"numerator": numerator, "denominator": denominator},
            "rounding": "floor"}


def build_styles(inputs, mechanics):
    from definitions import STYLE_NAMES
    for item, names in STYLE_NAMES.items():
        raw = inputs.collections["item"][inputs.selection["items"][item]]
        ticks = raw["params"]["14"]
        for label in names:
            identifier = style_id(item, label)
            ranged = label.startswith("ranged.")
            method = "ranged" if ranged else "melee"
            mode = label.split(".")[-1]
            attack_type = "ranged" if ranged else label.split(".")[1]
            accuracy_bonus = 3 if mode == "accurate" else 1 if mode == "controlled" else 0
            strength_bonus = 3 if mode == "aggressive" else 1 if mode == "controlled" else 0
            if ranged:
                strength_bonus = accuracy_bonus
            defence_bonus = 3 if mode in ("defensive", "longrange") else 1 if mode == "controlled" else 0
            xp = []
            if ranged:
                xp.append(damage_xp("skill.ranged", 20 if mode == "longrange" else 40))
                if mode == "longrange":
                    xp.append(damage_xp("skill.defence", 20))
            elif mode == "controlled":
                xp.extend(damage_xp("skill." + skill, 40, 3) for skill in ("attack", "strength", "defence"))
            else:
                xp.append(damage_xp("skill." + {"accurate": "attack", "aggressive": "strength", "defensive": "defence"}[mode], 40))
            xp.append(damage_xp("skill.hitpoints", 40, 3))
            source = inputs.rule_source("rule.combat.ranged" if ranged else "rule.combat.melee")
            negative = inputs.assumption("assumption.negative_combat_roll")
            mechanics["combat_styles"][identifier] = {
                "id": identifier, "method": method, "attack_type": attack_type,
                "attack": effective("skill.ranged" if ranged else "skill.attack", accuracy_bonus),
                "defence": effective("skill.defence", defence_bonus),
                "accuracy": bound("inclusive_opposed_rolls", source),
                "negative_rolls": bound("clamp_to_zero", negative),
                "maximum_hit": bound({"kind": "strength", "level": effective("skill.ranged" if ranged else "skill.strength", strength_bonus),
                                      "equipment_offset": 64, "additive": 320, "divisor": 640}, source),
                "damage": bound({"successful_minimum": 1, "cap_to_remaining_hitpoints": True}, source),
                "cycle_ticks": bound(ticks - int(ranged and mode == "rapid"), source),
                "reach": (9 if mode == "longrange" else 7) if ranged else 1,
                "damage_xp": xp, "projectile": "projectile.bronze_arrow" if ranged else None,
                "source": unique_sources(source + inputs.assumption("assumption.combat_xp_thirds")),
            }
    for identifier, rule in (("projectile.bronze_arrow", "rule.combat.ranged"),
                             ("projectile.wind_strike", "rule.magic.wind_strike")):
        source = inputs.rule_source(rule)
        mechanics["projectiles"][identifier] = {
            "id": identifier, "timing": unresolved(
                "Source projectile launch/impact tick conversion and distance timing remain unmeasured; "
                "animation-cycle metadata is not server tick timing.", source),
            "asset": None, "source": source,
        }
    source = inputs.rule_source("rule.magic.wind_strike")
    mechanics["combat_styles"]["style.magic.wind_strike"] = {
        "id": "style.magic.wind_strike", "method": "magic", "attack_type": "magic",
        "attack": effective("skill.magic"), "defence": effective("skill.defence"),
        "accuracy": bound("inclusive_opposed_rolls", source),
        "negative_rolls": bound("clamp_to_zero", inputs.assumption("assumption.negative_combat_roll")),
        "maximum_hit": bound({
            "kind": "level_table", "skill": "skill.magic", "basis": "current",
            "hits": {"1": 2, "5": 4, "9": 6, "13": 8},
        }, source),
        "damage": bound({"successful_minimum": 1, "cap_to_remaining_hitpoints": True}, source),
        "cycle_ticks": bound(5, source), "reach": 10,
        "damage_xp": [damage_xp("skill.magic", 20), damage_xp("skill.hitpoints", 40, 3)],
        "projectile": "projectile.wind_strike", "source": source,
    }


def style_id(item, label):
    return "style." + item.removeprefix("item.") + "." + label.removeprefix("melee.").removeprefix("ranged.")


def weapon(inputs, item):
    from definitions import STYLE_NAMES
    ammunition = None
    if item == "item.shortbow":
        source = inputs.rule_source("rule.combat.ranged")
        ammunition = {"slot": "slot.ammo", "compatible_items": ["item.arrow.bronze"], "per_attack": 1,
                      "break_chance": bound({"numerator": 1, "denominator": 5}, source),
                      "ground_policy": "ground_policy.ammunition"}
    return {"styles": [style_id(item, label) for label in STYLE_NAMES[item]], "ammunition": ammunition}


def build_ground_policies(inputs, mechanics):
    for identifier, public, expiry, rule in (
        ("ground_policy.tutorial", None, 50, "rule.inventory.drop"),
        ("ground_policy.fire_ashes", None, 300, "rule.firemaking.normal"),
        ("ground_policy.ammunition", None, None, "rule.combat.ranged"),
        ("ground_policy.death_supplies", None, 6000, "rule.death.repeat"),
    ):
        source = inputs.rule_source(rule)
        mechanics["ground_policies"][identifier] = {
            "id": identifier,
            "public_after": bound(public, source) if identifier == "ground_policy.tutorial" else unresolved(
                "Exact source owner-public ground visibility timing is unbound for this item origin.", source),
            "expires_after": (bound(expiry, source) if identifier in ("ground_policy.tutorial", "ground_policy.death_supplies")
                              else unresolved("Exact source expiry for this ground-item origin is unbound.", source)),
            "owner_can_take": True, "source": source,
        }


def npc_combat(inputs, identifier, raw):
    if identifier not in ("npc.tutorial_rat", "npc.tutorial_chicken", "npc.goblin.level_2"):
        return None
    goblin = identifier == "npc.goblin.level_2"
    rule = "rule.goblin.level_2" if goblin else "rule.combat.tutorial_rat"
    source = inputs.rule_source(rule)
    params = raw["params"] or {}
    bonuses = {
        "attack": {"stab": 0, "slash": 0, "crush": -21 if goblin else 0, "magic": 0, "ranged": 0},
        "defence": {kind: params.get(str(index + 5), 0) for index, kind in enumerate(("stab", "slash", "crush", "magic", "ranged"))},
        "melee_strength": -15 if goblin else 0, "ranged_strength": 0, "magic_damage_percent": 0, "prayer": 0,
    }
    if goblin:
        bonuses["defence"] = {kind: -15 for kind in bonuses["defence"]}
    elif identifier == "npc.tutorial_rat":
        bonuses["defence"]["ranged"] = -100
    stats = raw["stats"]
    return {
        "hitpoints": stats[3], "attack": stats[0], "strength": stats[2], "defence": stats[1],
        "ranged": stats[4], "magic": stats[5], "attack_speed_ticks": 4,
        "max_hit": 0 if "chicken" in identifier else 1, "bonuses": bonuses, "respawn_ticks": None,
        "aggressive": False, "drops": [],
        "mechanics": {
            "attack_type": "crush" if goblin else "stab", "attack_stat": "attack",
            "defence_stats": {kind: "magic" if kind == "magic" else "defence" for kind in ("stab", "slash", "crush", "ranged", "magic")},
            "effective_level_bonus": bound(9, source), "retaliation": True, "reach": 1,
            "accuracy": bound("inclusive_opposed_rolls", source),
            "negative_rolls": bound("clamp_to_zero", inputs.assumption("assumption.negative_combat_roll")),
            "damage": unresolved("The player minimum-successful-damage formula does not establish NPC outgoing damage rolls.", source),
            "respawn": bound({"kind": "fixed", "ticks": 35}, source) if goblin else unresolved(
                "Pinned tutorial/farm NPC sources do not supply the exact respawn tick duration.", source),
            "credit": bound("most_damage_then_first_contributor",
                            inputs.basis(inputs.activity_rules["rule.combat.tutorial_rat"]["credit_policy"]["basis"])),
            "loot": goblin_loot(inputs) if goblin else [
                {"kind": "guaranteed", "items": [{"item": "item.bones" if identifier == "npc.chicken" else "item.bones.tutorial",
                                                  "minimum": 1, "maximum": 1}]}],
        },
    }


def goblin_loot(inputs):
    original = inputs.rules["activities"]["loot_tables"][0]
    source = inputs.basis(original["basis"])
    entries = []
    members = []
    for row in original["primary"]:
        loot = [] if row["item_ref"] is None else [{"item": row["item_ref"], "minimum": row["quantity"], "maximum": row["quantity"]}]
        entries.append({"weight": row["weight"], "items": loot if not row["members_only"] else []})
        members.append({"weight": row["weight"], "items": loot})
    primary = [
        {"kind": "conditional", "guard": {"kind": "not", "guard": {"kind": "members_world"}},
         "pools": [{"kind": "exclusive", "total_weight": 128, "entries": entries}]},
        {"kind": "conditional", "guard": {"kind": "members_world"},
         "pools": [{"kind": "exclusive", "total_weight": 128, "entries": members}]},
    ]
    return [
        {"kind": "guaranteed", "items": [{"item": "item.bones", "minimum": 1, "maximum": 1}]},
        *primary,
        {"kind": "unresolved", "reason": "Selected level-2 goblin visual variant, Common energy-potion supplement, "
         "and source clue/tertiary ownership eligibility remain unbound. The complete 128-weight source table "
         "and all tertiary candidates are retained in mechanics-bindings; no zero-drop or guaranteed-coin substitute.",
         "source": source},
    ]


def shop_line(inputs, interval):
    source = inputs.rule_source("rule.shop.lumbridge")
    return {
        "pricing": {"kind": "stock_sensitive",
                    "buy": {"base_per_mille": 1300, "change_per_stock": 30, "minimum_per_mille": 300,
                            "maximum_per_mille": 6300, "minimum_price": 1, "rounding": "floor"},
                    "sell": {"base_per_mille": 400, "change_per_stock": 30, "minimum_per_mille": 100,
                             "maximum_per_mille": 1400, "minimum_price": 0, "rounding": "floor"},
                    "overstock": unresolved("Pinned article and shop calculator disagree for overstock; parent source decision required.",
                                           inputs.assumption("assumption.shop_overstock"))},
        "restock": {"interval_ticks": interval, "amount": 1, "phase": unresolved(
            "Source gives restock intervals but not the world-epoch versus stock-change phase.", source)},
    }
