# clubscape-simulation

Source-agnostic, deterministic **headless primitives**, depending only on
`clubscape-game-types` and the Rust standard library. No gamepack/build is pinned
here. No production items, XP table, tutorial route, gameplay rates, renderer,
network service, or persistence layer is supplied.

All test data under `tests/` is explicitly synthetic. Passing these tests is not
M1 gameplay, source-fidelity, presentation, audio, or RuneLite acceptance.

## Integration and atomicity

All public operations return shared `GameResult<T>` / typed `GameError`, except
explicit boolean predicates and immutable accessors. Slots use checked,
zero-based `usize` indices; convert intent `u8`/`u16` indices with `usize::from`.
`ItemId` and positive `Quantity` are the validated shared types.

Inventory, equipment, bank and character XP mutations commit only on success.
Failures preserve complete input state, including slot positions. Cross-system
actions can compose them with:

```rust
pub fn transact_character<T>(
    character: &mut CharacterState,
    operation: impl FnOnce(&mut CharacterState) -> GameResult<T>,
) -> GameResult<T>;
```

The closure must remain pure: external side effects cannot be rolled back.
Returned events/results become publishable only after the **server's durable
transaction commits**. The caller still owns authenticated actor access,
requirements/guards beyond these primitives, bank proximity/access, tick
scheduling, idempotency/replay checks, concurrency, persistence, and legitimate
item/XP acquisition. These helpers are not client grant or tutorial-advance
endpoints.

Load/compile/validate complete source content before accepting gameplay.
Local checks below reject invalid touched definitions and container state; they
do not replace full content compilation or whole-character validation. A
`CompiledContent` owner can pass its original `GameContent` and item map directly.

## Inventory (`inventory`)

`ItemDefinitions` is the public alias
`BTreeMap<ItemId, ItemDefinition>`. Signatures:

```rust
pub fn validate(inventory: &Inventory, items: &ItemDefinitions) -> GameResult<()>;
pub fn count(inventory: &Inventory, items: &ItemDefinitions, item: &ItemId) -> GameResult<u32>;
pub fn stack_at(inventory: &Inventory, slot: usize) -> GameResult<&ItemStack>;
pub fn add(inventory: &mut Inventory, items: &ItemDefinitions, stack: &ItemStack) -> GameResult<()>;
pub fn remove(inventory: &mut Inventory, items: &ItemDefinitions, stack: &ItemStack) -> GameResult<()>;
pub fn add_batch(inventory: &mut Inventory, items: &ItemDefinitions, stacks: &[ItemStack]) -> GameResult<()>;
pub fn remove_batch(inventory: &mut Inventory, items: &ItemDefinitions, stacks: &[ItemStack]) -> GameResult<()>;
pub fn apply_operations(inventory: &mut Inventory, items: &ItemDefinitions, operations: &[InventoryOperation]) -> GameResult<()>;
pub fn remove_from_slot(inventory: &mut Inventory, items: &ItemDefinitions, slot: usize, quantity: Quantity) -> GameResult<ItemStack>;
pub fn transfer(source: &mut Inventory, destination: &mut Inventory, items: &ItemDefinitions, stack: &ItemStack) -> GameResult<()>;
pub fn swap(inventory: &mut Inventory, items: &ItemDefinitions, from: usize, to: usize) -> GameResult<()>;
```

- `InventoryOperation::{Add(ItemStack), Remove(ItemStack)}` is evaluated in
  order against a draft. Repeated entries compose; all entries roll back on any
  failure. A production action can remove inputs before adding outputs.
- The fixed 28 slots never compact automatically. Adds fill the first free
  slots; removals by item use ascending slot order; swaps can also move to an
  empty slot. `remove_from_slot` never consumes other copies.
- Stackable items have one inventory stack per item, capped at 2,147,483,647.
  Nonstackable items require one slot per unit. Overflow never spills into a
  second stack. Existing stacks can grow in a full inventory.
