# clubscape-world-engine

Reusable, deterministic **headless action/tick core** over `clubscape-game-types`
and `clubscape-simulation`. **Not a complete M1 engine or an accepted journey.**
Several required mechanics cannot be represented by the current shared
contracts; they are explicitly unavailable, not replaced by scripted success.
The exact integration questions below are part of this deliverable.

All fixtures in `tests/` are synthetic. No source assets, product definitions,
source extraction, HTTP, database, protocol, rendering, networking, wall-clock
sleeps, or player-accessible RNG overrides are included. No source build/cache
is pinned by this crate. The selection remains the integration owner's latest
verified selection, not a requirement to downgrade to an older build.

## Public API and authority

```rust,ignore
WorldEngine::new(content: Arc<GameContent>) -> GameResult<WorldEngine>;
engine.initial_world() -> GameResult<WorldState>;
engine.character_from_initial(
    actor_id: ActorId,
    display_name: impl Into<String>,
    appearance: BTreeMap<String, u32>,
) -> GameResult<CharacterState>;
engine.apply_intent(
    world: &mut WorldState,
    authenticated_actor: &ActorId,
    intent: &GameIntent,
    random: &mut impl RandomSource,
) -> GameResult<Vec<ActorEvent>>;
engine.tick(
    world: &mut WorldState,
    random: &mut impl RandomSource,
) -> GameResult<Vec<ActorEvent>>;
engine.process_advanced_tick(
    world: &mut WorldState,
    random: &mut impl RandomSource,
) -> GameResult<Vec<ActorEvent>>;
```

`ActorEvent { actor_id, event: GameEvent }` is serializable. `RandomSource` has
one method, `draw_below(upper_exclusive: u32) -> GameResult<u32>`. The trusted
provider must supply independent uniform draws. Out-of-range draws fail.
Certain/impossible outcomes do not draw. The crate exports no seeded generator.

The caller **must supply compiler-validated content**. Constructor/runtime
checks add local safeguards; they are not the product content compiler,
reference-integrity acceptance, or source fidelity certification. The supplied
initial state is copied, not replaced with a boosted player/default inventory.
The caller authorizes name/appearance choices; appearance never changes
mechanics, flags, stages or possessions.

The server owns authentication, active-session leases, operation deduplication,
command sequences, durability, content migrations and world revisions. No engine
operation changes `WorldState.revision` or `last_command_sequence`. There is no
grant-items/set-flags/advance-stage client intent.

World time starts at zero. `tick` advances exactly one source tick; the caller
schedules the shared 600 ms cadence and feeds queued intents at its processing
boundary, **not on arbitrary network arrival**. At most one intent per actor
is accepted at a given world tick. Activity cadence is separate; canceling or
switching a target never brings a pending action forward. Requests accepted at
tick zero can have their first eight-tick gathering attempt at tick eight.
Actor iteration is sorted by `ActorId`, not a claim to reproduce OSRS PID ties.

### Storage-owned tick advancement

`GameStore::commit_tick(&lease, expected_tick, callback)` advances
`WorldState.tick` to `expected_tick + 1` **before** invoking the callback.
Inside that callback, call **`engine.process_advanced_tick(world, random)`**,
not `engine.tick`. The new entry point processes all due work at that supplied
positive tick without advancing or decrementing it. Tick zero is rejected.
Standalone/headless schedulers continue to use `tick`, which advances once and
executes the same extracted tick body.

Both paths are transactional. On error, `process_advanced_tick` leaves the
callback's supplied world (including its already advanced tick) unchanged; the
store then rolls back its transaction/clock advancement. Standalone `tick`
instead leaves the caller's pre-advance world unchanged. Neither path changes
reserved revisions or command sequences. Actor-tagged results still require the
server's routing/receipt adaptation and publication only after durable commit.

The engine entry point does not deduplicate repeated processing of the same
tick. Storage retains ownership of expected-tick admission, latest-receipt
replay and fencing; retries use the same `expected_tick`, and a stored replay
does not invoke the callback. Do not decrement reserved metadata to compensate
for two clock owners.

## Transaction and persistence contract

Public operations draft the complete world and commit only on `Ok`. A failed
operation leaves all gameplay state, inventory slots, entities, stock, clocks
and progression unchanged. Only returned events are candidates for publication,
and **publication/acknowledgement must wait for the parent's durable commit**.

During a tick, an expected activity failure (depletion, loss of reach/tools,
capacity, or a newly locked guard) rolls back that activity's mutations and
success events, stops it, and returns an explicit `Message`. Invalid/unsupported
content or an invalid trusted RNG rolls back the **entire** tick, including
earlier actors. Empty success-event lists for waiting ticks/failed gathering
rolls do not award anything.

