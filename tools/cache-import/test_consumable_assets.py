import copy
import gzip
import json
import shutil
import subprocess
import sys
import unittest
import uuid

import content_closure as closure
import import_cache as cache


class ConsumableAssetTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.publication = cache.read_json(closure.CONSUMABLE_PUBLICATION)
        cls.bundle, cls.collections = closure.load_published_inputs(closure.CONSUMABLE_PUBLICATION)
        cls.records = closure.record_index(cls.bundle["records"])
        cls.request = cache.read_json(closure.CONSUMABLE_REQUEST)
        cls.report = cache.read_json(cache.ROOT / cls.publication["closure_report"]["path"])
        cls.extraction = cache.read_json(cache.ROOT / cls.publication["extraction_inventory"]["path"])

    def setUp(self):
        self.directory = cache.ROOT / ".local/cache-import-tests" / uuid.uuid4().hex
        self.directory.mkdir(parents=True)

    def tearDown(self):
        shutil.rmtree(self.directory)

    def test_exact_eight_roots_and_only_two_new_source_placeholders(self):
        self.assertEqual(self.request["item_ids"], [229, 230, 1919, 1920])
        self.assertEqual(self.request["model_ids"], [561, 2548, 2747, 8234])
        self.assertEqual(self.report["remaining_missing_inputs"], [])
        self.assertEqual(self.report["resolved_product_asset_ids"], 8)
        self.assertEqual(set(self.report["new_asset_ids"]), {
            *(closure.identity("item", n) for n in [229, 230, 1919, 1920, 15245, 19159]),
            *(closure.identity("model", n) for n in [561, 2548, 2747, 8234]),
        })

    def test_inventory_and_held_models_are_distinct_original_inputs(self):
        for number, name, inventory, held in [(229, "Vial", 2747, 561), (1919, "Beer glass", 2548, 8234)]:
            item = self.collections["item"][closure.identity("item", number)]
            self.assertEqual(item["name"], name)
            self.assertEqual(item["inventoryModel"], inventory)
            self.assertEqual(item["maleModel0"], held)
            self.assertEqual(item["femaleModel0"], held)
            self.assertNotEqual(inventory, held)

    def test_notes_and_placeholders_keep_raw_source_links(self):
        items = self.collections["item"]
        for number, note_number, placeholder_number in [(229, 230, 15245), (1919, 1920, 19159)]:
            item, note, placeholder = (items[closure.identity("item", n)] for n in
                                       (number, note_number, placeholder_number))
            self.assertEqual(item["notedID"], note_number)
            self.assertEqual((note["notedID"], note["notedTemplate"]), (number, 799))
            self.assertEqual((note["name"], note["inventoryModel"]), ("null", 0))
            self.assertEqual(item["placeholderId"], placeholder_number)
            self.assertEqual((placeholder["placeholderId"], placeholder["placeholderTemplateId"]), (number, 14401))

    def test_definition_payload_hashes_and_full_archive_revision(self):
        expected = {
            229: "3f5352fe821670c938ba2dc6b2b8934108b89d0798aeacfc67b2bc1bc4effa6f",
            230: "fb35b2ef046a80e8f8b30874178326725cf7e1e89359f22726a6188871b0bc33",
            1919: "99abe6e80a5a7c5a90cd788fa3b8f78a209012755ae51a952a3f437112e188b4",
            1920: "121eef89ff5ade424a405f118bbd275a618ca0685c8f8f74222bb519a4914ef5",
        }
        for number, sha in expected.items():
            record = self.records[closure.identity("item", number)]
            self.assertEqual(record["source"][0]["sha256"], sha)
            self.assertEqual(record["source"][0]["group_key"], "2/10")
        self.assertEqual(self.bundle["groups"]["2/10"]["revision"], 1788780610)
        self.assertEqual(self.request["source_definition_origin"]["sha256"],
                         self.request["product_definitions_snapshot"]["sha256"])

    def test_native_topology_and_unrecentered_held_bounds(self):
        expected = {
            2548: (40, 88, [-8, -24, -8], [22, -1, 12]),
            2747: (64, 102, [-10, -28, -10], [10, 0, 10]),
            561: (72, 124, [-40, -106, -14], [-21, -82, 22]),
            8234: (41, 96, [-33, -99, -10], [-4, -76, 15]),
        }
        for number, bounds in expected.items():
            value = cache.read_json(closure.SOURCE / f"consumables/models/{number}.json.gz")
            stats = self.records[closure.identity("model", number)]["statistics"]
            cache.validate_model(value, stats)
            self.assertEqual((stats["vertices"], stats["faces"], stats["bounds_min"], stats["bounds_max"]), bounds)
            self.assertFalse(stats["source_empty_mesh"])

    def test_recentered_or_out_of_range_geometry_is_rejected(self):
        number = 561
        value = cache.read_json(closure.SOURCE / f"consumables/models/{number}.json.gz")
        stats = self.records[closure.identity("model", number)]["statistics"]
        wrong = copy.deepcopy(value)
        wrong["model"]["vertexY"] = [y + 82 for y in wrong["model"]["vertexY"]]
        with self.assertRaisesRegex(cache.InputError, "native bounds"):
            cache.validate_model(wrong, stats)
        wrong = copy.deepcopy(value)
        wrong["model"]["faceIndices1"][0] = wrong["model"]["vertexCount"]
        with self.assertRaisesRegex(cache.InputError, "out of bounds"):
            cache.validate_model(wrong, stats)

    def test_templates_models_and_quantity_font_reuse_exact_previous_records(self):
        prior = cache.read_json(cache.ROOT / self.publication["base_bundle"]["path"])
        prior_records = closure.record_index(prior["records"])
        repeated = {row["asset_id"] for row in self.extraction["records"]} & prior_records.keys()
        self.assertEqual(repeated, {
            closure.identity("item", 799), closure.identity("item", 14401),
            closure.identity("model", 0), closure.identity("model", 596), closure.identity("model", 2429),
            closure.identity("font", 494), closure.identity("sprite", 494),
        })
        for record in self.extraction["records"]:
            if record["asset_id"] in repeated:
                self.assertEqual(record, prior_records[record["asset_id"]])

    def test_all_previous_catalog_and_collection_values_survive(self):
        previous, collections = closure.load_published_inputs(closure.POTION_PUBLICATION)
        self.assertEqual(self.bundle["records"][:len(previous["records"])], previous["records"])
        self.assertEqual(len(self.bundle["records"]), 12410)
        self.assertEqual(len(closure.publication_chain(closure.CONSUMABLE_PUBLICATION)), 4)
        for kind, values in collections.items():
            for key, value in values.items():
                self.assertEqual(self.collections[kind][key], value)

    def test_missing_or_corrupt_original_model_bytes_fail(self):
        record = self.records[closure.identity("model", 8234)]["source"][0]
        path = self.directory / "model.bin"
        with self.assertRaisesRegex(cache.InputError, "Missing input"):
            cache.checked_file(path, record)
        path.write_bytes((closure.SOURCE / "consumables/raw/7/8234/0.bin").read_bytes())
        cache.checked_file(path, record)
        data = bytearray(path.read_bytes())
        data[-1] ^= 1
        path.write_bytes(data)
        with self.assertRaisesRegex(cache.InputError, "SHA-256"):
            cache.checked_file(path, record)

    def test_model_alias_in_ui_snapshot_is_rejected_before_publication(self):
        snapshot = cache.read_json(cache.ROOT / self.request["product_definitions_snapshot"]["path"])
        snapshot["items"]["229"]["definition"]["inventoryModel"] = 2548
        path = self.directory / "ui-items.json.gz"
        path.write_bytes(gzip.compress(json.dumps(snapshot).encode(), mtime=0))
        with self.assertRaisesRegex(cache.InputError, "model relationships differ"):
            closure.plan_consumables(path)

    def test_widened_or_duplicate_requested_scope_fails(self):
        for number in (229, 995):
            request = copy.deepcopy(self.request)
            request["item_ids"].append(number)
            with self.assertRaisesRegex(cache.InputError, "eight-root scope"):
                closure.validate_request(request)

    def test_duplicate_publication_outputs_fail(self):
        publication = copy.deepcopy(self.publication)
        publication["published_files"].append(publication["published_files"][0])
        path = self.directory / "publication.json"
        cache.write_json(path, publication)
        with self.assertRaisesRegex(cache.InputError, "duplicate published"):
            closure.validate_publication(path)

    def test_cli_requires_real_definition_snapshot(self):
        result = subprocess.run([sys.executable, str(cache.TOOL / "import_cache.py"), "plan-consumables"],
                                cwd=cache.ROOT, capture_output=True, text=True)
        self.assertEqual(result.returncode, 1)
        self.assertIn("cache-import: plan-consumables requires --definitions", result.stderr)

    def test_source_qualification_does_not_grant_gameplay_or_presentation_acceptance(self):
        self.assertFalse(self.publication["source_gameplay_or_presentation_accepted"])
        self.assertFalse(self.publication["owner_reference_pack_approved"])


if __name__ == "__main__":
    unittest.main()
