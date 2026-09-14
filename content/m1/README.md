# Source-bound M1 content

**Actual generated definitions and geometry; not a runnable or accepted M1.**
`game-content.json.gz` deserializes as the current shared `GameContent`, including
`interfaces` and recipe `tools`. The real Runtime compiler has been executed and
rejects the retained nonwalkable Death anchor. Other explicitly recorded
mechanic/source gaps also remain. Do not bypass compilation to load this pack.

## Contents

| Authored data | Count |
| --- | ---: |
| Items, including reciprocal notes | 114 |
| Ordinary note pairs | 52 |
| Skills / normal equipment slots | 24 / 11 |
| NPC / object definitions | 26 / 4,837 |
| Runtime object, NPC and item anchors | 478 |
| Recipes / dialogues / interfaces / shops | 5 / 21 / 26 / 1 |
| Runtime navigation regions / explicit cells | 29 / 46,358 |
| Full source regions / explicit cells | 61 / 999,424 |
| Individually retained source object placements | 151,019 |
| Source staircase/ladder pairs | 11 |
| Tutorial states / source transitions | 71 / 73 |
| Tutorial edges with existing-schema hooks | 37 |
| Cook states / source transitions | 10 / 22 |
| Cook edges with existing-schema hooks | 21 |
| Death source states / transitions | 4 / 6 |
| Source activity rules with actual-ID bindings | 52 |

Counts distinguish **authored/source-bound** from **implemented or executed**.
The five recipes are deterministic source conversions/modes, not substitute
100%-success fishing, mining or cooking. Source chance vectors and all missing
mechanic requirements are in the binding records.

The full source graph is retained in
[`graph-bindings.json`](../../research/m1-bindings/graph-bindings.json).
Unsupported edges have explicit gaps rather than a client `advance_tutorial`
command, invented event flags or collapsed stages. Existing hooks use real
dialogue choices, interface opens, equipment changes, successful production and
source-bound travel. They have not been exercised by a complete server journey.

## Build and validate

Read the repository's machine setup before builds. From the worktree root:

```sh
python3 tools/m1-content/build.py
python3 -m unittest discover -s tools/m1-content -p 'test_*.py' -q
python3 tools/m1-content/check.py
python3 tools/m1-content/verify_routes.py
python3 tools/m1-content/compile.py --compiler-manifest crates/content/Cargo.toml
```

The final command needs the **actual independently maintained compiler**, after
the Director integrates it. An existing compiler worktree can be supplied by
path. It runs Runtime mode without field stripping or fixture allowances and
persists the actual result. Failure is expected while the recorded gaps remain.
The ordinary Python build is offline and uses only the standard library and
committed source inputs; it does not fetch or re-extract the cache.

To produce an explicit JSON input and check the actual shared Rust type:

```sh
python3 tools/m1-content/build.py \
  --json-output tools/m1-content/.local/game-content.json
CARGO_TARGET_DIR="$PWD/tools/m1-content/.local/schema-target" \
TMPDIR="$PWD/tools/m1-content/.local" \
  cargo run --locked --offline --manifest-path \
  tools/m1-content/schema-check/Cargo.toml -- \
  tools/m1-content/.local/game-content.json
```

This Rust check is **deserialization, not a replacement compiler**.
The content manifest records source/tool/schema input hashes and compressed and
uncompressed output hashes. Gzip timestamps and embedded filenames are removed;
ordering and derivation are deterministic.

## World and source fidelity

The selected source remains live **build 240 / cache 2695**, because current
discovery selected it, not because the owner imposed that number. Original
terrain, object definitions, models, interface assets and fonts belong to the
source worker's asset namespace; no replacement meshes or icons are generated.

`geometry/*.json.gz` contains full four-plane `RegionDefinition` records.
The GameContent navigation projection retains explicit ground cells and source
upper-floor/placement cells in the connected tutorial/Lumbridge envelope.
Unlisted navigation cells have no walkability fallback. The full scene envelope
and all placements remain in
[`world-bindings.json.gz`](../../research/m1-bindings/world-bindings.json.gz),
including scenery beyond navigation bounds.

