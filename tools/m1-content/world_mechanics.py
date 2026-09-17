"""Source-bound gather sites, counter-driven mill, stationary anchors and physical door states."""

from collections import defaultdict
from copy import deepcopy

from common import (
    all_of, bound, constant_chance, counter_guard, has_items, item_stack,
    level_domain, position, set_counter, skill_chance, source_record,
    tile, tutorial_at, unresolved,
)
from geometry import World, SOURCE_DIRECTIONS, SOURCE_OPPOSITE, canonical_mask, wall_edges
from mechanics import cadence, completed_counter, create_counter
from spawns import interaction, stage_from


GATHER_OBJECTS = {
    "object.rock.copper": "rule.mining.copper", "object.rock.copper.11161": "rule.mining.copper",
    "object.rock.copper.tutorial": "rule.mining.copper",
    "object.rock.tin": "rule.mining.tin", "object.rock.tin.11361": "rule.mining.tin",
    "object.rock.tin.tutorial": "rule.mining.tin",
    "object.tree.normal": "rule.woodcutting.normal", "object.tree.tutorial": "rule.woodcutting.normal",
    "object.tree.tutorial.9731": "rule.woodcutting.normal", "object.tree.tutorial.9733": "rule.woodcutting.normal",
}
FIRST_ACTION_STAGE = {
    "rule.mining.copper": "stage.tutorial.mine_first", "rule.mining.tin": "stage.tutorial.mine_first",
    "rule.woodcutting.normal": "stage.tutorial.cut_logs", "rule.fishing.shrimps": "stage.tutorial.catch_shrimp",
}
EXIT_EDGE = {
    "object.tutorial.start_door": "transition.tutorial.guide_exit_permission",
    "object.tutorial.survival_gate": "transition.tutorial.shrimp_cooked",
    "object.tutorial.survival_gate.second": "transition.tutorial.shrimp_cooked",
    "object.tutorial.chef_exit": "transition.tutorial.bread",
    "object.tutorial.mine_gate": "transition.tutorial.dagger",
    "object.tutorial.mine_gate.second": "transition.tutorial.dagger",
    "object.tutorial.account_entry": "transition.tutorial.poll",
    "object.tutorial.account_exit": "transition.tutorial.account_explanation",
    "object.tutorial.chapel_exit": "transition.tutorial.prayer_explanation",
}


def gather_rule(inputs, identifier):
    rule = inputs.activity_rules[identifier]
    source = inputs.rule_source(identifier)
    mining = identifier.startswith("rule.mining.")
    fishing = identifier == "rule.fishing.shrimps"
    tools = ["item.pickaxe.bronze" if mining else "item.fishing_net.small" if fishing else "item.axe.bronze"]
    cycle = rule["timing"]["cycle_ticks"]
    phases = cadence(source + inputs.assumption("assumption.environment_timers"),
                     single=cycle, first=cycle, repeat=cycle)
    respawn = (bound({"kind": "fixed", "ticks": rule["timing"]["resource_respawn_ticks"]}, source) if mining else
               unresolved("No depletion/respawn or relocation observation for these tutorial fishing spots; "
                          "the source base method is nondepleting.", source) if fishing else
               bound({"kind": "uniform_inclusive", "minimum": 60, "maximum": 100},
                     source + inputs.assumption("assumption.environment_timers")))
    return {
        "skill": rule["skill_ref"], "required_level": 1, "tools": tools,
        "output": item_stack(rule["produced"][0]["item_ref"]), "xp_tenths": rule["xp_awards"][0]["xp_tenths"],
        "attempt_ticks": None, "success": skill_chance(rule["roll"]["low"], rule["roll"]["high"], maximum=14 if fishing else 99),
        "depletion": constant_chance(0 if fishing else 1), "respawn_ticks": None,
        "mechanics": {"method": identifier.replace("rule.", "action.", 1),
                      "levels": level_domain(maximum=14 if fishing else 99),
                      "cadence": phases, "tool_cadences": [{"tool": tools[0], "location": "inventory" if fishing else
                                                           "inventory_and_equipment", "cadence": phases}],
                      "respawn": respawn, "alternatives": [], "relocation": None},
        "animation": None, "sound": None,
    }


