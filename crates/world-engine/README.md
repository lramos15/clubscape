# clubscape-world-engine

Transactional headless execution of **content/artifact 3, persisted state 1,
runtime 1**. The final selector/traversal/combat-credit/recovery-UI/validation/
mode-phase integration scope is implemented for bound source data. Unknown
source bindings remain explicit errors; they are not unimplemented stand-ins.

This is not a claim of completed M1 gameplay, regenerated product-content
acceptance, rendering, browser performance or RuneLite compatibility. Test worlds
are explicitly synthetic, with independent source numeric vectors. No product
definitions, source extraction, network, DB, graphics or wall-clock sleeps live
in this crate. No gamepack/cache is pinned by the executor.

## Public APIs and fixed storage boundary

```rust,ignore
WorldEngine::new(content: Arc<GameContent>) -> GameResult<WorldEngine>;
engine.initial_world() -> GameResult<WorldState>;
engine.character_from_initial(actor: ActorId, name, appearance)
    -> GameResult<CharacterState>;
engine.apply_intent(world, authenticated_actor, intent, rng)
    -> GameResult<Vec<ActorEvent>>;
engine.tick(world, rng) -> GameResult<Vec<ActorEvent>>;
engine.process_advanced_tick(world, rng) -> GameResult<Vec<ActorEvent>>;
engine.tick_with_context(world, rng, &TickContext) -> GameResult<Vec<ActorEvent>>;
engine.process_advanced_tick_with_context(world, rng, &TickContext)
    -> GameResult<Vec<ActorEvent>>;
```

`ActorEvent { actor_id, event: GameEvent }` includes routing for credited actors
other than the finisher. The caller supplies **compiler-validated** content and
owns authentication, session leases, operation dedupe, command sequences,
content migration and durable commit. Events become publishable only after
that durable commit, never merely because the in-memory operation returned.

`GameStore::commit_tick` increments its draft before the callback. Call
`process_advanced_tick` or its context variant inside it: neither changes tick,
world revision or command sequences. Standalone `tick` advances exactly one
source tick through the same processing body. The caller schedules the shared
600 ms cadence; network arrival is not a game tick. No compensating metadata
decrement exists. The original advanced-tick regression tests remain.

`RandomSource::draw_below(upper_exclusive)` must be a trusted uniform source.
Ranges are checked, and no seeded generator or player-supplied draw is exported.
The caller also owns durable RNG cursor handling: a cloned gameplay draft
cannot roll back an external RNG.

## Source and wire changes for parent integration

