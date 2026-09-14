# Shared M1 game contracts

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

## Mechanics extension: content 2, persisted state 1/runtime 1

`CONTENT_SCHEMA_VERSION = 2` versions immutable definitions.
`GAME_SCHEMA_VERSION = 1` still versions the additive character/world envelope;
it is not a content version or a protocol-version change.
`RUNTIME_SCHEMA_VERSION = 1` versions the new `runtime` records. The compiler
artifact and identity-digest domain are version 2. Recompile content-1 artifacts;
do not relabel their headers or manufacture missing source inputs.

The authoritative Rust definitions are `game-types/src/mechanics.rs`,
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

The legacy engine compatibility arms intentionally return `Unavailable` for
new execution and pending typed work; they do not prove M1 gameplay. Downstream
work is: regenerate/compile source content 2; implement the declared schedulers,
targets, guards/effects/events, formulas and death/recovery lifecycle; wire
authoritative UI/presence and durable state/RNG ownership; then execute the
complete real starter journey. Resolve the localized source policies already
recorded for departure, NPC variant/loot supplements, projectile/respawn timing,
valuation/overflow and exact origins/arrivals. Source asset closure, presentation
approval, browser/performance and RuneLite acceptance remain separate.
