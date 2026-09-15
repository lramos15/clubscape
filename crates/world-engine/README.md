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
engine.apply_lifecycle(world, actor, LifecycleTransition) -> GameResult<Vec<ActorEvent>>;
engine.reconcile_presence(world, &verified_connected_actors) -> GameResult<Vec<ActorEvent>>;
engine.presence_view(world, actor) -> GameResult<PresenceView>;
engine.tick_context(world) -> GameResult<TickContext>;
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

## Derived physical collision reuse

The immutable initial map and at most eight derived maps are shared through
`Arc`. The bounded derived cache uses exact selected transform states, the
instance template and that instance's temporary-object definitions/tiles.
It is not keyed by tick or world revision: an accepted door/morph/temporary
change within a transaction must take effect immediately. Identical physical
instance copies may share a map; actor traversal/ownership/occupancy guards
remain outside the cache and execute for the actual actor.

Map construction still uses the original complete source replacement and
instance-rotation algorithm. Cached maps cannot be mutated by later opens,
closes, expiration or eviction. The cache is process-local, never persisted
as authority, and stores no RNG, actor permissions or gameplay outcomes.
Poisoned synchronization fails explicitly rather than substituting an old map.

The first canonical open-door profile exposed repeated full-map reconstruction:
roughly9.57 seconds per debug tick versus26-28ms while all objects were initial.
Exact-state reuse reduced the same open-door component to about125-128ms cold
and27-30ms warm in debug, with byte-identical world/event results; optimized
one/five-actor samples were below10ms. These are source-sized component results,
not final server/browser performance or full-journey acceptance.

Reproduce with the actual raw Runtime artifact, without altering its bytes:

```sh
cargo run --release --locked --manifest-path tools/m1-content/schema-check/Cargo.toml \
  --example collision_profile -- /path/to/m1.csc 60 5
```

The profile labels its controlled physical-door selection and retains all
536 canonical entities/46358 cells. The real five-second storage deadline and
600ms source cadence are unchanged; actual player-door traversal is validated
separately through the protocol journey.

## Live adapter: lifecycle and read-only views

The parent `2030e97` adapter can use `apply_lifecycle` for authenticated
join/rejoin, admitted real activity, requested logout, transport loss and auth
revocation. At coordinator restart, reconcile the verified connection set.
These control-plane transitions do not consume gameplay command sequences or
award progress. Repeated polls/reconciliation do not refresh idle time.

Tracked presence is Connected, Disconnecting or Offline; absent old state is
Untracked, not an assumption that the actor is online. Loss/revocation forbids
new input and closes UI, but acknowledged combat/projectiles and source life
phases remain mechanically present until existing source logout/engagement/
interruption rules allow departure. NPC combat does not disappear when an auth
token is revoked. Rejoin preserves acknowledged state. Tick entry points derive
tracked mechanical presence/idle clocks; a supplied legacy context cannot force
an offline actor online. The server still owns auth, leases and durable commit.

Read-only, serializable projections are exposed by `context_view`,
`dialogue_view`, `bank_view`, `bank_deposit_quote`, `bank_withdraw_quote`,
`shop_view`, `shop_buy_quote`, `shop_sell_quote`, `recovery_view`,
`recovery_quote`, `target_view`, `interaction_options` and `ground_item_views`.
Exact public signatures are in
[`spec/game-contracts.md`](../../spec/game-contracts.md#live-lifecycle-and-read-only-projection-boundary).
They reuse the actual engine guards, morph resolution, price formulas and pure
container plans. They never execute an intent, effects/progression, tick or RNG,
and never mutate world state. The server need not copy source price or access
logic. Recovery quotes label full selected quantities; actual partial-capacity
transfers reprice only what commits. Permissions evaluate source preconditions,
not invented outcomes of random/stateful effects.

The precise vital policy `raise_if_at_old_base_otherwise_preserve` implements
the `41d3919` inference: base 10 -> 11 maps current 5/10/15 to 5/11/15.
Mining/Cooking-only awards never require an unrelated HP/Prayer level-up policy.
That source resolution also corrects projectile impact checks to original
identity/life/presence, not old range/LOS or rerolled accuracy. New projectiles
retain target snapshots and use same-edge footprint distance. Death supplies
have tagged, offline-paused active lifetimes in retained world state; ordinary
Office entries share capacity by item key without losing recovery identities.

The `594a4fd` potion decision is **approved_adaptation**, not verified OSRS odds.
Tests independently materialize its 1/16 event/equal 1-4-dose distribution as a
separate 64-weight pool, leaving the 128-weight primary pool unchanged. Product
binding supplies the source items and policy; the runtime hardcodes neither.

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
that session. The original tick wrappers do not invent absent online/idle facts. Tracked
lifecycle state supplies them; legacy callers can use explicit context variants.
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
guessed. The parent's source-resolution inventory in `41d3919` gives exact dispositions
for the original binding paths. The vital enum and approved potion policy are
implemented here; inactive alternatives remain distinct from active requirements.
Product integration must apply the source data and recompute its actual report,
not treat the historical unresolved count as missing executor mechanisms.

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

The current native suite passes 185 engine tests, 107 simulation tests, 82
compiler tests and 12 shared-type tests. The prior closure also passed the
parent isolated PostgreSQL suite and independent account lifecycle; repeat it
for lifecycle/storage integration changes. WASM checks remain compilation/lint,
not browser execution.

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
