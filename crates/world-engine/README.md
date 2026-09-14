# clubscape-world-engine

Deterministic, transactional headless execution of the **content-2 /
state-1 / runtime-1** contracts in `clubscape-game-types`. The former
mechanics-v2 rejection arms have been replaced with typed execution. Existing
inventory, equipment, bank, XP and collision primitives remain the mutation
and navigation foundation.

**The complete M1 task remains blocked by the specific integration questions
below.** This is not a claim that the regenerated real content, full Tutorial
Island, Cook's Assistant, source presentation or RuneLite acceptance passed.
Tests use explicitly synthetic content and independent source numeric literals.
There is no product content, source extraction, HTTP, DB, graphics, networking,
wall-clock sleeping or production test-RNG implementation in this crate.

No source revision/cache is hardcoded. The caller supplies validated content
from the current selected source; historical cache 2695 is not a downgrade
requirement or an engine pin.

The authorized parent decoder fix
`7150464f46f0771a01a32231de858fb025a7b6da` is integrated. Tagged/bound
`MaximumHitFormula::LevelTable` now decodes canonical JSON numeric keys while
retaining the source table, not a fixed hit. Owned regression cases deserialize
complete synthetic content and execute Wind Strike at levels
1/4/5/8/9/12/13/99, checking the actual 2/4/6/8 damage bands and rune spending.
Malformed/ambiguous keys are also rejected through the full content wrapper.

The parent reports the actual M1 content rebuilt and strictly reloaded with
raw SHA-256
`88a6f32810712b2f64e2e9a8baa5cdfe0f817e49ac8352f4e4d656c9be995e68`
and **111** unresolved bindings, not 112. The level-table decoder is resolved,
not a remaining engine/source blocker. That artifact verification is separate
from this crate's synthetic execution evidence.

## API, tick boundary and authority

```rust,ignore
WorldEngine::new(Arc<GameContent>) -> GameResult<WorldEngine>;
engine.initial_world() -> GameResult<WorldState>;
engine.character_from_initial(
    actor: ActorId, name: impl Into<String>, appearance: BTreeMap<String, u32>,
) -> GameResult<CharacterState>;
engine.apply_intent(
    world: &mut WorldState, actor: &ActorId, intent: &GameIntent,
    rng: &mut impl RandomSource,
) -> GameResult<Vec<ActorEvent>>;
engine.tick(world: &mut WorldState, rng: &mut impl RandomSource)
    -> GameResult<Vec<ActorEvent>>;
engine.process_advanced_tick(world: &mut WorldState, rng: &mut impl RandomSource)
    -> GameResult<Vec<ActorEvent>>;
```

`ActorEvent { actor_id, event: GameEvent }` is serializable. A tick can return
events for multiple authenticated characters, including the credited contributor
when another player finishes an NPC. The server owns authentication, leases,
operation deduplication, command sequences, content migration and durability.
The engine never changes world revision or acknowledged command sequences.
Do not publish returned events until the **caller's durable commit succeeds**.

`tick` advances one 600 ms source tick. **`process_advanced_tick` never advances
it** and rejects tick zero. `GameStore::commit_tick` already increments its
draft before its callback: use `process_advanced_tick`, not `tick`, inside that
callback. There is no decrement or compensating metadata adjustment. Both
entry points execute the same body; storage retains exactly-once admission.

Source clocks which pause for online/idle/UI state require trusted authority
facts. Use:

```rust,ignore
TickContext {
    actors: BTreeMap<ActorId, ActorPresence>,
}
ActorPresence {
    online: bool,
    idle_milliseconds: u64,
    grave_interface: Option<DeathId>,
}

engine.tick_with_context(world, rng, &context);
engine.process_advanced_tick_with_context(world, rng, &context);
```

