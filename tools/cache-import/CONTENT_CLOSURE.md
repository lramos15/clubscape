# Original compiled-content asset closure

This is a **source-data/source-view boundary**, not full-game execution, a
ClubScape renderer, an authenticated journey, or reference-pack approval.
It extends the verified OSRS revision240/cache2695 inputs without changing the
original cache, runtime JAR, 559-file publication, reference images, or audio
runtime files. Upstream notices remain in `THIRD_PARTY_NOTICES.txt` and
`assets/source/osrs/NOTICE.txt`.

## Scope and actual outputs

The request is derived from the exact lists in
`content/m1/asset-references.json`, cross-checked against the item, NPC and
interface bindings and the unchanged original full-bundle inventory.
No ID list is inferred from a milestone summary.

| Kind | Requested missing roots | New records after dependency closure |
| --- | ---: | ---: |
| Item definitions | 72 | 113 |
| Original models | 68 | 72 |
| NPC definitions | 6 | 6 |
| Interface groups | 13 | 13 |
| Font metrics | 0 | 2 |
| Sprite groups, including glyph atlases | 0 | 40 |
| Sequences | 0 | 2 |
| Legacy animation frames | 0 | 20 |

All **5,254 real product asset references** resolve in the extended catalog.
The 268 new records produce 1,129 original decoded/raw outputs (924,287 bytes),
plus four collection shards and a dependency graph. The 72 new models contain
5,643 vertices and 9,465 faces; none is an empty source mesh. All 580 overlapping
re-decoded records, including output hashes and source metadata, exactly match
the original inventory.

The six new NPC IDs are 306, 311, **4626**, 4628, 7941 and 9244. Cook4626 retains
his original body/head parts, including model13897; Cook4627 remains a distinct
unchanged source variant. The four additional transitive models are
16863-16866. New fonts1446/1447 retain their 256 native glyphs, metrics and source
sprite groups. New sequences3950/3997 retain 20 original frames from groups
10488/10490 and their existing source skeletons.

Three texture definitions, six skeletons and the other overlapping inputs are
reused unchanged, not relabeled as new output. No audio-runtime data or
whole-cache sample set is republished.

## Reproduce without downloads

Read `docs/machines/sparky.md` and verify the current host first. Run from this
worktree/repository root with Python3.12 and the documented JDK17:

```bash
python3 tools/cache-import/import_cache.py reuse \
  --reuse-source /home/lramos15/clubscape/.worktrees/m1-runtime-inputs/.local/current-source
python3 tools/cache-import/import_cache.py plan-closure
python3 tools/cache-import/import_cache.py validate-closure-request
python3 tools/cache-import/import_cache.py extract-closure
python3 tools/cache-import/import_cache.py publish-closure
python3 tools/cache-import/import_cache.py validate-published
python3 -m unittest discover -s tools/cache-import -p 'test_*.py' -q
python3 tools/cache-import/import_cache.py test-integrity
python3 tools/source-capture/capture.py --verify-only
python3 tools/source-capture/capture.py --profile hud --verify-only
```

`reuse` verifies both sides and **copies**, rather than hardlinks, the 25
original disk files. Runtime/dependency copies are also checked against their
locks. It never requests newer metadata or downloads the cache. Java runs with
isolated worktree `user.home`/scratch directories, headless AWT, and no network
or account handlers.

`plan-closure` is for the recorded compiled-content revision. After the parent
refreshes bindings, retain this frozen request and run `extract-closure`
directly to reproduce it. A new product closure requires a new versioned
request, not an overwrite of this historical one. Original product input hashes
remain recorded; the small original definition snapshot is copied byte-for-byte
under `research/current-source/` so regeneration does not depend on mutable
compiler output paths.

Full, narrow extraction stays in `.local/current-source/content-closure/`.
The normal legacy extraction remains `.local/current-source/extracted/`.
`publish-closure` refuses conflicting existing bytes and only publishes IDs
absent from the old catalog. Repeating extraction/publication is idempotent.
The old `publish` command explicitly rejects a closure bundle.

## Source and dependency conventions

`CacheExtractor` uses the same pinned original decoder and native output
conventions as `extraction-contract.json`. NPC/config group revisions and the
interface **index** revision configure their respective loaders; game build240
is not substituted for either. Raw payload/group hashes, full32-bit revisions,
CRC and low16-bit disk trailers retain their distinct meanings.

Closure follows:

* Item note/base/template, bought, placeholder and quantity-variant links;
  inventory/wear/head models; replacement textures; native small-font icon
  quantity labels. Original icon zoom/rotations/offsets, lighting, scale and
  recolor/retexture fields are not baked or normalized.
* NPC morph destinations, ordered body/head parts, head-icon sprites,
  replacement textures and all source animation fields.
* Every file of each requested widget group, including hidden/alternate
  widgets, alternate models/sprites/animations, native font references,
  encoded inventory item references and explicit CS1 item-count references.
  `InterfaceDefinition.textureId` is the upstream name for the sprite-angle
  field, not a texture archive ID.
* Model texture references; texture-layer sprite groups; font glyph atlases;
  sequence equipment overrides, frames, sounds and cached-curve skeleton inputs.

Models remain native signed integer XYZ with negative-up Y, original origins,
ordered triangles, HSL colors, source alpha/priority, skin groups, texture
coordinates and animation associations. No recoloring, resizing, retargeting,
equipment fitting, generic icon generation, LOD or replacement mesh occurs.
Two new257-byte font metric files have no kerning table; none is silently
dropped. Native sprite offsets, palettes, pixel indices and straight-alpha
RGBA atlases remain unchanged.

The dependency graph identifies every edge by source asset ID and original
field. Arbitrary server-populated containers, dynamically computed CS2 values,
player appearance choices and state/timing are not inferred from widget
definitions. Listeners remain preserved original data, not a claim that every
interface script or live gameplay state has run. The exact compiled product
asset gap is closed; broader game behavior and presentation gates remain
separate.

## Canonical additive manifests

* `research/current-source/m1-content-closure-request.json`: exact requested and
  audited IDs, immutable source identities and original binding input hashes.
* `research/current-source/m1-content-closure-definitions.json.gz`: unchanged
  original product-worker definition snapshot, not generated placeholders.
* `research/current-source/m1-content-closure.json`: machine-readable
  requested/resolved counts, every new ID, native model bounds and interface
  glyph references, with an empty `remaining_missing_inputs` list.
* `research/current-source/m1-content-closure-validation.json`: exact commands,
  tool/source hashes, 48 importer/closure tests, 20 original-capture tests and
  3,329 byte-identical repeated outputs. All559 original source files,
  109 reference images and258 audio-runtime files retain their hashes.
* `assets/manifests/osrs/cache2695-content-v2-bundle.json.gz`: canonical merged
  catalog; all12,128 original records remain byte-value-identical and ordered
  before the268 additions.
* `assets/manifests/osrs/cache2695-content-v2-extraction.json.gz`: actual narrow
  extraction inventory, including the580 exact re-decodes.
* `assets/manifests/osrs/cache2695-content-v2-published.json`: original manifest
  locks, new output paths/hashes, collection shards, graph and reproduction.
* `assets/source/osrs/cache2695/content-v2/`: complete outputs for the268 new
  records. `published_files[].extraction_path` maps each original bundle-relative
  output to its committed `path`; no prior asset file is overwritten.

`validate-published` needs only committed files/Python. It verifies both
publications, exact new model geometry and frame transform bounds, all widgets
and glyph references, raw/decoded/image hashes, original definition equality,
dependency endpoints, duplicate/conflicting records and complete output
coverage. Negative tests use isolated copies, never edit source/reference files.

## Parent binding refresh

Do not overwrite the old bundle or reaggregate its collections. Update the
parent-owned content input adapter to use the merged catalog and overlay the
nonconflicting collection shards:

```python
import sys
sys.path.insert(0, str(ROOT / "tools/cache-import"))
from content_closure import load_published_inputs

bundle, source_collections = load_published_inputs()
assets = {record["asset_id"]: record for record in bundle["records"]}
collections = {
    kind: {value["id"]: value for value in values.values()}
    for kind, values in source_collections.items()
}
```

Specifically refresh `tools/m1-content/common.py`'s `Inputs` catalog/collections
and its definition provenance resolver. For a new definition, point provenance
at the relevant `collection_extensions[kind].path#asset_id`, not the unchanged
old collection. Include the new publication/catalog/request/collection/output
hashes in the parent content input lock and change its emitted asset-manifest
pointer to the merged catalog. Retain the existing checks against the original
definition supplement; these definitions agree exactly.

Then run the existing parent-owned `build.py`, `check.py`, content tests and
compiler commands. Its refreshed missing item/model/NPC/interface lists should
all be empty. This worker does not edit or rebuild those separately owned
content artifacts, runtime mechanics, specs, renderer, audio or reference packs.
