"""Independent assertions about source bindings and safe authored projections, not a live game."""

from collections import Counter
from copy import deepcopy
import unittest

from common import BINDINGS, CONTENT, Inputs, counter_value, item_stack, load, position
from definitions import xp_thresholds
from geometry import World, canonical_mask, clipped_footprint, wall_edges
from check import check_content
from state_oracles import Oracle, OracleRefusal


def evaluate(guard, stage, items, flags=None):
    kind = guard["kind"]
    if kind == "always":
        return True
    if kind == "not":
        return not evaluate(guard["guard"], stage, items, flags)
    if kind == "all":
        return all(evaluate(value, stage, items, flags) for value in guard["guards"])
    if kind == "any":
        return any(evaluate(value, stage, items, flags) for value in guard["guards"])
    if kind == "quest_stage":
        return guard["quest"] == "quest.cooks_assistant" and stage == guard["stage"]
    if kind == "tutorial_stage":
        return guard["stage"] == "stage.tutorial.mainland"
    if kind == "has_items":
        return all(items[item["item"]] >= item["quantity"] for item in guard["items"])
    if kind == "flag":
        return (flags or {}).get(guard["name"], 0) == guard["equals"]
    if kind == "entitlement_claimed":
        return False
    raise AssertionError(f"Test interpreter does not silently permit {kind}")


class SourceArithmeticTests(unittest.TestCase):
    def test_all_99_source_thresholds(self):
        thresholds = xp_thresholds()
        self.assertEqual(len(thresholds), 99)
        self.assertEqual(thresholds[:5], [0, 830, 1740, 2760, 3880])
        self.assertEqual(thresholds[9], 11540)
        self.assertEqual(thresholds[49], 1013330)
        self.assertEqual(thresholds[-1], 130344310)
        self.assertEqual(thresholds, sorted(set(thresholds)))

    def test_exact_canonical_mask_permutation(self):
        expected = {1: 128, 2: 1, 4: 16, 8: 2, 16: 32, 32: 4, 64: 64, 128: 8}
        for source, target in expected.items():
            self.assertEqual(canonical_mask(source), target)
        self.assertEqual(canonical_mask(255), 255)
        self.assertEqual(canonical_mask(0x100 | 0x200000), 0)

    def test_all_source_wall_orientations(self):
        straight = [(128,), (2,), (8,), (32,)]
        diagonal = [(1,), (4,), (16,), (64,)]
        corner = [(128, 2), (2, 8), (8, 32), (32, 128)]
        for orientation in range(4):
            self.assertEqual(wall_edges(0, orientation), straight[orientation])
            self.assertEqual(wall_edges(1, orientation), diagonal[orientation])
            self.assertEqual(wall_edges(3, orientation), diagonal[orientation])
            self.assertEqual(wall_edges(2, orientation), corner[orientation])

    def test_source_rotated_footprints(self):
        definition = {"sizeX": 2, "sizeY": 3}
        self.assertEqual([clipped_footprint(definition, orientation) for orientation in range(4)],
                         [(2, 3), (3, 2), (2, 3), (3, 2)])

    def test_diagonal_needs_both_cardinal_routes(self):
        class Cells(World):
            def __init__(self):
                self.cells = {(x, y, 0): {"walkable": True, "blocked_movement": 0}
                              for x in (1, 2) for y in (1, 2)}

            def cell(self, x, y, plane):
                return self.cells.get((x, y, plane))
        world = Cells()
        self.assertTrue(world.can_step((1, 1, 0), (2, 2, 0)))
        world.cells[(2, 1, 0)]["blocked_movement"] = 1
        self.assertFalse(world.can_step((1, 1, 0), (2, 2, 0)))
        world.cells[(2, 1, 0)]["blocked_movement"] = 0
        del world.cells[(1, 2, 0)]
        self.assertFalse(world.can_step((1, 1, 0), (2, 2, 0)))
        self.assertFalse(world.can_step((1, 1, 0), (1, 1, 1)))


class AuthoredContentTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.inputs = Inputs()
        cls.world = World(cls.inputs)
        cls.content = load(CONTENT / "game-content.json.gz")
        cls.graph = load(BINDINGS / "graph-bindings.json")
        cls.bindings = {
            **cls.graph,
            "spawns": load(BINDINGS / "spawn-bindings.json.gz")["spawns"],
            "travel": load(BINDINGS / "travel-bindings.json")["links"],
        }
        cls.mechanics = load(BINDINGS / "mechanics-bindings.json.gz")

    def test_product_structure(self):
        result = check_content(self.content, self.inputs, self.world, self.bindings)
        self.assertTrue(result["structural_checks_passed"])
        self.assertFalse(result["runtime_compile_passed"])

    def test_exact_source_variants_not_similarly_named_npcs(self):
        npcs, items = self.content["npcs"], self.content["items"]
        self.assertEqual(npcs["npc.survival_expert"]["source_id"], 8503)
        self.assertEqual(npcs["npc.cook"]["source_id"], 4626)
        self.assertEqual(npcs["npc.tutorial_rat"]["source_id"], 3313)
        self.assertEqual(npcs["npc.tutorial_chicken"]["source_id"], 3316)
        self.assertFalse({3314, 3315, 9483, 225, 2895, 2896, 3306}.intersection(
            npc["source_id"] for npc in npcs.values()))
        self.assertEqual(items["item.shrimps.burnt"]["source_id"], 7954)
        self.assertEqual(items["item.bones.tutorial"]["source_id"], 2530)
        self.assertEqual(items["item.milk.bottomless_bucket"]["source_id"], 33089)
        self.assertEqual(items["item.milk.bottomless_bucket"]["stackable"]["source_mode"], 2)
        self.assertEqual(items["item.milk.bottomless_bucket"]["charges"]["maximum"], 10000)
        self.assertEqual(self.inputs.collections["item"][33089]["stackable"], 2)

    def test_source_dairy_cows_are_objects_and_fishing_spots_are_npcs(self):
        objects = self.content["objects"]
        self.assertEqual(objects["object.dairy_cow"]["source_id"], 8689)
        self.assertEqual(objects["object.dairy_cow.east"]["source_id"], 60788)
        self.assertNotIn("npc.dairy_cow", self.content["npcs"])
        fish = [spawn for spawn in self.content["spawns"].values()
                if spawn["kind"].get("npc") == "npc.tutorial.fishing_spot"]
        self.assertEqual({position(spawn["tile"]) for spawn in fish},
                         {(3099, 3090, 0), (3101, 3092, 0), (3103, 3092, 0)})
        self.assertTrue(all(not self.world.cell(**spawn["tile"])["walkable"] for spawn in fish))

    def test_all_original_scenery_and_explicit_cells_retained(self):
        world = load(BINDINGS / "world-bindings.json.gz")
        self.assertEqual(len(world["placements"]), 151019)
        self.assertEqual(len({row[0] for row in world["placements"]}), 151019)
        self.assertEqual(len(world["regions"]), 61)
        self.assertEqual(len(world["objects"]), 4837)
        self.assertEqual(sum(len(load(CONTENT / f"geometry/{region['source_square']}.json.gz")["cells"])
                             for region in world["regions"]), 999424)
        self.assertEqual(world["objects"]["object.tree.normal"]["models"], [1570])
        self.assertEqual(world["objects"]["object.tree.normal"]["recolor_from"], [3470])
        self.assertEqual(world["objects"]["object.tree.normal"]["recolor_to"], [5029])

    def test_source_initial_not_seeded_completed_or_boosted(self):
        initial = self.content["initial_state"]
        self.assertEqual(initial["tutorial_stage"], "stage.tutorial.appearance")
        self.assertEqual(initial["inventory"]["slots"], [None] * 28)
        self.assertEqual(initial["bank"], {"capacity": 400, "slots": []})
        self.assertEqual(initial["quest_points"], 0)
        self.assertEqual(initial["run_energy"], 10000)
        self.assertEqual(initial["hitpoints"], 10)
        self.assertEqual(initial["skills"]["skill.hitpoints"], {"current_level": 10, "xp_tenths": 11540})
        self.assertEqual(len(initial["skills"]), 24)
        self.assertEqual(initial["skills"]["skill.sailing"]["xp_tenths"], 0)
        self.assertEqual(initial["flags"], {})
        self.assertIsNone(initial["runtime"]["settings"]["experience"])
        self.assertFalse(initial["runtime"]["settings"]["appearance_confirmed"])
        self.assertEqual(initial["runtime"]["counters"]["counter.tutorial.departed"], counter_value(False))

    def test_xp_stop_and_ceiling_are_not_conflated(self):
        for stage in self.content["tutorial"].values():
            if stage["id"] == "stage.tutorial.mainland":
                self.assertEqual(stage["xp_stop_levels"], {})
                self.assertEqual(stage["xp_caps_tenths"], {})
            else:
                self.assertEqual(stage["xp_stop_levels"]["skill.mining"], 3)
                self.assertEqual(stage["xp_caps_tenths"]["skill.mining"], 2759)
                self.assertNotEqual(stage["xp_caps_tenths"]["skill.mining"], 1740)
                self.assertEqual(stage["xp_caps_tenths"]["skill.hitpoints"], 11540)

    def test_source_probability_not_linear_endpoint_or_forced_success(self):
        rules = {rule["id"]: rule for rule in self.mechanics["rules"]}
        copper = rules["rule.mining.copper"]["success_count_by_level_1_through_99"]
        shrimp = rules["rule.fishing.shrimps"]["success_count_by_level_1_through_99"]
        self.assertEqual(copper[0], 101)
        self.assertEqual(shrimp[0], 49)
        self.assertEqual(shrimp[13], 77)
        self.assertTrue(all(0 <= count <= 256 for count in copper + shrimp))
        self.assertEqual(rules["rule.mining.copper"]["source_rule"]["timing"]["cycle_ticks"], 8)

    def test_reciprocal_notes_and_all_normal_equipment_slots(self):
        items = self.content["items"]
        for definition in items.values():
            if note := definition["noted_variant"]:
                self.assertEqual(items[note]["unnoted_variant"], definition["id"])
                self.assertTrue(items[note]["stackable"])
                self.assertIsNone(items[note]["equipment"])
                self.assertIsNone(items[note]["healing"])
        self.assertEqual(len(self.content["equipment_slots"]), 11)
        self.assertEqual(set(items["item.shortbow"]["equipment"]["occupied_slots"]), {"slot.weapon", "slot.shield"})

    def test_hammer_is_required_not_consumed(self):
        recipe = self.content["recipes"]["recipe.smithing.bronze_dagger"]
        self.assertEqual(recipe["tools"], ["item.hammer"])
        self.assertEqual(recipe["inputs"], [item_stack("item.bar.bronze")])
        self.assertEqual(recipe["xp"], [{"skill": "skill.smithing", "amount_tenths": 125}])
        smelt = self.content["recipes"]["recipe.smelting.bronze"]
        self.assertEqual(smelt["xp"], [{"skill": "skill.smithing", "amount_tenths": 62}])
        self.assertEqual(self.content["recipes"]["recipe.cooks.milk"]["target_objects"],
                         ["object.dairy_cow", "object.dairy_cow.east"])

    def test_cook_all_six_partial_delivery_orders(self):
        source = self.inputs.rules["expected-scenarios"]["quest_delivery_orders"]
        orders = source["orders"]
        nodes = {node["id"]: node for node in self.content["dialogues"]["dialogue.cook"]["nodes"]}
        item_ids = {"milk": "item.milk.bucket", "flour": "item.flour.pot", "egg": "item.egg"}
        self.assertEqual(len(orders), 6)
        for order in orders:
            stage = "stage.cooks.delivered.none"
            inventory, delivered = Counter(), set()
            for ingredient in order:
                inventory[item_ids[ingredient]] += 1
                choices = [choice for choice in nodes[stage]["choices"]
                           if evaluate(choice["guard"], stage, inventory)]
                self.assertEqual(len(choices), 1, (order, stage))
                for effect in choices[0]["effects"]:
                    if effect["kind"] == "take_items":
                        for item in effect["items"]:
                            inventory[item["item"]] -= item["quantity"]
                    elif effect["kind"] == "set_quest_stage":
                        stage = effect["stage"]
                    else:
                        self.fail("Partial delivery unexpectedly awards value")
                delivered.add(ingredient)
                expected = "stage.cooks.delivered." + "_".join(name for name in ("milk", "flour", "egg") if name in delivered)
                self.assertEqual(stage, expected)
            self.assertEqual(stage, "stage.cooks.delivered.milk_flour_egg")
            self.assertTrue(all(amount == 0 for amount in inventory.values()))

    def test_cook_precollected_batch_does_not_require_gather_flags(self):
        nodes = {node["id"]: node for node in self.content["dialogues"]["dialogue.cook"]["nodes"]}
        items = Counter({"item.milk.bucket": 1, "item.flour.pot": 1, "item.egg": 1})
        start = nodes["stage.cooks.not_started"]["choices"]
        self.assertTrue(any(choice["id"] == "transition.cooks.accept" and
                            evaluate(choice["guard"], "stage.cooks.not_started", items) for choice in start))
        choices = [choice for choice in nodes["stage.cooks.delivered.none"]["choices"]
                   if evaluate(choice["guard"], "stage.cooks.delivered.none", items)]
        self.assertEqual(len(choices), 1)
        self.assertEqual(choices[0]["id"], "transition.cooks.deliver.none_to_milk_flour_egg")

    def test_no_coin_reward_or_duplicate_completion_escape(self):
        nodes = {node["id"]: node for node in self.content["dialogues"]["dialogue.cook"]["nodes"]}
        ready = nodes["stage.cooks.delivered.milk_flour_egg"]["choices"][0]
        self.assertTrue(evaluate(ready["guard"], "stage.cooks.delivered.milk_flour_egg", Counter()))
        self.assertEqual(nodes["stage.cooks.completed"]["choices"], [])
        reward = ready["effects"][0]
        self.assertEqual(reward["kind"], "once")
        self.assertFalse(any(effect["kind"] == "give_items" for effect in reward["effects"]))
        self.assertIn({"kind": "add_quest_points", "amount": 1}, reward["effects"])
        self.assertIn({"kind": "award_xp", "rewards": [{"skill": "skill.cooking", "amount_tenths": 3000}]}, reward["effects"])
        model = Oracle(self.content)
        model.data["tutorial_stage"] = "stage.tutorial.mainland"
        model.data["quests"]["quest.cooks_assistant"]["stage"] = "stage.cooks.delivered.milk_flour_egg"
        model.data["run_energy"] = 120
        model.effects(ready["effects"])
        self.assertEqual(model.data["quest_points"], 1)
        self.assertEqual(model.data["skills"]["skill.cooking"]["xp_tenths"], 3000)
        self.assertEqual(model.data["run_energy"], 10000)
        self.assertFalse(model.guard(ready["guard"]))
        with self.assertRaisesRegex(OracleRefusal, "already claimed"):
            model.effects(ready["effects"])
        self.assertEqual(model.data["quest_points"], 1)

    def test_full_graphs_and_three_floor_mill_no_shortcut(self):
        self.assertEqual(len(self.graph["tutorial"]), 73)
        self.assertEqual(len(self.graph["cooks"]), 22)
        self.assertEqual(len(self.content["tutorial"]), 71)
        self.assertEqual(len(self.content["quests"]["quest.cooks_assistant"]["journal"]), 10)
        self.assertEqual(len(self.bindings["travel"]), 11)
        self.assertTrue(all(edge["runtime_hooks"] and not edge["gaps"] for edge in self.graph["tutorial"]))
        self.assertEqual(self.content["schema_version"], 2)
        self.assertFalse(any(recipe["outputs"] == [{"item": "item.flour.pot", "quantity": 1}]
                             for recipe in self.content["recipes"].values()))
        pairs = {record["id"]: record for record in self.bindings["travel"]}
        self.assertEqual([point[3] for point in pairs["mill_lower"]["source_endpoints"]], [0, 1])
        self.assertEqual([point[3] for point in pairs["mill_upper"]["source_endpoints"]], [1, 2])

    def test_real_shop_stock_not_fixture_inventory(self):
        shop = self.content["shops"]["shop.lumbridge.general_store"]
        self.assertEqual(shop["currency"], "item.coins")
        self.assertEqual(len(shop["stock"]), 15)
        stock = {item["item"]: item for item in shop["stock"]}
        self.assertEqual(stock["item.pot"]["buy_price"], 1)
        self.assertEqual(stock["item.bucket"]["buy_price"], 2)
        self.assertEqual(stock["item.bucket"]["base_stock"], 3)
        self.assertEqual(stock["item.hammer"]["restock_ticks"], 100)
        self.assertEqual(stock["item.bucket"]["mechanics"]["pricing"]["kind"], "stock_sensitive")

    def test_first_visible_bank_entitlement_is_atomic_and_never_reseeded(self):
        model = Oracle(self.content)
        self.assertEqual(model.count("item.coins", "bank"), 0)
        bank = next(action for action in self.content["spawns"]["spawn.tutorial.banker"]["interactions"]
                    if action["action"]["kind"] == "open_bank")
        model.data["tutorial_stage"] = "stage.tutorial.bank_open"
        self.assertFalse(model.event({"kind": "interface_opened", "interface": "interface.bank"}))
        model.effects(bank["action"]["before_open"])
        self.assertEqual(model.count("item.coins", "bank"), 25)
        self.assertTrue(model.event({"kind": "interface_presented", "interface": "interface.bank",
                                    "context": {"kind": "bank", "banker": "spawn.tutorial.banker"}}))
        model.take("item.coins", 25, "bank")
        model.give("item.coins", 25)
        model.effects(bank["action"]["before_open"])
        self.assertEqual(model.count("item.coins", "bank"), 0)
        self.assertEqual(model.count("item.coins"), 25)

    def test_partial_initial_supply_does_not_regrant_satisfied_dropped_line(self):
        model = Oracle(self.content)
        model.give("item.logs.normal", 27)
        grant = [{"kind": "grant", "grant": "grant.tutorial.melee_gear"}]
        model.effects(grant)
        self.assertEqual(model.count("item.sword.bronze"), 1)
        self.assertEqual(model.count("item.shield.wooden"), 0)
        self.assertFalse(model.grant_claimed("entitlement.tutorial.melee_gear"))
        model.take("item.sword.bronze", 1)
        model.effects(grant)
        self.assertEqual(model.count("item.sword.bronze"), 0)
        self.assertEqual(model.count("item.shield.wooden"), 1)
        self.assertTrue(model.grant_claimed("entitlement.tutorial.melee_gear"))
        model.take("item.logs.normal", 1)
        model.effects([{"kind": "grant", "grant": "grant.tutorial.melee_gear.recovery"}])
        self.assertEqual(model.count("item.sword.bronze"), 1)
        self.assertEqual(model.count("item.shield.wooden"), 1)

    def test_ranged_top_up_counts_equipped_arrows_and_atomic_failure_rolls_back(self):
        model = Oracle(self.content)
        model.data["equipment"]["slot.ammo"] = item_stack("item.arrow.bronze", 10)
        model.effects([{"kind": "grant", "grant": "grant.tutorial.ranged_gear"}])
        self.assertEqual(model.count("item.arrow.bronze", "inventory_and_equipment"), 50)
        model.effects([{"kind": "grant", "grant": "grant.tutorial.ranged_gear.recovery"}])
        self.assertEqual(model.count("item.arrow.bronze", "inventory_and_equipment"), 50)
        full = Oracle(self.content)
        full.give("item.logs.normal", 27)
        before = deepcopy(full.data)
        with self.assertRaisesRegex(OracleRefusal, "Container full"):
            full.effects([{"kind": "grant", "grant": "grant.tutorial.survival_tools"}])
        self.assertEqual(full.data, before)

    def test_real_mill_hopper_controls_bin_conserve_items_and_reject_overflow(self):
        model = Oracle(self.content)
        model.data["tutorial_stage"] = "stage.tutorial.mainland"
        def operation(object_id, action):
            return next(action_definition for spawn in self.content["spawns"].values()
                        if spawn["kind"].get("object") == object_id for action_definition in spawn["interactions"]
                        if action_definition["name"] == action)
        hopper = operation("object.mill.hopper", "Fill")
        controls = operation("object.mill.controls", "Operate")
        bin_action = operation("object.mill.flour_bin", "Empty")
        self.assertFalse(model.guard(hopper["guard"]))
        model.give("item.grain", 1)
        self.assertTrue(model.guard(hopper["guard"]))
        model.effects(hopper["action"]["effects"])
        self.assertEqual(model.count("item.grain"), 0)
        self.assertEqual(model.count("item.flour.pot"), 0)
        self.assertTrue(model.guard(controls["guard"]))
        model.effects(controls["action"]["effects"])
        self.assertFalse(model.guard(controls["guard"]))
        self.assertEqual(model.counter("counter.mill.flour"), counter_value(1))
        self.assertFalse(model.guard(bin_action["guard"]))
        model.give("item.pot", 1)
        self.assertTrue(model.guard(bin_action["guard"]))
        model.effects(bin_action["action"]["effects"])
        self.assertEqual(model.count("item.flour.pot"), 1)
        self.assertEqual(model.count("item.pot"), 0)
        self.assertEqual(model.counter("counter.mill.flour"), counter_value(0))
        model.data["runtime"]["counters"]["counter.mill.flour"] = counter_value(30)
        model.give("item.grain", 1)
        self.assertFalse(model.guard(hopper["guard"]))
        before = deepcopy(model.data)
        with self.assertRaisesRegex(OracleRefusal, "Counter overflow"):
            model.effects([{"kind": "add_counter", "counter": "counter.mill.flour", "delta": 1}])
        self.assertEqual(model.data, before)

    def test_learning_reward_requires_valid_selected_spell_not_kill_or_generic_hit(self):
        model = Oracle(self.content)
        model.data["tutorial_stage"] = "stage.tutorial.wind_strike"
        model.data["quests"]["quest.learning_the_ropes"]["stage"] = "stage.learning_the_ropes.in_progress"
        model.data["run_energy"] = 500
        target = "spawn.tutorial_chicken.3138.3093.p0"
        self.assertFalse(model.event({"kind": "hit", "target": target, "damage": 2, "style": "magic"}))
        self.assertFalse(model.event({"kind": "npc_killed", "target": target, "npc": "npc.tutorial_chicken",
                                      "life": 1, "method": "magic", "credited": True, "tile": {"x": 3138, "y": 3093, "plane": 0}}))
        event = {"kind": "spell_resolved", "spell": "spell.wind_strike", "target": target,
                 "outcome": "invalidated", "damage": 0, "tile": {"x": 3138, "y": 3093, "plane": 0}}
        self.assertFalse(model.event(event))
        event["outcome"] = "splash"
        self.assertTrue(model.event(event))
        self.assertEqual(model.data["quest_points"], 1)
        self.assertEqual(model.data["run_energy"], 10000)
        self.assertEqual(model.data["tutorial_stage"], "stage.tutorial.departure_offer")
        self.assertFalse(model.event(event))
        self.assertEqual(model.data["quest_points"], 1)

    def test_burnt_shrimp_is_an_authoritative_failed_recipe_not_a_stage_shortcut(self):
        model = Oracle(self.content)
        model.data["tutorial_stage"] = "stage.tutorial.cook_shrimp"
        self.assertFalse(model.event({"kind": "produced", "recipe": "recipe.cooking.shrimps.fire",
                                      "outputs": [item_stack("item.shrimps.burnt")]}))
        self.assertTrue(model.event({
            "kind": "production_resolved", "recipe": "recipe.cooking.shrimps.fire",
            "method": "action.cooking.shrimps", "facility": {"kind": "temporary_object", "object": "dynamic_object.oracle.fire"},
            "outcome": "failure", "outputs": [item_stack("item.shrimps.burnt")],
        }))
        self.assertEqual(model.data["tutorial_stage"], "stage.tutorial.survival_exit")
        self.assertEqual(model.data["skills"]["skill.cooking"]["xp_tenths"], 0)

    def test_death_topics_gate_portal_and_do_not_use_fake_quest_stages(self):
        model = Oracle(self.content)
        model.data["life"] = "first_death_office"
        death = self.content["dialogues"]["dialogue.death"]
        intro = next(node for node in death["nodes"] if node["id"] == "stage.death.introduction")["choices"][0]
        model.effects(intro["effects"])
        topics = next(node for node in death["nodes"] if node["id"] == "stage.death.topics")["choices"]
        done = topics[-1]
        self.assertFalse(model.guard(done["guard"]))
        for choice in topics[:3]:
            model.effects(choice["effects"])
        self.assertTrue(model.guard(done["guard"]))
        model.effects(done["effects"])
        self.assertTrue(model.guard(self.content["mechanics"]["travels"]["travel.death.exit"]["guard"]))
        self.assertEqual(model.data["tutorial_stage"], "stage.tutorial.appearance")
        self.assertEqual(set(model.data["quests"]), {"quest.cooks_assistant", "quest.learning_the_ropes"})
        self.assertEqual(self.content["mechanics"]["death"]["first_office"]["value"]["tile"],
                         {"x": 3174, "y": 5726, "plane": 0})
        self.assertNotEqual(self.content["spawns"]["spawn.death"]["tile"],
                            self.content["mechanics"]["death"]["first_office"]["value"]["tile"])

    def test_reject_dangling_ids(self):
        content = deepcopy(self.content)
        content["recipes"]["recipe.cooking.dough"]["outputs"][0]["item"] = "item.unknown"
        with self.assertRaisesRegex(ValueError, "Dangling item"):
            check_content(content, self.inputs, self.world, self.bindings)

    def test_reject_fixture_provenance(self):
        content = deepcopy(self.content)
        content["items"]["item.pickaxe.bronze"]["source"][0]["status"] = "test_fixture"
        with self.assertRaisesRegex(ValueError, "Fixture/unknown"):
            check_content(content, self.inputs, self.world, self.bindings)

    def test_reject_relocated_object_or_cleared_source_wall(self):
        content = deepcopy(self.content)
        cell = content["regions"]["region.osrs.12336"]["cells"][0]
        cell["blocked_movement"] ^= 1
        with self.assertRaisesRegex(ValueError, "differs from actual source"):
            check_content(content, self.inputs, self.world, self.bindings)


if __name__ == "__main__":
    unittest.main()