RNG-provider state is external and cannot be rolled back by cloning the world.
If retries must reproduce the exact draws, the server must transactionally
restore/persist the RNG cursor as well. Do not expose RNG seeds to players.

Scheduling and access state survive serialization in the existing
`CharacterState.flags` map, without a fork of the shared structs:

* `__world_engine.command_seen`: distinguishes a new character from an accepted
  tick-zero action; `last_action_tick` is still the actual accepted tick.
* `__world_engine.gather_interaction` / `dialogue_interaction`: one-based indices
  into the selected spawn's interaction list, tied to `content_revision`.
* `__world_engine.access.bank:<SpawnId>` / `access.shop:<SpawnId>`: selected
  source interaction index. Only a successful, nearby source interaction creates
  these sessions. Every transfer rechecks target, range, LOS and source guard.
* `__world_engine.food_ready` / `attack_ready`: independent absolute deadlines;
  ordinary food adds three ticks, and cancellation does not clear them.

These keys are engine-owned, documented persisted metadata, not source flags
or client capabilities. Initial content and `SetFlag` effects cannot write
the namespace. Source guards may read it. Closing UI, movement, dialogue and
other interrupting actions revoke relevant access. A future typed shared
runtime-state field can replace these keys with a deliberate migration.
Deadline values are bounded by `i64::MAX`.

The caller must distinguish online/simulated characters from offline persisted
characters. This engine does not invent session presence, idle-time, offline
regeneration or grave-clock rules. World cloning prioritizes correctness; it is
not a performance result for a populated source world. Idle/not-yet-due actors
do not require additional per-actor world drafts.

## Implemented execution

* **Static navigation and interaction:** real bounded pathfinding over explicit
  source collision cells; one legal walking step per tick; no missing-cell,
  cross-plane or diagonal-corner shortcuts. Uses the shared collision/LOS
  primitive, not straight-line movement. Interactions validate live entity
  availability, guard, plane, range, sight and short-range movement edges.
  Object quarter-turn facing is 0..3; multi-tile footprints participate in
  reach. Guarded source travel links can change plane/region.
* **Gathering:** chosen source interaction, current skill requirement,
  inventory/equipped alternative tools, source attempt cadence, trusted integer
  success/depletion rolls, output, XP, depletion and fixed respawn. Full capacity,
  another actor's depletion and interruption never duplicate resources or XP.
  Failed attempts award nothing and retry at the full cadence.
* **Production:** exact facility recipe allowlist and guard, current-level
  requirements, all nonconsumed tools, queued cadence, transactional input/output
  conversion, actual failure outputs, and success-only source XP. Capacity is
  checked for every possible outcome before rolling. Walking/canceling loses
  no inputs; a missing/depleted facility stops before consumption. Item-on-item
  and item-on-world use can resolve an unambiguous compiled recipe; ambiguous
  selection requires the explicit `Produce` intent.
* **Inventory/equipment/bank:** existing primitives, including stable slots,
  notes, source equipment slots, displacement, overflow, and base-level equip
  requirements. Source bank sessions are mandatory. Up-to-quantity banking uses
  bounded trials of the transactional exact primitive and retains remainder.
* **Fixed-price shops:** source-declared rows, guarded open sessions, shared
  finite stock, exact integer currency, bounded partial batches, zero-price
  sales, and per-row periodic movement toward base stock. Row restocks are
  phased from world tick zero, with the next aggregate deadline persisted.
  This is **not** Lumbridge General Store's stock-sensitive pricing.
* **Ordinary food:** `ItemDefinition.healing`, one owned item, healing capped at
  XP-derived base HP without removing an existing overheal, and independent
  three-tick food/attack delays. No food XP; burnt/nonfood items are rejected.
  This binding currently uses semantic `skill.hitpoints` and ordinary M1 food,
  not potions, combo foods, stat boosts or complex item effects.
* **Progression:** bounded declarative guards/effects, atomic item/XP/flag/
  interface/travel/quest mutations, exact canonical event matching, guarded
  dialogue entries and choices, and no arbitrary node/speaker injection.
  Current source stage XP caps and stop levels are passed to shared XP
  primitives. No activity XP is implicitly re-awarded by progression.
* **Source ground spawns:** legitimate take at the actor's tile, owner/public/
  expiry checks, exact inventory transfer and respawn generation. Existing
  expiring ground entries expire on source ticks. Ordinary player drops and
  NPC/death loot creation remain unavailable.

### Chance binding: do not lose the rounding or add `+1` twice

`ChanceRule` endpoints are **unclamped success-count endpoints**. Evaluation is:

```text
min(denominator, floor((n1*(99-level) + n99*(level-1) + 49)/98))
```