- Unknown IDs and mismatched definition-map keys fail. Invalid stored
  nonstackable quantities or split stackable stacks fail rather than being
  silently normalized. `stack_at` only checks slot bounds/ownership; it does not
  have content with which to validate the item.

## Equipment (`equipment`)

```rust
pub fn validate(equipment: &BTreeMap<SlotId, ItemStack>, content: &GameContent) -> GameResult<()>;
pub fn occupant<'a>(equipment: &'a BTreeMap<SlotId, ItemStack>, content: &GameContent, slot: &SlotId) -> GameResult<Option<(&'a SlotId, &'a ItemStack)>>;
pub fn equip(character: &mut CharacterState, content: &GameContent, inventory_slot: usize) -> GameResult<GameEvent>;
pub fn equip_with_level_basis(character: &mut CharacterState, content: &GameContent, inventory_slot: usize, level_basis: LevelBasis) -> GameResult<GameEvent>;
pub fn unequip(character: &mut CharacterState, content: &GameContent, slot: &SlotId) -> GameResult<ItemStack>;
```

An item is stored **once**, at `EquipmentDefinition.slot`. Its occupied set is
the union of that slot and `occupied_slots`; listing the primary slot once in
`occupied_slots` is allowed. Every slot must be in `GameContent.equipment_slots`.
There are no hardcoded functional slots, hidden gear entries, or cosmetic layer.
`occupant`/`unequip` also resolve a secondary occupied slot to that single owner.

Equip takes the entire selected, owned inventory stack. Every conflicting owner
is displaced once into the draft inventory, including conflicts on arbitrary
multi-slot equipment. The selected slot is freed first. Insufficient space for
all displaced items rolls back everything. Same-item stackable equipment merges
with an overflow check. The `Equipped` event contains the resulting equipped
stack, not an additional grant.

`equip` checks XP-derived **base** levels. `equip_with_level_basis` lets a caller
explicitly select `skills::LevelBasis::Current` when source rules allow boosts.
No skill, HP, prayer, or unrelated character progress is reset.

## Bank (`bank`)

```rust
pub fn validate(bank: &Bank, items: &ItemDefinitions) -> GameResult<()>;
pub fn count(bank: &Bank, items: &ItemDefinitions, item: &ItemId) -> GameResult<u32>;
pub fn deposit(character: &mut CharacterState, content: &GameContent, inventory_slot: usize, quantity: Quantity) -> GameResult<ItemStack>;
pub fn withdraw(character: &mut CharacterState, content: &GameContent, bank_slot: usize, quantity: Quantity, noted: bool) -> GameResult<ItemStack>;
```

- Capacity is the explicit shared `Bank.capacity`. `slots` may be a shorter
  allocated prefix, but may not exceed capacity. Holes are stable and reused
  before appending; no operation compacts or silently increases capacity.
- Every bank item has one **unnoted** stack, including items nonstackable in
  inventory. Deposits of notes aggregate into that same base stack. All stacks
  obey the shared maximum; duplicate or noted persisted bank entries fail.
- A deposit slot selects an item ID, not an implicit grant. Its quantity may
  span other inventory copies of that exact ID: consume the selected slot first,
  then other copies in ascending order. Notes and unnoted inventory copies are
  not implicitly combined by a single deposit request.
- Withdrawal requests are exact, not “as many as fit.” Nonstackable withdrawals
  split into inventory slots; note withdrawals stack. Full inventories/banks,
  insufficient ownership and overflow leave both containers unchanged.
- Note conversion requires reciprocal `noted_variant` / `unnoted_variant`
  mappings and a stackable note. Missing IDs, cycles and malformed mappings fail.
  `noted = true` is a **strict requested form**: an item without a note mapping
  returns `InvalidInput`, rather than silently withdrawing another form. A
  source-specific bank UI/action adapter can choose ordinary withdrawal for
  known unnoteable items if its source toggle behavior requires that.
