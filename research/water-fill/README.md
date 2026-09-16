# Bounded M1 water-fill source closure

**Both exact water candidates now pass the bounded native dispatch gates.**
`M1-WATER-ITEM-ON-DISPATCH` closes the previously reproduced0/4 positive cases
through an explicit optional recipe target rule, not a fabricated sink menu.
The source sink still has no `Fill`, `Use` or other object operation. No wall,
placement, reach fallback or anywhere-water conversion is introduced.

The approval is `approve_bounded_water_fill_upgrade`, hash
`438ba1e44c35f3ad7ebbe8e4c099b02f594df851bd2092a7f767bc4928b4c9d1`.
It authorizes this source work and a **later separately reviewed/fenced** upgrade,
not migration, account access, resets, grants, repeated progress or acceptance.
This work reads only the supplied public content copies, never private checkpoints.

## Explicit item-on target API

`RecipeDefinition.item_on_target` is
`Option<SourceBinding<ItemOnTargetRule>>`, with typed `reach` and `guard`.
Absent values are omitted from serialization; the old content/artifact4
profiles do not acquire a new null field or version. A present rule requires
source records, positive reach, nonempty actual target objects, the
inventory-conversion lifecycle, bound positive single timing and zero menu
delay. Unresolved/malformed rules and mixed Production interaction/menu/direct
declarations are rejected.

Water alone gains the source-qualified reach1/mainland rule. Shared
`TargetAdmission` extracts the existing interaction guard/geometry path:
availability, instance, footprint, source access sides, collision/sight,
stationary access and near-face contact remain the same. Start, projection and
pending execution share the single-conversion mode check. Low-level Produce
can request one matching conversion; Make-X (even quantity1), Make-All and
automatic batches cannot use this channel. No recipe ID is hardcoded in the
generic engine.

## Source rule and qualifications

The pinned Bucket of water creation recipe specifies one bucket1925, Water as
the facility, one bucket of water1929, no skill requirements/XP and one tick per
conversion. The pinned Recipe module establishes quantity1 defaults and an empty
skill/XP list, distinct from its unknown-XP placeholder. There are no tools,
byproducts, currency changes or stochastic outcomes.

`recipe.water.bucket` uses the existing inventory-conversion lifecycle.
Single is the documented one tick. First/repeat reuse that per-conversion
duration as a labeled source-supported interpretation; this is not a captured
startup phase or evidence of automatic batching/Make-X availability. No extra
menu delay is introduced by the intended direct item-on operation.

The raw cache definition is exact: sink14868/model8245,1x2,rotation0,
movement-clipped, not projectile-clipped, access-side mask0, no operations.
Its scenery `animationID=-1` says nothing about actor dispatch. Water-filling
motion stays explicitly **unverified**, not a similarly named sequence or
deliberate silence. Current5b gets one unverified recipe-motion entry; legacy5e's
absent actor-authority block remains absent.

The similarly named Sink wiki article is about building POH sinks13563/13564
for300 Construction XP/five ticks. The filling-buckets money guide is about
Lunar Humidify. Both were inspected and excluded, not imported as the source rule.

The three prior unknowns remain independent and unchanged:
normal-grave Bank-All permission, dough motion and container-Empty motion.
Water motion does not resolve or relabel any of them.

## Two immutable consumer bases

| Profile | Original raw artifact SHA-256 |
| --- | --- |
| Current5b | `5b3ba5f108ed3fec8f8b5f7f49b429c059e21b6f192ec99a0569616e08330059` |
| Legacy5e | `5e0aa8a28851752ae0b8b0a08c635c6c8d2979f509f3a3e564ee74e2abfc8e6f` |

`baselines.json` pins the two public source-format inputs under `inputs/`.
The current compiler reproduces both original5b and5e raw and gzip hashes
exactly; `item-on-compatibility.json` records the actual profile-private compiler
commands and byte identities.
The canonical candidate changes only `/revision`, `/recipes/recipe.water.bucket`
and `/ui/actor_animations/recipes/recipe.water.bucket`. The legacy candidate
changes only the first two paths: it imports none of the current profile's UI,
audio, actor or interface metadata. Exact inverse projection plus full
source/provenance hashes protects every other value, not just gameplay counts.

Canonical outputs are `content/m1/game-content.{json,csc}.gz`.
Legacy outputs are `content/m1/legacy5e-water/game-content.{json,csc}.gz` with
their own manifest. `handoff.json` records current exact hashes, paths and
readiness. `consumer-baseline-differences.json` records all82 existing5e-to5b
differences so a broad upgrade cannot be confused with the minimal water delta.

## Reproduction and native gates

From the worktree root:

```sh
python3 tools/m1-content/validate.py --repeat
```

This executes the existing strict pipeline, both exact old-profile recompiles,
both water native profiles, source oracles/routes and offline driver unit tests.
It exits nonzero on any failed gate; repeatable failure is not acceptance.
Every profile has a distinct worktree-local Cargo target. Generation and tests
that consume generated content run sequentially.

Individual offline gates using the committed source inputs:

```sh
python3 tools/m1-content/water_fill.py
python3 tools/m1-content/build.py
python3 tools/m1-content/build_water_legacy.py
python3 tools/m1-content/verify_water_fill.py
```

Source refresh/recapture is outside this execution. Builds use only committed
inputs and existing locked toolchains. No cache extraction, global installs,
login, network gameplay or migration is performed.

Each native profile exercises three source-clear contacts plus full-inventory
replacement. Both now pass4/4 source conversions and15/15 negative requests,
including the previously masked plane guard. The single Produce primitive,
pending state round trip, target changes before resolution and separately
labeled absent/unresolved definition variants also pass.

Each conversion consumes exactly one empty bucket, produces one water bucket
after one tick, emits one resolution and preserves all skill XP and unrelated
owned state. Repeating the consumed-input command is refused without another
output. That is not a durable server operation/receipt replay test; no server,
account journal or saved state was used. The Office-instance negative case uses
the existing source entry in controlled memory, not the protected account.

`item-on-before.json` preserves the executed before results and causal
plane-guard masking. `item-on-validation.json` records the final native/source
gates and exact target pins. Target hashes describe candidates for review, not
an authorized migration or a newly observed browser journey.

## Approach and Director handoff

The original footprint and wall/sight cells yield these conservative cardinal
contacts: `3205,3214,0`, `3205,3217,0`, `3206,3216,0`. The guessed
`3206,3215,0` is blocked. The walkable west tile `3204,3216,0` has an intervening
wall and is excluded. These are source-derived planning candidates, not source
session observations or substitutes for authorization.

The driver obtains the explicit recipe-specific bound rule before planning,
refuses absent/unresolved/mixed dispatch before any input/reacquisition, and reuses existing
flour/water/bucket instead of repeating already satisfied acquisition. It does
not invent a `Fill` menu, read private state or run a continuation.

Director must review the exact new code/pins/deltas and separately test/fence
any same-world upgrade. New candidates require the corresponding item-on-aware
engine/compiler; retaining content4 is not permission to pair new bytes with an
old implementation.
Migration, actual gameplay, source observations, visual/audio/performance and
owner acceptance remain distinct and unadmitted by these artifacts.