The original wrappers supply an **unbound** context, not made-up online/idle
facts. They still support content that does not request such clock inputs;
otherwise the missing input is explicitly unavailable. The parent server must
capture validated session/UI facts for `process_advanced_tick_with_context`.
`TickContext::all_active(world)` is an explicit standalone-simulation opt-in,
not a production default or a player assertion. Loaded offline characters have
paused regeneration deadlines advanced without being healed or acting.
Unloading/reloading characters requires the server to preserve paused clock
phases; the engine cannot infer a missing offline interval from `Activity`.

`RandomSource::draw_below(upper_exclusive)` is trusted-only and checked for range.
All live random outcomes come from that source, including duration, accuracy,
damage, ammunition loss and loot. No client intent can supply rolls, give
items, set counters, award XP or advance a stage.

The caller supplies **compiler-validated content**. Local checks are additional
safeguards, not the strict source parser/compiler or source-fidelity validation.
`SourceBinding::Unresolved` is evaluated through `require()` when that policy
is needed. Unknown input is never a zero timer, guaranteed success, free
recovery, invented arrival or guessed source rate.

## Transactions and restart state

Every public operation drafts gameplay state and commits only on success.
Inventory slots, equipment, bank, NPC lives/contributions, projectiles,
collision transforms, counters, grants, quest rewards, travel and recovery all
participate in the transaction. Invalid content/RNG and failed system phases
roll back the entire tick. Expected ordinary activity interruption rolls back
that actor's attempt and emits an explicit stop message instead of its success
events. A later actor's fatal failure also rolls back earlier actor mutations.

External RNG cursor state is not a member of `WorldState`; the caller must
transactionally restore/persist it when retry-identical draws are required.
Cloning gameplay state does not roll back a trusted external RNG, and this
crate does not fabricate replacement draws after an error.

New characters use declared initial settings/counters and typed scheduling.
Matching-revision legacy scheduling is migrated with
`CharacterState::migrate_engine_metadata`, including validated pending indices
and food/attack deadlines. The engine no longer writes `__world_engine.*`
flags. Unknown legacy keys, orphaned work and conflicting representations fail;
inventory, XP, other flags, quests and acknowledged sequences are preserved.
Life/counter/entitlement migrations are not silently inferred from defaults.

Shared `validate_runtime` and `validate_ledger_successor` enforce reference,
instance ownership, bounds and append-only entitlement accounting. Grants track
actual delivered quantities and satisfied lines. Dropping an acknowledged
initial grant cannot reset its entitlement. Missing-tool recovery is a separate
guarded grant, not a replay of the initial supply.

## Mechanisms executing now

