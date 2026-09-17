"""Bind the declared content-3 execution selectors without changing source geometry or rewards."""

from copy import deepcopy
from itertools import product

from common import (
    BINDINGS, all_of, always, bound, canonical, constant_chance, counter_guard,
    item_stack, load, position, requirement, sha, source_record, tutorial_at, unique_sources,
)
from geometry import World, canonical_mask
from dialogue_entries import normalize_tutorial_choices, normalize_tutorial_recovery
from mechanics import completed_counter, damage_xp, effective
from progression import within_points
from spawns import stage_from
from world_mechanics import EXIT_EDGE, wall_contributions


VERSION = 3


def evidence(inputs, identifiers, note):
    records = {record["id"]: record for record in load(BINDINGS / "runtime3-sources.json")["sources"]}
    return unique_sources([
        source_record(records[identifier]["url"], note + " Source SHA-256: " + records[identifier]["sha256"]
                      + ". Public source-supported reconstruction/interpretation, not a live observation.",
                      "inference", records[identifier]["revision"]) for identifier in identifiers
    ] + [source_record("research/m1-bindings/runtime3-policy.json", note, "inference", "runtime3-source-profile-v1")])


def apply_selectors(inputs, world, content, bindings):
    policy = load(BINDINGS / "runtime3-policy.json")
    before = deepcopy(content)
    mechanics = content["mechanics"]
    content["schema_version"] = VERSION
    mechanics["traversal"] = {}
    mechanics["collision_groups"] = {}
    ground_selectors(inputs, content, policy)
    combat_selectors(inputs, content, bindings, policy)
    traversal_selectors(inputs, content, bindings)
    combined_collisions(inputs, world, content, bindings)
    mill_morph(inputs, content)
    recovery_selectors(inputs, content, policy)
    consume_recipes(inputs, content)
    choice_entries = normalize_tutorial_choices(content)
    recovery_entries = normalize_tutorial_recovery(content)
    for stage in content["tutorial"].values():
        allowed = stage["allowed_actions"]
        if "produce" in allowed and "produce_selected" not in allowed:
            allowed.append("produce_selected")
        if stage["id"] == "stage.tutorial.mainland":
            allowed.extend(name for name in ("open_grave", "open_death_office") if name not in allowed)
    audit = {
        "content_schema_version": VERSION, "artifact_version": VERSION, "runtime_schema_version": 1,
        "policy": "research/m1-bindings/runtime3-policy.json",
        "policy_sha256": sha(canonical(policy)),
        "source_index": "research/m1-bindings/runtime3-sources.json",
        "source_index_sha256": sha((BINDINGS / "runtime3-sources.json").read_bytes()),
        "v2_applied_behavior_sha256": bindings["application"]["applied_behavior_sha256"],
        "new_ground_policies": sorted(set(mechanics["ground_policies"]) - set(before["mechanics"]["ground_policies"])),
        "unarmed_styles": policy["unarmed"]["styles"],
        "traversal_definitions": len(mechanics["traversal"]),
        "collision_groups": len(mechanics["collision_groups"]),
        "combined_collision_selections": sum(len(group["states"]["value"]) for group in mechanics["collision_groups"].values()),
        "npc_selector_ids": [npc["id"] for npc in content["npcs"].values() if npc["combat"]],
        "fresh_normal_manual_drop_profile": policy["fresh_normal_manual_drop_profile"],
        "death_timing": mechanics["death"]["timing"], "recovery_interfaces": mechanics["death"]["interfaces"],
        "tutorial_choice_entries": choice_entries,
        "tutorial_recovery_entries": recovery_entries,
        "new_consume_only_recipes": [identifier for identifier, recipe in content["recipes"].items()
                                     if recipe["mechanics"]["lifecycle"]["kind"] == "consume_only"],
        "morph_domain_projection": {
            "object": "object.mill.flour_bin", "counter": "counter.mill.flour",
            "reachable_selectors": list(range(31)), "source_fallback": None,
            "runtime_fallback": "object.mill.flour_bin.empty",
            "proof": "Every valid integer0..30 is explicitly mapped. Out-of-domain counter values fail before "
                     "morph resolution; an unreachable fallback is totalized to the real empty variant. "
                     "All reachable variants have identical footprint/clipping and remain per-character; "
                     "the complete original source morph array/fallback remains in the source bindings.",
        },
        "source_geometry_preserved": True, "source_supported_inference_not_observed": True,
        "gameplay_acceptance_claimed": False, "presentation_approved": False,
    }
    from verify_assets import behavior_projection
    audit["content_behavior_sha256"] = sha(canonical(behavior_projection(content)))
    bindings["runtime3"] = audit
    bindings["application"]["runtime3"] = audit
    bindings["application"]["remaining_selector_hooks"] = []
    bindings["application"]["executor_conformance_requirements"] = "research/m1-bindings/contract-gaps.json"
    bindings["application"]["full_target_follow_up"] = [
        "The retained beginner-clue/member-only tertiary candidates need their own full-target acquisition "
        "and ownership rules before enabling those outcomes; the potion approval does not certify them.",
        "Reevaluate the six explicit inactive-dependency proofs before extending acquisition, depletion "
        "or the ordinary Office item universe.",
        "Owner-private ground persists in the current authoritative world across logout and restart. "
        "Cross-world transfer remains outside the single-world M1 profile and must carry the owner, "
        "remaining lifetime and frozen clock without duplication before enabling world hopping.",
    ]
    return content


