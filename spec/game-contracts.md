# Shared M1 game contracts

## Authoritative gameplay UI contract, version 1

`GameplayUiView` / `GameplayUiRequest` in `game-types/src/gameplay_ui.rs` and
`GameplayUiView` / `GameplayUiIntent` in `web/shared/contracts.ts` are the exact
shared M1 UI contract. The additive capability is `game.ui.v1`; `WorldView.ui`
is absent only for older unsupported servers, not a successful empty view.
The current view must have `version = 1`. Nullable members mean a genuinely
closed/not-applicable production, reward, confirmation, bank or recovery view.
Rust/protobuf names use snake_case; the TypeScript projection uses the declared
camelCase names. Item DTOs map to the existing `ItemView`; renderer/audio ABIs
are unchanged.

Requests include identity-bound production selection and inventory actions,
native bank entry/tab/preferences controls, presentation dismissal, owned
recovery discard/coffer confirmation and public chat. They never specify
outcomes or grant amounts. Bank entry/reward/confirmation IDs are opaque stable
strings. XP tenths, clocks, bank revisions, monetary totals/coffer balances and
weights cross JavaScript as decimal strings. Local bank search and prayer/spell
filters remain UI filtering over complete authoritative data. Only declared
appearance parameters and the approved penguin base are exposed; audio/minimap/
preview controls retain their separate ownership.

This commit publishes the contract before its consumers. Capability
advertisement requires the subsequent engine/data/protocol implementation and
validation; the types alone are not an implementation or acceptance claim.

This extends the working account service; it does not replace it or mark any
M1 acceptance gate passed. Source selection may change under
`milestones/m1-fleet-resumption.json`.

## Ownership and execution boundaries

`clubscape-game-types` owns versioned content, state, action, event and error
types. It is native/WASM-compatible and has no graphics, networking, database
or platform dependency. Stable kind-first IDs are validated on construction
and deserialization. Item quantities, coordinates and collection bounds are
checked; external cache IDs live in source mappings rather than canonical
runtime indexing.

`clubscape-simulation` will implement deterministic reusable mechanics over
those types. It must not depend on rendering or HTTP. A caller supplies
authoritative time and random draws; tests can control both without exposing
them to players. Source content supplies exact rates, durations, rewards,
guards and initial state. Test fixtures are not real content or acceptance
evidence.

`clubscape-content` will validate/compile source definitions, reference
integrity, graph dependencies and collision/asset mappings. Unknown or
unimplemented action/effect/guard definitions must fail explicitly. Do not
silently substitute a default player, a walkable empty map or free items.

The server owns authentication, one active world-session lease per character,
command sequence/idempotency, the fixed source tick, simulation scheduling and
durable commit boundaries. A browser and the headless client use the same
versioned protocol and authoritative state. The renderer/UI cannot award
items/XP, change quest flags, advance tutorial stages or teleport a player.

## IDs, coordinates, inventory and state

Use kind-first stable IDs for items, skills, regions, spawns, quests,
interfaces, equipment slots, actions and assets. Preserve explicit source
game/build/category/numeric-ID mappings separately. Spawn IDs identify fixed
content placements; actor IDs identify persistent player characters.

World coordinates are OSRS tile X (east), Y (north), and plane 0-3. Rendering
conversion is a separate documented boundary; do not change travel distances.
Regions contain explicit terrain/collision cells and source object placements.
Unlisted navigation cells are not implicitly walkable. Directional movement
and line-of-sight masks are separate. Doors, ladders and teleports use
source-defined guarded interactions, not arbitrary client movement.
Canonical mask bits are N=1, E=2, S=4, W=8, NE=16, SE=32, SW=64, NW=128;
importers must explicitly map source collision flags to them. Diagonal travel
must check both adjoining cardinal paths, not clip through corners.

An ordinary inventory has 28 slots. Stack quantities are positive, at most
2,147,483,647; nonstackable items occupy separate slots. Inventory, equipment,
bank, shop and ground-item mutations are atomic across full-capacity, overflow,
requirement, cancellation and concurrent/replayed-command failures.
Equipment occupancy is data-defined so two-handed conflicts and every
baseline functional slot remain representable without extra cosmetic layers.
Use integer XP tenths and explicit source XP thresholds/rounding; no floating
point accumulation or invented tutorial caps.
Exact XP ceilings and stopping further awards at a level are distinct content
rules. Do not clamp an award at a level threshold when the source instead
allows its final award to cross that threshold and then stops further XP.

Character creation requires a loaded, validated normal-account starting-state
definition. Account registration does not create or pre-complete a character.
Appearance/experience choices may select only supported source-defined
branches. Exact source gaps remain in the fixture provenance; a local
development assumption is not a verified source observation.

## Generic interactions and progression

Runtime content separates reusable action mechanics from quest/tutorial
progression. Movement, gathering, production, combat, banking, shops, food,
prayer and transport produce authoritative events. Declarative transition
guards consume those events and atomically apply allowed effects. Progression
is never a client `advance_stage` command.