For the source skilling low/high formula, the compiler must supply **low+1 and
high+1**, and must not clamp the high endpoint before interpolation. Copper is
`101, 351, 256` (not `100,350,256` or `101,256,256`); shrimp fishing is
`49,257,256`. Level-one outcomes are 101/256 and 49/256. Literal constant
probabilities (including depletion) do **not** receive another `+1`.
Nonconstant rules outside levels 1..99 are explicitly unbound.

Level-dependent recipe chance currently needs exactly one distinct required
skill; multi-skill/unspecified chance bases return `Unavailable`. This is not a
silent selection of the first skill.

### Progression authoring contract

Transition names/identities match `GameEvent.kind()` / `primary_target()`.
`target: None` is a wildcard; a non-`None` target is exact. Undefined event
names, nodes and stage references fail. A missing flag does not satisfy a
predicate, even under negation. `HasItems` aggregates duplicate requirements.

Each graph can take at most one edge per operation. Guards see one
post-mechanic snapshot before transition effects. Emitted progression/XP
events do not recursively skip stages. A graph already advanced by the
interaction/dialogue's direct effects cannot advance again in that operation.
Multiple edges matching the same event are invalid, not arbitrary first-wins.
Across events, the earliest matching event selects the graph's edge.

Completed quests cannot be reset. An `AddQuestPoints` operation must atomically
complete a previously incomplete quest; a reward-only/replay grant is rejected.
Source content must still guard the correct quest, amounts and claim ledger.
Partial delivery can use ordinary guarded dialogue effects and quest states:
it does not require post-start acquisition or provenance flags.

`TutorialStageDefinition.allowed_actions` is the **effective current allowed
set**, not entry-only unlock deltas. The compiler must carry earlier unlocked
recovery actions forward while retaining the rat/chicken/exit restrictions and
XP limits until actual departure. `[]` locks actions; `["*"]` is an explicit
unrestricted stage. Close/cancel/logout remain possible. Recognized keys are:

```text
walk, interact, interact:<SpawnId>, interact:<SpawnId>:<interaction-name>,
dialogue, open_interface, open_interface:<InterfaceId>, equip, unequip, drop,
take_ground_item, use_item, move_inventory, eat, produce, produce:<RecipeId>,
bank, shop, shop:<ShopId>, gather, gather:<SpawnId>,
combat_style, cast, cast:<spell>, prayer, prayer:<prayer>
```

Gather/bank/shop interactions additionally check their own family permission.
Individual interface unlocks remain mandatory even in `*` stages. Allowing a
key does not enable an unimplemented mechanic.

## Exact contract extensions/integration blockers

These are **required for complete M1**. No caller should enable real M1 on the
basis of the synthetic tests or bypass these errors.

1. **Running/navigation fidelity:** add signed source item weight (including
   equipped/inventory exceptions), a source Agility binding, persistent run
   setting, activation/drain/regeneration policy and corresponding intent/event.
   `Walk { running: true }` and persisted running currently return `Unavailable`;
   the shared primitive's two-step support is not free infinite run energy.
   Shared BFS currently ties N,E,S,W,...; the source contract requires
   W,E,S,N,... plus bounded fallback/corner compression and approach/repath.
   The static shortest path is real but not that complete source router.
2. **Doors/fire/mill:** add typed dynamic collision/object-transform effects
   and persisted overrides, temporary owned objects/fire lifetimes, and their
   action timing. `SetFlag` cannot open collision, create a fire or operate an
   unbound hopper. Firemaking needs the placed log retained on failure, actual
   ignition, competing ownership, legal post-light step, lifetime and ashes.
   Random tree respawn and moving fishing spots also need timer/movement rules;
   current `respawn_ticks` is fixed. Capacity predicates/per-character bounded
   counters are needed for faithful partial grants and mill overflow.
3. **Recipes:** add explicit `chance_skill` plus single/first/repeat/menu timing
   rather than one `ticks` value. Current fixed cadence is not source Make-X
   (e.g. bronze single 6, first 4, repeat 5; cooking first/repeat differ).
   Source tool location/tier-dependent cadence and alternative catches also
   need explicit rules; do not represent higher-level net fishing as permanent
   shrimp-only output.
4. **Combat:** add source style definitions (attack/defence types, base/current
   skill bindings, style modifiers, per-style reach/cadence and XP splits),
   compatible equipped ammo, spell definitions/rune costs, max-hit scaling,
   projectile launch/impact rules, and persisted independent cooldown/projectile
   state. Add NPC outgoing attack type, retaliation target/deadline, per-life
   damage contributions and kill-credit ties. `Attack`, `SetCombatStyle`,
   `Cast`, persisted combat/casting, and aggressive NPC content are unavailable.
   **Ammo/runes are not currently consumed by live combat; no combat acceptance
   or nonfatal/cap integration is claimed.**
