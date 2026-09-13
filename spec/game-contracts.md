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

## Parallel implementation

The Director serializes changes to this contract, `crates/game-types`,
Protobuf schemas, migrations and the workspace lockfile. Simulation workers
own `crates/simulation`; content workers own isolated content/compile paths;
renderer and UI workers start only after the source pack is owner-approved.
Every worker reports SQL todo completion/blockers and an independently
reproducible result. The initial source/rule workers do not modify these
shared contracts.