Guards cover item ownership/capacity, skill levels, equipment, location,
interface unlocks and flags/stages. Effects cover source-defined grants,
consumption, XP, flags, unlocks, travel and dialogue. A quest reward must have a
one-time guarded transition; repeated delivery/completion cannot duplicate
value. Interruption, death, disconnect and restart are normal state transitions,
not exception paths that lose acknowledged state.

Full Tutorial Island, including its source restrictions, and Cook's Assistant
must be assembled from these shared mechanics. Do not implement isolated
tutorial-only pretend fishing, combat, smithing or quest rewards.

## Network, durability and availability

Extend the existing Protobuf envelope additively; preserve account tags and
version-1 account compatibility. Game messages will negotiate `game.v1`,
establish a world session, submit sequenced intents and retrieve authoritative
revisions/deltas. Use explicit operation IDs for durable deduplication.
Content/region assets stream separately from compact dynamic actor updates;
do not repeatedly transmit the complete world.

Persist validated character/world state and processed commands transactionally
before acknowledging mutations. Lock/lease ownership prevents two sessions
mutating the same character concurrently. Source tick cadence is 600 ms;
network throughput must not allow actions faster than game ticks. A timeout
does not prove rollback: retries use the durable operation result rather than
replaying a grant.

Keep the existing account-only configuration working. When game content is
not configured, report unavailability explicitly. When configured content is
invalid, fail startup instead of silently falling back to account-only mode.
Gameplay availability is not presentation or milestone acceptance.

### Storage/engine tick boundary

`GameStore::commit_tick(lease, expected_tick, callback)` advances the stored
draft to `expected_tick + 1` before invoking its callback. Call
`WorldEngine::process_advanced_tick(&mut WorldState, &mut impl RandomSource)
-> GameResult<Vec<ActorEvent>>` inside that callback. It processes due work at
the supplied positive tick without changing the tick, revision or command
sequences. It rejects tick zero and rolls back all engine mutations on error;
the store owns rollback of its preceding clock advancement and durable
acknowledgement. Actor-event routing remains the server adapter's responsibility.

`WorldEngine::tick` remains the standalone/headless wrapper: it advances one
tick and runs the same shared processing body atomically. Never call it inside
`commit_tick` or decrement reserved metadata to compensate. The engine does
not deduplicate this processing API; storage retains expected-tick validation,
latest tick-receipt replay and lease fencing unchanged.

## Parallel implementation

The Director serializes changes to this contract, `crates/game-types`,
Protobuf schemas, migrations and the workspace lockfile. Simulation workers
own `crates/simulation`; content workers own isolated content/compile paths;
renderer and UI workers start only after the source pack is owner-approved.
Every worker reports SQL todo completion/blockers and an independently
reproducible result. The initial source/rule workers do not modify these
shared contracts.

## Runtime source bindings

`GameContent.interfaces` is the logical source interface registry. Initial
unlocks and interface guards/effects must resolve there; the registry does
not itself claim a rendered control/state matrix. Each definition keeps its
source widget-group IDs and provenance.

`RecipeDefinition.tools` are required but not consumed, and must be held in
inventory or equipment. They are distinct from recipe inputs. Gather-rule
tool lists instead identify acceptable alternatives for the declared gather
method; bait or consumed resources belong in explicit production/action rules.

Canonical run energy is 0 through 10,000 hundredths of one percent. Source
percentages/rates must be converted explicitly; this choice of units does not
authorize a changed run/regeneration formula.

Transition event names match `GameEvent::kind()`. Optional targets match
`GameEvent::primary_target()`; `None` is a wildcard. Movement, death, recovery
and ordinary message events have no primary identity; use explicit state
guards for tile, actor or progression conditions. The mapping is shared rather
than independently guessed by each content/runtime worker.

## Mechanics extension: content 3, persisted state 1/runtime 1

`CONTENT_SCHEMA_VERSION = 3` versions immutable definitions.
`GAME_SCHEMA_VERSION = 1` still versions the additive character/world envelope;
it is not a content version or a protocol-version change.
`RUNTIME_SCHEMA_VERSION = 1` versions the new `runtime` records. The compiler
artifact and identity-digest domain are version 3. Recompile content-1/2 artifacts;
do not relabel their headers or manufacture missing source inputs.

The authoritative Rust definitions are `game-types/src/mechanics.rs`, `execution.rs`,
`runtime_state.rs`, `content.rs` and `intent.rs`. `GameContent.mechanics` holds
typed registries, not an extensible JSON bag. Strict source JSON must contain
every definition field, including explicit `null`/empty collections where
absence is intentional. Ordinary `ItemStack.instance` may be omitted; it means
an ordinary non-instanced item, never an empty charged container.

`SourceBinding<T>` is either `bound { value, source }` or
`unresolved { reason, source }`. Both require provenance. `require()` on an
unresolved binding returns `Unavailable`. The compiler reports all such paths
in `ValidationReport.unresolved_bindings`. A compiled unresolved timing,
probability, valuation or policy is not permission to run a substitute. The
compiler validates structure, references and bounded arithmetic; it does not
authenticate a source observation or approve an inference.

