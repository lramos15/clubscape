# M1 consumed-container source assets

Current publication:
`assets/manifests/osrs/cache2695-consumables-published.json`

SHA-256:
`2340ea5e3e6eeeeecdbda5de7f371f3ff2eb57576f4c783009344a1c348dbede`

This fourth immutable layer follows base, content-v2 and potions. It publishes
only the original source inputs for existing M1 Drink/Empty replacement
containers, not additional acquisition content, gameplay, sound, or presentation
approval. Existing source IDs, catalogs, shards and payload files remain
bit-identical. The original selection and extraction contract are unchanged.

| Source item | Role | Inventory model | Held model, both source sexes | Note | Placeholder |
| --- | --- | ---: | ---: | ---: | ---: |
| 229 | Vial | 2747 | 561 | 230 | 15245 |
| 1919 | Beer glass | 2548 | 8234 | 1920 | 19159 |

The four requested definitions and four models are closed. The only new
transitive definitions are original bank placeholders15245/19159. Templates
799/14401, models0/596/2429 and quantity font/glyph group494 are reused with exact
prior record/output hashes. No unrelated definitions, sprites or textures are
imported. Native note definitions retain `name="null"` and `inventoryModel=0`;
their reciprocal base links and note template799 must drive composition.

The UI worker's CRC/hash-pinned definition snapshot is copied byte-for-byte into
`research/current-source/m1-consumable-definitions.json.gz`. Independent
extraction from the private, verified cache confirms all fields and payload
hashes. Config archive2/10 keeps full revision1788780610, not game build240.
Raw HSL/palette, source transformations, scale, triangle order, materials,
offsets and held-model origins are preserved under the existing
`research/current-source/extraction-contract.json`; no model alias or new art
replaces the containers.

## Reproduce

Read `docs/machines/sparky.md` and verify the host before using its JDK paths.
Run from this repository/worktree root:

```bash
python3 tools/cache-import/import_cache.py reuse \
  --reuse-source /home/lramos15/clubscape/.worktrees/m1-potion-assets/.local/current-source
python3 tools/cache-import/import_cache.py plan-consumables \
  --definitions /home/lramos15/clubscape/.worktrees/m1-gameplay-ui-contracts/research/m1-bindings/ui-item-definitions.json.gz
python3 tools/cache-import/import_cache.py extract-closure \
  --request research/current-source/m1-consumable-request.json --output .local/current-source/consumables
python3 tools/cache-import/import_cache.py publish-closure \
  --request research/current-source/m1-consumable-request.json --output .local/current-source/consumables
python3 tools/cache-import/import_cache.py validate-published
python3 -m unittest discover -s tools/cache-import -p 'test_*.py' -q
python3 tools/cache-import/import_cache.py test-integrity
```

After the first plan, use the frozen request and owned definition snapshot
directly; do not re-plan from mutable backend outputs. `reuse` verifies and
privately copies all original disk files and locked tooling. No `Store` write
handle is opened on another worker's cache, and no source download, account,
global installation, renderer or GPU is involved.

New outputs are under `assets/source/osrs/cache2695/consumables/`. Exact root and
transitive IDs, model bounds and the empty missing list are recorded in
`research/current-source/m1-consumable-closure.json`. The current manifest/hash,
consumer mapping, commands, repeatability and protected-input checks are in
`research/current-source/m1-consumable-validation.json`.

## Backend ContentAssetRef hookup

Select `assets/manifests/osrs/cache2695-consumables-published.json` in the
parent-owned `tools/m1-content/common.py` publication constant. Continue using
the existing chained adapter:

```python
from content_closure import load_published_inputs, publication_chain

bundle, source_collections = load_published_inputs(PUBLICATION)
layers = publication_chain(PUBLICATION)  # oldest first, all four layers
```

Merge every layer's publication metadata, `published_files` and
`collection_extensions` into provenance/output-path resolution and the content
input lock. Retain the old layers rather than overwriting their catalogs or
reaggregating their collections.

The exact compiled item `asset` references are:

| Backend item | Original source asset ID |
| --- | --- |
| `item.vial` | `asset.source.osrs.cache2695.item.229` |
| `item.vial.noted` | `asset.source.osrs.cache2695.item.230` |
| `item.beer_glass` | `asset.source.osrs.cache2695.item.1919` |
| `item.beer_glass.noted` | `asset.source.osrs.cache2695.item.1920` |

`build_items` derives these references when the selected catalog contains the
definitions. In the Rust content boundary this is `ItemDefinition.asset`
(`Option<AssetId>`); `AssetManifest.assets` must be projected from the complete
merged catalog's record IDs. Do not bind an item directly to a model ID, alias
the held/inventory models, set required assets to `None`, or drop the existing
Drink/Empty replacement outputs to avoid the membership failure.

Keep UI source definitions and their exact note links in the backend loader.
Then run its normal build/check/asset-verification/compile commands. That
separately owned content rebuild is the consumer integration step; publishing
these bytes does not claim gameplay execution or new fidelity acceptance.
