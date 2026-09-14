# Three-dose potion source supplement

This closes the exact publication gap in compiled content at `c9f97d9`:
item3010, its note3011 and model2697. It does not implement the approved loot
probability, change gameplay, or grant presentation/reference-pack approval.

Original item3010 is **Energy potion(3)** and uses model2697. Note3011 retains
its raw `name="null"` and `inventoryModel=0`, reciprocal `notedID=3010` and
`notedTemplate=799`. Do not replace these raw fields: native note composition
uses the potion and original note template/model2429.

Original bank-placeholder definition19365 is the one necessary additional
asset: potion3010 references it, and it points back to3010 with template14401.
It is an original cache placeholder variant, not generated replacement art.
Templates799/14401, their models, model0 and quantity font/glyph group494 are
reused byte-identically. No unrelated content is imported.

Model2697 has 77 vertices, 128 faces and native bounds
`[-18,-28,-18]` to `[18,0,18]`. All original HSL, transforms, recolors, scale,
topology, skin/texture data and icon-framing fields follow the unchanged
[source contract](../../research/current-source/extraction-contract.json).

## Reproduce

Run from the worktree root, using the previously verified cache/tooling:

```bash
python3 tools/cache-import/import_cache.py reuse \
  --reuse-source /home/lramos15/clubscape/.worktrees/m1-asset-closure/.local/current-source
python3 tools/cache-import/import_cache.py plan-potions
python3 tools/cache-import/import_cache.py extract-closure \
  --request research/current-source/m1-potion-request.json --output .local/current-source/potions
python3 tools/cache-import/import_cache.py publish-closure \
  --request research/current-source/m1-potion-request.json --output .local/current-source/potions
python3 tools/cache-import/import_cache.py validate-published
python3 -m unittest discover -s tools/cache-import -p 'test_*.py' -q
```

Do not re-plan after the parent refreshes mutable content bindings. Retain the
frozen request/definition snapshot and run extraction/publication directly.
No download, latest lookup, account or renderer is involved.

New decoded/raw outputs and the three-entry item shard live under
`assets/source/osrs/cache2695/potions/`. The canonical merged inventory is
`assets/manifests/osrs/cache2695-potions-bundle.json.gz`; its publication
manifest is `cache2695-potions-published.json` in the same directory. Every
preceding catalog, shard and published asset file remains unchanged.

The exact root/transitive IDs, hashes, empty missing list and bounds are in
`research/current-source/m1-potion-closure.json`. Commands, repeatability,
protected-input hashes and test results are in `m1-potion-validation.json`.

## Parent binding refresh

Set the parent content adapter's `PUBLICATION` to
`assets/manifests/osrs/cache2695-potions-published.json`.
`content_closure.load_published_inputs(PUBLICATION)` now loads all three
immutable publication layers and returns the complete catalog/collections.

Use `content_closure.publication_chain(PUBLICATION)` (oldest first) when
assembling definition provenance, publication/output-path lists and input
locks: visit each layer's `published_files`, `collection_extensions` and
metadata/request locks. Do not only inspect the latest three-entry item
shard, or prior v2 definitions would receive incorrect legacy provenance.
The existing `common.py`, `build.py` and `verify_assets.py` contain those
parent-owned single-layer readers.

Then rebuild/check the content and run its asset verification using the
existing parent commands. The two item IDs and model2697 must disappear from
its missing list; the source report already resolves all 5,257 required asset
IDs. Reference images do not need to be rerendered: their input and image
hashes are unchanged. No full-game or presentation acceptance is inferred.