5. **Loot:** `Vec<DropDefinition>` cannot distinguish guaranteed drops,
   mutually exclusive weighted primary pools, independent tertiary rolls and
   unresolved supplements. Add typed pools and per-life drop/owner identity.
   Do not turn goblin weights into independent drops, or substitute guaranteed
   bones/coins for the primary table/unknown energy-potion supplement.
6. **Prayer/vitals:** add prayer definitions, allowed interface, modifiers,
   persisted activation/fractional drain accumulator and altar/vital effects.
   Thick Skin's 105/100 Defence and 60-tick zero-bonus drain must be bound, not
   guessed from a string. HP regeneration/level-up vital restoration policy is
   also not implemented. `SetPrayer` is explicitly unavailable.
7. **Death/recovery:** add source death valuation (not `base_value`), retained
   per-unit items/layout, grave/Office storage, first-item-losing-death state,
   all three topic acknowledgements, source respawns, active-time pause clocks,
   fees/coffer/bank payment, ownership, partial reclaim and repeat-death policy.
   HP-zero actors fail rather than receiving a fake recovery/reset. The
   synthetic three-topic portal test proves only dialogue guards/travel, not
   a grave, death occurrence or real Death's Office.
8. **Event-specific progression:** add typed guards on `Interacted.action`,
   `DialogueSelected.choice`, produced success/output, combat style/spell/
   hit outcome and relevant authoritative event facts; add explicit spell-
   resolved, item-transfer, food, prayer and recovery events. State-only guards
   are insufficient for the full source vocabulary. Current valid Wind Strike
   must complete Learning the Ropes and award its one QP before departure,
   without requiring a chicken kill; an arbitrary `Hit` cannot substitute.
9. **Bank/interface/grants:** add contextual interface binding/open events to
   bank/shop interactions; generic unlocked tabs cannot prove a contextual UI
   was legitimately opened. Add guarded bank-item grants for the one-time
   first-visible 25 coins, inventory-capacity guards for sequential partial
   instructor grants, and a typed container reconciliation effect/policy.
   Departure normalization remains an explicit **provisional** source decision,
   not an observed inventory/equipment/bank dump or an implemented teleport.
10. **Shop/ground policy:** add a stock-sensitive pricing enum/formula with
    per-unit rounding/clamps and unstocked-row behavior. Fixed `buy_price` /
    `sell_price` cannot encode Lumbridge's 1300/400/30-per-mille policy.
    Source per-row clock phase should be confirmed/bound. Ordinary/tutorial/
    ammunition ground-item visibility/expiry policies are absent; `Drop`
    deliberately fails, rather than inventing a public/indefinite bag.

`source_math` supplies independently tested integer skilling, energy, combat
roll/damage/XP, General Store pricing and death-fee calculations for these
future bindings. **Arithmetic functions are not enabled gameplay systems.**
The negative-roll clamp, XP thirds, run rounding, overstock interpretation,
fire parameters, death ties and departure/goblin assumptions keep the
classifications in `research/journey-rules/decisions.json`.

## Validation

Run from the worktree root; all compiler/scratch output remains in the owned
ignored target directory:

```bash
mkdir -p crates/world-engine/target/scratch
export CARGO_TARGET_DIR="$PWD/crates/world-engine/target"
export TMPDIR="$PWD/crates/world-engine/target/scratch"
cargo fmt -p clubscape-world-engine --check
cargo test -p clubscape-world-engine --quiet
cargo clippy -p clubscape-world-engine --all-targets --quiet -- -D warnings
cargo build -p clubscape-world-engine --target wasm32-unknown-unknown --quiet
cargo test -p clubscape-world-engine --target wasm32-unknown-unknown --no-run --quiet
```

Tests execute the actual pure-state API, not mocked results: collision/planes/
reach, cadence/spam, controlled success/failure and deterministic restart,
depletion/competition, XP cap/stop differences, inputs/tools/failure outputs,
full-container rollback, session-scoped banking/shops, food cooldowns,
remote/locked dialogue, undefined/ambiguous transitions, all six partial
delivery orders and reward replay. Numeric literals were independently authored
in `research/journey-rules/expected-scenarios.json`, not captured from this
implementation. The fixture generator does not generate those oracles.

The current implementation passes 84 native tests, formatting, warnings-denied
Clippy, and WASM library/test-binary compilation.
Native unit/Clippy and WASM **compilation** are code/portability gates only.
WASM execution, real compiled-content integration, full M1 gameplay, durable
server commits/restarts, presentation approval, browser/performance and RuneLite
compatibility remain separate acceptance gates.