- `deposit`/`withdraw` return the actual transferred form and quantity, not the
  resulting container total. `count` accepts a base or explicitly mapped note ID.

## XP and requirements (`skills`)

```rust
pub fn validate_definition(definition: &SkillDefinition) -> GameResult<()>;
pub fn level_for_xp(definition: &SkillDefinition, xp_tenths: u64) -> GameResult<u16>;
pub fn xp_to_next_level(definition: &SkillDefinition, xp_tenths: u64) -> GameResult<Option<u64>>;
pub fn award_xp(state: &mut SkillState, definition: &SkillDefinition, amount_tenths: u64, limits: XpLimits, current_level_policy: CurrentLevelPolicy) -> GameResult<XpAward>;
pub fn award_character_xp(character: &mut CharacterState, content: &GameContent, rewards: &[XpReward], current_level_policy: CurrentLevelPolicy) -> GameResult<Vec<GameEvent>>;
pub fn check_requirements(states: &BTreeMap<SkillId, SkillState>, definitions: &BTreeMap<SkillId, SkillDefinition>, requirements: &[SkillRequirement], basis: LevelBasis) -> GameResult<()>;
```

Threshold index zero is level one and must be zero XP; subsequent thresholds
must strictly increase. The table has 1..=65535 levels and its last threshold
must not exceed `maximum_xp_tenths`. No formula, interpolation, whole-XP rounding,
or virtual levels are invented. Values are integer tenths throughout. XP above
the source maximum is invalid state, not silently clamped.

`XpLimits { cap_tenths: Option<u64>, stop_level: Option<u16> }` defaults to no
extra restriction. `XpLimits::from_stage(stage: &TutorialStageDefinition,
skill: &SkillId) -> XpLimits` reads the corresponding two stage maps.

- The source maximum and an exact cap limit the awarded **amount**. A lower
  cap never subtracts existing XP.
- A stop level checks the XP-derived level **before** an award. The last award
  may cross its threshold, even by several levels; further rewards then stop.
  Temporary boosts/drains do not change that decision.
- Limits outside the source table/maximum fail explicitly.
- `CurrentLevelPolicy::Preserve` changes XP only.
  `AddBaseLevelGains` explicitly adds the base-level increase to the temporary
  current level, preserving its additive boost/drain rather than resetting it.
  A `u16` overflow rejects the entire award. Neither option modifies the
  separate `CharacterState.hitpoints` or `prayer_points`.
- `XpAward` reports `skill`, actual `amount_tenths`, `previous_xp_tenths`,
  `xp_tenths`, `previous_level`, and `level`.
  `XpAward::event(&self) -> Option<GameEvent>` returns `XpGained` only for a
  nonzero award.
- `award_character_xp` requires the character's declared stage to exist. It
  applies all rewards atomically and in order; duplicate skill rewards remain
  distinct for stop-level evaluation. Unknown skill/stage content never falls
  back to unrestricted awards. For non-tutorial systems with explicitly supplied
  limits, use `award_xp`, optionally inside `transact_character`.

## Navigation (`navigation::CollisionMap`)

```rust
pub fn from_regions<'a>(regions: impl IntoIterator<Item = &'a RegionDefinition>) -> GameResult<Self>;
pub fn cell(&self, tile: Tile) -> Option<&CollisionCell>;
pub fn can_step(&self, from: Tile, to: Tile) -> bool;
pub fn find_path(&self, start: Tile, goal: Tile, max_visited: usize) -> GameResult<Vec<Tile>>;
pub fn line_of_sight(&self, from: Tile, to: Tile) -> bool;
pub fn step_path(&self, position: &mut Tile, path: &mut Vec<Tile>, running: bool) -> GameResult<Vec<GameEvent>>;
```