### Chance, requirements and activity scheduling

`ChanceRule.domain` distinguishes literal constant probabilities from a
skill-domain curve. Skill endpoints are **unclamped success counts**, already
including the source `+1`. The source constructor takes low/high parameters:

```rust,ignore
let levels = LevelDomain {
    minimum: 1, maximum: 99, basis: SkillLevelBasis::Current,
};
let copper = ChanceRule::source_skilling(100, 350, levels)?;
assert_eq!(copper.numerator(1)?, 101); // stored endpoints 101, 351; denominator 256
let shrimp = ChanceRule::source_skilling(48, 256, levels)?;
assert_eq!(shrimp.numerator(1)?, 49);  // stored endpoints 49, 257
```

The curve is `min(d, (n1*(99-L) + n99*(L-1) + 49)/98)` with integer
division, after domain validation. Do not clamp endpoints, interpolate with
floor-only rounding, or add another `+1`. Constant rules require equal
endpoints in `0..=d`. A level outside the declared source domain is unavailable,
not clamped into it. `SkillRequirement.basis` is explicit base/current level;
legacy simulation APIs with a caller-selected basis must be supplied that
source basis.

`GatherMechanics` declares the method ID, skill domain, per-tool/location
cadence, bounded respawn distribution and optional relocation. Ordered
alternative catches are tried before the base output's roll. This can bind the
level-15 small-net alternative without making mainland fishing shrimp-only.
`RecipeMechanics` declares its method, **direct-intent as well as menu guard**,
chance skill, tool ownership, success/failure effects and XP, and lifecycle.
`ActionCadence` separates single/first/repeat/menu timing; an unknown single
timing is unresolved, not zero. `TickDuration::UniformInclusive` requires an
independent bounded draw, not the midpoint or fixed minimum.

Legacy uniform fields are explicit compatibility modes: gather
`attempt_ticks`/`respawn_ticks` and recipe `ticks` must be `null` when their
`mechanics` is present. NPC typed combat similarly excludes legacy respawn and
independent-drop fields. A typed weapon excludes the old string-style/speed
projection. New fields must not be ignored while executing the legacy mode.

### Counters, dynamic scenery and live targets

`CounterDefinition` declares character/world/instance scope, boolean or bounded
integer type, explicit initial value and optional source varp/varbit mapping.
Character initial counter maps must exactly match their declarations.
`validate_value` and `checked_add` reject wrong types, underflow and overflow.
`Guard::Counter`, `SetCounter` and `AddCounter` operate on that declared scope
inside the same transaction as item conversion. Hopper grain and flour units
0..30 are counters, not inventory items or quest stages. Source flags remain
available; source definitions cannot initialize/write `__world_engine.*`.

`ObjectTransformDefinition` belongs to a world or instance and supplies named
states with an object identity, tile, placement, optional door position and
explicit collision replacements. Initial state must agree with the source
spawn/cells; all alternatives cover the same cells. Movement and sight
clipping remain independent. Resolve current appearance, footprint, location
and collision from the selected state together. Source object and NPC morphs
reference declared source-variable counters and defined variants; they are not
permission to mutate arbitrary flags or erase clipping.

`NpcNavigation` distinguishes mobile actors from stationary anchors. Mobile
**whole footprints** and all player/travel destinations remain explicitly
walkable. Nonwalking resources require a real gather interaction and walkable
access tiles. Scenery-bound actors additionally require a matching occupied
object placement. A noncombat scripted stationary actor can instead carry
explicit anchor provenance and access tiles. These policies preserve water,
chairs, walls and native clipping; they do not move fishing NPC 3317 to shore or
guess that Death's anchor walks. Choose the policy supported by the source.
`SpawnDefinition.placement` separately records source shape, layer and
quarter-turn orientation, rather than conflating those with NPC facing.

Temporary objects have their own owned dynamic IDs, source object definition,
legal-placement guard, interactions, lifetime, clipping and expiry outputs/
ground policy. Firemaking's lifecycle places the owned input on the ground,
retains it on failure, replaces it only on ignition, awards success XP once,
then performs the declared cardinal step attempts. It does not destroy a log
on failure or duplicate it when another actor wins the tile.

`WorldTarget` distinguishes a static spawn from a live temporary object.
Use `InteractWith`/`ProduceAt` and `ItemTarget::TemporaryObject` for a real fire;
`ItemTarget::Ground` addresses a placed log. Static `Interact`/`Produce` and
their persisted receipts remain compatible. `Activity::ProducingAt` retains
dynamic-facility queues across serialization. `ProductionResolved.facility`
identifies the actual target; a static facility predicate matches only that
spawn, and an absent predicate is a wildcard. Never fabricate a static spawn
ID to make a dynamic fire cookable.

