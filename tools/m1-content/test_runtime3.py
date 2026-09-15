"""Source-content v3 selector invariants; controlled states are not a fresh gameplay journey."""

from copy import deepcopy
import unittest

from common import BINDINGS, CONTENT, Inputs, counter_value, item_stack, load, position
from runtime_application import RESOLUTIONS, classify_residuals
from state_oracles import Oracle, OracleRefusal
from verify_routes import (
    ContentCollisionMap, check_group_products, check_group_source_cells, check_travel_destinations,
)


class Runtime3Tests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.content = load(CONTENT / "game-content.json.gz")
        cls.mechanics = cls.content["mechanics"]
        cls.inputs = Inputs()
        cls.doors = load(BINDINGS / "door-bindings.json")
        cls.policy = load(BINDINGS / "runtime3-policy.json")

    def model(self, stage, tile=None):
        model = Oracle(self.content)
        model.data["tutorial_stage"] = stage
        if tile is not None:
            model.data["tile"] = dict(zip(("x", "y", "plane"), tile, strict=True))
        return model

    def eligible(self, model, npc, method, style=None):
        rules = self.content["npcs"][npc]["combat"]["mechanics"]["eligibility"]["value"]
        return any(rule["method"] == method and (rule["style"] is None or rule["style"] == style)
                   and model.guard(rule["guard"]) for rule in rules)

    def test_v3_and_every_new_selector_have_explicit_bound_values(self):
        self.assertEqual(self.content["schema_version"], 3)
        for name in ("unarmed", "engagement"):
            self.assertEqual(self.mechanics["player_combat"][name]["status"], "bound")
        for npc in self.content["npcs"].values():
            if npc["combat"]:
                for name in ("loot_ground_policy", "eligibility", "engagement", "attribution"):
                    self.assertEqual(npc["combat"]["mechanics"][name]["status"], "bound")
        residuals = classify_residuals(self.content, load(RESOLUTIONS))
        self.assertEqual(residuals["active_or_conditionally_active_count"], 0)
        self.assertEqual(residuals["inactive_full_target_count"], 6)
        self.assertEqual(self.mechanics["vitals"]["level_up"]["value"],
                         "raise_if_at_old_base_otherwise_preserve")

    def test_unarmed_and_weapon_defaults_are_explicit_offered_styles(self):
        unarmed = self.mechanics["player_combat"]["unarmed"]["value"]
        self.assertEqual(unarmed["default_style"], "style.unarmed.punch")
        self.assertEqual(set(unarmed["styles"]), {"style.unarmed.punch", "style.unarmed.kick", "style.unarmed.block"})
        self.assertIsNone(unarmed["ammunition"])
        for identifier in unarmed["styles"]:
            style = self.mechanics["combat_styles"][identifier]
            self.assertEqual((style["method"], style["attack_type"], style["reach"], style["cycle_ticks"]["value"]),
                             ("melee", "crush", 1, 4))
        for item in self.content["items"].values():
            if item["equipment"] and item["equipment"]["weapon"]:
                weapon = item["equipment"]["weapon"]
                self.assertIn(weapon["default_style"], weapon["styles"])
                self.assertTrue(set(weapon["styles"]).issubset(self.mechanics["combat_styles"]))
        self.assertEqual(self.content["items"]["item.axe.bronze"]["equipment"]["weapon"]["default_style"],
                         "style.axe.bronze.slash.accurate")

    def test_stage_and_npc_ground_origins_are_not_conflated(self):
        selector = self.mechanics["player_drop"]
        self.assertEqual(set(selector["stages"]), set(self.content["tutorial"]) - {"stage.tutorial.mainland"})
        for stage, selected in selector["stages"].items():
            self.assertEqual(selected["status"], "bound")
            policy = self.mechanics["ground_policies"][selected["value"]]
            self.assertIsNone(policy["public_after"]["value"])
            self.assertEqual(policy["expires_after"]["value"], 50)
            self.assertEqual(policy["clock"]["value"], "world_ticks")
        ordinary = self.mechanics["ground_policies"][selector["ordinary"]["value"]]
        self.assertEqual((ordinary["public_after"]["value"], ordinary["expires_after"]["value"]), (100, 300))
        self.assertEqual(ordinary["clock"]["value"], "world_ticks")
        fresh = self.mechanics["ground_policies"][selector["before_playtime"]["value"]["ground_policy"]]
        self.assertIsNone(fresh["public_after"]["value"])
        self.assertEqual(fresh["expires_after"]["value"], 300)
        self.assertEqual(fresh["clock"]["value"], "owner_online_ticks")
        self.assertEqual(selector["untradeable"]["value"], fresh["id"])
        self.assertEqual(selector["before_playtime"]["value"]["played_ticks_below"] * 600, 72000 * 1000)
        for identifier in ("npc.tutorial_rat", "npc.tutorial_chicken", "npc.goblin.level_2"):
            policy = self.content["npcs"][identifier]["combat"]["mechanics"]["loot_ground_policy"]["value"]
            self.assertEqual(policy, "ground_policy.npc_loot" if identifier == "npc.goblin.level_2" else "ground_policy.tutorial")
        scope = self.policy["fresh_normal_manual_drop_profile"]
        self.assertEqual(scope["expiry_clock"], "owner_online_ticks")
        self.assertFalse(scope["post_threshold_release_verified"])

    def test_every_ground_clock_is_explicit_and_distinguishes_owner_online_from_world_time(self):
        for identifier, policy in self.mechanics["ground_policies"].items():
            self.assertEqual(policy["clock"]["status"], "bound")
            expected = "owner_online_ticks" if identifier in (
                "ground_policy.death_supplies", "ground_policy.player_drop.m1_fresh",
            ) else "world_ticks"
            self.assertEqual(policy["clock"]["value"], expected)

    def test_typed_rat_eligibility_checks_actor_stage_method_position_and_prior_kill(self):
        pen = tuple(self.doors["rat_pen_cells"][0])
        outside = (3105, 9509, 0)
        self.assertNotIn(list(outside), self.doors["rat_pen_cells"])
        melee = self.model("stage.tutorial.melee_rat", pen)
        self.assertTrue(self.eligible(melee, "npc.tutorial_rat", "melee"))
        self.assertFalse(self.eligible(melee, "npc.tutorial_rat", "ranged"))
        self.assertFalse(self.eligible(self.model("stage.tutorial.melee_rat", outside), "npc.tutorial_rat", "melee"))
        melee.data["runtime"]["counters"]["counter.tutorial.melee_kill"] = counter_value(True)
        self.assertFalse(self.eligible(melee, "npc.tutorial_rat", "melee"))
        ranged = self.model("stage.tutorial.ranged_rat", outside)
        self.assertTrue(self.eligible(ranged, "npc.tutorial_rat", "ranged"))
        self.assertFalse(self.eligible(self.model("stage.tutorial.ranged_rat", pen), "npc.tutorial_rat", "ranged"))
        ranged.data["runtime"]["counters"]["counter.tutorial.ranged_kill"] = counter_value(True)
        self.assertFalse(self.eligible(ranged, "npc.tutorial_rat", "ranged"))
        magic = self.model("stage.tutorial.wind_strike", outside)
        self.assertTrue(self.eligible(magic, "npc.tutorial_rat", "magic", "style.magic.wind_strike"))
        self.assertFalse(self.eligible(magic, "npc.tutorial_rat", "magic", "style.unarmed.punch"))

    def test_chicken_requires_the_actual_wind_strike_lesson_and_goblin_requires_mainland(self):
        model = self.model("stage.tutorial.wind_strike")
        self.assertTrue(self.eligible(model, "npc.tutorial_chicken", "magic", "style.magic.wind_strike"))
        self.assertFalse(self.eligible(model, "npc.tutorial_chicken", "ranged"))
        self.assertFalse(self.eligible(self.model("stage.tutorial.melee_rat"), "npc.tutorial_chicken",
                                       "magic", "style.magic.wind_strike"))
        model.data["runtime"]["counters"]["counter.tutorial.chicken_cast"] = counter_value(True)
        self.assertFalse(self.eligible(model, "npc.tutorial_chicken", "magic", "style.magic.wind_strike"))
        for method in ("melee", "ranged", "magic"):
            self.assertFalse(self.eligible(model, "npc.goblin.level_2", method))
            self.assertTrue(self.eligible(self.model("stage.tutorial.mainland"), "npc.goblin.level_2", method))

    def test_engagement_and_mixed_method_attribution_remain_source_inferences(self):
        player = self.mechanics["player_combat"]["engagement"]
        self.assertEqual(player["value"], {"combat_state_ticks": 9, "logout_lock_ticks": 16, "travel_lock_ticks": 9})
        self.assertTrue(all(source["status"] == "inference" for source in player["source"]))
        for npc in self.content["npcs"].values():
            if not npc["combat"]:
                continue
            mechanics = npc["combat"]["mechanics"]
            self.assertEqual(mechanics["attribution"]["value"], "most_damage_then_first_method")
            self.assertEqual(mechanics["engagement"]["value"], self.policy["npc_engagement"])
            self.assertIsNone(mechanics["engagement"]["value"]["aggression"])
            self.assertFalse(mechanics["engagement"]["value"]["reset_life_on_return"])

    def test_opening_a_shared_tutorial_exit_does_not_grant_another_actor_permission(self):
        tested = 0
        for traversal in self.mechanics["traversal"].values():
            if traversal["id"].endswith((".enter", ".exit")):
                continue
            opener = self.model("stage.tutorial.mainland")
            follower = self.model("stage.tutorial.mainland")
            self.assertFalse(follower.guard(traversal["guard"]))
            def complete(guard):
                if guard["kind"] == "counter":
                    opener.data["runtime"]["counters"][guard["counter"]] = counter_value(True)
                else:
                    self.assertEqual(guard["kind"], "all")
                    for child in guard["guards"]:
                        complete(child)
            complete(traversal["guard"])
            self.assertTrue(opener.guard(traversal["guard"]))
            self.assertFalse(follower.guard(traversal["guard"]))
            for edge in traversal["edges"]:
                before, after = position(edge["from"]), position(edge["to"])
                self.assertEqual(before[2], after[2])
                self.assertEqual(abs(before[0] - after[0]) + abs(before[1] - after[1]), 1)
            tested += 1
        self.assertEqual(tested, 7)

    def test_rat_pen_entry_is_directional_but_exit_is_not_progression_locked(self):
        enter = next(value for key, value in self.mechanics["traversal"].items() if key.endswith(".enter"))
        leave = next(value for key, value in self.mechanics["traversal"].items() if key.endswith(".exit"))
        self.assertEqual(leave["guard"], {"kind": "always"})
        for edge in enter["edges"]:
            self.assertFalse(edge["bidirectional"])
            self.assertNotIn(list(position(edge["from"])), self.doors["rat_pen_cells"])
            self.assertIn(list(position(edge["to"])), self.doors["rat_pen_cells"])
        for stage in self.content["tutorial"]:
            allowed = stage in ("stage.tutorial.enter_rat_pen", "stage.tutorial.melee_rat")
            self.assertEqual(self.model(stage).guard(enter["guard"]), allowed)
            self.assertTrue(self.model(stage).guard(leave["guard"]))

    def test_all_combined_states_reconstruct_original_object_clipping(self):
        self.assertEqual(check_group_products(self.content), (136, 38))
        self.assertGreater(check_group_source_cells(self.content, self.inputs), 500)
        collision = ContentCollisionMap(self.content)
        original = deepcopy(collision.cells)
        collision.select_transforms(dict.fromkeys(self.mechanics["object_transforms"], "object_state.open"))
        self.assertNotEqual(collision.cells, original)
        collision.select_transforms({key: value["initial"] for key, value in self.mechanics["object_transforms"].items()})
        self.assertEqual(collision.cells, original)

    def test_combined_cell_corruption_and_incomplete_state_products_are_rejected(self):
        changed = deepcopy(self.content)
        group = next(iter(changed["mechanics"]["collision_groups"].values()))
        group["states"]["value"][0]["collision"][0]["blocked_sight"] ^= 1
        with self.assertRaisesRegex(ValueError, "independent source object insertion"):
            check_group_source_cells(changed, self.inputs)
        group["states"]["value"].pop()
        with self.assertRaisesRegex(ValueError, "exact transform state product"):
            check_group_products(changed)

    def test_source_player_arrivals_are_not_nonwalking_npc_anchors(self):
        self.assertEqual(len(check_travel_destinations(self.content)), 28)
        cells = ContentCollisionMap(self.content)
        for identifier in ("spawn.death", "spawn.tutorial.fishing_spot.3099.3090.p0"):
            self.assertFalse(cells.cell(**self.content["spawns"][identifier]["tile"])["walkable"])
        arrival = self.mechanics["death"]["first_office"]["value"]["tile"]
        self.assertTrue(cells.cell(**arrival)["walkable"])
        self.assertNotEqual(arrival, self.content["spawns"]["spawn.death"]["tile"])

    def test_mill_all_valid_player_selectors_keep_source_clipping_and_reject_invalid_values(self):
        parent = self.content["objects"]["object.mill.flour_bin"]
        self.assertEqual(set(parent["morph"]["variants"]), set(map(str, range(31))))
        self.assertEqual(parent["morph"]["fallback"], "object.mill.flour_bin.empty")
        self.assertIsNone(parent["morph"]["collision"])
        for variant in set(parent["morph"]["variants"].values()):
            for field in ("size_x", "size_y", "clip"):
                self.assertEqual(self.content["objects"][variant][field], parent[field])
        for value in (-1, 31):
            model = self.model("stage.tutorial.mainland")
            with self.assertRaisesRegex(OracleRefusal, "Counter overflow"):
                model.effects([{"kind": "set_counter", "counter": "counter.mill.flour", "value": counter_value(value)}])

    def test_selected_single_and_make_x_one_retain_different_source_phases(self):
        cadence = self.content["recipes"]["recipe.cooking.bread.lumbridge_range"]["mechanics"]["cadence"]
        self.assertEqual([cadence[key]["value"] for key in ("single", "first", "repeat")], [1, 3, 4])
        for stage in self.content["tutorial"].values():
            if "produce" in stage["allowed_actions"]:
                self.assertIn("produce_selected", stage["allowed_actions"])

    def test_consume_only_burial_is_not_a_fake_output_conversion(self):
        for suffix, item in (("ordinary", "item.bones"), ("tutorial", "item.bones.tutorial")):
            recipe = self.content["recipes"]["recipe.prayer.bones." + suffix]
            self.assertEqual(recipe["inputs"], [item_stack(item)])
            self.assertEqual(recipe["outputs"], [])
            self.assertEqual(recipe["failed_outputs"], [])
            self.assertEqual(recipe["mechanics"]["lifecycle"], {"kind": "consume_only"})
            self.assertEqual(recipe["xp"], [{"skill": "skill.prayer", "amount_tenths": 45}])
            self.assertEqual(recipe["mechanics"]["cadence"]["single"]["value"], 2)

    def test_contextual_recovery_and_source_death_phases_are_explicit(self):
        death = self.mechanics["death"]
        self.assertEqual(death["timing"]["value"], {"dying_ticks": 2, "respawn_ticks": 4})
        self.assertEqual(death["interfaces"], {"grave": "interface.grave", "office": "interface.death_retrieval"})
        for interface in death["interfaces"].values():
            self.assertEqual(self.content["interfaces"][interface]["access"], "contextual")
            self.assertNotIn(interface, self.content["initial_state"]["interfaces"])
        self.assertIn("open_grave", self.content["tutorial"]["stage.tutorial.mainland"]["allowed_actions"])
        self.assertIn("open_death_office", self.content["tutorial"]["stage.tutorial.mainland"]["allowed_actions"])


if __name__ == "__main__":
    unittest.main()