| Area | Actual execution |
| --- | --- |
| Movement | One/two tile ticks; signed inventory/equipment weight contributions; source run activation, rounding, drain, exhaustion, persistent toggle and regeneration. Explicit movement/sight masks and diagonal corner checks. |
| Routing | Source W,E,S,N,SW,SE,NW,NE BFS order, 128-square search, 101-square candidate window, bounded 21-square fallback with distance/path/x/y ties, and a 25-corner route prefix. Interaction still requires authoritative reach; controllers can walk before retrying interaction. |
| World geometry | Named object states and explicit collision replacements, closed-door face interaction without walking through it, object access sides, morph selection, whole-footprint instance rotation, private ownership and source entity/ground placements. Temporary owned objects have actual placement/lifetime/clipping/expiry outputs. |
| Gathering | Current/base source domains, alternate catches, tool/location cadence, actual probability success/failure, XP, competition, fixed/random respawn and source relocation. No unlisted navigation or guaranteed catches. |
| Production | Direct-intent and facility guards, required nonconsumed tools, explicit requirement bases and chance skill, single/first/repeat/menu timing, transactional inputs, real success/failure outputs/XP/effects, typed method/facility/outcome events and interruption. A completed single action does not reserve an unrelated Make-X repeat delay. |
| Firemaking | Owned input is placed on the ground, retained on failure and retryable, removed only on ignition, converted to a real cookable temporary object, awarded XP once, followed by declared legal cardinal step attempts, expiry and ashes/declared outputs. Dynamic targets are not fake static spawn IDs. |
| Inventory/bank | Existing primitives handle stable slots, stack limits, note conversion, equipment displacement and partial up-to transfers. Bank proximity, source guard and the actual opened interaction are rechecked. |
| Grants/counters | Character/world/instance counters enforce declared types/bounds. Atomic and ordered-partial grants implement add/missing/top-up, source container selection, reciprocal entitlements, partial line satisfaction and once-only bank seeding. |
| Progression/UI | Actual `EventCondition::matches`, canonical kind/primary identity, wildcard versus exact targets, post-operation snapshots, one transition per graph, bounded effects, guarded opened speaker/node/choice identity, contextual bank/shop presentation, appearance/experience choices, once-only quest/reconciliation claims and vital restoration. |
| Player combat | Equipped source styles, effective levels/prayer modifiers, attack/defence types, opposed inclusive accuracy, maximum-hit formulas/tables, real misses/damage, damage XP/caps, equipped compatible ammunition and rune consumption. Independent attack/spell/food deadlines survive cancel/switch/restart. |
| Projectiles | Bound launch/flight timing, launch XP, delayed or launch-time damage, source target-life and optional range/LOS/instance recheck, retained spent-resource receipts, and no second spend/XP/hit at visual impact. Unresolved timing rejects the attack. |
| NPC combat | Retaliation, whole-footprint chasing/clipping, separate NPC stat selectors and deadlines, source outgoing damage/nonfatal tutorial constraints, contribution ordering, most-damage credit, no-drop life resolution and source respawn. One player cannot perpetually push back the NPC's retaliation deadline by attacking it. |
| Loot selection | Guaranteed, weighted-exclusive including explicit no-drop entries, independent and guarded pools, bounded quantities, aggregate overflow and unresolved supplements. **Materializing nonempty NPC loot is blocked by the missing ground-policy selector described below.** |
| Food/prayer/vitals | Source food restoration and independent delays, base maxima, declared level-up policy, HP regeneration, prayer requirements/interface/exclusions/modifiers, exact fractional drain with bonus and proper persisted remainders, zero-point deactivation and altar/restoration effects. |
| Shops | Fixed and stock-sensitive per-unit pricing/rounding/clamps, finite shared stock, partial bounded batches, zero-price sales, whole-stack capacity edges and world/explicit/since-change restock phases. Unstocked rows work subject to the validator limitation below. |
| Travel | Source/experience/previous-respawn destinations, channels, interruption causes, cooldown start, private instance creation, completion-only effects and entitled reconciliation. Unresolved reconciliation is not a guessed departure kit. |
| Death | Actual NPC-caused lethal damage, pinned per-unit valuation/retention/layout, protected bank/XP/quest state, first item-losing-death Office, required topics, guarded portal, source restoration, owned graves, active-time pauses/expiry, Office storage and repeat-death resource/supply rules. |
| Recovery | Owner/range/LOS/instance checks; actual fitting quantities only; original layout/optional auto-equip; source fee bands/percentage/payment order; retained remainder identity; no duplicate items/charges on replay; Office limits. Source-explicit pending `Respawning` deadlines resume without rerunning retention. |

A `RecoveryCompleted` ID can identify a partially reclaimed entry; its actual
transferred quantity is in `ItemTransferred`. The entry remains in storage and
is not added to the fully reclaimed ledger until empty. Fee maxima currently
apply to the selected recovery batch, consistent with the selected-value
arithmetic vectors; a lifetime-per-death cap would need cumulative accounting.

Normal death arrival is atomic under the current `DeathPolicy`, which does not
declare an animation/respawn delay. Externally persisted `Respawning` has an
explicit deadline/destination and is supported. Legacy/`Dying` state is not
reinterpreted as a new death (which would lose items twice); it needs explicit
life/phase migration or a source phase scheduler. No presentation timing claim
is made from the atomic arrival.

## Source authoring and integration questions that remain

These are **new, localized issues against the integrated v2 types**, not the
old missing-combat/prayer/death/timing-type list.