`WorldLocation` is a definition-time destination with an optional instance
**template**. `RuntimeLocation` and `GroundItem.instance` use a live
`InstanceId`. Each `InstanceState` owns its entity/counter/object-state maps.
Chunk mappings preserve source/destination regions, coordinates, planes and
rotation. `TravelDefinition` declares guards, destination/experience branches,
channel time, interruption causes, cooldown start and completion effects.
Reconciliation runs on completed transport, not on a request or animation.

### Authoritative facts, grants and entitlements

The new settings/appearance/experience/reclaim/dynamic-target intents are
requests. There is still no grant, counter-write, XP-award, death-reset or
stage-advance intent. Appearance selections must resolve the declared choices.
Run settings are persistent; a walk request does not confer free energy.

`EventCondition` checks actual interaction/choice identity, production
method/facility/output/outcome, combat style/outcome, credited NPC kill/method,
spell resolution, contextual interface, travel phase, setting, inspection,
death or recovery facts. It is legal in post-event progression guards/effects,
not pre-action state guards. Its kind/target must agree with the transition.
Old `Produced`/`Hit` events do not satisfy resolved-production/spell predicates.
An invalidated cast is not a hit or splash. Valid Wind Strike progression must
use the selected spell/outcome predicate, not an arbitrary hit or chicken kill.

`GameEvent::kind()`/`primary_target()` remain canonical. New targets are:
experience, interface, recipe, spawn (combat/kill/inspection), spell, travel,
item (food), prayer, temporary-object **definition**, transform and counter IDs.
Death/recovery/setting/transfer/appearance events have no static primary target.
`InterfaceAccess::Contextual` cannot be presented by generic tab opening.
`OpenBank`/`OpenShop` establish the source context and commit their
`before_open` effects **before** publishing `InterfacePresented`.

`FreeCapacity` and `OwnsItems` distinguish capacity from combined
inventory/equipment/bank ownership. `GrantDefinition` specifies target
container, ordered lines, add/missing-only/top-up semantics and atomic versus
ordered-partial capacity. Missing-only supplies one missing item; top-up is a
target total, not repeated addition. A bank entitlement can supply exactly 25
coins before the first presentation without changing the initial empty bank.
Pre-presentation grants must be atomic, bank-targeted and once-only.

Grant entitlements are reciprocal with their definition. Their durable ledger
stores amounts actually delivered and the set of satisfied lines (including
lines satisfied by already owned items); `complete` agrees with that set.
Ordered partial supply must never mark the remaining lines claimed or supply
the first line again after it is dropped. Missing-tool recovery is a separate
source handler, not resetting an initial entitlement.

`Effect::Once` wraps an atomic reward with an atomic-reward entitlement.
Do not put partial grants inside it. Reusing a key for different bundles,
nested claims, mismatched purposes, duplicate stage writes and repeatable quest
reward branches fail compilation. `validate_ledger_successor` forbids removing,
regressing or changing acknowledged claims; storage checks it transactionally.
Use `RestoreVital` for HP/prayer/run-energy restoration in the same reward
transaction. Departure uses an entitled `ReconciliationDefinition`, whose
policy can remain explicitly unresolved instead of normalizing possessions.

### Combat, prayers, loot, death and recovery

Styles bind attack type/method, effective level and base/current basis, source
accuracy/negative-roll/max-hit/damage formulas, cycle/reach, XP ratios/rounding
and projectiles. Equipment supplies typed style IDs and compatible equipped
ammo/cost/break/ground policy. Spells bind requirements, runes, launch XP and a
combat or travel action. NPCs separately bind their outgoing stat/type,
effective-level bonus, all five defence stat selectors, retaliation, respawn
and credit policy. These declarations do not implement attacks or spend runes.

Player/NPC deadlines, per-life damage contributions and first/last hit ordering,
retaliation and resolved-loot markers persist independently of activity
cancellation. Projectiles retain source/target life/instance, launch/impact
deadlines, spent resources and resolved outcome. Preserve them on restart;
never re-spend resources or award a kill from an old NPC life.

Loot pools are guaranteed, weighted exclusive (including explicit no-drop
entries), independent, conditional, or unresolved. Exclusive weights must sum
to their declared total; simultaneous maxima cannot overflow an item stack.
Unknown supplements block that resolution, not silently become zero chance,
bones-only loot or guaranteed coins.

Prayers bind interfaces, requirements, modifiers, reciprocal exclusions and
fractional drain/bonus parameters. Proper fractional remainders are persisted.
Vital policy binds HP/prayer skills, regeneration pauses, level-up behavior
and independent ordinary-food/attack delays. Run policy binds Agility,
hundredths-of-percent units, signed item-weight contributions, activation,
drain rounding, exhaustion and regeneration. Eligible online/idle/UI clock
facts come from authority, not client flags; do not infer offline progress or
ten-second idle time merely from an activity enum.