Coordinates are X east, Y north, planes 0–3 and 128 source units per tile.
Clipping reconstruction preserves separate movement/sight masks, directional
walls, diagonal restrictions, rotated footprints and bridge plane projection.
Its published algorithm reference is older than build 240; it is explicitly
an **inference**, not an observed live collision dump. Tile settings, source
heights, roof/floor/bridge metadata and all original model transforms remain in
the source assets. Blank source object names have descriptive registry labels;
the exact original names remain in world bindings, not invented source tooltips.

The route checker finds all 30 required walking segments **only with the
declared source Open-able door leaves removed from a separate check map**.
It does not mutate the authored closed-door geometry, approve doors or execute
the tutorial. The 11 actual staircase/ladder pairs connect the cave, cellar,
castle bank and all three mill floors. Landing tiles are explicit inferred
candidates. Death's Office live instancing and tutorial departure remain separate
unrepresented connections.

## Important source identities

- Lumbridge Cook is **4626**, not the initially extracted 225/2895/2896 candidates.
- Current Survival Expert is **8503**; Henja **3306** is a different location.
- The normal used rat is **3313**; 3314/3315 are unused and 9483 is the 2020 variant.
- Tutorial chicken is **3316**. Fishing spots are **NPC 3317**, not cache objects,
  and remain at the three published water coordinates.
- Milkable cows are **objects 8689/60788**, not ordinary Cow NPCs.
- Tree **1277 → model 1570**, recolor **3470 → 5029**, is preserved.
- Goblin **3028** is a level-2 visual-variant candidate; location rows do not prove
  that exact variant at every tile. Its weighted primary loot and uncertain
  potion supplement are not rewritten into independent or guaranteed drops.
- Penguin **2063/model 21547** remains a source candidate, not an approved player.
- Burnt shrimp is **7954**. The valid charged milk alternative is full **33089** /
  empty **33091**; full 33089 and ensouled head 13447 use source stackability
  opcode 160/mode 2 and are not silently coerced into boolean runtime items.

## Starting state, quests and remaining work

Normal initial state has 24 skills, Hitpoints 10 / 1,154 XP, other skills level 1
with zero XP, zero QP and **10,000 = 100% run energy**. No tutorial or quest is
precompleted and no ingredients or starter kit are injected. Empty fresh
containers and the walkable starting-house coordinate are explicitly provisional.
The source requires 25 bank coins **before the first visible opening**; that
once-only bank entitlement is retained, not repeatedly seeded or placed in
inventory.

Both tutorial XP rules are preserved: stop additional awards once base level 3
is reached, and the source contract's provisional ceiling of 275.9 XP. This is
not a clamp to exactly 174 XP. All 20 source assumptions, including initial
containers, departure's 18-kind kit/bank normalization alternatives, first-bread
behavior, rune top-ups, mill ownership and death ties, remain unapproved in
[`policy-bindings.json`](../../research/m1-bindings/policy-bindings.json).

Cook's ordinary ingredient graph accepts precollected items and all partial
delivery orders; it does not require self-gathering or gathering after quest
acceptance. Ground pot/bucket/egg spawns, dairy cows, wheat, hopper, controls,
per-player flour-bin morph and the complete three-floor route are bound.
There is **no grain-plus-pot-to-flour shortcut**. Charged milk needs actual charge
state. Completion retains 1 QP, 300 Cooking XP and range permission, but is blocked
until its source energy refill can be atomic. Learning the Ropes completion
remains tied to valid chicken Wind Strike, not a kill or arbitrary hit.

The Director must resolve the precise
[`contract-gaps.json`](../../research/m1-bindings/contract-gaps.json) requests:
source chance/timing, conditional recipes, authoritative input/result events,
partial/top-up and bank grants, run/departure/death/charged-item state, mill
counters, weighted loot, stock-sensitive prices, dynamic door/morph/interaction
geometry, stationary NPC anchors and source asset closure. Exact source NPC and
arrival/camera observations also remain open; map pins are not observations.

No ClubScape presentation, source-account login, EULA acceptance, GPU test,
audio playback claim, owner approval or gameplay/RuneLite acceptance is supplied.
