import copy
import json
import shutil
import subprocess
import sys
import unittest
from unittest.mock import patch
import uuid

import content_closure as closure
import import_cache as importer


class ContentClosureTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.publication = importer.read_json(closure.PUBLICATION)
        cls.request = importer.read_json(closure.REQUEST)
        cls.base = importer.read_json(closure.BASE_BUNDLE)
        cls.extracted = importer.read_json(importer.ROOT / cls.publication["extraction_inventory"]["path"])
        cls.records, cls.additions, cls.repeated = closure.merge_records(cls.base, cls.extracted)
        cls.decoded = {}
        for record in cls.publication["published_files"]:
            if record["path"].endswith((".json", ".json.gz")):
                cls.decoded[record["source_asset_id"]] = importer.read_json(importer.ROOT / record["path"])
        for kind in ("item", "npc", "sequence", "frame", "skeleton", "texture"):
            cls.decoded.update(importer.read_json(closure.SOURCE / "collections" / f"{kind}.json.gz"))
        for record in cls.extracted["records"]:
            key = record["asset_id"]
            output = closure.decoded_output(record)
            if key not in cls.decoded and output and (closure.SOURCE / output["path"]).is_file():
                cls.decoded[key] = importer.read_json(closure.SOURCE / output["path"])

    def setUp(self):
        self.directory = importer.ROOT / ".local/cache-import-tests" / uuid.uuid4().hex
        self.directory.mkdir(parents=True)

    def tearDown(self):
        shutil.rmtree(self.directory)

    def test_exact_product_missing_lists_are_resolved(self):
        expected = {"item_definition_ids": 72, "model_ids": 68, "npc_definition_ids": 6, "interface_groups": 13}
        actual = importer.read_json(importer.ROOT / self.publication["closure_report"]["path"])
        self.assertEqual(actual["requested"], expected)
        self.assertEqual(actual["resolved"], expected)
        self.assertEqual(actual["remaining_missing_inputs"], [])
        self.assertEqual(actual["resolved_product_asset_ids"], 5254)
        for field, (kind, _) in closure.FIELDS.items():
            for number in self.request["reported_missing_ids"][field]:
                self.assertIn(closure.identity(kind, number), self.records)

    def test_real_cook_and_source_variant_are_distinct(self):
        cook = self.decoded[closure.identity("npc", 4626)]
        prior_cook = self.decoded[closure.identity("npc", 4627)]
        self.assertEqual(cook["name"], "Cook")
        self.assertIn(13897, cook["models"])
        self.assertNotEqual(cook, prior_cook)
        self.assertEqual(self.records[closure.identity("npc", 4626)]["source"][0]["sha256"],
                         "43c4790ee88b0a0134775024e414589e2b44d0b302aa80866d539116d40393f4")

    def test_existing_records_and_full_archive_revisions_unchanged(self):
        self.assertEqual(len(self.base["records"]), 12128)
        self.assertEqual(len(self.repeated), 580)
        self.assertEqual(len(self.additions), 268)
        self.assertEqual(self.extracted["groups"]["2/9"]["revision"], 1788780609)
        self.assertEqual(self.extracted["groups"]["2/10"]["revision"], 1788780610)
        self.assertEqual(self.extracted["groups"]["2/12"]["revision"], 1788780611)
        for record in self.base["records"]:
            self.assertEqual(self.records[record["asset_id"]], record)

    def test_duplicate_and_invalid_requested_ids_fail(self):
        for values in ([4626, 4626], [-1], [True], [4626.0]):
            with self.subTest(values=values), self.assertRaises(importer.InputError):
                closure.identifiers(values, "npc_ids")

    def test_duplicate_json_input_keys_fail(self):
        path = self.directory / "duplicate.json"
        path.write_text('{"npc_ids":[4626],"npc_ids":[4627]}')
        with self.assertRaisesRegex(importer.InputError, "Duplicate JSON"):
            importer.read_json(path)

    def test_duplicate_source_records_fail(self):
        row = self.extracted["records"][0]
        with self.assertRaisesRegex(importer.InputError, "Duplicate"):
            closure.record_index([row, row])

    def test_changed_existing_payload_or_output_fails(self):
        altered = copy.deepcopy(self.extracted)
        row = next(record for record in altered["records"] if record["asset_id"] in self.repeated)
        row["outputs"][0]["sha256"] = "0" * 64
        with self.assertRaisesRegex(importer.InputError, "Existing original asset"):
            closure.merge_records(self.base, altered)

    def test_game_build_cannot_replace_archive_revision(self):
        altered = copy.deepcopy(self.extracted)
        altered["groups"]["2/10"]["revision"] = 240
        with self.assertRaisesRegex(importer.InputError, "archive CRC/full revision"):
            closure.merge_records(self.base, altered)

    def test_requested_list_cannot_omit_an_actual_missing_id(self):
        request = copy.deepcopy(self.request)
        request["npc_ids"].remove(4626)
        with self.assertRaisesRegex(importer.InputError, "exact reported IDs"):
            closure.validate_request(request)

    def test_missing_request_field_is_an_explicit_input_error(self):
        request = dict(self.request)
        del request["npc_ids"]
        with self.assertRaisesRegex(importer.InputError, "Incomplete content-closure request: missing npc_ids"):
            closure.validate_request(request)

    def test_definition_snapshot_is_required_and_hash_checked(self):
        path = self.directory / "definitions.json.gz"
        source = importer.ROOT / self.request["product_definitions_snapshot"]["path"]
        path.write_bytes(source.read_bytes())
        request = copy.deepcopy(self.request)
        request["product_definitions_snapshot"] = importer.file_record(path)
        closure.validate_request(request)
        data = bytearray(path.read_bytes())
        data[-1] ^= 1
        path.write_bytes(data)
        with self.assertRaisesRegex(importer.InputError, "SHA-256"):
            closure.validate_request(request)
        path.unlink()
        with self.assertRaisesRegex(importer.InputError, "Missing input"):
            closure.validate_request(request)

    def test_parent_refresh_does_not_invalidate_frozen_source_publication(self):
        original = importer.checked_file
        product_paths = {importer.ROOT / record["path"] for record in self.request["product_inputs"]}

        def changed_product(path, record):
            if path in product_paths:
                raise importer.InputError("Parent refreshed compiled bindings")
            return original(path, record)

        with patch.object(importer, "checked_file", side_effect=changed_product):
            closure.validate_request(self.request)
            with self.assertRaisesRegex(importer.InputError, "Parent refreshed"):
                closure.validate_request(self.request, verify_product_inputs=True)

    def test_all_new_models_have_exact_nonempty_native_bounds(self):
        models = [row for row in self.additions if row["kind"] == "model"]
        self.assertEqual(len(models), 72)
        for row in models:
            value = self.decoded[row["asset_id"]]
            importer.validate_model(value, row["statistics"])
            self.assertGreater(row["statistics"]["vertices"], 0)
            self.assertGreater(row["statistics"]["faces"], 0)
        self.assertEqual(sum(row["statistics"]["vertices"] for row in models), 5643)
        self.assertEqual(sum(row["statistics"]["faces"] for row in models), 9465)

    def test_changed_model_bounds_fail_even_with_valid_indices(self):
        key = closure.identity("model", 13897)
        model = copy.deepcopy(self.decoded[key])
        model["model"]["vertexX"] = [number + 1 for number in model["model"]["vertexX"]]
        with self.assertRaisesRegex(importer.InputError, "native bounds"):
            importer.validate_model(model, self.records[key]["statistics"])

    def test_non_native_fractional_model_coordinates_fail(self):
        key = closure.identity("model", 13897)
        model = copy.deepcopy(self.decoded[key])
        model["model"]["vertexX"][0] += 0.5
        with self.assertRaisesRegex(importer.InputError, "vertex array"):
            importer.validate_model(model, self.records[key]["statistics"])

    def test_new_model_bad_triangle_and_skin_cardinality_fail(self):
        key = closure.identity("model", 13897)
        model = copy.deepcopy(self.decoded[key])
        model["model"]["faceIndices1"][0] = model["model"]["vertexCount"]
        with self.assertRaisesRegex(importer.InputError, "out of bounds"):
            importer.validate_model(model, self.records[key]["statistics"])
        model = copy.deepcopy(self.decoded[key])
        model["model"]["packedVertexGroups"] = [0]
        with self.assertRaisesRegex(importer.InputError, "vertex attribute cardinality"):
            importer.validate_model(model, self.records[key]["statistics"])

    def test_all_requested_widgets_and_native_glyph_references(self):
        for number in self.request["interface_groups"]:
            key = closure.identity("interface", number)
            result = closure.validate_widgets(self.records[key], self.decoded[key], self.records, self.decoded)
            self.assertGreater(result["widgets"], 0)
        for number in (1446, 1447):
            font = self.decoded[closure.identity("font", number)]
            sprite = self.decoded[closure.identity("sprite", number)]
            self.assertEqual(font["glyph_sprite_group"], number)
            self.assertEqual(len(font["advances"]), 256)
            self.assertEqual(len(sprite["frames"]), 256)

    def test_duplicate_or_missing_interface_widgets_fail(self):
        key = closure.identity("interface", 642)
        widgets = copy.deepcopy(self.decoded[key])
        widgets.append(widgets[0])
        with self.assertRaisesRegex(importer.InputError, "Duplicate/missing native widget"):
            closure.validate_widgets(self.records[key], widgets, self.records, self.decoded)
        with self.assertRaisesRegex(importer.InputError, "Duplicate/missing native widget"):
            closure.validate_widgets(self.records[key], self.decoded[key][1:], self.records, self.decoded)

    def test_missing_metrics_or_glyph_atlas_fail(self):
        key = closure.identity("interface", 642)
        for missing in (closure.identity("font", 1446), closure.identity("sprite", 1446)):
            decoded = dict(self.decoded)
            del decoded[missing]
            with self.subTest(missing=missing), self.assertRaisesRegex(importer.InputError, "Missing interface"):
                closure.validate_widgets(self.records[key], self.decoded[key], self.records, decoded)

    def test_wrong_glyph_group_or_out_of_bounds_glyph_fails(self):
        key = closure.identity("interface", 642)
        font_key, sprite_key = closure.identity("font", 1446), closure.identity("sprite", 1446)
        decoded = dict(self.decoded)
        decoded[font_key] = copy.deepcopy(decoded[font_key])
        decoded[font_key]["glyph_sprite_group"] = 1447
        with self.assertRaisesRegex(importer.InputError, "glyph reference"):
            closure.validate_widgets(self.records[key], self.decoded[key], self.records, decoded)
        decoded = dict(self.decoded)
        decoded[sprite_key] = copy.deepcopy(decoded[sprite_key])
        decoded[sprite_key]["frames"][0]["atlas_x"] = 1_000_000
        with self.assertRaisesRegex(importer.InputError, "outside original atlas"):
            closure.validate_widgets(self.records[key], self.decoded[key], self.records, decoded)

    def test_duplicate_native_glyph_index_fails(self):
        key, sprite_key = closure.identity("interface", 642), closure.identity("sprite", 1446)
        decoded = dict(self.decoded)
        decoded[sprite_key] = copy.deepcopy(decoded[sprite_key])
        decoded[sprite_key]["frames"][1]["frame"] = 0
        with self.assertRaisesRegex(importer.InputError, "Duplicate/missing native glyph"):
            closure.validate_widgets(self.records[key], self.decoded[key], self.records, decoded)

    def test_variant_model_font_texture_animation_edges_resolve(self):
        for record in self.extracted["records"]:
            key = record["asset_id"]
            if key not in self.decoded:
                continue
            for target, field in closure.dependencies(record, self.decoded[key]):
                with self.subTest(source=key, field=field):
                    self.assertIn(target, self.records)

    def test_new_native_animation_transform_groups_are_valid(self):
        frames = [row for row in self.additions if row["kind"] == "frame"]
        self.assertEqual(len(frames), 20)
        for row in frames:
            importer.validate_frame(self.decoded[row["asset_id"]])
        key = frames[0]["asset_id"]
        frame = copy.deepcopy(self.decoded[key])
        frame["indexFrameIds"][0] = frame["framemap"]["length"]
        with self.assertRaisesRegex(importer.InputError, "outside the original skeleton"):
            importer.validate_frame(frame)
        frame = copy.deepcopy(self.decoded[key])
        frame["translator_x"].pop()
        with self.assertRaisesRegex(importer.InputError, "cardinality"):
            importer.validate_frame(frame)

    def test_missing_required_dependency_fails(self):
        key = closure.identity("npc", 4626)
        records = dict(self.records)
        del records[closure.identity("model", 13897)]
        with self.assertRaisesRegex(importer.InputError, "Missing original content dependencies"):
            closure.check_graph(records, self.decoded, {key}, {"required_asset_ids": [key]})

    def test_original_definition_disagreement_fails(self):
        decoded = dict(self.decoded)
        key = closure.identity("npc", 4626)
        decoded[key] = copy.deepcopy(decoded[key])
        decoded[key]["widthScale"] = 127
        with self.assertRaisesRegex(importer.InputError, "differs from product source bytes"):
            closure.validate_definitions(self.extracted, decoded, self.request)

    def test_no_new_asset_output_can_overwrite_existing_bytes(self):
        path = self.directory / "source.bin"
        closure.immutable_bytes(path, b"original")
        closure.immutable_bytes(path, b"original")
        with self.assertRaisesRegex(importer.InputError, "SHA-256"):
            closure.immutable_bytes(path, b"replaced")
        self.assertEqual(path.read_bytes(), b"original")

    def test_duplicate_extraction_output_paths_fail(self):
        key = closure.identity("model", 13897)
        value = self.decoded[key]
        path = self.directory / "model.json"
        path.write_text(json.dumps(value))
        output = {"path": "model.json", "size_bytes": path.stat().st_size, "sha256": importer.digest(path)}
        rows = [{"asset_id": closure.identity("model", number), "kind": "model", "source": [],
                 "statistics": self.records[key]["statistics"], "outputs": [output]} for number in (1, 2)]
        (self.directory / "bundle.json").write_text(json.dumps({
            "cache_id": 2695, "scope": "content-asset-closure", "records": rows, "groups": {}, "counts": {"model": 2},
        }))
        with self.assertRaisesRegex(importer.InputError, "Duplicate extraction output path"):
            importer.validate_bundle(self.directory)

    def test_bad_request_cli_has_explicit_nonzero_error(self):
        request = copy.deepcopy(self.request)
        request["npc_ids"].append(4626)
        path = self.directory / "bad-request.json"
        path.write_text(json.dumps(request))
        result = subprocess.run(
            [sys.executable, str(importer.TOOL / "import_cache.py"), "validate-closure-request", "--request", str(path)],
            cwd=importer.ROOT, capture_output=True, text=True)
        self.assertEqual(result.returncode, 1)
        self.assertIn("cache-import: Duplicate requested source ID", result.stderr)
        self.assertNotIn("Traceback", result.stderr)

    def test_published_catalog_and_parent_collection_loader(self):
        result = closure.validate_publication()
        self.assertEqual(result["remaining_missing_inputs"], [])
        self.assertEqual(result["new_published_files"], 1129)
        bundle, values = closure.load_published_inputs()
        self.assertEqual(len(bundle["records"]), 12396)
        self.assertEqual(values["npc"][closure.identity("npc", 4626)]["name"], "Cook")
        self.assertEqual(values["item"][closure.identity("item", 33089)]["stackable"], 2)

    def test_missing_or_duplicate_published_output_is_rejected(self):
        for duplicate in (False, True):
            publication = copy.deepcopy(self.publication)
            if duplicate:
                publication["published_files"].append(publication["published_files"][0])
            else:
                publication["published_files"].pop()
            path = self.directory / "publication.json"
            importer.write_json(path, publication)
            with self.subTest(duplicate=duplicate), self.assertRaises(importer.InputError):
                closure.validate_publication(path)

    def test_merged_inventory_cannot_misreport_counts(self):
        publication = copy.deepcopy(self.publication)
        merged = importer.read_json(importer.ROOT / publication["merged_inventory"]["path"])
        merged["counts"]["model"] -= 1
        publication["merged_inventory"] = importer.write_json(self.directory / "merged.json.gz", merged)
        path = self.directory / "publication.json"
        importer.write_json(path, publication)
        with self.assertRaisesRegex(importer.InputError, "inventory counts"):
            closure.validate_publication(path)

    def test_duplicate_inventory_output_cannot_be_hidden_by_a_map(self):
        publication = copy.deepcopy(self.publication)
        extracted = copy.deepcopy(self.extracted)
        merged = importer.read_json(importer.ROOT / publication["merged_inventory"]["path"])
        key = self.additions[0]["asset_id"]
        for bundle in (extracted, merged):
            row = next(row for row in bundle["records"] if row["asset_id"] == key)
            row["outputs"].append(row["outputs"][0])
        publication["merged_inventory"] = importer.write_json(self.directory / "merged.json.gz", merged)
        publication["extraction_inventory"] = importer.write_json(self.directory / "extracted.json.gz", extracted)
        path = self.directory / "publication.json"
        importer.write_json(path, publication)
        with self.assertRaisesRegex(importer.InputError, "Duplicate closure inventory output path"):
            closure.validate_publication(path)


if __name__ == "__main__":
    unittest.main()