Death is `LifeState` plus owned `DeathRecord`/grave/Office storage, not a fake
quest or tutorial reset. The policy declares its normal unsafe non-PvP domain,
retention/ties, a pinned value-provider reference, respawn and private Office
mapping, arrival/Office-exit restoration, all three first-item-losing-death
topics, active-time pauses, fees/payment order, capacities, overflow and repeat
death behavior. Stored item layouts and effective per-unit values survive.
Retained records describe already owned items; they are not a second spendable
container. Reclaim moves only fitting owned items, charges only that transfer,
and records reclaimed identities. It must not supply free replacement items.
Do not use shop `base_value` as a death-price fallback.

### Instance items and M1 scope

Ordinary `stackable` JSON remains a boolean (`Stackability::Simple`). Source
mode 2 is a `Conditional { source_mode: 2, rule: SourceBinding<...> }` record,
never a coerced boolean. Unbound contexts remain unavailable.
`ItemStack.instance` carries a unique per-item ID, charge kind/remaining amount
and optional origin. Charged variants must be reciprocal, quantities must be
one, and zero charges require the empty variant. Duplicate live instance
ownership fails validation. Charge consumption specifies its inventory-item
selection policy; it consumes charges, not the reusable bucket.

The recorded ordinary Cook route acquires an ordinary bucket of milk. The
retained contracts also name the bottomless milk alternative but do not supply
its acquisition as an ordinary starter-route dependency. Its source IDs
33089/33091, charge behavior and mode-2 alternatives are not deleted from the
full target. **Director decision:** confirm the reachable M1 acquisition/access
boundary before enabling that alternative. This schema neither invents a rare
acquisition/grant nor makes that acquisition a new mandatory starter step.

### Persisted compatibility and downstream integration

Missing `CharacterState.runtime`/`WorldState.runtime`/`EntityState.runtime` and
ordinary instance fields deserialize through documented additive defaults.
Character defaults explicitly mean **legacy engine/life state and unbound
settings**, not a source choice, reset player or completed journey.
`CharacterRuntime::from_initial_definition` copies declared settings/counters
only for new creation. Existing creation retries return the stored character.

Call `CharacterState::migrate_engine_metadata(&content)` only with the matching
persisted content revision and when the executor supports typed scheduling:

| Legacy engine key | Typed destination |
| --- | --- |
| `command_seen` | `runtime.engine.Typed.schedule.command_seen` |
| `gather_interaction`, `dialogue_interaction` | same schedule, preserved one-based indices |
| `access.bank:<SpawnId>`, `access.shop:<SpawnId>` | typed `ContainerSession` |
| `food_ready` | `runtime.food_ready` |
| `attack_ready` | `runtime.combat.attack_ready` |

All keys above include the `__world_engine.` prefix. Migration validates
indices, target kinds, dialogue identity, deadlines and orphaned pending work.
Unknown keys or conflicts fail without mutation. It preserves inventory,
equipment, bank, XP, quests, flags outside that namespace, activity, dialogue
and acknowledged sequences, and is idempotent after success. It does not
silently convert legacy combat/casting or choose new source counter defaults.
Use `CharacterState::validate_runtime` and `WorldState::validate_runtime` with
validated content, alongside existing container/XP/navigation validation.
Storage also validates shape/bounds and append-only reward ledgers.

The executor implements the bound selectors, schedulers, effects, combat and
death/recovery lifecycle. Integration still requires recompiling source content
3 and wiring authoritative presence, requests and durable state/RNG ownership.
Unresolved source bindings for departure, NPC variants/supplements, timing,
valuation/overflow and exact arrivals remain source gates, not invented defaults.
Source asset closure, presentation approval, browser/performance and RuneLite
acceptance remain separate.

### Final selector closure and wire requirements

Content/artifact 3 deliberately rejects the old immutable shape rather than
silently upgrading policy values. Persisted envelopes and runtime remain version
1 with additive defaults: absent contact/history/provenance/arrival fields mean
legacy evidence is absent, not fresh contact or a newly earned entitlement.
Existing acknowledged inventory, XP, quest state and grant ledgers are preserved.

`MechanicsDefinition` adds:

| Field | Contract |
| --- | --- |
| `player_drop: Option<PlayerDropPolicy>` | Explicit ordinary `SourceBinding<GroundPolicyId>` and stage overrides. A selected unresolved override does not fall back to ordinary. |
| `player_combat: Option<PlayerCombatPolicy>` | Source unarmed `WeaponDefinition` and bound combat-state/logout/travel lock durations. |
| `traversal: BTreeMap<TraversalId, TraversalDefinition>` | World/instance-scoped source cardinal edges, directionality and actor guards, separate from opener/key guards. |
| `collision_groups: BTreeMap<CollisionGroupId, CollisionGroupDefinition>` | Explicit combined replacements for every combination of member transform states, bounded to 4096 combinations. |

Every typed weapon has `default_style`, which must be in its offered styles.
Equipping/unequipping retains a still-valid style or selects the declared weapon/
unarmed default without resetting attack deadlines. There is no "first registry
entry" default.