def ground_selectors(inputs, content, policy):
    mechanics = content["mechanics"]
    source = evidence(inputs, ["wiki_items", "wiki_drop"], policy["fresh_normal_manual_drop_basis"])
    manual = policy["fresh_normal_manual_drop"]
    for identifier, public, expiry in (
        ("ground_policy.player_drop.ordinary", manual["ordinary_full_target_public_after"],
         manual["ordinary_full_target_expires_after"]),
        ("ground_policy.player_drop.m1_fresh", manual["public_after"], manual["expires_after"]),
    ):
        mechanics["ground_policies"][identifier] = {
            "id": identifier, "public_after": bound(public, source), "expires_after": bound(expiry, source),
            "clock": bound("owner_online_ticks" if identifier.endswith(".m1_fresh") else "world_ticks", source),
            "owner_can_take": True, "source": source,
        }
    mechanics["player_drop"] = {
        "ordinary": bound("ground_policy.player_drop.ordinary", source),
        "stages": {stage: bound("ground_policy.tutorial", source) for stage in content["tutorial"]
                   if stage != "stage.tutorial.mainland"},
        "untradeable": bound("ground_policy.player_drop.m1_fresh", source),
        "before_playtime": bound({
            "played_ticks_below": policy["fresh_normal_manual_drop_profile"]["documented_playtime_below_ticks"],
            "ground_policy": "ground_policy.player_drop.m1_fresh",
        }, source),
        "source": source,
    }
    source = evidence(inputs, ["wiki_drops"], policy["npc_loot_basis"])
    mechanics["ground_policies"]["ground_policy.npc_loot"] = {
        "id": "ground_policy.npc_loot",
        "public_after": bound(policy["npc_loot_ground"]["public_after"], source),
        "expires_after": bound(policy["npc_loot_ground"]["expires_after"], source),
        "clock": bound("world_ticks", source),
        "owner_can_take": True, "source": source,
    }
    for definition in mechanics["ground_policies"].values():
        if "clock" not in definition:
            definition["clock"] = bound(
                "owner_online_ticks" if definition["id"] == "ground_policy.death_supplies" else "world_ticks",
                definition["source"],
            )