1. **Ground-policy selection is absent for two producers.**
   `AmmunitionRequirement`, temporary objects and repeat-death supplies reference
   a `GroundPolicyId`; `GameIntent::Drop` and `NpcCombatMechanics.loot` do not.
   Add an explicit player/stage drop policy selection and
   `NpcCombatMechanics.loot_ground_policy` (or equivalent typed selectors).
   Do not choose the first registry entry or infer a policy from its name.
   Until then ordinary drop and nonempty NPC loot materialization fail explicitly.
   Goblin guaranteed bones cannot substitute for its primary/unknown supplement,
   and Tutorial rat loot cannot be quietly omitted to allow a kill.
2. **Attack eligibility and engagement boundaries need precise bindings.**
   A spawn's `Attack` guard cannot inspect the requested style/method, and
   `CombatStyleDefinition` has no target guard. Add a pre-action method/style
   predicate or allowed-method/style rules for chicken magic-only and the
   tutorial rat method restrictions. The registry also lacks an unarmed/default
   style selector. There is no source disengagement/leash/postcombat
   logout/Home-Teleport lock policy; active retaliation remains conservative
   until defeat/explicit disengagement. Do not call this exact fleeing or chicken
   departure fidelity. Aggressive acquisition likewise needs explicit source
   radius/eligibility; ordinary M1 rat/goblin bindings are nonaggressive.
3. **Mixed-method kill attribution is not stored.**
   `DamageContribution` stores damage and first/last order, not method history.
   `NpcKilled.method` currently names the actual finishing attack, including a
   separately routed credited-owner event. A source rule requiring the credited
   contributor's method for mixed attacks needs a declared attribution policy
   and corresponding per-life history. Current same-method contribution/credit
   behavior is executable; do not infer mixed-method tutorial progression.
4. **Contextual grave UI needs a producer/session binding.**
   `InterfaceContext::Grave` and clock pauses exist, but there is no Open-Grave
   intent/target or grave-interface selector in `DeathPolicy`, and
   `ContainerSession` only covers bank/shop. Reclaim itself validates physical
   access, but a production caller must not fabricate an open grave UI to pause
   its clock. Bind that request/session lifecycle and supply verified
   `TickContext` facts. Global `OpenInterface` cannot bypass contextual access.
5. **Two compiler/runtime-validation restrictions conflict with valid actions.**
   The compiler requires nonempty successful outputs for every
   `InventoryConversion` except firemaking. Legitimate two-tick bone burial and
   timed counter-only mill conversions need consume-only outcomes, not fake
   output items. The executor supports these; the compiler must permit them
   explicitly. Separately, `UnstockedShopPolicy` allows `SinceLastStockChange`,
   but `WorldState::validate_runtime` rejects stock deadlines for rows absent
   from `ShopDefinition.stock`. Permit clocks for authorized unstocked rows;
   do not replace that phase with a guessed epoch.
6. **Mode/phase and overlapping geometry need explicit semantics where used.**
   Shared door collision has no actor-specific traversal guard: once one actor
   opens a shared tutorial exit, another actor can walk its cleared edge without
   satisfying the opener's interaction guard. Add a guarded-edge/traversal
   policy for those exits; do not reinterpret ordinary one-use-key Open guards
   as walking permissions or assume a private tutorial despite shared rat
   contribution behavior.
   Existing production intents distinguish quantities, not Single versus
   Make-X-of-one. This implementation treats quantity one as `single` and larger
   requests as `first`/`repeat`; add a request mode for a separately represented
   Make-X-of-one. Overlapping active object transforms require compatible
   combined replacement cells; conflicting replacements fail instead of
   last-writer-wins clipping. Geometry-changing morphs require explicit clipping
   transitions, not erased native masks. A source death phase delay needs a
   policy binding rather than relabeling an animation duration as a timer.