`NpcCombatMechanics` additionally requires four explicit bindings:
`loot_ground_policy`, `eligibility`, `engagement`, and `attribution`.
Eligibility is a list of method, optional exact style and authoritative state
guard rules; spell/weapon ownership and costs are checked independently.
Engagement declares source leash, inactivity, reacquisition/acquisition delay,
return-to-spawn/reset-life behavior and optional guarded aggression. Accepted
outgoing/incoming attacks refresh persisted contact clocks, including misses;
pause/lock lengths remain supplied source values. Acquisition uses eligible
online actors, source range/LOS and deterministic distance/actor ordering.

Method attribution can explicitly select the finishing attack, first/last
contributing method, or greatest damage by method with first/last ties.
`DamageContribution.methods` retains exact per-method damage/order and
`methods_complete` distinguishes complete new history from legacy totals.
Non-finishing attribution cannot fabricate missing legacy history.
`EntityRuntime.kill` persists the credited actor, method, NPC life and tick;
the same result owns drops and routed progression.

Generated ground items use a checked `WorldRuntime.next_ground_id`, with typed
`ground_provenance` recording policy and player/activity or NPC spawn/life/
instance/ordinal origin. IDs are opaque runtime identities, not fabricated
content placements. Ground capacity is bounded to 32768 entries; aggregate
quantity/capacity failure rolls back defeat/XP/loot together. Pickup/expiry
removes live provenance but never rewinds the identity counter.

Ground policies may explicitly bind `clock` to `world_ticks` or
`owner_online_ticks`. New drops freeze the selected clock in their persisted
`GroundProvenance`; later playtime/profile changes do not change an existing
drop's lifetime. Owner-online visibility and expiry deadlines both pause while
the retained owner is mechanically offline. The source item's original
player/activity/NPC/death origin is preserved. Legacy records without a clock
are migrated from their declared policy, or retain the previous origin-based
behavior when legacy content has no clock declaration (DeathSupply owner-online,
other origins world ticks). No lifetime is reset or made infinite.

Player-drop selection orders explicit tutorial-stage overrides, untradeable
policy, bound `before_playtime` threshold, then ordinary policy. The source M1
threshold is 120000 played ticks (20 hours at 600 ms); it is not a quest-point,
total-level or trade/GE requirement. Fresh/private and untradeable drops use
the private 300 owner-online-tick policy, while ordinary tradeable drops retain
100/300 world-tick visibility/expiry. The threshold mapping is a source-supported
inference, not an observed live boundary.

`CharacterRuntime.played_time` records monotonic played ticks and the last
accounted world tick. Newly created characters start at zero when the content
uses the selector; legacy absence remains unknown and requires an explicit
authority-backed migration before a playtime-dependent drop. Unknown history
must not be silently reset to zero. Tracked playtime requires trusted presence,
counts mechanically present disconnecting actors, pauses offline, and cannot
be counted twice at the same processed tick. Ground provenance, deadlines and
playtime are part of the same persisted world transaction and survive restart.
M1 exposes one authoritative world; cross-world transfer of owner-private
ground requires a separate transfer implementation before enabling world hopping.

Solid rectangular source objects are reachable at their near cardinal face:
the target's own solid/opaque footprint does not require center-to-center
movement or sight. The source access-side mask, approaching tile's wall/sight
edges, plane, distance, instance and target availability still apply. This
query does not modify collision; a reachable cooking range remains nonwalkable.
Wall-layer doors retain their explicit, separately validated face-query rules.

Physical collision maps are immutable and use a bounded eight-entry derived
cache. Its exact identity is the current transform selection, instance
template, and instance-scoped temporary definitions/tiles, never tick or world
revision. Same-tick changes therefore cannot reuse stale geometry. Actor
traversal, occupancy and ownership checks remain separate and uncached.
This is process-local derived data, not persisted authority; future physical
inputs added to map construction must also be represented in the cache key.
The original map-construction rules and all source masks remain unchanged.

Traversal guards apply while routing and again to every consumed walking/running
step. A diagonal checks both cardinal routes. Instance rules map source edges
through their chunk rotation; a shared open door never grants another actor
the opener's tutorial permission. NPC physical movement does not acquire player
progression permissions. Overlapping mutable transforms require a combined
group; there is no last-writer-wins clipping. Geometry-changing/absent object
morphs require a source-variable `ObjectMorphCollision` bridge for each placement,
covering all selectors/fallback and matching initial collision. Updating that
counter and its object identity/collision is atomic.

Three new `GameIntent` variants are requests:

```json
{"kind":"produce_selected","recipe":"recipe.example","target":null,"quantity":1,"mode":"make_x"}
{"kind":"open_grave","death":"death.example"}
{"kind":"open_death_office"}
```

`ProductionMode` is `single` or `make_x`; single requires quantity one, while
Make-X-of-one uses the declared first phase. `Activity::ProducingSelected`
persists mode. Existing `Produce`/`ProduceAt` requests and pending activities
retain quantity-derived compatibility semantics. `RecipeLifecycle::ConsumeOnly`
allows actual input consumption with XP/counter/effects and no fake direct
output stack. Ordinary inventory conversion still requires outputs; firemaking
retains its distinct lifecycle. All recursive guard/effect/provenance checks
remain in force, including newly added eligibility/aggression/traversal guards.

