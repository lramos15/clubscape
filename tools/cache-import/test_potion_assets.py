import copy
import shutil
import unittest
import uuid

import content_closure as closure
import import_cache as cache


class PotionAssetTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.publication = cache.read_json(closure.POTION_PUBLICATION)
        cls.bundle, cls.collections = closure.load_published_inputs(closure.POTION_PUBLICATION)
        cls.records = closure.record_index(cls.bundle["records"])
        cls.request = cache.read_json(closure.POTION_REQUEST)
        cls.report = cache.read_json(cache.ROOT / cls.publication["closure_report"]["path"])

    def setUp(self):
        self.directory = cache.ROOT / ".local/cache-import-tests" / uuid.uuid4().hex
        self.directory.mkdir(parents=True)

    def tearDown(self):
        shutil.rmtree(self.directory)

    def test_exact_three_roots_and_only_necessary_extra_placeholder(self):
        self.assertEqual(self.request["item_ids"], [3010, 3011])
        self.assertEqual(self.request["model_ids"], [2697])
        self.assertEqual(self.request["npc_ids"], [])
        self.assertEqual(self.request["interface_groups"], [])
        self.assertTrue(all(not values for values in self.request["audit_existing"].values()))
        self.assertEqual(self.report["new_asset_ids"], [
            closure.identity("item", 19365), closure.identity("item", 3010),
            closure.identity("item", 3011), closure.identity("model", 2697),
        ])
        self.assertEqual(self.report["remaining_missing_inputs"], [])
        self.assertEqual(self.report["resolved_product_asset_ids"], 5257)

    def test_real_three_dose_potion_and_reciprocal_note(self):
        items = self.collections["item"]
        potion, note = (items[closure.identity("item", number)] for number in (3010, 3011))
        self.assertEqual(potion["name"], "Energy potion(3)")
        self.assertEqual(potion["inventoryModel"], 2697)
        self.assertEqual(potion["notedID"], 3011)
        self.assertEqual(note["notedID"], 3010)
        self.assertEqual(note["notedTemplate"], 799)
        self.assertEqual(note["inventoryModel"], 0)
        self.assertEqual(note["name"], "null")
        self.assertEqual(items[closure.identity("item", 799)]["inventoryModel"], 2429)

    def test_original_bank_placeholder_is_not_a_generated_substitute(self):
        items = self.collections["item"]
        self.assertEqual(items[closure.identity("item", 3010)]["placeholderId"], 19365)
        placeholder = items[closure.identity("item", 19365)]
        self.assertEqual(placeholder["placeholderId"], 3010)
        self.assertEqual(placeholder["placeholderTemplateId"], 14401)
        self.assertIn(closure.identity("item", 14401), self.records)

    def test_pinned_potion_and_note_payload_hashes(self):
        for number, sha in {
            3010: "58e0d540f327a55f0365bd0a62c03e3372e685a6de38108185e719b03f35e2f0",
            3011: "44e0bc0c7d578139dfd9e1eebdb996e9797fa2465370f479f36c809886af81df",
        }.items():
            source = self.records[closure.identity("item", number)]["source"][0]
            self.assertEqual(source["group_key"], "2/10")
            self.assertEqual(source["file"], number)
            self.assertEqual(source["sha256"], sha)
        self.assertEqual(self.bundle["groups"]["2/10"]["revision"], 1788780610)

    def test_native_model_bounds_and_bad_indices(self):
        model = cache.read_json(closure.SOURCE / "potions/models/2697.json.gz")
        stats = self.records[closure.identity("model", 2697)]["statistics"]
        cache.validate_model(model, stats)
        self.assertEqual((stats["vertices"], stats["faces"]), (77, 128))
        self.assertEqual(stats["bounds_min"], [-18, -28, -18])
        self.assertEqual(stats["bounds_max"], [18, 0, 18])
        model["model"]["faceIndices1"][0] = 77
        with self.assertRaisesRegex(cache.InputError, "out of bounds"):
            cache.validate_model(model, stats)

    def test_old_catalog_and_shards_are_preserved_through_layers(self):
        prior, prior_collections = closure.load_published_inputs()
        self.assertEqual(self.bundle["records"][:len(prior["records"])], prior["records"])
        self.assertEqual(len(self.bundle["records"]), len(prior["records"]) + 4)
        self.assertEqual(len(self.bundle["extensions"]), len(prior["extensions"]) + 1)
        for kind, values in prior_collections.items():
            for key, value in values.items():
                self.assertEqual(self.collections[kind][key], value)
        self.assertEqual(len(closure.publication_chain(closure.POTION_PUBLICATION)), 3)

    def test_all_dependencies_and_repeated_source_hashes_are_closed(self):
        self.assertEqual(self.report["redecoded_existing_assets_identical"], 7)
        graph = cache.read_json(cache.ROOT / self.publication["dependency_graph"]["path"])
        for edge in graph["edges"]:
            self.assertIn(edge["from"], self.records)
            self.assertIn(edge["to"], self.records)
        self.assertEqual(self.report["new_asset_counts"], {"item": 3, "model": 1})

    def test_relative_request_file_records_are_supported(self):
        relative = closure.POTION_REQUEST.relative_to(cache.ROOT)
        self.assertEqual(cache.file_record(relative), cache.file_record(closure.POTION_REQUEST))

    def test_missing_and_corrupted_new_source_bytes_fail(self):
        source = self.records[closure.identity("model", 2697)]["source"][0]
        path = self.directory / "model.bin"
        with self.assertRaisesRegex(cache.InputError, "Missing input"):
            cache.checked_file(path, source)
        path.write_bytes((closure.SOURCE / "potions/raw/7/2697/0.bin").read_bytes())
        cache.checked_file(path, source)
        data = bytearray(path.read_bytes())
        data[-1] ^= 1
        path.write_bytes(data)
        with self.assertRaisesRegex(cache.InputError, "SHA-256"):
            cache.checked_file(path, source)

    def test_duplicate_potion_output_fails(self):
        publication = copy.deepcopy(self.publication)
        publication["published_files"].append(publication["published_files"][0])
        path = self.directory / "publication.json"
        cache.write_json(path, publication)
        with self.assertRaisesRegex(cache.InputError, "duplicate published"):
            closure.validate_publication(path)

    def test_no_gameplay_or_presentation_approval_is_inferred(self):
        self.assertFalse(self.publication["owner_reference_pack_approved"])
        self.assertFalse(self.publication["source_gameplay_or_presentation_accepted"])


if __name__ == "__main__":
    unittest.main()
