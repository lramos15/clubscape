"""Regression gates for the bounded original-asset refresh, not source gameplay acceptance."""

from copy import deepcopy
import unittest
from unittest.mock import patch

from common import BINDINGS, CONTENT, ROOT, Inputs, canonical, load, sha
from verify_assets import BASELINE, CATALOG_SHA256, behavior_projection, verify


class AssetRefreshTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.inputs = Inputs()
        cls.content = load(CONTENT / "game-content.json.gz")
        cls.baseline = load(BASELINE)
        application = BINDINGS / "application-result.json"
        cls.application = load(application) if application.exists() else None

    def test_complete_exact_original_asset_closure_and_hashes(self):
        report = verify()
        self.assertEqual(report["resolved"], {"item_definition_ids": 72, "model_ids": 68,
                                              "npc_definition_ids": 6, "interface_groups": 13})
        self.assertEqual(report["product_assets_resolved"], 5257)
        self.assertEqual(report["merged_inventory_records"], 12400)
        self.assertEqual(report["new_original_assets"], 272)
        self.assertEqual(report["new_original_outputs"], 1137)
        self.assertEqual(report["remaining_missing_inputs"], [])

    def test_new_definitions_use_the_correct_additive_collection_shard(self):
        for kind, number in (("npc", 4626), ("npc", 4628), ("item", 2347), ("item", 1171)):
            source = self.inputs.definition_source(kind, number)[0]["reference"]
            self.assertEqual(source, f"assets/source/osrs/cache2695/content-v2/collections/{kind}.json.gz#"
                                    f"asset.source.osrs.cache2695.{kind}.{number}")
        self.assertEqual(self.inputs.definition_source("item", 1265)[0]["reference"],
                         "assets/source/osrs/cache2695/collections/item.json.gz#asset.source.osrs.cache2695.item.1265")

    def test_no_remaining_nullable_definition_asset_or_guessed_model(self):
        for category, kind in (("items", "item"), ("npcs", "npc"), ("objects", "object")):
            for identifier, definition in self.content[category].items():
                if self.application and identifier in self.application["item_extensions"]:
                    self.assertIsNotNone(definition["asset"])
                    self.assertEqual(definition["source_id"], self.application["item_extensions"][identifier]["source_id"])
                    continue
                self.assertEqual(definition["asset"], self.inputs.asset(kind, definition["source_id"]))
                self.assertIsNotNone(definition["asset"])
        cook = self.inputs.collections["npc"][4626]
        self.assertIn(13897, cook["models"])
        self.assertEqual(self.content["npcs"]["npc.cook"]["asset"], "asset.source.osrs.cache2695.npc.4626")
        self.assertEqual(self.inputs.asset("model", 13897), "asset.source.osrs.cache2695.model.13897")
        self.assertEqual(self.inputs.asset("interface", 679), "asset.source.osrs.cache2695.interface.679")

    def test_original_catalog_records_and_collection_values_are_unchanged(self):
        base = load(ROOT / "assets/manifests/osrs/cache2695-full-bundle.json.gz")
        self.assertEqual(self.inputs.bundle["records"][:len(base["records"])], base["records"])
        self.assertEqual(len(base["records"]), 12128)
        self.assertEqual(sha((ROOT / self.inputs.catalog_path).read_bytes()), CATALOG_SHA256)
        for number, original in self.inputs.supplement["npcs"].items():
            self.assertEqual(self.inputs.collections["npc"][int(number)], original)
        for number, original in self.inputs.supplement["items"].items():
            self.assertEqual(self.inputs.collections["item"][int(number)], original["definition"])

    def test_all_behavior_and_the_parent_repaired_spell_table_are_preserved(self):
        expected = self.application["runtime3"]["content_behavior_sha256"] if self.application else self.baseline["behavior_sha256"]
        self.assertEqual(sha(canonical(behavior_projection(self.content))), expected)
        maximum = self.content["mechanics"]["combat_styles"]["style.magic.wind_strike"]["maximum_hit"]
        self.assertEqual(maximum["status"], "bound")
        self.assertEqual(maximum["value"]["hits"], {"1": 2, "5": 4, "9": 6, "13": 8})
        self.assertEqual(len(self.content["tutorial"]), 71)
        graph = load(BINDINGS / "graph-bindings.json")
        self.assertEqual(len(graph["tutorial"]), 73)
        self.assertEqual(len(graph["cooks"]), 22)
        self.assertEqual(len(self.content["quests"]["quest.cooks_assistant"]["journal"]), 10)

    def test_behavior_fingerprint_rejects_non_asset_edits(self):
        changed = deepcopy(self.content)
        changed["initial_state"]["run_energy"] = 100
        self.assertNotEqual(sha(canonical(behavior_projection(changed))), self.baseline["behavior_sha256"])
        changed = deepcopy(self.content)
        changed["regions"]["region.osrs.12336"]["cells"][0]["blocked_movement"] ^= 1
        self.assertNotEqual(sha(canonical(behavior_projection(changed))), self.baseline["behavior_sha256"])

    def test_source_supplement_disagreements_fail_without_mutating_source_files(self):
        from common import load as source_load
        supplement = deepcopy(self.inputs.supplement)
        supplement["npcs"]["4626"]["name"] = "Incorrect source identity"
        def changed_load(path):
            return supplement if path == BINDINGS / "definitions.json.gz" else source_load(path)
        with patch("common.load", side_effect=changed_load):
            with self.assertRaisesRegex(ValueError, "Supplement disagrees with original NPC 4626"):
                Inputs()


if __name__ == "__main__":
    unittest.main()