`DeathPolicy.interfaces` binds distinct contextual grave/Office interfaces.
Opening validates the owner, source range/LOS/instance and interface unlock,
then persists `ContainerSession::Grave` or `DeathOffice` and emits
`InterfacePresented`. Closing, movement, travel, invalidation and interruption
revoke the session and emit closure. Global tab opening cannot grant it.
`TickContext` still supplies trusted online/idle facts, but its old
`ActorPresence.grave_interface` field is ignored: grave-clock pause comes only
from the actual owned engine session. Reclaim remains an independently
owner/range-checked, atomic action.

`DeathPolicy.timing: SourceBinding<DeathTiming>` separately declares dying and
respawn delays; explicit zero is permitted, unresolved is not zero. Retention
and the owned `DeathArrival` receipt are created once at lethal damage.
`LifeState::Dying` then `Respawning` honor recorded deadlines, restore vitals
only on arrival, and never rerun retention after restart. `GraveState` records
its start tick so the source 1500 active ticks begin after actual arrival or
the authorized first-Office portal exit.

Authorized unstocked shop rows may persist `SinceLastStockChange` clocks.
Runtime validation checks the accepting policy, item form, line limit and
actual stock row instead of rejecting all non-default lines. Source JSON also
rejects collection entries that would be silently normalized away, such as
duplicate members of a declared set.

### Live lifecycle and read-only projection boundary

`LevelUpVitalPolicy::RaiseIfAtOldBaseOtherwisePreserve` encodes the
source-supported inference from `41d3919`: on a base increase, raise current
HP/Prayer to the new base only when it equaled the old base. For base 10 -> 11,
current 5/10/15 becomes 5/11/15. The same conditional applies to that vital
skill's current-level projection. Do not read the vital policy for awards to
unrelated skills or when the vital base level did not increase.

The following engine-owned APIs supplement the existing transactional intent
and advanced-tick APIs. They do not authenticate tokens/leases; callers must
validate authority and durably commit lifecycle changes before acknowledgment.
They never increment world revision or command sequences.

```rust,ignore
apply_lifecycle(&mut WorldState, &ActorId, LifecycleTransition)
    -> GameResult<Vec<ActorEvent>>;
reconcile_presence(&mut WorldState, &BTreeSet<ActorId>)
    -> GameResult<Vec<ActorEvent>>;
presence_view(&WorldState, &ActorId) -> GameResult<PresenceView>;
tick_context(&WorldState) -> GameResult<TickContext>;
```

`LifecycleTransition` is Join, Rejoin, Activity, RequestedLogout, TransportLost,
AuthenticationRevoked or CoordinatorRestart. It is a **trusted control-plane**
transition, not a `GameIntent` outcome setter. Join/rejoin preserves gameplay
state, with no repeated grants, XP or activity restart. Repeated reconciliation
of an already-connected actor does not refresh its idle clock. Admitted real
input can report Activity; automatic polls/heartbeats must not.

Additive `CharacterRuntime.presence` is Untracked (legacy absence), Connected,
Disconnecting or Offline. Transport loss/revocation immediately prevents new
input and closes interfaces. Noncombat activity is interrupted; already
acknowledged combat/projectiles and source life phases remain mechanically
present until the existing source logout/combat/travel interruption rules permit
departure. There is no invented disconnect grace/forced-logout timer, erased
retaliation target or forced-online assumption. Requested logout remains
rejectable while combat/projectiles/life phases prevent it.

At coordinator restart, reconcile the **verified live connection set**, rather
than waiting indefinitely for every persisted actor to rejoin. Disconnected
combat bodies continue source processing; safely inactive actors become offline.
The engine derives tracked mechanical presence and idle milliseconds from
acknowledged state on each tick. A supplied legacy TickContext cannot override
tracked offline state. Pending-disconnect presence means a vulnerable simulated
body, not an authenticated connection. Untracked actors still require explicit
legacy TickContext or lifecycle reconciliation.

Read-only engine query APIs return serializable source-owned views:

```rust,ignore
dialogue_view(&WorldState, &ActorId) -> GameResult<Option<DialogueView>>;
bank_view(&WorldState, &ActorId) -> GameResult<BankView>;
bank_deposit_quote(&WorldState, &ActorId, u8, Quantity) -> GameResult<BankQuote>;
bank_withdraw_quote(&WorldState, &ActorId, u16, Quantity, bool)
    -> GameResult<BankQuote>;
shop_view(&WorldState, &ActorId) -> GameResult<ShopView>;
shop_buy_quote(&WorldState, &ActorId, &ShopId, u16, Quantity, Option<&ItemId>)
    -> GameResult<ShopQuote>;
shop_sell_quote(&WorldState, &ActorId, &ShopId, u8, Quantity)
    -> GameResult<ShopQuote>;
recovery_view(&WorldState, &ActorId, &DeathId, RecoveryStorage)
    -> GameResult<RecoveryView>;
recovery_quote(&WorldState, &ActorId, &DeathId, RecoveryStorage, &[RecoveryItemId])
    -> GameResult<RecoveryQuote>;
target_view(&WorldState, &ActorId, &WorldTarget) -> GameResult<Option<TargetView>>;
interaction_options(&WorldState, &ActorId, &WorldTarget)
    -> GameResult<Vec<InteractionView>>;
ground_item_views(&WorldState, &ActorId) -> GameResult<Vec<GroundItemView>>;
context_view(&WorldState, &ActorId) -> GameResult<ContextView>;
```

