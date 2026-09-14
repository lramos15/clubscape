# Source-bound M1 content, schema 2

**The actual schema-2 pack compiles in strict Runtime mode and roundtrips through
the version-2 artifact loader.** It contains real definitions and typed mechanics,
not a relabeled v1 envelope. This is not full gameplay, presentation or source
acceptance: exact non-executable source bindings are listed in
[`unresolved-bindings.json`](../../research/m1-bindings/unresolved-bindings.json).

## Build and verify

From the worktree root, with the documented existing Python/Rust toolchains:

```sh
python3 tools/m1-content/build.py
python3 tools/m1-content/check.py
python3 -m unittest discover -s tools/m1-content -p 'test_*.py' -q
python3 tools/m1-content/verify_routes.py
python3 tools/m1-content/verify_state_oracles.py
```

The build uses committed inputs, then invokes the **actual** `clubscape-content`
Runtime compiler. No fixture mode, permissive validator or field-stripping is
used. `check.py` invokes the real strict parser/compiler and artifact loader.
`game-content.json.gz` is the source-format product; `game-content.csc.gz` is the
compressed compiled artifact. The manifest records their exact versions and
compressed/uncompressed hashes. Gzip timestamps/filenames are normalized.

| Content | Count |
| --- | ---: |
| Items / reciprocal ordinary note pairs | 116 / 52 |
| Skills / normal equipment slots | 24 / 11 |
| NPC / object definitions | 26 / 4,837 |
| Runtime spawns: NPC / object / item | 166 / 360 / 10 |
| Recipes / dialogues / interfaces / shops | 11 / 21 / 26 / 1 |
| Typed counters / grants / entitlements | 134 / 17 / 12 |
| Physical door transforms / source door groups | 68 / 49 |
| Transit pairs / typed travel definitions | 11 / 26 |
| Combat styles / spells / projectiles / prayers | 24 / 2 / 2 / 1 |
| Runtime navigation regions / explicit cells | 29 / 46,358 |
| Full source regions / explicit cells | 61 / 999,424 |
| Individually retained source object placements | 151,019 |
| Tutorial states / source edges, all bound | 71 / 73 |
| Cook states / source edges, all bound | 10 / 22 |
| Death source states / source edges | 4 / 6 |

`manifest.json` is authoritative if generation changes counts.

## What is now represented

Source skilling curves retain unclamped endpoints including `+1`: copper/tin
**101/351/256**, shrimp **49/257/256**, with exact level domains and round-nearest
interpolation. Typed cadence distinguishes single/first/repeat/menu delays,
per-tool ownership and bounded respawn distributions. Five cooking/facility
variants preserve success/burn outputs, XP and source range guards; firemaking
creates an owned temporary fire, retains the ground log on failure, and uses
the source cardinal step order. No stochastic activity is replaced by guaranteed
success.

Every tutorial edge now has an authoritative schema-2 hook. Real dialogue
choices, source actor targets, UI contexts, production method/outcome, credited
kill method, valid Wind Strike hit/splash and completed source travel are checked.
There is no client stage-advance command or v1-disabled group of 36 edges.
Interacting is not interchangeable with succeeding, and a generic hit/kill or
invalidated cast cannot award Learning the Ropes.

Ordered grants, line satisfaction and durable entitlements implement partial
Vannaka supplies and missing-only/top-up recovery. The normal inventory remains
empty at creation. **25 bank coins are granted once before first presentation**,
not repeatedly seeded or placed in inventory. Quest rewards use `Once` and
source run-energy restoration. Canonical full energy is **10,000**, not 100.

The mill uses character counters for hopper grain and flour units 0–30,
source varbit 5325 and the actual empty/full flour-bin variants. Loading consumes
grain; controls process only a filled hopper; collecting consumes a pot and one
flour unit. Its original three floors and both ladders remain connected. There is
no grain-plus-pot-to-flour shortcut.

Cook's ordinary milk/flour/egg graph accepts precollected ingredients and every
partial-delivery order. Rewards are exactly 1 QP, 300 Cooking XP, source energy
restoration and range permission, never coins. Bottomless milk 33089/33091 and
its 10,000-charge capacity are retained as a charged full-target alternative.
No Brutus/Ides of Milk acquisition is invented or made a new starter requirement.

Death has a private source-chunk instance, a separate walkable **player** arrival,
all three required topics, guarded portal exit, retained-item/valuation policy,
grave/Office fees, clock pauses and storage limits. It is not a fake quest or a
tutorial reset. Unbound valuation, overflow/restoration and other policies remain
explicitly unavailable rather than giving free recovery.

## Coordinates, doors and stationary NPCs

X increases east, Y north; planes are 0–3 and source scale is 128 units per tile.
Unlisted cells are blocked. Every original scenery placement remains in the full
source bindings, including beyond the navigation envelope. Movement/sight masks,
source placement shape/layer/quarter-turn, bridge-plane data and model transforms
remain separate.

Door states change actual source leaf placement and explicit clipping, without
deleting whole walls or clearing terrain. Double leaves share consistent collision
coverage. Open/close roundtrips restore source cells. Thirty source route segments
connect through those **actual declared open-state masks**, not the v1 door-omission
check map. Source hinge/landing candidates remain inferences, not captured
observations or owner-approved rendering.

Death remains at source map pin **3180,5727**, using `ScriptedActor` navigation and
walkable access tiles. Player arrival is a different, walkable candidate
**3174,5726** inside the private source mapping. Fishing NPC **3317** stays at all
three water coordinates with `NonWalkingResource` policy and real gather
interactions. One published mobile goblin candidate conflicting with source
clipping is retained in the source-only candidate ledger, not moved or exempted
as a stationary combat actor.

## Remaining limits

The former 17 missing v1 representation categories are implemented with v2
registries. Localized `SourceBinding::Unresolved` records remain for observations
such as projectile/NPC/transit phases, exact cooking phases, departure
reconciliation, conditional stacking, price/stock phase conflicts and death
valuation/recovery details. They are compile-valid, **not runtime permission**.
Initial setting/arrival/hinge candidates and all 20 source assumptions retain
their inference and approval status.

The shared tagged numeric-key decoder is now repaired. Wind Strike's known
1/5/9/13 → 2/4/6/8 source table is bound as `MaximumHitFormula::LevelTable`,
not an unresolved field or a constant
max hit.

Current original asset references are retained. Missing model/definition/widget
closure is listed exactly in `asset-references.json`; no substitute art is made.
Penguin NPC 2063/model 21547 remains a candidate, not an approved player.
Source captures, live mechanics execution, persistence, presentation/audio,
browser performance, owner approval and RuneLite acceptance remain separate.
