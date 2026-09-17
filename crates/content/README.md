# Strict content compiler

`clubscape-content` validates the canonical
[`clubscape_game_types::GameContent`](../game-types/src/content.rs), builds
ordered lookup indexes, and compiles a bounded, versioned binary artifact.
It has no rendering, database, network, or platform-service dependency.
The CLI is a native filesystem front end; the library also builds for WASM.

**This is compiler infrastructure, not M1 content or acceptance evidence.**
No production definitions, source exports, or presentation assets are supplied
here. All definitions under `tests/` are explicitly synthetic.

## API

```rust,ignore
use clubscape_content::{
    compile_content, compile_content_with_manifest, encode_compiled,
    load_compiled, read_content_json, AssetManifest, ValidationMode,
};

let definition = read_content_json(json_bytes)?;
let compiled = compile_content(definition, ValidationMode::Runtime)?;
let bytes = encode_compiled(&compiled)?;
let loaded = load_compiled(&bytes, ValidationMode::Runtime)?;
```

- `compile_content(GameContent, ValidationMode) -> GameResult<CompiledContent>`
- `read_content_json(&[u8]) -> GameResult<GameContent>`: bounded **strict parsing**;
  additionally call `compile_content` for semantic validation.
- `encode_compiled(&CompiledContent) -> GameResult<Vec<u8>>`
- `load_compiled(&[u8], ValidationMode) -> GameResult<CompiledContent>`: verify,
  decode, **revalidate**, and rebuild indexes.
- `compile_content_with_manifest(GameContent, ValidationMode, &AssetManifest)`
  additionally checks referenced asset IDs against a supplied manifest projection.
- `CompiledContent::check_asset_manifest(&mut self, &AssetManifest)` performs
  the same optional membership check after loading.
- `sha256(&[u8]) -> String` returns lowercase, whole-file SHA-256.

`CompiledContent` has private, immutable definitions/indexes and these accessors:

- `definition() -> &GameContent`, `report() -> &ValidationReport`, `counts()`;
- `collision(Tile) -> Option<&CollisionCell>`, `region_at(Tile)`;
- `spawn(&SpawnId)`, `spawns_at_tile(Tile)` (ordered iterator);
- `item`, `skill`, `region`, `object`, `npc`, `recipe`, `dialogue`,
  `dialogue_node(&DialogueId, &str)`, `tutorial_stage`, `quest`, `shop`, `interface`;
- `has_equipment_slot(&SlotId)` and `referenced_assets()`.

The ID lookups take references to their corresponding shared ID types.
Mutation requires compiling a new definition, so stale index caches cannot be
introduced through the public API. Input/output maps and indexes use `BTreeMap`
and `BTreeSet`. Authored vectors retain their order: e.g. dialogue choices,
source records, equipment slots, effects, and collision-cell lists. Identical
definitions produce identical bytes; JSON whitespace/object-key order does not
affect the artifact.

Do not deserialize untrusted JSON directly into `GameContent`: shared serde
map implementations can discard duplicate keys and structs can ignore unknown
fields **before** this crate receives them. `read_content_json` detects duplicate
keys at every nesting level, rejects unknown fields (including internally tagged
enum fields), enforces input limits, and rejects trailing documents. The shared
canonical JSON field is `schema_version`, not `schemaVersion`.
Despite persisted-state serde compatibility defaults, content-4 source input
must explicitly contain every definition field, including `mechanics`, recipe
tools and intentional `null`/empty declarations. Only an ordinary stack's
absent `instance` has the same meaning as explicit `null`; a charged stack
cannot use that default. Missing source inputs are never supplied by parsing.

`RecipeDefinition.item_on_target` is a deliberate additive exception: absence
is omitted on serialization, preserving the exact old content4 artifacts.
A present `SourceBinding<ItemOnTargetRule>` must bind source evidence, positive
reach and a state guard for one inventory conversion with explicit single
timing and zero menu delay. It requires nonempty actual object targets.
Unresolved/malformed rules and mixed Production interactions, source menus or
one-click Production declarations are rejected. Its guard participates in
source auditing, iterative depth/work preflight, safe rejection/drop and
effect-reachability analysis. No new object operation or batching permission
is derived from this field.

