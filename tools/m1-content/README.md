# Deterministic M1 source compiler input

The product and counts are documented in
[`content/m1/README.md`](../../content/m1/README.md). This directory owns source
binding/generation and bounded reference checks, not shared types or the
production executor.

`build.py` generates actual `GameContent` schema 3 and typed mechanics from the
committed cache/source contracts, checks source geometry/IDs and invokes the real
strict Runtime compiler. `compile.py` preserves compiler diagnostics, the
version-3 compressed artifact and the exact unresolved-binding paths/reasons.
`check.py` uses `schema-check/` to run `read_content_json`, `compile_content`,
`encode_compiled` and `load_compiled` against the actual shared library. It is
not a replacement validator. It also constructs the real engine, checks fresh
creation/APIs and runs native source-policy probes. Failures remain nonzero
exits, not successful compile-shaped fallbacks.

```sh
python3 tools/m1-content/validate.py --repeat
```

The complete runner repeats all gates and records hashes/status without rewriting
the original runtime-binding source evidence. Individual gates are:

```sh
python3 tools/m1-content/build.py
python3 tools/m1-content/check.py
python3 tools/m1-content/verify_assets.py
python3 tools/m1-content/verify_runtime_bindings.py
python3 -m unittest discover -s tools/m1-content -p 'test_*.py' -q
python3 tools/m1-content/verify_routes.py
python3 tools/m1-content/verify_state_oracles.py
```

All normal build inputs are committed and generation is offline. Read the
machine setup before builds. Existing Rust dependencies are used with
`--locked --offline`; scratch/targets stay under the owned `.local/` directory.
No global tools, graphics changes or source-account operations are needed.

The route checker selects actual content-3 combined collision states, checks
all136 state products (38 mixed leaves), independently reconstructs636 cells by
original object insertion, and verifies closed/open/closed restoration. All30
walking dependencies and28 explicit player destinations are checked, without
making closed runtime doors disappear or granting another actor permission.
`state_oracles.py` is a strict **post-authoritative-event reference model** for
testing authored guards, entitlements, counters and atomic inventory effects.
Unknown operations are errors; it never executes unresolved transport/valuation
or claims a production game run. `verify_state_oracles.py` maps the 23 separately
authored source graph vectors to actual content IDs and source events.

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
- `runtime_application.py`: applies all104 source replacements and seven coupled
  updates before serialization, then the sole owner-approved independent potion
  event. Source-target/coupled fingerprints remain mandatory.
- `selectors3.py`: actual declared v3 registries, explicit defaults/ground
  origins/eligibility/traversal/collision/recovery and ConsumeOnly burial.
- `schema-check/src/probes.rs`: isolated native checks using the unmodified
  source pack. Controlled test state/RNG boundaries are not a fresh journey.

## Canonical source application

`research/runtime-bindings/resolutions.json` is immutable source evidence.
`tools/runtime-bindings/apply.py` consumes it with the audited
`application-context.json`: eight exact asset-provenance updates and one exact
already-bound parent Wind Strike value are recognized, not broadly ignored.
The generic applicator is idempotent for exact replacements/coupled fields,
refuses changed targets and works on a draft rather than partially mutating input.

`research/m1-bindings/application-result.json` records all applied/retained fingerprints, coupled
grants/entitlements/defaults, approved loot lowering, fixed-price extension and
active/inactive proofs. `verify_runtime_bindings.py` checks canonical equality
against source replacements,8192 joint primary/potion outcomes, five currency
cases, replay/capacity rollback and scoped residual proofs.

The original source oracles remain reproducible against their hash-locked inputs:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 tools/runtime-bindings/validate.py \
  --baseline-commit e9073ab --self-test
```

Do not refresh that original evidence to match a new artifact. The canonical
application adds the resolved fields and owner-approved policy separately.
Ninety-eight source-supported inferences are not observations or owner approvals.

## Optional source reproduction

The source cache is not fetched again. The existing decoded source and small
original-definition supplement have pinned group, payload and decoder hashes.

```sh
python3 tools/m1-content/import_references.py
python3 tools/m1-content/bind_wiki.py
python3 tools/m1-content/import_code_references.py
python3 tools/m1-content/import_runtime_sources.py
python3 tools/m1-content/import_execution_sources.py
```

The runtime importer replays pinned Fire, Shop and Bottomless milk bucket pages.
The execution importer verifies27 pinned code/wiki references for v3 policies;
public reconstruction remains inference. Raw prose stays in
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

The new three-dose source definition and note are reproduced narrowly with the
same command plus:

```sh
  --item-ids 3010 --output research/m1-bindings/application-item-definitions.json.gz
```

This uses original verified cache bytes, not regenerated model/asset data.
Original model2697 and definitions3010/3011 now resolve through the additive
potion publication, including source placeholder19365; all5257 references close.

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

`asset-references.json` identifies the merged catalog and all three publication
manifests. Its asset output records keep canonical extraction-relative paths;
consumers resolve committed extension files through the additive publication's
`published_files[].extraction_path -> path` mapping. All original item/NPC model
references and interface groups are included without invented mesh links.

`verify_assets.py` checks the frozen parent behavior/geometry/graph hashes,
the exact72/68/6/13 requested closure roots, all5257 product references and shard
provenance. When canonical source application is present, it accepts only the
audited before/after behavior fingerprints, exact source/coupled changes and the
two verified approved item identities. It does not skip old asset or geometry
checks. `asset-refresh-baseline.json` is immutable historical evidence.
`runtime3-input-context.json` pins the exact pre-existing upstream audio
publication change; the old258-file export is not relabeled unchanged.

`input-lock.json` hashes source/schema/generator inputs, including the merged
catalog, additive publication, frozen closure request/definition snapshot,
all1137 additive source outputs, collection shards and dependency graph, plus the
source resolution/oracle/value/feed records, exact application context and owner
approval. The content manifest
records compressed/uncompressed output hashes. Ordering and gzip metadata are
deterministic. Source inference is not approval; successful compile, geometry
and reference-model checks remain separate from executing the complete
headless/server/browser journey and approving its presentation.

## Native source-policy conformance

`check.py` and the repeated validation runner pass strict compile/reload,
engine construction, fresh-state/read-only APIs, native ConsumeOnly burial,
valid west-side range contact with distinct Single/Make-X-one deadlines, fresh
manual-drop offline pauses, and the actual120000-tick playtime policy boundary.
`research/m1-bindings/contract-gaps.json` retains the historical failures and
their precise resolutions. No source wall is removed, timer made infinite,
result event injected or type fork introduced to force pass. These component
checks still do not execute the full legitimate journey or accept presentation.