def combat_selectors(inputs, content, bindings, policy):
    mechanics = content["mechanics"]
    source = evidence(inputs, ["combat_constants", "player_combat_common"], policy["unarmed_basis"])
    modes = (("punch", "attack", 3, 0, 0), ("kick", "strength", 0, 3, 0), ("block", "defence", 0, 0, 3))
    for name, xp_skill, attack_bonus, strength_bonus, defence_bonus in modes:
        identifier = "style.unarmed." + name
        mechanics["combat_styles"][identifier] = {
            "id": identifier, "method": "melee", "attack_type": policy["unarmed"]["attack_type"],
            "attack": effective("skill.attack", attack_bonus), "defence": effective("skill.defence", defence_bonus),
            "accuracy": bound("inclusive_opposed_rolls", inputs.rule_source("rule.combat.melee")),
            "negative_rolls": bound("clamp_to_zero", inputs.assumption("assumption.negative_combat_roll")),
            "maximum_hit": bound({"kind": "strength", "level": effective("skill.strength", strength_bonus),
                                  "equipment_offset": 64, "additive": 320, "divisor": 640}, source),
            "damage": bound({"successful_minimum": 1, "cap_to_remaining_hitpoints": True}, inputs.rule_source("rule.combat.melee")),
            "cycle_ticks": bound(policy["unarmed"]["cycle_ticks"], source), "reach": policy["unarmed"]["reach"],
            "damage_xp": [damage_xp("skill." + xp_skill, 40), damage_xp("skill.hitpoints", 40, 3)],
            "projectile": None, "source": source,
        }
    for definition in content["items"].values():
        equipment = definition["equipment"]
        if equipment and equipment["weapon"]:
            weapon = equipment["weapon"]
            wanted = ("style.shortbow.accurate" if definition["id"] == "item.shortbow" else
                      "style." + definition["id"].removeprefix("item.") + "." +
                      ("stab.controlled" if definition["id"] == "item.spear.bronze" else
                       "slash.accurate" if definition["id"] == "item.axe.bronze" else "stab.accurate"))
            if wanted not in weapon["styles"]:
                raise ValueError("Declared source weapon default is not an offered style: " + definition["id"])
            weapon["default_style"] = wanted
    engagement = evidence(inputs, ["combat_constants", "player_extensions", "player_hit_processor", "wiki_logout"],
                          policy["player_engagement_basis"])
    mechanics["player_combat"] = {
        "unarmed": bound({"styles": policy["unarmed"]["styles"], "default_style": policy["unarmed"]["default_style"],
                          "ammunition": None}, source),
        "engagement": bound(policy["player_engagement"], engagement),
        "source": unique_sources(source + engagement),
    }
    pen = within_points([tuple(point) for point in bindings["rat_pen_cells"]])
    for identifier, npc in content["npcs"].items():
        if not npc["combat"]:
            continue
        combat = npc["combat"]["mechanics"]
        loot_policy = "ground_policy.tutorial" if identifier.startswith("npc.tutorial") else "ground_policy.npc_loot"
        combat["loot_ground_policy"] = bound(loot_policy, unique_sources(
            mechanics["ground_policies"][loot_policy]["source"] +
            evidence(inputs, ["wiki_items", "wiki_drops"], policy["npc_loot_basis"])))
        combat["attribution"] = bound(policy["method_attribution"], evidence(
            inputs, ["wiki_combat"], policy["method_attribution_basis"]))
        if identifier == "npc.tutorial_rat":
            eligibility = [
                {"method": "melee", "style": None, "guard": all_of(tutorial_at("stage.tutorial.melee_rat"),
                    counter_guard("counter.tutorial.melee_kill", False), pen)},
                {"method": "ranged", "style": None, "guard": all_of(tutorial_at("stage.tutorial.ranged_rat"),
                    counter_guard("counter.tutorial.ranged_kill", False), {"kind": "not", "guard": pen})},
                {"method": "magic", "style": "style.magic.wind_strike", "guard":
                    stage_from(inputs, "stage.tutorial.wind_strike")},
            ]
        elif identifier == "npc.tutorial_chicken":
            eligibility = [{"method": "magic", "style": "style.magic.wind_strike",
                            "guard": all_of(tutorial_at("stage.tutorial.wind_strike"),
                                            counter_guard("counter.tutorial.chicken_cast", False))}]
        else:
            eligibility = [{"method": method, "style": None, "guard": tutorial_at("stage.tutorial.mainland")}
                           for method in ("melee", "ranged", "magic")]
        combat["eligibility"] = bound(eligibility, inputs.rule_source("rule.tutorial.restrictions"))
        combat["engagement"] = bound(policy["npc_engagement"], evidence(
            inputs, ["combat_commons", "combat_constants", "npc_extensions", "npc_modes", "npc_player_combat"],
            policy["npc_engagement_basis"]))
    # Method restrictions live in eligibility, not a broad verb guard that would
    # accidentally forbid manual magic on rats after their weapon lessons.
    for spawn in content["spawns"].values():
        if spawn["kind"].get("npc") in ("npc.tutorial_rat", "npc.tutorial_chicken"):
            for action in spawn["interactions"]:
                if action["action"]["kind"] == "attack":
                    action["guard"] = always()