def near_access(world, point, size=1, allowed=None):
    x, y, plane = point
    result = []
    for cx in range(x - 1, x + size + 1):
        for cy in range(y - 1, y + size + 1):
            if x <= cx < x + size and y <= cy < y + size:
                continue
            cell = world.cell(cx, cy, plane)
            if cell and cell["walkable"] and (allowed is None or (cx, cy, plane) in allowed):
                result.append(tile(cx, cy, plane))
    return result


def wire_world(inputs, world, content, bindings):
    spawns, npcs, mechanics = content["spawns"], content["npcs"], content["mechanics"]
    allowed = {position(cell["tile"]) for region in content["regions"].values() for cell in region["cells"]}
    deferred = []
    for identifier, spawn in list(spawns.items()):
        kind = spawn["kind"]
        if kind["kind"] == "npc":
            npc = npcs[kind["npc"]]
            if kind["npc"] not in ("npc.death", "npc.tutorial.fishing_spot") and any(
                    not (cell := world.cell(spawn["tile"]["x"] + dx, spawn["tile"]["y"] + dy, spawn["tile"]["plane"]))
                    or not cell["walkable"]
                    for dx in range(npc["size"]) for dy in range(npc["size"])):
                deferred.append({"spawn": identifier, "source_tile": spawn["tile"],
                                 "reason": "Published mobile NPC candidate conflicts with current source whole-footprint clipping; "
                                           "retained in source bindings, not relocated or exempted as stationary."})
                del spawns[identifier]
                continue
            if kind["npc"] == "npc.tutorial.fishing_spot":
                spawn["interactions"] = [interaction("Net", {"kind": "gather", "rule": gather_rule(inputs, "rule.fishing.shrimps")},
                                                    stage_from(inputs, "stage.tutorial.catch_shrimp"))]
            elif npc["combat"]:
                npc_id = kind["npc"]
                if npc_id == "npc.tutorial_rat":
                    guard = {"kind": "any", "guards": [tutorial_at("stage.tutorial." + stage)
                             for stage in ("melee_rat", "ranged_rat", "wind_strike", "departure_offer",
                                           "departure_confirmation", "home_teleport")]}
                elif npc_id == "npc.tutorial_chicken":
                    guard = all_of(tutorial_at("stage.tutorial.wind_strike"),
                                   counter_guard("counter.tutorial.chicken_cast", False))
                else:
                    guard = tutorial_at("stage.tutorial.mainland")
                spawn["interactions"] = [interaction("Attack", {"kind": "attack"}, guard)]
            elif kind["npc"] in ("npc.shopkeeper", "npc.shop_assistant"):
                spawn["interactions"] = [action for action in spawn["interactions"] if action["name"] != "Trade"] + [
                    interaction("Trade", {"kind": "open_shop", "shop": "shop.lumbridge.general_store",
                                         "interface": "interface.shop", "before_open": []}, tutorial_at("stage.tutorial.mainland"))]
            elif kind["npc"] == "npc.tutorial.banker":
                spawn["interactions"] = [action for action in spawn["interactions"] if action["name"] != "Bank"] + [
                    interaction("Bank", bank_action(True), stage_from(inputs, "stage.tutorial.bank_open"))]
        elif kind["kind"] == "object":
            obj = kind["object"]
            if obj in GATHER_OBJECTS:
                rule = GATHER_OBJECTS[obj]
                name = "Mine" if "rock." in obj else "Chop down"
                spawn["interactions"] = [interaction(name, {"kind": "gather", "rule": gather_rule(inputs, rule)},
                                                    stage_from(inputs, FIRST_ACTION_STAGE[rule], include_mainland=True))]
            elif obj == "object.tutorial.bank_booth":
                spawn["interactions"] = [interaction("Use", bank_action(True), stage_from(inputs, "stage.tutorial.bank_open"))]
            elif obj == "object.lumbridge.bank_booth":
                spawn["interactions"] = [interaction("Bank", bank_action(False), tutorial_at("stage.tutorial.mainland")),
                                        interaction("Collect", {"kind": "unavailable", "reason": "Grand Exchange collection is outside the bounded M1 interactions; no fake success."})]
            elif obj == "object.tutorial.poll_booth":
                spawn["interactions"] = [interaction(
                    "Use", {"kind": "effects", "effects": [{"kind": "inspect", "target": identifier,
                     "explanation": "tutorial_poll_explanation"}]}, stage_from(inputs, "stage.tutorial.poll_inspect"))]
            elif obj in ("object.range.tutorial", "object.range.lumbridge"):
                suffix = "range" if obj.endswith("tutorial") else "lumbridge_range"
                spawn["interactions"] = [interaction("Cook", {"kind": "production", "recipes": [
                    "recipe.cooking.shrimps." + suffix, "recipe.cooking.bread." + suffix]})]
            elif obj == "object.mill.hopper":
                spawn["interactions"] = [interaction("Fill", {"kind": "effects", "effects": [
                    {"kind": "take_items", "items": [item_stack("item.grain")]},
                    set_counter("counter.mill.hopper_grain")]}, all_of(
                    tutorial_at("stage.tutorial.mainland"), has_items([item_stack("item.grain")]),
                    counter_guard("counter.mill.hopper_grain", False),
                    counter_guard("counter.mill.flour", minimum=0, maximum=29)))]
            elif obj == "object.mill.controls":
                spawn["interactions"] = [interaction("Operate", {"kind": "effects", "effects": [
                    set_counter("counter.mill.hopper_grain", False),
                    {"kind": "add_counter", "counter": "counter.mill.flour", "delta": 1}]}, all_of(
                    tutorial_at("stage.tutorial.mainland"), counter_guard("counter.mill.hopper_grain"),
                    counter_guard("counter.mill.flour", minimum=0, maximum=29)))]
            elif obj == "object.mill.flour_bin":
                spawn["interactions"] = [interaction("Empty", {"kind": "effects", "effects": [
                    {"kind": "take_items", "items": [item_stack("item.pot")]},
                    {"kind": "give_items", "items": [item_stack("item.flour.pot")]},
                    {"kind": "add_counter", "counter": "counter.mill.flour", "delta": -1}]}, all_of(
                    tutorial_at("stage.tutorial.mainland"), has_items([item_stack("item.pot")]),
                    counter_guard("counter.mill.flour", minimum=1, maximum=30)))]
            elif obj.startswith("object.altar."):
                spawn["interactions"] = [interaction("Pray-at", {"kind": "effects", "effects": [
                    {"kind": "restore_vital", "vital": "prayer", "restoration": {"kind": "to_base_maximum"}}]},
                    stage_from(inputs, "stage.tutorial.prayer_explanation", include_mainland=True))]
    for npc_id, npc in npcs.items():
        placed = [spawn for spawn in spawns.values() if spawn["kind"].get("npc") == npc_id]
        if npc_id in ("npc.death", "npc.tutorial.fishing_spot"):
            access = {position(value): value for spawn in placed for value in near_access(world, position(spawn["tile"]), npc["size"], allowed)}
            anchor = {"kind": "scripted_actor" if npc_id == "npc.death" else "non_walking_resource",
                      "access_tiles": [access[key] for key in sorted(access)]}
            if npc_id == "npc.death":
                anchor["source"] = [inputs.wiki("Death (NPC)", "The source Office NPC map pin is retained at3180,5727 "
                                               "without clearing the chair/scenery. Walkable adjacent access is separate from player arrival.", "inference")]
            npc["navigation"] = {"kind": "stationary", "anchor": anchor}
        else:
            radius = max((bindings["spawns"].get(spawn["id"], {}).get("marker", {}).get("radius") or 0 for spawn in placed), default=0)
            npc["navigation"]["wander_radius"] = radius
    bindings["placement_issues"]["deferred_mobile_footprints"] = deferred
    content["objects"]["object.mill.flour_bin"]["morph"] = {
        "counter": "counter.mill.flour", "variants": {str(value): "object.mill.flour_bin.empty" if value == 0 else
                                                     "object.mill.flour_bin.full" for value in range(31)},
        "fallback": None,
    }
    fire_source = inputs.rule_source("rule.firemaking.normal") + inputs.assumption("assumption.environment_timers")
    mechanics["temporary_objects"]["temporary_object.fire.normal"] = {
        "id": "temporary_object.fire.normal", "object": "object.fire.normal",
        "lifetime": bound({"kind": "uniform_inclusive", "minimum": 100, "maximum": 199}, fire_source),
        "placement_guard": stage_from(inputs, "stage.tutorial.light_fire", include_mainland=True),
        "interactions": [interaction("Cook", {"kind": "production", "recipes": ["recipe.cooking.shrimps.fire"]})],
        "owner_only_use": False, "blocks_movement": False, "blocks_projectiles": False,
        "expired_items": [item_stack("item.ashes")], "ground_policy": "ground_policy.fire_ashes", "source": fire_source,
    }