The authoritative additions are in `game-types/src/execution.rs` and
[`spec/game-contracts.md`](../../spec/game-contracts.md#final-selector-closure-and-wire-requirements).
The definition changes are deliberately breaking: **recompile source input as
content/artifact 3**, including every new explicit field/null. Do not relabel
content-2 artifacts. Persisted envelope/runtime versions remain 1 with checked
additive defaults.

| New definition surface | Meaning |
| --- | --- |
| `MechanicsDefinition.player_drop` | Explicit ordinary and per-stage `GroundPolicyId` bindings. No registry-order/name inference or fallback from an unresolved stage override. |
| `MechanicsDefinition.player_combat` | Bound unarmed weapon/default and combat-state/logout/travel lock periods. |
| `WeaponDefinition.default_style` | An actually offered source style; selected on creation/gear changes when the previous style is no longer valid, without resetting deadlines. |
| `NpcCombatMechanics.eligibility` | Method, optional exact style and state guard rules, evaluated before spending resources. |
| `NpcCombatMechanics.engagement` | Leash, inactivity/acquisition/reacquisition timing, source return/reset policy and optional guarded aggression. |
| `NpcCombatMechanics.attribution` | Declared finishing/first/last/most-damage method attribution, separate from actor kill-credit selection. |
| `NpcCombatMechanics.loot_ground_policy` | Source ownership/publicity/expiry of real nonempty loot. |
| `MechanicsDefinition.traversal` | Actor-guarded, directed/bidirectional cardinal edges in world or live-instance space. |
| `MechanicsDefinition.collision_groups` | Complete, bounded source combinations for overlapping transform replacements. |
| `SourceObjectMorph.collision` | Per-placement source-variable-to-transform bridge for geometry/clipping changes or disappearance. |
| `DeathPolicy.interfaces` / `timing` | Actual contextual grave/Office UI selectors and source dying/respawn delays. |
| `RecipeLifecycle::ConsumeOnly` | Real input consumption/XP/counter effects without fake output items. |

New requests (not outcome setters):

```json
{"kind":"produce_selected","recipe":"recipe.example","target":null,"quantity":1,"mode":"make_x"}
{"kind":"open_grave","death":"death.example"}
{"kind":"open_death_office"}
```

`ProductionMode::{Single, MakeX}` represents Make-X-of-one independently of a
single operation. Single requires quantity one. Existing `Produce`/`ProduceAt`
and pending activities retain quantity-derived compatibility semantics.
`Activity::ProducingSelected` persists explicit mode.

The parent GUI adapter should send `OpenGrave`/`OpenDeathOffice` and consume
`InterfacePresented`/`InterfaceClosed`; it must not forge access with a generic
`OpenInterface`. Real opened sessions live in
`EngineSchedule.access::{Grave, DeathOffice}`. `TickContext` provides trusted
online/idle facts, not a client claim:

```rust,ignore
ActorPresence { online, idle_milliseconds, grave_interface }
```

The last field is retained only for adapter source compatibility and is
**ignored for authority**. Grave pause is derived from the validated engine
session. Closing, moving, leaving the instance, expiry and interruption revoke
that session. The original tick wrappers do not invent absent online/idle
facts; use context variants for policies requiring them.
`TickContext::all_active` is an explicit standalone-test/simulator opt-in.

## Actual execution coverage

* **Movement and geometry:** real shared clipping/LOS primitives, source-order
  bounded routing, one/two-tile ticks, signed weight contributions, run setting,
  exact drain/regen rounding and exhaustion. Traversal guards are checked while
  routing and again for every consumed step, including both diagonal cardinal
  routes. A shared open exit does not unlock a second actor. Source edges and
  whole object/NPC footprints rotate into live instances; ownership is preserved.
* **World state:** explicit object/door transformations, access sides, reachable
  closed-door faces, complete combined-state collision replacements, and atomic
  geometry-changing morph bridges. There is no last-writer-wins clipping or
  erased native wall fallback. Temporary owned objects have source placement,
  interactions, lifetime, clipping and expiry products.
* **Gathering/production:** source level domains/bases, tools/location cadence,
  alternative catches, real success/failure and XP, contention/depletion,
  fixed/random respawn and relocation; single/first/repeat/menu phases, direct
  and facility guards, unconsumed tools, atomic input/output/outcome effects,
  consume-only actions and interrupted/full-inventory behavior. Firemaking
  retains the actual placed log on failure, creates a real cookable temporary
  target only on success, steps legally and produces expiry outputs.
* **Inventory, grants and progression:** existing inventory/equipment/bank/XP
  primitives, exact slot/note/overflow behavior, bounded partial transfers;
  typed counters, add/missing/top-up grants, ordered partial line satisfaction,
  once-only entitlements, first-visible bank seeding and vital restoration.
  Dialogue requires the opened speaker/node and choice guards. Transitions use
  actual canonical event kind/identity and typed payload conditions, with at
  most one edge per graph per operation. Pre-collected and partial quest
  deliveries do not require fake provenance flags.
* **Combat and loot:** source equipment/default/unarmed styles, method/style
  eligibility, effective levels/prayer modifiers, opposed inclusive accuracy,
  maximum-hit formula/table, cardinal melee contact, real misses/damage/XP,
  ammo/runes and independent cooldowns. Projectiles preserve launch/impact,
  spent resources and target life across restart. Retaliation, clipping-aware
  chase/return, aggression eligibility and source lock/timeouts execute.
  Contribution totals and complete per-method history choose one persisted
  kill resolution; it drives both credited events and actual owned guaranteed/
  exclusive/independent/conditional loot. Unknown supplements are not dropped
  from the table or replaced by guaranteed bones/coins.
* **Food/prayer/shops:** source healing and independent food/attack delays,
  proper fractional prayer drain/bonus/modifier/exclusion state and HP/vital
  policies; guarded bank/shop sessions, fixed and per-unit stock-sensitive
  prices, partial bounded batches, zero-price sales and all declared restock
  phases, including authorized unstocked since-change clocks.
* **Travel/death/recovery:** source/experience/previous-respawn destinations,
  channels/interruption/cooldowns and completion-only reconciliation. Actual
  lethal damage creates retention/valuation/layout and a death-phase receipt
  once; persisted dying/respawning deadlines restore vitals only at arrival.
  First item-losing death reaches the private Office; all required topics gate
  the portal. Source-owned grave/Office UI, active-time pauses/expiry, repeat
  death/supplies, capacities, partial reclaim/layout/auto-equip, fee/payment
  order and remainder identity are transactional and restartable.

The source Wind Strike level-table decoder fix is retained. JSON-decoded live
spell executions verify the 1/5/9/13 -> 2/4/6/8 table and all boundaries, not a
fixed hit. Other numeric cases preserve copper 101/256, shrimp 49/256, source
rounding/+1, eight-tick bronze pickaxe, smelting 62/dagger 125 XP-tenths, actual
burnt cooking progression, exact cap versus stop-level, shortbow 4/3 ticks and
7/9 range, 20% arrow loss, food delay three and nonfatal tutorial combat.

## State integrity and migration

Public failures preserve touched gameplay state. Expected ordinary activity
interruption rolls back that attempt and emits a stop message; fatal
content/RNG/system failures roll back the whole tick, including earlier actors.
Acknowledged grant/reward ledgers remain append-only.

Checked legacy scheduling migration removes recognized `__world_engine.*`
metadata into typed fields and rejects unknown/conflicting/orphaned data.
The executor writes no new legacy flags. New optional contact/arrival fields
default to absent, and old contribution totals get **incomplete** method
history, never invented history. A method-attribution policy requiring complete
history fails explicitly until that legacy state is migrated.

Generated ground IDs use a persisted monotonic counter; typed provenance
records the policy and actual player/activity or NPC spawn/life/instance/
ordinal. Pickup and expiry remove live provenance, not the counter. A 32768
ground-entry bound and aggregate stack checks prevent partial defeat/loot
commits. `EntityRuntime.kill` records the same credited actor/method used for
ground ownership and progression.

Death receipts retain `DeathArrival` phase deadlines and completion, and
`GraveState.started_at_tick` prevents charging the arrival tick against active
grave duration. Restart does not repeat retention, healing, supplies or
entitlements. A partially reclaimed entry retains its identity and remainder
until fully reclaimed; `ItemTransferred` records the actual moved quantity.

## Remaining acceptance and source boundaries

The seven **code/selector integration gaps are closed**. Source inputs still
require capture/binding; unresolved projectile timings, particular loot
supplements, valuation/overflow, exact arrivals and departure policies are not
guessed. The parent's last content-2 reload reported 111 unresolved bindings;
that is source evidence, not a count of missing executor mechanisms. Recompute
the source report after generating content 3.

Departure normalization and goblin supplement assumptions remain explicit,
not observations. Optional inaccessible charged/bottomless milk alternatives
remain outside the ordinary M1 acquisition route and require instance-aware
inventory primitives before enabling; their full-target parity is not removed.
Current high-value recovery caps apply to the selected batch; ordinary starter
values are below the fee threshold. Nonstackable bank grants are bounded when
composing the existing exact deposit primitive. These are not full-game claims.

Product-data refresh and the live server/protocol/GUI adapter remain owned by
their respective workers. This code does not self-certify the complete real
journey, persistence-service rollout, owner-approved source pack, presentation,
browser/performance or RuneLite gates.

## Reproduce validation

The closure revision passes 169 engine tests (the previous 148 plus 21 direct
closure scenarios), 107 simulation tests, 81 compiler tests and 12 shared-type
tests. Warnings-denied native/WASM Clippy and both builds pass. The parent
isolated PostgreSQL runner also passed 26 selected integration tests and its
independent account lifecycle, with owned resources cleaned up.

From the worktree root, keeping build/scratch/evidence under this crate:

```bash
mkdir -p crates/world-engine/target/scratch
export CARGO_TARGET_DIR="$PWD/crates/world-engine/target"
export TMPDIR="$PWD/crates/world-engine/target/scratch"
cargo fmt -p clubscape-game-types -p clubscape-content -p clubscape-world-engine --check
cargo test -p clubscape-game-types -p clubscape-simulation -p clubscape-content -p clubscape-world-engine --quiet
cargo clippy -p clubscape-game-types -p clubscape-content -p clubscape-world-engine --all-targets --quiet -- -D warnings
cargo clippy -p clubscape-game-types -p clubscape-content -p clubscape-world-engine --all-targets --target wasm32-unknown-unknown --quiet -- -D warnings
cargo build -p clubscape-game-types -p clubscape-content -p clubscape-world-engine --quiet
cargo build -p clubscape-game-types -p clubscape-content -p clubscape-world-engine --target wasm32-unknown-unknown --quiet
python3 tools/dev.py integration --report crates/world-engine/target/storage-integration.json
```

The isolated parent PostgreSQL runner uses a uniquely named disposable,
loopback-only database, runs the real ignored storage/account suites and an
independent client, then removes its processes/container and secrets. Its
report is not a gameplay/presentation acceptance claim. WASM builds and Clippy
are portability gates, not browser execution evidence.