def traversal_selectors(inputs, content, bindings):
    mechanics = content["mechanics"]
    source = inputs.rule_source("rule.tutorial.restrictions")
    for group in bindings["doors"]:
        if group["status"] != "bound_inference":
            continue
        guards = []
        rat = False
        edges = []
        for identifier in group["spawns"]:
            spawn = content["spawns"][identifier]
            obj = spawn["kind"]["object"]
            if obj in EXIT_EDGE:
                guards.append(counter_guard(completed_counter(EXIT_EDGE[obj])))
            rat |= obj.startswith("object.tutorial.rat_gate")
            x, y, plane = position(spawn["tile"])
            orientation = spawn["placement"]["quarter_turns"]
            dx, dy = ((-1, 0), (0, 1), (1, 0), (0, -1))[orientation]
            edges.append({"from": deepcopy(spawn["tile"]), "to": {"x": x + dx, "y": y + dy, "plane": plane},
                          "bidirectional": True})
        if not guards and not rat:
            continue
        identifier = "traversal." + group["spawns"][0].removeprefix("spawn.")
        if rat:
            pen = {tuple(point) for point in bindings["rat_pen_cells"]}
            entrance_edges, exit_edges = [], []
            for edge in edges:
                inside, outside = (edge["from"], edge["to"]) if position(edge["from"]) in pen else (edge["to"], edge["from"])
                entrance_edges.append({"from": outside, "to": inside, "bidirectional": False})
                exit_edges.append({"from": inside, "to": outside, "bidirectional": False})
            guard = {"kind": "any", "guards": [tutorial_at(stage) for stage in
                     inputs.rules["tutorial"]["rat_pen_permission"]["entry_stage_refs"]]}
            mechanics["traversal"][identifier + ".enter"] = {
                "id": identifier + ".enter", "scope": "world", "edges": entrance_edges, "guard": guard, "source": source}
            mechanics["traversal"][identifier + ".exit"] = {
                "id": identifier + ".exit", "scope": "world", "edges": exit_edges, "guard": always(), "source": source}
            opener = {"kind": "any", "guards": [guard, within_points(sorted(pen))]}
            for spawn_id in group["spawns"]:
                for action in content["spawns"][spawn_id]["interactions"]:
                    action["guard"] = all_of(opener, counter_guard(group["counter"], action["name"] == "Close"))
        else:
            mechanics["traversal"][identifier] = {
                "id": identifier, "scope": "world", "edges": edges, "guard": all_of(*guards), "source": source}


def combined_collisions(inputs, world, content, bindings):
    mechanics = content["mechanics"]
    transforms = mechanics["object_transforms"]
    rows = {inputs.object_spawn_id(row): row for _, row in world.placements}
    members = {transform["spawn"] for transform in transforms.values()}
    bare = World(inputs, omit_placements=[tuple(rows[identifier]) for identifier in members])
    contributions = {}
    for identifier, transform in transforms.items():
        row = rows[transform["spawn"]]
        raw = inputs.collections["object"][row[0]]
        contributions[identifier] = {}
        for state_id, state in transform["states"].items():
            contributions[identifier][state_id] = {
                point: value | ((value << 9) if raw["blocksProjectile"] else 0)
                for point, value in wall_contributions(row, state["placement"]["quarter_turns"]).items()}
    occupied = set()
    for group in bindings["doors"]:
        if group["status"] != "bound_inference":
            continue
        identifiers = sorted(group["transforms"])
        coverage = [tuple(point) for point in group["coverage"]]
        if occupied.intersection(coverage):
            raise ValueError("Distinct source door groups overlap; source joint states must be authored, not overwritten")
        occupied.update(coverage)
        states = []
        for choices in product(*[list(transforms[identifier]["states"]) for identifier in identifiers]):
            selection = dict(zip(identifiers, choices, strict=True))
            cells = []
            for point in coverage:
                square, index = bare.locate(*point)
                flags = bare.flags[square][index]
                for identifier, transform in transforms.items():
                    selected = selection.get(identifier, transform["initial"])
                    flags |= contributions[identifier][selected].get(point, 0)
                cell = deepcopy(world.cell(*point))
                cell["blocked_movement"] = 255 if not cell["walkable"] else canonical_mask(flags)
                cell["blocked_sight"] = 255 if flags & 0x20000 else canonical_mask(flags >> 9)
                cells.append(cell)
            states.append({"selection": selection, "collision": cells})
        identifier = "collision_group." + group["spawns"][0].removeprefix("spawn.")
        mechanics["collision_groups"][identifier] = {
            "id": identifier, "scope": "world", "transforms": identifiers, "states": bound(states, group["source"]),
            "source": group["source"],
        }
        group["collision_group"] = identifier


