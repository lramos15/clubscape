"""Source-application/approval/replay invariants; these are not production journey claims."""

from copy import deepcopy
import gzip
import json
import subprocess
import unittest

from common import CONTENT, ROOT, Inputs, load
from runtime_application import CONTEXT, RESOLUTIONS, classify_residuals, source_applier
from state_oracles import Oracle, OracleUnresolved
from verify_runtime_bindings import verify


class RuntimeApplicationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.content = load(CONTENT / "game-content.json.gz")
        cls.report = load(RESOLUTIONS)
        cls.context = load(CONTEXT)
        cls.applier = source_applier()
        cls.parent = json.loads(gzip.decompress(subprocess.check_output(
            ["git", "show", cls.context["parent_base"] + ":content/m1/game-content.json.gz"], cwd=ROOT)))

    def test_all_source_bindings_coupled_updates_approved_loot_and_currency_oracles(self):
        result = verify()
        self.assertEqual(result["source_bindings_consumed"], 104)
        self.assertEqual(result["source_bindings_checked"]["source_supported_inference"], 98)
        self.assertEqual(result["coupled_updates_checked"], 7)
        self.assertEqual(result["loot"]["joint_primary_potion_vectors"], 8192)
        self.assertEqual(len(result["departure"]["currency_vectors"]), 5)

    def test_exact_parent_table_is_preserved_without_reapplying_old_placeholder(self):
        candidate, audit = self.applier.prepare_with_audit(self.parent, self.report, self.context)
        old = self.parent["mechanics"]["combat_styles"]["style.magic.wind_strike"]["maximum_hit"]
        self.assertEqual(candidate["mechanics"]["combat_styles"]["style.magic.wind_strike"]["maximum_hit"], old)
        self.assertEqual(sum(row["action"] == "already_applied" for row in audit["bindings"]), 1)
        self.assertEqual(sum(row["action"] == "applied" for row in audit["bindings"]), 103)
        repeated, again = self.applier.prepare_with_audit(candidate, self.report, self.context)
        self.assertEqual(candidate, repeated)
        self.assertEqual(sum(row["action"] == "already_applied" for row in again["bindings"]), 104)
        self.assertTrue(all(row["action"] == "already_applied" for row in again["coupled_updates"]))

    def test_changed_target_fingerprint_is_rejected(self):
        content = deepcopy(self.parent)
        content["mechanics"]["projectiles"]["projectile.bronze_arrow"]["timing"]["reason"] += " changed"
        with self.assertRaisesRegex(ValueError, "Input binding changed"):
            self.applier.prepare(content, self.report, self.context)

    def test_different_bound_wind_semantics_are_rejected(self):
        content = deepcopy(self.parent)
        content["mechanics"]["combat_styles"]["style.magic.wind_strike"]["maximum_hit"]["value"]["hits"]["9"] = 7
        with self.assertRaisesRegex(ValueError, "Input binding changed"):
            self.applier.prepare(content, self.report, self.context)

    def test_asset_provenance_exception_does_not_ignore_changed_notes(self):
        content = deepcopy(self.parent)
        content["npcs"]["npc.cook"]["navigation"]["step_ticks"]["source"][0]["notes"] += " modified"
        with self.assertRaisesRegex(ValueError, "Input binding changed"):
            self.applier.prepare(content, self.report, self.context)

    def test_coupled_mutation_rejected_and_original_left_unchanged(self):
        content = deepcopy(self.parent)
        content["mechanics"]["travels"]["travel.tutorial.departure"]["completion_effects"] = []
        before = deepcopy(content)
        with self.assertRaisesRegex(ValueError, "Coupled field changed"):
            self.applier.prepare(content, self.report, self.context)
        self.assertEqual(content, before)

    def test_no_source_resolution_reclassification_by_modified_report(self):
        report = deepcopy(self.report)
        row = report["resolutions"]["mechanics.death.office_overflow"]
        row["apply_to_ordinary_profile"] = True
        with self.assertRaisesRegex(ValueError, "different source-resolution report"):
            self.applier.prepare(self.parent, report, self.context)

    def test_three_dose_source_identity_and_death_value_are_not_substitutes(self):
        inputs = Inputs()
        raw = inputs.collections["item"][3010]
        self.assertEqual(raw["name"], "Energy potion(3)")
        self.assertEqual(raw["inventoryModel"], 2697)
        definition = self.content["items"]["item.energy_potion.three_dose"]
        self.assertEqual(definition["source_id"], 3010)
        self.assertEqual(definition["noted_variant"], "item.energy_potion.three_dose.noted")
        self.assertEqual(self.content["items"]["item.energy_potion.three_dose.noted"]["source_id"], 3011)
        self.assertIsNone(definition["asset"])
        values = self.content["mechanics"]["value_providers"]["value_provider.osrs.death"]["values"]["value"]
        self.assertEqual(values["item.energy_potion.three_dose"], 123)
        self.assertEqual(values["item.energy_potion.three_dose.noted"], 123)
        self.assertNotEqual(values["item.energy_potion.three_dose"], 108)
        self.assertIn("item.energy_potion.three_dose", self.content["mechanics"]["death"]["repeat"]["value"]["supply_items"])

    def test_inactive_dependency_proofs_are_scoped_and_not_blanket_exemptions(self):
        residuals = classify_residuals(self.content, self.report)
        self.assertEqual(residuals["inactive_full_target_count"], 6)
        changed = deepcopy(self.content)
        changed["spawns"]["spawn.tutorial.fishing_spot.3099.3090.p0"]["interactions"][0]["action"]["rule"]["depletion"]["numerator_at_level_1"] = 1
        with self.assertRaisesRegex(ValueError, "Nondepleting fishing proof"):
            classify_residuals(changed, self.report)
        changed = deepcopy(self.content)
        for index in range(3):
            definition = deepcopy(changed["items"]["item.tinderbox"])
            definition["id"] = f"item.capacity_oracle_{index}"
            changed["items"][definition["id"]] = definition
        with self.assertRaisesRegex(ValueError, "Item universe exceeds"):
            classify_residuals(changed, self.report)

    def test_nonvital_xp_never_requires_the_conditional_vital_policy(self):
        model = Oracle(self.content)
        model.data["tutorial_stage"] = "stage.tutorial.mainland"
        model.effects([{"kind": "award_xp", "rewards": [{"skill": "skill.cooking", "amount_tenths": 3000}]}])
        self.assertEqual(model.data["skills"]["skill.cooking"]["xp_tenths"], 3000)
        self.assertEqual(model.data["hitpoints"], 10)
        self.assertEqual(model.data["prayer_points"], 1)

    def test_vital_conditional_rule_uses_only_the_actual_declared_binding(self):
        policy = self.content["mechanics"]["vitals"]["level_up"]
        for current, expected in ((5, 5), (10, 11), (15, 15)):
            model = Oracle(self.content)
            model.data["tutorial_stage"] = "stage.tutorial.mainland"
            model.data["hitpoints"] = current
            amount = self.content["skills"]["skill.hitpoints"]["xp_thresholds_tenths"][10] - model.data["skills"]["skill.hitpoints"]["xp_tenths"]
            effect = [{"kind": "award_xp", "rewards": [{"skill": "skill.hitpoints", "amount_tenths": amount}]}]
            if policy["status"] == "bound":
                model.effects(effect)
                self.assertEqual(model.data["hitpoints"], expected)
            else:
                before = deepcopy(model.data)
                with self.assertRaises(OracleUnresolved):
                    model.effects(effect)
                self.assertEqual(model.data, before)


if __name__ == "__main__":
    unittest.main()