The full extension API and migration contract are in
[`spec/game-contracts.md`](../../spec/game-contracts.md#mechanics-extension-content-3-persisted-state-1runtime-1).
New registry definitions are available through `definition().mechanics`.
`report().unresolved_bindings` lists exact unresolved source-binding paths.
Compiling those explicit gaps does not make them executable.

Content/artifact 3 introduced the final source-selector contract: player/stage drop
policies, NPC loot policy/eligibility/engagement/method attribution, explicit
weapon and unarmed defaults, actor traversal edges, combined collision states,
geometry-morph bridges, recovery interface selectors and death-phase timing.
All references, scopes, source records and arithmetic bounds are validated.
Combined groups cover the complete member-state product (at most 4096);
overlapping mutable transforms cannot rely on last-writer-wins replacements.
New guard locations participate in iterative preflight and safe rejection/drop.
Declared sets cannot hide duplicate input entries through serde normalization.

Content/artifact4 adds the explicit `GameContent.ui` definition (`null` for a
non-UI profile). It validates complete semantic interface classification,
source production interfaces, native style/ability labels, exact atomic quest
reward payloads, selected-item action lifecycles, bank preferences, document
bounds and coffer/chat/appearance policy provenance. UI source guards participate
in the same bounded scans. It does not approve the external asset publication
or UI presentation.

Existing content-1/2/3 artifacts must be regenerated, not relabeled. Persisted
state/runtime remain additive version 1; missing contact/method/provenance/phase
history is explicit legacy absence, never a fabricated source default or reward.

## CLI

```sh
cargo run -p clubscape-content -- \
  --input path/to/game-content.json \
  --output path/to/compiled-content.csc
```

The only alternate provenance mode is the explicitly named `--test-fixture`.
There is no permissive development mode, implicit seed data, default player,
empty-map fallback, or “ignore validation” switch.

Success prints one JSON object to stdout with `ok`, output path, `file_bytes`,
whole-file `sha256`, artifact/codec/schema versions, revision, baseline, category
counts, and the complete validation report. Validation/missing-file failures
exit nonzero and print JSON diagnostics to stderr. Argument errors use Clap's
usage diagnostics and nonzero exit status.

Validation finishes before touching the output. The compiler writes and syncs a
sibling `.part` file, renames it over the requested artifact, and removes its
partial file on ordinary write failures. Existing artifacts survive invalid
input. Source and output cannot refer to the same existing file. Output parent
directories are created only after successful compilation.

## Validation contract

### Identity, definitions, provenance

- Only content schema version 4 is accepted. Revision and baseline are required,
  bounded, non-placeholder single-line identities. No particular OSRS cache/build
  number is hardcoded; choosing/verifying the baseline remains source work.
- Every map key equals its contained ID. All M1 definition categories and the
  equipment-slot list must be nonempty. This necessary check is **not** proof of
  complete Tutorial Island, Cook's Assistant, or source-inventory coverage.
- `GameContent.interfaces` is a required, independent logical registry. Initial
  unlocks, guards, effects, and event filters must resolve there. An unlock cannot
  declare an unknown interface, and a registry entry does not unlock itself.
  Interface names/provenance are validated like other definitions.
- Items' non-null numeric source IDs, and skills', NPCs', and objects' numeric
  source IDs, are unique **within their category and this content baseline**.
  Equal numbers across categories are normal. Multiple `None` item mappings are
  allowed for content with no external numeric mapping. The shared model cannot
  represent multiple source games/builds within one numeric category mapping.
- Interface source widget-group IDs are unique within each definition and across
  the interface registry. This format does not infer widget aliases or silently
  choose one owner. A logical interface may map multiple distinct widget groups;
  an explicit empty mapping is permitted for original interfaces with provenance.
- Every definition and initial state requires source records in both modes.
  Records need an identifiable locator, revision, status, and explanatory notes.
  Duplicate `(reference, revision)` records on one owner are errors, even when
  their status/notes differ. Reuse across different owners is allowed.
  Nested bound/unresolved mechanics and scripted stationary anchors are checked
  too; each source-record occurrence is counted once.
- `Runtime` (also the default/development policy) rejects `TestFixture` records
  and fixture identities. `TestFixture` permits explicit `fixture:` records;
  it does **not** relax reference, numeric, graph, or initial-state checks.
- `VerifiedReference`, `Inference`, and `ApprovedAdaptation` remain separate in
  the validated definition, artifact, and evidence occurrence counts. Compilation
  does not authenticate a URL, source observation, release identity, or owner
  approval. An inference is never promoted to source verification.

### Items, quantities, skills, initial state

- Note mappings are distinct, reciprocal, and connect a nonstackable base to a
  stackable note. Notes cannot themselves be equipment or food.
- Equipment uses declared slots, a nonempty duplicate-free occupancy set
  containing its primary slot, defined skill requirements, and consistent
  attack-style/positive-speed data. Initial equipment is keyed only by primary
  slot; overlapping occupied slots, including two-handed/offhand conflicts, fail.
  No special cosmetic slots or hardcoded weapon/shield names are invented.
- Quantity bounds and 28-slot inventory shape come from shared checked types.
  Nonstackable items in inventory/equipment occupy one item per slot. Stackable
  inventory entries cannot duplicate the same item across slots. Equipment may
  stack when its item definition permits it (e.g. ammunition).
  Source mode 2 is retained as conditional stackability, not a boolean. Charged
  items use unique per-item instance data, reciprocal variants and bounded
  charges; they cannot be manufactured through ordinary quantity grants.
- Banks have positive capacity and at most that many represented slots. Omitted
  trailing slots are empty, not additional capacity. A bank has one unnoted stack
  per item, including nonstackable items. This differs intentionally from ordinary
  inventory slots.
- Recipe/guard/reward/ground-item stacks are **quantity batches**, not occupied
  inventory slots: a batch of two nonstackable items is valid. Duplicate item
  IDs in one batch are rejected; authors must aggregate their quantities.
  Inventory-bound batches must fit in 28 otherwise-empty slots. Item effects and
  independent NPC drops cannot exceed the stack limit even in aggregate.
- Skill thresholds begin at zero, strictly increase, fit the level type, and do
  not exceed positive `maximum_xp_tenths`. Requirements and XP rewards reference
  defined skills and valid levels/maxima. XP additions are overflow-checked.
  Every requirement declares base/current level basis. Typed weapon, style,
  NPC stat, rune, ammunition and prayer bindings resolve their own registries.
- Each skill and quest needs an explicit initial state, with no unknown keys.
  Initial skill levels match their unboosted XP thresholds. Initial quests must
  be at their declared initial stages, with no pre-awarded quest points. Starting
  hitpoints are positive; equipment requirements must already be satisfied.
- Initial run energy is 0 through shared `MAX_RUN_ENERGY` (10,000), in hundredths
  of one percent. Values are never implicitly rescaled: `100` means 1%, not 100%.
  This validates representation/range, not source drain or regeneration formulas.
- Tutorial XP ceilings and XP-stop levels are checked **separately**. Compilation
  never rewrites a stop-level rule into a ceiling or clamps a source XP award.
- Every flag read/written by a guard/effect has an explicit initial global value.
  Quest-local flag maps are retained but cannot be addressed by the shared
  global-only `Flag`/`SetFlag` types.
  Engine-reserved flags cannot be initial source data. Typed counters separately
  declare scope/type/bounds and exact character initial values. They are not
  fake inventory items or quest flags.

### Regions, placements, actions, shops

- Region bounds are inclusive and ordered on X/Y/plane. Every region has explicit
  cells, all inside its bounds. **Every tile has at most one cell globally**:
  duplicate cells are errors even if identical or in the same region.
- Bounds are envelopes, not ownership claims. Overlapping bounds are permitted
  only because ownership comes exclusively from distinct explicit cells.
  `region_at` never guesses from bounds. Shared source map squares may be split
  across disjoint cell owners; duplicate map-square IDs inside one region fail.
- Unlisted cells have no navigation definition and no walkability fallback.
  Starting/travel destinations and complete mobile NPC footprints must be
  walkable. Stationary resources, occupied-scenery actors and source-provenanced
  noncombat scripted actors use explicit access-tile policies without clearing
  water/terrain/scenery clipping or relocating the anchor. Objects/ground items
  may sit on explicit blocked cells. Movement and sight masks stay independent.
- Spawn IDs are map-unique. Repeating the same `(tile, spawn kind, definition ID)`
  is an error regardless of facing. Different definitions/kinds may share a tile;
  these remain separate, ordered spawns. No placement silently overwrites another.
  Facing is limited to the eight canonical direction values 0–7, not source angles.
  Source object shape/layer/quarter-turn placement is separately declared and
  checked. Dynamic transform states keep source-matching initial placement and
  consistent explicit collision coverage, rather than inferred clipping.
- All spawn kinds, interactions, dialogue/shop/recipe/object targets, gathering
  tools/outputs, equipment/skill requirements, and travel destinations resolve.
  Interaction names are unique per spawn. An `Attack` requires a combat NPC.
  Object-targeted recipes must allow the object that offers them.
- Every `RecipeDefinition.tools` entry is a distinct, defined, unnoted item
  required in inventory or equipment, **not consumed**. Tools cannot also be
  recipe inputs. All recipe tools are required; gather tools are acceptable
  alternatives. A bounded placement search verifies that inputs and required
  tools can fit inventory plus non-overlapping equipment, including two-handed
  conflicts. Compilation does not grant tools or require them at character
  creation; execution must enforce actual possession and requirements.
- Bound durations/respawns and probability denominators are positive. Literal
  probabilities are bounded; skill-curve success-count endpoints are **not**
  clamped before interpolation (copper 101/351/256, shrimp 49/257/256).
  Skill domains, rounding, single/first/repeat cadence, chance skill, bounded
  respawn/relocation and tool-location cadence are explicit.
  Gathering/recipe success cannot be impossible throughout its domain. Explicit
  empty gathering-tool lists mean a tool-free action, not an unknown default.
  Inventory-conversion recipes require inputs and successful outputs;
  explicit `ConsumeOnly` recipes require actual inputs and no direct output
  items, retaining the same XP/effect/guard/provenance validation. Failed
  outputs and XP may be empty. Firemaking instead has a ground-input/fire
  lifecycle and real temporary interactions. Static and dynamic production
  targets are distinct; a fire is not a fabricated static spawn.
- NPC size/combat hitpoints/durations are positive; drop ranges/probabilities are
  valid. Legacy drop entries describe independent rolls. Typed pools distinguish
  guaranteed, weighted-exclusive, independent, conditional and unresolved loot;
  they cannot coexist with legacy drop/respawn projections.
  Multiple entries for the same item are allowed only within a safe combined
  maximum quantity. Zero combat stats/max hit and zero-chance drops are representable.
- Shop currency is an unnoted stackable item. Stock IDs are unique and not the
  currency itself. Stocks/prices fit stack quantities, restock durations and buy
  prices are positive, and sell price cannot exceed buy price. Baseline stock and
  sell price may be zero. An empty shop is allowed only when it accepts general
  items. This is not an economy/arbitrage proof across multiple shops.
  Typed rows validate stock-sensitive coefficients, per-unit rounding/clamps,
  base-stock price agreement and per-row restock phase/interval. Unstocked
  acceptance requires a price formula rather than missing fixed-price rows.

### Source mechanics and runtime state

`GameContent.mechanics` validates typed grants/entitlements/reconciliation,
counter/morph bindings, temporary objects, instance chunk/plane mappings,
experience/appearance choices, travel lifecycle, combat/spell/projectile/prayer
definitions, weight/run/vital policy and death/valuation/recovery policy.
Unknown references, mismatched purposes/scopes, invalid bounds and missing
provenance fail. Unresolved source policies remain identified as unavailable;
the compiler never supplies a zero probability, fixed timer, free recovery,
default arrival or departure kit.

`EventCondition` reads resolved authoritative facts only in progression
contexts. Wrong event kinds/targets, mismatched production outcomes/facilities
and spell/kill definitions fail. Generic tab opening cannot claim contextual
bank presentation. Bank pre-presentation grants require atomic bank-targeted,
once-only accounting. Partial grants retain ordered line satisfaction instead
of claiming an unfinished package.

`Once`, recipe outcomes, temporary-object effects and travel completion effects
participate in recursive bounds, reward and graph validation. Counter and
entitlement dependencies must have rooted producers. Conditional loot is
included in iterative preflight and safe destruction of rejected owned trees.
The shared runtime validators check typed counter, item-instance, deadline,
ledger, live-instance and recovery references in addition to storage shape.
Legacy scheduling migration is explicit and preserves acknowledged possessions,
XP/progression and pending operations; see the shared contract.

### Dialogue and progression

- Dialogue node IDs are unique per dialogue; choice IDs are unique **per node**.
  Entries and next-node references resolve. Every declared node must be reachable
  from some entry by structurally possible edges. Deliberately repeatable
  conversation loops are allowed; a dialogue need not be acyclic or have an exit.
  Choice effects may change guards before entering the next node.
- Every tutorial and quest stage/effect reference resolves. All declared stages
  must be reachable from the initial stage. Tutorial progression must have at
  least two stages and a path to a terminal stage (no outgoing different-stage
  edge); quests must have distinct initial/completed stages and paths to completion.
  Closed progression components without completion paths fail. Completed quests
  cannot be reset by a potentially applicable stage-writing effect.
- Effects on spawns, dialogue choices, tutorial transitions, and quest transitions
  all participate in this analysis, including conditional effects.
- A bounded fixed-point pass starts from the explicit initial stages/flags/unlocks.
  Advancement-event dependencies need a rooted producer; character creation does
  not fabricate a `tutorial_advanced` or `quest_advanced` event. A stage/flag/unlock
  whose only producer requires itself is not accepted as reachable. Non-progression
  gameplay events and item/skill/location conditions remain conservative potential
  inputs, not claims that the corresponding real journey has been executed.
- Interface-open events require a potentially reachable unlock. Message triggers
  need a rooted message effect; sound/animation triggers need reachable declared
  gather sound/animation references. These are possible authoritative signals,
  not client playback acknowledgments or proof that asset files/presentation work.
- A single action/choice/transition may write each progression owner’s stage only
  once, including nested conditionals. Multiple conditional alternatives must be
  split into separately guarded choices/transitions, not rely on last-write-wins.
- Value-bearing quest effects (`GiveItems`, `AwardXp`, `AddQuestPoints`) need a
  guaranteed destination in the same unconditional effect branch (possibly an
  enclosing branch), with guards excluding that destination. A reward outside
  the conditional that changes its quest stage is invalid. Quest-point awards
  without a quest destination fail. Ordinary non-quest repeated dialogue/actions
  are not mislabeled as one-time quest rewards.
- Guards/effects are checked recursively only after an iterative bounded
  preflight. Even rejecting an excessively deep **owned Rust** definition uses
  iterative destruction, rather than overflowing the recursive drop stack.
- Constant guards, immutable initial flags, conflicting equalities, and known
  current progression stages eliminate impossible edges. Unknown gameplay
  conditions are conservatively possible. Analysis is not a SAT solver, pathfinder,
  item-acquisition planner, event scheduler, or proof of actual source behavior.
  Event ordering/reentrancy and all guarded reward replay behavior still require
  simulation/integration tests.

### Shared event identities

`allowed_actions` accepts canonical intent names, documented engine families,
`*` and reference-checked selectors such as `gather:<SpawnId>`,
`interact:<SpawnId>:<action>`, `produce:<RecipeId>`, `cast:<SpellId>` and
`prayer:<PrayerId>`. Unknown names and selectors fail; allowing one does not
enable an unimplemented runtime operation.

`ProgressTransition.event` matches shared `GameEvent::kind()` and optional
`target` matches `GameEvent::primary_target()`, as defined in
[`spec/game-contracts.md`](../../spec/game-contracts.md). `None` is a wildcard,
**never** the progression destination. Tests cover every current event variant:

| Event | Optional target |
| --- | --- |
| `interacted`, `dialogue_selected`, `gathered`, `hit`, `defeated` | Spawn ID |
| `produced` | Recipe ID |
| `equipped` | Equipment slot ID |
| `xp_gained` | Skill ID |
| `interface_opened` | Registry interface ID |
| `tutorial_advanced` | Tutorial stage ID |
| `quest_advanced` | Quest ID |
| `sound` | Asset ID (syntax and optional manifest membership) |
| `animation` | Defined spawn ID or syntactically valid dynamic actor ID |
| `moved`, `died`, `recovered`, `message` | Must be absent; qualify using guards |
| `experience_selected` | Experience ID |
| `interface_closed`, `interface_presented` | Interface ID |
| `production_resolved` | Recipe ID |
| `combat_resolved`, `npc_killed`, `inspected` | Spawn ID |
| `spell_resolved` | Spell ID |
| `teleport` | Travel ID |
| `food_eaten` | Item ID |
| `prayer_changed` | Prayer ID |
| `temporary_object_created` | Temporary-object definition ID |
| `object_transformed` | Object-transform ID |
| `counter_changed` | Counter ID |
| `appearance_confirmed`, `setting_changed`, `item_transferred`, `death_occurred`, `death_topic_completed`, `recovery_completed`, `grave_expired` | Must be absent |

Dialogue/gather/combat spawn filters must actually offer the corresponding
interaction/capability. Dynamic actor existence belongs to world-state validation,
not the immutable content registry. Unknown event kinds fail. Source-defined
progression destinations are explicit `SetTutorialStage`/`SetQuestStage` effects.
Compilation and canonical identity checks do not implement or certify an FSM,
event scheduling, visual/audio playback, or authoritative runtime matching.

## Binary artifact version 4

Gameplay data uses the mature **rmp-serde MessagePack** codec, with named
maps of the typed definition's canonical JSON projection. This gives integer
morph/level-table keys their JSON string representation and deterministically
orders object keys. It supports the shared internally tagged serde enums (unlike
non-self-describing codecs that cannot handle `deserialize_any`). There is no
custom gameplay field serializer, compression, serialized index cache, or
serialized “validated” claim.

The fixed 84-byte envelope is:

| Offset | Bytes | Meaning |
| --- | ---: | --- |
| 0 | 8 | ASCII `CLSCONT` followed by NUL |
| 8 | 2 | Little-endian artifact version, `4` |
| 10 | 2 | Little-endian codec ID, `1` = named MessagePack |
| 12 | 8 | Little-endian exact payload byte length |
| 20 | 32 | SHA-256 of payload bytes |
| 52 | 32 | Identity SHA-256, below |
| 84 | declared length | Named MessagePack of canonical JSON-projected `GameContent` |

Identity SHA-256 covers, in order:

1. the bytes `clubscape.content.identity.v4` followed by NUL;
2. shared schema version as little-endian `u32`;
3. revision UTF-8 length as little-endian `u64`, then revision bytes;
4. baseline UTF-8 length as little-endian `u64`, then baseline bytes.

The CLI’s `sha256` additionally identifies the **entire file**, including header.
These hashes detect corruption and identity mismatch; they are not signatures,
proof of provenance, or protection against an attacker who can replace the file
and all its hashes. Callers must independently authenticate trusted build artifacts.

Loading checks header/version/codec, bounded exact size, checksum, strict payload
structure, canonical byte-for-byte re-encoding (rejecting trailing/noncanonical
MessagePack), identity, and full semantic validation under the **caller’s** mode.
It reconstructs all indexes and all validation claims. A previously supplied
asset manifest must be checked again; no external manifest check survives loading.
Schema/codec changes that alter this representation need an explicit version and
migration strategy, not silent permissive decoding.

### Resource bounds

| Limit | Value |
| --- | ---: |
| JSON input or entire compiled artifact | 64 MiB |
| Binary payload | 64 MiB minus 84 bytes |
| Decoded value nesting | 120 |
| Decoded values plus map keys | 2,000,000 |
| Any decoded collection | 250,000 entries |
| Decoded string | 65,536 bytes |
| Decoded map key | 256 bytes |
| Guard/effect/conditional-loot nesting | 32 |
| Rule nodes in the whole pack | 100,000 |
| Static graph-analysis work per pass | 5,000,000 steps |
| Tool/equipment placement search per recipe | 100,000 steps |

Individual semantic fields have tighter limits where appropriate (shared ID
limits, skill level representation, bank capacity, 256-byte identity/name, etc.).
The decoder does not reserve a vector/map from an untrusted size hint. MessagePack
decoding borrows the bounded input, so malicious string/binary length prefixes
cannot request their declared multi-gigabyte buffers. Encoding uses a bounded
writer. Filesystem callers should also bound reads **before** allocating the input;
the CLI uses `Read::take(MAX_INPUT_BYTES + 1)`.

## Checks and acceptance boundaries

Every successful compilation exposes an explicit list of checks, separate source
status occurrence counts, unresolved-binding paths, asset counts, and `false`
flags for source/approval/presentation verification.

The Director-owned shared contract update
`3b2b7cdbb8afe7bffb809a7062014fdf4ae0c558` resolves the former registry, recipe-tool,
run-energy representation, and event-identity gaps. Successful compilation now
sets `interface_definition_validation_performed = true` and
`recipe_tool_reference_validation_performed = true` **after actually checking**
those definitions/references. Empty registries, unknown unlock IDs, missing source
records, invalid tools, and out-of-range initial energy are errors, including when
loading a checksum-valid artifact. Old incomplete packs must be completed and
recompiled; shared serde defaults are not a migration that supplies missing data.

Scope boundaries that remain distinct from compiler completion:

- Simulation must enforce tools' actual inventory/equipment possession without
  consuming them, and implement the shared event/FSM contracts. Compiling matching
  event identities is not runtime execution or replay-safety evidence.
- Source-specific bindings/formulas/placement policies now have typed,
  reference-checked representations. Their actual execution, source truth,
  source completeness and unresolved choices remain separate. The legacy
  engine's explicit unavailable arms are not an implemented M1 journey.
- The model has no explicit tutorial completed-stage set or required/optional
  progression-node markers. Terminal tutorial stages are currently defined
  structurally, and every declared graph node is treated as required.

The component does not certify a production pack or waive any M1 acceptance gate.
Logical interface-registry validation does not certify rendered controls or
their behavior. Source fidelity, real acquisition/progression routes, runtime
atomicity, and concrete presentation remain independently verifiable requirements.

`AssetManifest { identity, assets }` is only an identifiable **projection of IDs**,
not a replacement for the asset workers’ canonical manifests. Optional absent
assets do not block independently valid headless mechanics. Missing concrete
terrain, models, UI, animations, audio, source captures, approvals, performance,
and RuneLite evidence remain separate M1 presentation/acceptance blockers.

## Reproducible validation

From the workspace root:

```sh
cargo fmt -p clubscape-content -- --check
cargo test -p clubscape-content -p clubscape-game-types
cargo clippy -p clubscape-content --all-targets -- -D warnings
cargo build -p clubscape-content --lib --target wasm32-unknown-unknown
cargo clippy -p clubscape-content --lib --target wasm32-unknown-unknown -- -D warnings
```

Tests exercise the actual compiler, strict JSON parser, binary loader, and native
CLI. They include positive synthetic definitions, hundreds of invalid mutations,
source policies, all header truncations, valid-checksum invalid payloads,
duplicate keys/cells/source IDs, unsafe quantities/XP/graphs, deep owned-tree
rejection, fake enormous length prefixes, round-trip determinism, and output
preservation. Extension cases exercise unclamped chance vectors, dynamic
facilities/anchors, typed counters, partial ledgers, combat/loot/prayer references,
charges/mode 2, live instances/death policies and legacy metadata preservation.
CLI test files live in the crate's ignored `test-output/` directory
and are removed by the tests; no system temporary directory is used.

Native tests and WASM code generation are compiler/toolchain evidence only, not
gameplay, visual/audio, browser, performance, or RuneLite acceptance.