def mill_morph(inputs, content):
    parent = content["objects"]["object.mill.flour_bin"]
    counter = content["mechanics"]["counters"]["counter.mill.flour"]
    if counter["value_type"] != {"type": "integer", "minimum": 0, "maximum": 30}:
        raise ValueError("Mill morph domain proof changed")
    if set(parent["morph"]["variants"]) != {str(value) for value in range(31)}:
        raise ValueError("Mill morph has an unbound reachable selector")
    for identifier in set(parent["morph"]["variants"].values()):
        variant = content["objects"][identifier]
        if (variant["size_x"], variant["size_y"], variant["clip"]) != (parent["size_x"], parent["size_y"], parent["clip"]):
            raise ValueError("Player-local mill morph now changes shared collision")
    parent["morph"]["fallback"] = "object.mill.flour_bin.empty"
    parent["morph"]["collision"] = None
    parent["source"] = unique_sources(parent["source"] + [source_record(
        "research/m1-bindings/runtime3-selectors.json#morph_domain_projection",
        "All valid player-local flour selectors0..30 have identical source footprint/clipping. Out-of-domain values "
        "are rejected by the counter; the unreachable runtime fallback names the real empty variant, not erased geometry.",
        "inference", "content3-domain-projection")])


def recovery_selectors(inputs, content, policy):
    death = content["mechanics"]["death"]
    death["timing"] = bound(policy["death_timing"], evidence(inputs, ["death_operations"], policy["death_timing_basis"]))
    death["interfaces"] = {"grave": "interface.grave", "office": "interface.death_retrieval"}
    for identifier in death["interfaces"].values():
        if content["interfaces"][identifier]["access"] != "contextual":
            raise ValueError("Recovery selector does not reference a contextual source interface")
    for graph in [content["tutorial"], content["quests"]]:
        def unlock(value):
            if isinstance(value, dict):
                if value.get("kind") == "set_tutorial_stage" and value["stage"] == "stage.tutorial.mainland":
                    return
                for child in value.values():
                    unlock(child)
            elif isinstance(value, list):
                if any(isinstance(child, dict) and child.get("kind") == "set_tutorial_stage"
                       and child.get("stage") == "stage.tutorial.mainland" for child in value):
                    for identifier in death["interfaces"].values():
                        effect = {"kind": "unlock_interface", "interface": identifier}
                        if effect not in value:
                            value.append(effect)
                for child in value:
                    unlock(child)
        unlock(graph)


def consume_recipes(inputs, content):
    source = inputs.rule_source("rule.prayer.bones")
    from mechanics import cadence
    for item, suffix, guard, evidence_records in (
        ("item.bones", "ordinary", tutorial_at("stage.tutorial.mainland"), source),
        ("item.bones.tutorial", "tutorial", stage_from(inputs, "stage.tutorial.prayer_explanation"),
         unique_sources(source + inputs.basis(inputs.activity_rules["rule.prayer.bones"]["tutorial_variant_basis"]))),
    ):
        identifier = "recipe.prayer.bones." + suffix
        content["recipes"][identifier] = {
            "id": identifier, "name": "Bury source bones", "inputs": [item_stack(item)], "outputs": [], "failed_outputs": [],
            "tools": [], "requirements": [requirement("skill.prayer")],
            "xp": [{"skill": "skill.prayer", "amount_tenths": 45}], "ticks": None, "success": constant_chance(),
            "target_objects": [],
            "mechanics": {"method": "action.prayer.bones", "guard": guard, "chance_skill": None,
                          "cadence": cadence(evidence_records, single=2, first=2, repeat=2),
                          "tool_ownership": "inventory", "failed_xp": [], "success_effects": [], "failure_effects": [],
                          "lifecycle": {"kind": "consume_only"}},
            "source": evidence_records,
        }
