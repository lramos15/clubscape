"""Exact source water addition, not a grant, source observation or migration approval."""

from copy import deepcopy
import unittest

from common import CONTENT, ROOT, Inputs, canonical, load, sha
from water_fill import (
    DIRECTORY, OBJECT, RECIPE, SINK, baseline_content, baselines, contact_candidates,
    recipe_definition, semantic_hash, verify_delta, without_water,
)


class WaterFillSourceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.content = load(CONTENT / "game-content.json.gz")
        cls.facts = load(DIRECTORY / "facts.json")

    def test_literal_source_conversion_not_construction_or_humidify(self):
        recipe = self.content["recipes"][RECIPE]
        self.assertEqual(recipe, recipe_definition())
        self.assertEqual(recipe["inputs"], [{"item": "item.bucket", "quantity": 1, "instance": None}])
        self.assertEqual(recipe["outputs"], [{"item": "item.water.bucket", "quantity": 1, "instance": None}])
        for field in ("failed_outputs", "tools", "requirements", "xp"):
            self.assertEqual(recipe[field], [])
        self.assertEqual(recipe["target_objects"], [OBJECT])
        target = recipe["item_on_target"]
        self.assertEqual(target["status"], "bound")
        self.assertEqual(target["value"], {
            "reach": 1, "guard": {"kind": "tutorial_stage", "stage": "stage.tutorial.mainland"},
        })
        self.assertTrue(any(row["status"] == "inference" for row in target["source"]))
        self.assertEqual(recipe["mechanics"]["lifecycle"], {"kind": "inventory_conversion"})
        self.assertEqual(recipe["mechanics"]["guard"], {"kind": "tutorial_stage", "stage": "stage.tutorial.mainland"})
        self.assertEqual([entry["page"] for entry in self.facts["excluded_nonmatching_sources"]],
                         ["Sink", "Money making guide/Filling buckets with water"])

    def test_per_conversion_tick_is_source_defined_and_phase_interpretation_is_labeled(self):
        cadence = self.content["recipes"][RECIPE]["mechanics"]["cadence"]
        self.assertEqual([cadence[name]["value"] for name in ("single", "first", "repeat", "menu_delay")], [1, 1, 1, 0])
        self.assertEqual(self.facts["source_recipe_parameters"]["ticks"], "1")
        self.assertEqual(self.facts["seconds_per_conversion"], "0.6")
        self.assertTrue(any(row["status"] == "inference" for row in cadence["first"]["source"]))
        self.assertTrue(any(row["status"] == "inference" for row in cadence["repeat"]["source"]))
        self.assertFalse(self.facts["source_observation_claimed"])

    def test_empty_sink_object_menu_and_original_definition_are_preserved(self):
        inputs = Inputs()
        raw = inputs.collections["object"][14868]
        self.assertEqual(raw["ops"], {"ops": [], "subOps": [], "conditionalOps": [], "conditionalSubOps": []})
        self.assertEqual(self.content["spawns"][SINK]["interactions"], [])
        self.assertEqual(sha(canonical(raw)), self.facts["sink"]["definition_sha256"])
        self.assertEqual(self.content["objects"][OBJECT]["asset"], "asset.source.osrs.cache2695.object.14868")

    def test_actual_footprint_and_walls_produce_three_nonarbitrary_contact_candidates(self):
        self.assertEqual(contact_candidates(self.content), [
            {"x": 3205, "y": 3214, "plane": 0},
            {"x": 3205, "y": 3217, "plane": 0},
            {"x": 3206, "y": 3216, "plane": 0},
        ])
        cells = {tuple(cell["tile"][axis] for axis in ("x", "y", "plane")): cell
                 for region in self.content["regions"].values() for cell in region["cells"]}
        self.assertFalse(cells[(3206, 3215, 0)]["walkable"])
        self.assertTrue(cells[(3204, 3216, 0)]["walkable"])
        self.assertNotIn({"x": 3204, "y": 3216, "plane": 0}, contact_candidates(self.content))

    def test_current_product_changes_exactly_one_recipe_and_revision(self):
        proof = verify_delta(self.content)
        self.assertEqual(proof["profile"], "current5b")
        self.assertEqual(set(proof["changed_json_paths"]),
                         {"/revision", "/recipes/" + RECIPE, "/ui/actor_animations/recipes/" + RECIPE})
        original = baseline_content("current5b")
        projected = without_water(self.content)
        projected["revision"] = original["revision"]
        self.assertEqual(projected, original)
        self.assertEqual(len(self.content["tutorial"]), 71)
        self.assertEqual(len(self.content["quests"]["quest.cooks_assistant"]["journal"]), 10)

    def test_all_later_ui_audio_actor_bindings_and_three_prior_unknowns_are_unchanged(self):
        original = baseline_content("current5b")
        self.assertEqual(without_water(self.content)["ui"], original["ui"])
        self.assertEqual(self.content["interfaces"], original["interfaces"])
        proof = verify_delta(self.content)
        self.assertEqual(proof["preexisting_source_unknowns_preserved"],
                         ["normal_grave_bank_all", "recipe.cooking.dough.motion", "empty_container.motion"])
        self.assertEqual(proof["water_actor_motion"]["classification"], "unresolved_source")
        self.assertIsNone(proof["water_actor_motion"]["sequence"])
        self.assertFalse(proof["water_actor_motion"]["no_animation_verified"])
        self.assertEqual(self.content["ui"]["actor_animations"]["recipes"][RECIPE]["rule"]["kind"], "unverified")

    def test_a_fabricated_fill_menu_or_unrelated_geometry_policy_change_is_rejected(self):
        modified = deepcopy(self.content)
        modified["spawns"][SINK]["interactions"] = [{
            "name": "Fill", "reach": 1, "guard": {"kind": "always"},
            "action": {"kind": "production", "recipes": [RECIPE]},
        }]
        with self.assertRaisesRegex(ValueError, "unrelated source"):
            verify_delta(modified)
        for path in (("initial_state", "quest_points"), ("initial_state", "run_energy")):
            modified = deepcopy(self.content)
            modified[path[0]][path[1]] += 1
            with self.assertRaisesRegex(ValueError, "unrelated source"):
                verify_delta(modified)
        modified = deepcopy(self.content)
        modified["regions"]["region.osrs.12850"]["cells"][0]["blocked_movement"] ^= 1
        with self.assertRaisesRegex(ValueError, "unrelated source"):
            verify_delta(modified)

    def test_recipe_source_and_amount_mutations_cannot_hide_in_projection(self):
        for field, value in (("xp", [{"skill": "skill.construction", "amount_tenths": 3000}]),
                             ("target_objects", []), ("item_on_target", None),
                             ("item_on_target", {"status": "bound", "value": {"reach": 2, "guard": {"kind": "always"}}, "source": []})):
            modified = deepcopy(self.content)
            modified["recipes"][RECIPE][field] = value
            with self.assertRaisesRegex(ValueError, "exact audited definition"):
                without_water(modified)

    def test_legacy_profile_keeps_its_own_ui_and_other_source_values(self):
        original = baseline_content("legacy5e")
        candidate = deepcopy(original)
        candidate["revision"] += ".water.test"
        candidate["recipes"][RECIPE] = recipe_definition()
        proof = verify_delta(candidate)
        self.assertEqual(proof["profile"], "legacy5e")
        self.assertNotEqual(candidate["ui"], self.content["ui"])
        self.assertEqual(candidate["ui"], original["ui"])
        self.assertEqual(candidate["interfaces"], original["interfaces"])
        self.assertEqual(semantic_hash(without_water(candidate)), baselines()["profiles"]["legacy5e"]["semantic_sha256_without_revision"])

    def test_public_baseline_inputs_are_exact_immutable_hashes(self):
        for record in baselines()["profiles"].values():
            self.assertEqual(sha((ROOT / record["input"]).read_bytes()), record["source_sha256"])


if __name__ == "__main__":
    unittest.main()