def bank_action(tutorial):
    return {"kind": "open_bank", "interface": "interface.bank",
            "before_open": [{"kind": "grant", "grant": "grant.tutorial.bank_coins"}] if tutorial else []}


def wall_contributions(row, rotation=None):
    _, x, y, plane, shape, original = row
    result = defaultdict(int)
    for edge in wall_edges(shape, original if rotation is None else rotation):
        dx, dy, _ = next(vector for vector in SOURCE_DIRECTIONS if vector[2] == edge)
        result[(x, y, plane)] |= edge
        result[(x + dx, y + dy, plane)] |= SOURCE_OPPOSITE[edge]
    return result


def build_doors(inputs, world, content, bindings):
    rows = {inputs.object_spawn_id(row): row for _, row in world.placements}
    candidates = []
    for identifier, spawn in content["spawns"].items():
        if spawn["kind"]["kind"] != "object" or spawn["placement"]["shape"] > 3:
            continue
        raw = inputs.collections["object"][rows[identifier][0]]
        ops = [op["text"] for op in raw["ops"]["ops"] if op]
        if "Open" in ops and any(word in raw["name"].lower() for word in ("door", "gate")):
            candidates.append(identifier)
    candidates.sort()
    groups, unused = [], set(candidates)
    for identifier in candidates:
        if identifier not in unused:
            continue
        unused.remove(identifier)
        row = rows[identifier]
        pair = next((other for other in sorted(unused)
                     if rows[other][3:] == row[3:] and abs(rows[other][1] - row[1]) + abs(rows[other][2] - row[2]) == 1
                     and ("gate" in inputs.collections["object"][row[0]]["name"].lower()
                          or "large door" in inputs.collections["object"][row[0]]["name"].lower())
                     and inputs.collections["object"][rows[other][0]]["name"] ==
                         inputs.collections["object"][row[0]]["name"]), None)
        members = [identifier]
        if pair:
            unused.remove(pair)
            members.append(pair)
        groups.append(members)
    bare = World(inputs, omit_placements=[tuple(rows[identifier]) for identifier in candidates])
    initial_contributions = {}
    for identifier in candidates:
        raw = inputs.collections["object"][rows[identifier][0]]
        contribution = wall_contributions(rows[identifier])
        if raw["blocksProjectile"]:
            contribution = {point: value | (value << 9) for point, value in contribution.items()}
        initial_contributions[identifier] = contribution
    explicit = {position(cell["tile"]) for region in content["regions"].values() for cell in region["cells"]}
    output = []
    for members in groups:
        opened = {}
        rotations = {}
        for identifier in members:
            row = rows[identifier]
            rotation = (row[5] + 1) % 4
            if len(members) == 2:
                axis = 2 if row[5] % 2 == 0 else 1
                lower = row[axis] == min(rows[key][axis] for key in members)
                rotation = (3 if lower else 1) if axis == 2 else (0 if lower else 2)
            rotations[identifier] = rotation
            raw = inputs.collections["object"][row[0]]
            opened[identifier] = {point: value | ((value << 9) if raw["blocksProjectile"] else 0)
                                  for point, value in wall_contributions(row, rotation).items()}
        coverage = sorted({point for identifier in members for mapping in
                           (initial_contributions[identifier], opened[identifier]) for point in mapping})
        if not set(coverage).issubset(explicit):
            output.append({"spawns": members, "status": "outside_navigation_coverage",
                           "reason": "An opening leaf touches unlisted navigation cells; full source scenery remains retained."})
            continue
        open_cells = []
        for point in coverage:
            number, index = bare.locate(*point)
            flags = bare.flags[number][index]
            for owner, contribution in initial_contributions.items():
                flags |= (opened[owner] if owner in members else contribution).get(point, 0)
            cell = deepcopy(world.cell(*point))
            cell["blocked_movement"] = 255 if not cell["walkable"] else canonical_mask(flags)
            cell["blocked_sight"] = 255 if flags & 0x20000 else canonical_mask(flags >> 9)
            open_cells.append(cell)
        source = [source_record("research/m1-bindings/world-bindings.json.gz#placements",
                                "Initial door origin, model, shape and orientation are original cache data. "
                                "Open hinges retain the same source leaf/model and rotate to the adjoining wall edge; "
                                "double leaves open toward opposite outer edges. Opening direction is an explicit "
                                "source-geometry inference pending live state capture, not approved rendering.",
                                "inference", "240/cache2695")]
        name = members[0].removeprefix("spawn.")
        state_counter = create_counter(content["mechanics"], "counter.door." + name, False, source, scope="world")
        ids = []
        for identifier in members:
            spawn = content["spawns"][identifier]
            transform_id = "transform." + identifier.removeprefix("spawn.")
            states = {}
            for state_name in ("closed", "open"):
                placement = deepcopy(spawn["placement"])
                if state_name == "open":
                    placement["quarter_turns"] = rotations[identifier]
                states["object_state." + state_name] = {
                    "object": spawn["kind"]["object"], "tile": spawn["tile"], "door": state_name,
                    "placement": placement,
                    "collision": [world.cell(*point) for point in coverage] if state_name == "closed" else open_cells,
                }
            content["mechanics"]["object_transforms"][transform_id] = {
                "id": transform_id, "scope": "world", "spawn": identifier, "initial": "object_state.closed",
                "states": states, "source": source,
            }
            ids.append(transform_id)
        permission = []
        for identifier in members:
            object_id = content["spawns"][identifier]["kind"]["object"]
            if object_id in EXIT_EDGE:
                permission.append(counter_guard(completed_counter(EXIT_EDGE[object_id])))
        gate = all_of(*permission)
        if any(content["spawns"][identifier]["kind"]["object"].startswith("object.tutorial.rat_gate") for identifier in members):
            gate = {"kind": "any", "guards": [tutorial_at("stage.tutorial." + stage) for stage in
                                             ("enter_rat_pen", "melee_rat", "leave_rat_pen")]}
        for identifier in members:
            content["spawns"][identifier]["interactions"] = [
                interaction(label.capitalize(), {"kind": "effects", "effects": [
                    *[{"kind": "transform_object", "transform": key, "state": "object_state." + label} for key in ids],
                    set_counter(state_counter, label == "open")]},
                    all_of(gate, counter_guard(state_counter, label != "open")))
                for label in ("open", "closed")
            ]
            content["spawns"][identifier]["interactions"][1]["name"] = "Close"
        output.append({"spawns": members, "transforms": ids, "counter": state_counter,
                       "coverage": [list(point) for point in coverage], "status": "bound_inference",
                       "source": source})
    bindings["doors"] = output