`from_regions(&regions)` accepts a slice; `from_regions(content.regions.values())`
accepts the original `GameContent` map without copying region definitions.
Region bounds are inclusive in X/Y/plane. Reversed bounds, outside cells,
duplicate region identities, or duplicate cells fail; unlisted cells remain
blocked. Adjacent explicit cells can cross region boundaries but never planes.
Rebuild the immutable map from changed source-derived cells when authoritative
doors or other interactions change collision.

Movement uses the shared canonical mask directions:
N=1, E=2, S=4, W=8, NE=16, SE=32, SW=64, NW=128.
Both endpoints must permit an edge. Diagonals also require both adjoining
cardinal routes (all four edges), including their opposite-side flags and cells.

Pathfinding is bounded breadth-first search with unit-cost eight-direction
steps. It returns a shortest path excluding start and including goal; start
equals goal yields an empty path only for a valid walkable start.
Deterministic tie order is N, E, S, W, NE, SE, SW, NW. `max_visited` is positive
and counts discovered cells including start; exhaustion or an unreachable
destination returns an error, not a partial or straight-line route. Storage
grows only with discovered cells, not the requested bound.

LOS is symmetric integer **center-to-center supercover** traversal through
listed cells, separately using `blocked_sight`. Nonwalkable cells with clear
sight masks can be seen through. Corner crossings check diagonal masks and both
cardinal routes. Identical listed endpoints need no edge crossing. This is an
explicit reusable rasterization policy, not proof of an unverified source's
complete projectile/target-volume rules.

`step_path` consumes one walking or at most two running steps for one
caller-scheduled tick, emits an event for each traversed tile, and retains the
suffix. It validates the whole consumed prefix before changing anything; a
blocked second running step rejects that call so the caller can replan.
It does not invent run-energy costs, cadence, region changes, transport,
interaction reach, or rendering. The caller must maintain character region
metadata when crossing boundaries.

## Errors and remaining shared-contract decisions

Unknown definitions use `UnknownContent`; malformed definitions use
`InvalidContent`; invalid slots/state use `InvalidInput`; empty selected slots
use `NotOwned`; insufficient quantities use `InsufficientItems`; overflow uses
`StackOverflow`; unmet skill requirements use `RequirementNotMet`.
Navigation uses `OutOfReach` for missing endpoints/different planes and `Blocked`
for impassable, unreachable, invalid-step or search-bound failures, with a
specific message.

No shared-type change is required to use these primitives. Integration should
retain these explicit decisions rather than infer unsupported source behavior:

- `GameErrorCode` has no bank-specific capacity variant. A full bank currently
  uses `InventoryFull` with a bank-specific message; a future `BankFull` variant
  would allow client branching without interpreting that message.
- `SkillRequirement` does not encode base-versus-current per requirement.
  Equip defaults to base; the explicit override applies one basis to the whole
  operation. Mixed source requirements would need a shared rule field.
- XP definition/stage types do not specify HP restoration or temporary-level
  effects on level-up. Callers must supply the explicit current-level policy and
  apply any verified HP/prayer effect atomically themselves.
- Bank note-toggle fallback and precise source LOS/target-volume policies are
  caller/content integration decisions; the strict form and supercover APIs
  above do not silently invent them.

## Validation

Run from the workspace root. Build output and compiler scratch stay inside this
owned crate (the local `target/` is ignored):

```bash
mkdir -p crates/simulation/target/scratch
export CARGO_TARGET_DIR="$PWD/crates/simulation/target"
export TMPDIR="$PWD/crates/simulation/target/scratch"
cargo fmt -p clubscape-simulation
cargo test -p clubscape-simulation --quiet
cargo clippy -p clubscape-simulation --all-targets --quiet -- -D warnings
cargo build -p clubscape-simulation --target wasm32-unknown-unknown --quiet
```

Native tests cover successful, negative, boundary, rollback, conservation,
deterministic tie, and exhaustive small-fixture cases. WASM compilation is a
portability check only, not browser gameplay/rendering acceptance. Workspace
lockfile changes remain the integration owner's responsibility.