These queries reuse the executor's actual guards, resolved morphs, access
sessions, source prices and container/fee planners. They do not execute a
speculative intent, effects/progression, tick or RNG and do not mutate world
state. Bank/shop quotes use the same pure container plans as execution, including
per-unit stock changes and partial quantities. The server must not duplicate
prices or infer permissions. Recovery quotes explicitly price **full selected
quantities**, not promise a future partial transfer will fit. Office views can
show owned remote-grave entries with the Office fee policy; actual transfer and
capacity enforcement remain transactional.

`Permission` carries an allowed flag and typed denial; malformed content remains
an error, not a fabricated empty view. Dialogue exposes only eligible choices
from the actually opened speaker/node. Target views resolve current source
object/NPC identity and availability for the observer. Ground lists omit
private/expired/other-instance items and return checked pickup permissions.
Interaction permission evaluates source preconditions, not an invented outcome
of random or stateful effects. The server still owns interest-window/wire bounds.

The source hit-delay resolution in `41d3919` clarifies that projectile impact
rechecking means original target identity/life/presence, **not old range/LOS or a
second accuracy roll**. Flight uses nearest target-footprint Chebyshev distance.
New projectiles retain an additive target identity/location snapshot; a missing
target invalidates rather than rerolling or spending resources again.

Death-supply ground origins are explicitly tagged; their active expiration
deadline pauses with an offline owner in the retained world. Office capacity
counts ordinary merged item keys (or unique item-instance keys), while original
recovery identities/layouts/fees remain separately recorded. The source's
116-item/120-slot inactive-overflow argument is not valid if that universe or
merge rule changes.

The owner-approved potion policy in `594a4fd` is **approved_adaptation**, not
verified OSRS odds: an independent 1/16 event with equal 1-4-dose weights and
the original 128-weight primary pool unchanged. Existing independent pool
composition supports its exact distribution as a separate 64-weight pool
(four one-weight dose entries and a 60-weight no-drop entry). Runtime code does
not choose those values or source item identities; product binding supplies
them. Executable tests verify independence, one dose, actual ground ownership
and replay safety.

### Shop row identity and empty-row lifetime

Fixed source catalogue rows retain their indices, including rows whose base and
current stock are both zero. An extra row with zero base stock is reclaimed with
its clock after its last purchase or decay. Successful shop trades and ticks also
reclaim previously serialized empty extras. Sale quotes count live extra rows,
not old empty keys. Cleanup does not reset another row's restock phase, and a
positive-base extra at zero remains eligible for its source replenishment.

Extra-row indices are not stable identities: another customer's sale or an
empty-row removal can change their meaning. A shop buy binds the displayed
index to the actual row's `ItemId` with optional `expected_item`:

```json
{"kind":"shop_buy","shop":"shop.example","item_index":2,"quantity":1,"expected_item":"item.tin"}
```

`GameIntent::ShopBuy.expected_item` is `Option<ItemId>` with Serde `default` and
`skip_serializing_if = "Option::is_none"`. Shared TypeScript uses
`expected_item?: string`. The existing `clubscape.game.v1.ShopBuy` Protobuf
message keeps `string shop = 1`, `uint32 item_index = 2`, and `uint32 quantity = 3`,
and adds `optional string expected_item = 4`. Both `WorldInput.shop_buy` (tag 25)
and `QuoteRequest.shop_buy` (tag 3) use that same message. Present identities must
be valid item IDs; an empty string is not an absent identity.

The immutable buy planner checks identity before pricing or transferring items.
Mismatches return `StaleCommand` atomically, including when the expected item
still exists at a different index. Refresh the shop view rather than silently
substituting another row. Identity-less requests may target only immutable
fixed source catalogue indices; extra-row requests without identity return
`RequirementNotMet`. Supplied identities are checked for fixed rows too.

Read-only buy quotes accept the same optional identity as their final argument
and use the same planner as execution. Views, quotes and execution skip old empty
zero-base extras consistently without mutating state during a query. An identity
is not a price lock: every unit is priced from current authoritative stock and
source rules, never a client-supplied price or stale displayed total.

Absent or JSON-null identities serialize without the new field. Golden tests
retain exact legacy JSON, canonical sorted JSON, domain-separated v1 SHA-256
hashes and Protobuf bytes, including historically committed identity-less extra
requests. Their encoding is preserved for durable receipt replay; that does not
authorize a new identity-less extra purchase. No protocol, content or persisted
state version is changed by this additive request field.
