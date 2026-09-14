# Deterministic M1 source compiler input

The product and counts are documented in
[`content/m1/README.md`](../../content/m1/README.md). This directory owns source
binding/generation and bounded reference checks, not shared types or the
production executor.

`build.py` generates actual `GameContent` schema 2 and typed mechanics from the
committed cache/source contracts, checks source geometry/IDs and invokes the real
strict Runtime compiler. `compile.py` preserves compiler diagnostics, the
version-2 compressed artifact and the exact unresolved-binding paths/reasons.
`check.py` uses `schema-check/` to run `read_content_json`, `compile_content`,
`encode_compiled` and `load_compiled` against the actual shared library. It is
not a replacement validator.

```sh
python3 tools/m1-content/build.py
python3 tools/m1-content/check.py
python3 tools/m1-content/verify_assets.py
python3 -m unittest discover -s tools/m1-content -p 'test_*.py' -q
python3 tools/m1-content/verify_routes.py
python3 tools/m1-content/verify_state_oracles.py
```

All normal build inputs are committed and generation is offline. Read the
machine setup before builds. Existing Rust dependencies are used with
`--locked --offline`; scratch/targets stay under the owned `.local/` directory.
No global tools, graphics changes or source-account operations are needed.

The route checker uses actual `ObjectTransformState.collision` records, verifies
closed-state equality and open/close restoration, and checks all 30 source
walking dependencies. It does not make closed runtime doors disappear.
`state_oracles.py` is a strict **post-authoritative-event reference model** for
testing authored guards, entitlements, counters and atomic inventory effects.
Unknown operations are errors; it never executes unresolved transport/valuation
or claims a production game run. `verify_state_oracles.py` maps the 23 separately
authored source graph vectors to actual content IDs and schema-2 events.

## Module ownership

- `common.py`: source records, checked inputs and deterministic serialization.
- `definitions.py`, `mechanics.py`: real item/skill/NPC/recipe/style/grant/shop/
  vital definitions and localized bound/unresolved source values.
- `geometry.py`, `world_mechanics.py`: native clipping, source placement, typed
  stationary policy, physical doors, real gather sites, mill counters and fire.
- `travel.py`, `travel_policies.py`: actual stairs/ladders, experience branches,
  source departure/reconciliation and private Death's Office dependencies.
- `progression.py`: all 71/73 tutorial and 10/22 Cook source graphs, rewards,
  recovery and death topics; no stage-skip command.

## Optional source reproduction

The source cache is not fetched again. The existing decoded source and small
original-definition supplement have pinned group, payload and decoder hashes.

```sh
python3 tools/m1-content/import_references.py
python3 tools/m1-content/bind_wiki.py
python3 tools/m1-content/import_code_references.py
python3 tools/m1-content/import_runtime_sources.py
```

The last command replays only the pinned Fire, Shop and Bottomless milk bucket
pages to close concrete runtime identity/limit questions. Raw prose stays in
owned ignored storage; factual revisions/hashes remain committed. `--discover`
on the earlier importer is for explicit new research, not routine freshness
checks.

To reproduce the bounded additional original definitions:

```sh
python3 tools/m1-content/import_definitions.py \
  --cache-dir /path/to/existing/current-source/cache-2695 \
  --tooling-dir /path/to/existing/current-source/tooling \
  --npc-catalogue /path/to/existing/current-source/dependency-scan/npc-catalogue.json \
  --java-home /path/to/verified/jdk-17
```

The importer reads JS5 sectors with `rb`, checks selected group SHA/CRC/revision
and every pinned library hash, and never opens a writable cache `Store`. It
does not regenerate models or alter source-worker assets. Ordinary source
stackability stays boolean; mode 2 is a real conditional record, not coercion.

The content input adapter calls the source worker's `load_published_inputs()`.
It validates the original and additive publications, overlays only nonconflicting
collection shards, and retains exact equality checks against the original
item/NPC supplement. New definition provenance points at the actual
`content-v2/collections/{kind}.json.gz#asset_id`, never the unchanged old
collection. No source-worker file is overwritten or reaggregated.

`asset-references.json` identifies the merged catalog and both publication
manifests. Its asset output records keep canonical extraction-relative paths;
consumers resolve committed extension files through the additive publication's
`published_files[].extraction_path -> path` mapping. All original item/NPC model
references and interface groups are included without invented mesh links.

`verify_assets.py` checks the frozen parent behavior/geometry/graph hashes,
the exact72/68/6/13 requested closure roots, all5254 product references, shard
provenance and unchanged111 behavior bindings. `asset-refresh-baseline.json`
is the immutable pre-refresh evidence, not an alternative game dataset.

`input-lock.json` hashes source/schema/generator inputs, including the merged
catalog, additive publication, frozen closure request/definition snapshot,
all1129 new source outputs, collection shards and dependency graph. The content manifest
records compressed/uncompressed output hashes. Ordering and gzip metadata are
deterministic. Source inference is not approval; successful compile, geometry
and reference-model checks remain separate from executing the complete
headless/server/browser journey and approving its presentation.