**Optional/full-target boundaries:** shared simulation primitives intentionally
reject `ItemStack.instance` and conditional stackability. The optional
bottomless/charged milk path therefore cannot be enabled just by its v2 type;
it needs instance-aware primitives and an owner-confirmed reachable acquisition
scope. Ordinary milk and the ordinary M1 quest path do not require that rare
alternative. Partial bank grants are bounded to 57,344 nonstackable units when
composing the existing exact deposit primitive; no ordinary M1 grant approaches
that bound. These limits are not a claim of full-target parity.

The following are **content/source gates**, not excuses to omit their executable
bound counterparts: unresolved projectile timing, NPC variant/supplement,
gather/recipe timing/domain, relocation, death valuation/overflow, exact travel
destinations and departure reconciliation. The executor uses supplied bound
values and preserves provenance. Departure container normalization and goblin
supplement decisions remain explicit assumptions/gaps, not observations.

## Preserved authoring rules

`allowed_actions` is the current effective permitted set. Carry earlier recovery
actions forward while retaining source locks, nonfatal protection and XP caps
until actual departure. Empty locks actions; `*` is explicit unrestricted scope.
Close/cancel/logout remain requests, not ways to reset combat deadlines.

The engine matches `GameEvent::kind()` and `primary_target()` exactly.
`target: None` is a wildcard. `EventCondition` is evaluated only against actual
post-operation events. Old `Produced`/`Hit` cannot satisfy resolved
production/spell predicates. A valid Wind Strike hit/splash can complete a
guarded, entitled Learning the Ropes reward before a chicken kill, while island
caps remain. No graph duplicates the XP its real operation already awarded.

`ChanceRule` supplies the shared interpolation/domain implementation. Source
low/high map to **unclamped low+1/high+1**: copper 101/351/256, shrimp
49/257/256. Do not clamp the high endpoint before interpolation or add a second
`+1` to constant probabilities. Equipment requirements use declared base/current
bases, while recipes and gathering use their separately declared bases.
The source smelting 62 XP-tenths, dagger 125, eight-tick bronze pickaxe,
actual burnt cooking outcomes, shortbow 4/3 ticks and 7/9 range, 20% arrow
loss and three-tick food delay have executable numeric/state cases.

## Validation and ownership

Run from this worktree; build and scratch output stay in the owned ignored
directory. No root lockfile change is required by this implementation.

```bash
mkdir -p crates/world-engine/target/scratch
export CARGO_TARGET_DIR="$PWD/crates/world-engine/target"
export TMPDIR="$PWD/crates/world-engine/target/scratch"
cargo fmt -p clubscape-world-engine --check
cargo test -p clubscape-world-engine --quiet
cargo clippy -p clubscape-world-engine --all-targets --quiet -- -D warnings
cargo clippy -p clubscape-world-engine --all-targets \
  --target wasm32-unknown-unknown --quiet -- -D warnings
cargo build -p clubscape-world-engine --quiet
cargo build -p clubscape-world-engine --target wasm32-unknown-unknown --quiet
cargo test -p clubscape-world-engine --target wasm32-unknown-unknown --no-run --quiet
```

Tests execute pure-state mechanics rather than mocked results, including
negative/full-container/rollback, competing actors, single/queued timing,
true misses/burns, projectile launch/impact/restart, resource spending,
source XP/nonfatal constraints, dynamic doors/fires/instances, partial grants,
typed event predicates, pre-collected/partial quest delivery, real lethal
retaliation/Office/fees/reclaim/expiry and checked legacy migration.
Numeric literals are independent of the implementation, from the retained
journey oracles. Synthetic geometry/content is not a real M1 content pack.

This revision passes **148 native tests**, formatting, warnings-denied Clippy
for native and WASM (all targets), native/WASM builds, and WASM test-binary
compilation. The original advanced-tick regression cases remain in the suite.

Native/WASM compilation and strict Clippy are portability/code gates, not WASM
execution, actual content-compiler journey integration, durable service
acceptance, browser/presentation/performance evidence or RuneLite compatibility.
