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
  `dialogue_node(&DialogueId, &str)`, `tutorial_stage`, `quest`, `shop`;
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

- Only shared schema version 1 is accepted. Revision and baseline are required,
  bounded, non-placeholder single-line identities. No particular OSRS cache/build
  number is hardcoded; choosing/verifying the baseline remains source work.
- Every map key equals its contained ID. All M1 definition categories and the
  equipment-slot list must be nonempty. This necessary check is **not** proof of
  complete Tutorial Island, Cook's Assistant, or source-inventory coverage.
- Items' non-null numeric source IDs, and skills', NPCs', and objects' numeric
  source IDs, are unique **within their category and this content baseline**.
  Equal numbers across categories are normal. Multiple `None` item mappings are
  allowed for content with no external numeric mapping. The shared model cannot
  represent multiple source games/builds within one numeric category mapping.
- Every definition and initial state requires source records in both modes.
  Records need an identifiable locator, revision, status, and explanatory notes.
  Duplicate `(reference, revision)` records on one owner are errors, even when
  their status/notes differ. Reuse across different owners is allowed.
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
- Each skill and quest needs an explicit initial state, with no unknown keys.
  Initial skill levels match their unboosted XP thresholds. Initial quests must
  be at their declared initial stages, with no pre-awarded quest points. Starting
  hitpoints are positive; equipment requirements must already be satisfied.
- Tutorial XP ceilings and XP-stop levels are checked **separately**. Compilation
  never rewrites a stop-level rule into a ceiling or clamps a source XP award.
- Every flag read/written by a guard/effect has an explicit initial global value.
  Quest-local flag maps are retained but cannot be addressed by the shared
  global-only `Flag`/`SetFlag` types.

### Regions, placements, actions, shops

- Region bounds are inclusive and ordered on X/Y/plane. Every region has explicit
  cells, all inside its bounds. **Every tile has at most one cell globally**:
  duplicate cells are errors even if identical or in the same region.
- Bounds are envelopes, not ownership claims. Overlapping bounds are permitted
  only because ownership comes exclusively from distinct explicit cells.
  `region_at` never guesses from bounds. Shared source map squares may be split
  across disjoint cell owners; duplicate map-square IDs inside one region fail.
- Unlisted cells have no navigation definition and no walkability fallback.
  Starting/travel destinations and NPC anchors must be explicitly walkable and
  belong to the stated region. Objects/ground items may sit on explicitly blocked
  cells (e.g. an item on an object). Movement and sight masks remain independent.
- Spawn IDs are map-unique. Repeating the same `(tile, spawn kind, definition ID)`
  is an error regardless of facing. Different definitions/kinds may share a tile;
  these remain separate, ordered spawns. No placement silently overwrites another.
  Facing is limited to the eight canonical direction values 0–7, not source angles.
  Rotated footprints/object layers are **not** inferred from the current fields.
- All spawn kinds, interactions, dialogue/shop/recipe/object targets, gathering
  tools/outputs, equipment/skill requirements, and travel destinations resolve.
  Interaction names are unique per spawn. An `Attack` requires a combat NPC.
  Object-targeted recipes must allow the object that offers them.
- Durations/respawns and probability denominators are positive; probabilities are
  bounded. Gathering/recipe success cannot be impossible at every level. Explicit
  empty gathering-tool lists mean a tool-free action, not an unknown default.
  Recipes require inputs and successful outputs; failed outputs and XP lists may
  be explicitly empty.
- NPC size/combat hitpoints/durations are positive; drop ranges/probabilities are
  valid. Drop entries describe independent rolls, not one normalized table.
  Multiple entries for the same item are allowed only within a safe combined
  maximum quantity. Zero combat stats/max hit and zero-chance drops are representable.
- Shop currency is an unnoted stackable item. Stock IDs are unique and not the
  currency itself. Stocks/prices fit stack quantities, restock durations and buy
  prices are positive, and sell price cannot exceed buy price. Baseline stock and
  sell price may be zero. An empty shop is allowed only when it accepts general
  items. This is not an economy/arbitrage proof across multiple shops.

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

### String conventions pending shared typed contracts

`allowed_actions` uses the exact snake-case `GameIntent` variant names, not
arbitrary interaction labels. Unknown names fail.

`ProgressTransition.event` uses these authoritative `GameEvent` names. `target`
is an optional filter, **never** the progression destination:

| Event | Optional target |
| --- | --- |
| `interacted`, `dialogue_selected`, `gathered`, `hit`, `defeated` | Spawn ID |
| `produced` | Recipe ID |
| `equipped` | Equipment slot ID |
| `xp_gained` | Skill ID |
| `interface_opened` | Declared interface ID |
| `tutorial_advanced` | Tutorial stage ID |
| `quest_advanced` | Quest ID |
| `moved`, `died`, `recovered` | Must be absent; qualify using guards |

Dialogue/gather/combat spawn filters must actually offer the corresponding
interaction/capability. Presentation-only `message`, `sound`, `animation` events
are not supported progression triggers. Unknown event kinds fail. Source-defined
progression destinations are explicit `SetTutorialStage`/`SetQuestStage` effects.

## Binary artifact version 1

Gameplay data uses the mature **rmp-serde MessagePack** codec, with named
struct maps. It supports the shared internally tagged serde enums (unlike
non-self-describing codecs that cannot handle `deserialize_any`). There is no
custom gameplay field serializer, compression, serialized index cache, or
serialized “validated” claim.

The fixed 84-byte envelope is:

| Offset | Bytes | Meaning |
| --- | ---: | --- |
| 0 | 8 | ASCII `CLSCONT` followed by NUL |
| 8 | 2 | Little-endian artifact version, `1` |
| 10 | 2 | Little-endian codec ID, `1` = named MessagePack |
| 12 | 8 | Little-endian exact payload byte length |
| 20 | 32 | SHA-256 of payload bytes |
| 52 | 32 | Identity SHA-256, below |
| 84 | declared length | Canonical named MessagePack `GameContent` |

Identity SHA-256 covers, in order:

1. the bytes `clubscape.content.identity.v1` followed by NUL;
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
| Guard/effect nesting | 32 |
| Guard/effect nodes in the whole pack | 100,000 |
| Static graph-analysis work per pass | 5,000,000 steps |

Individual semantic fields have tighter limits where appropriate (shared ID
limits, skill level representation, bank capacity, 256-byte identity/name, etc.).
The decoder does not reserve a vector/map from an untrusted size hint. MessagePack
decoding borrows the bounded input, so malicious string/binary length prefixes
cannot request their declared multi-gigabyte buffers. Encoding uses a bounded
writer. Filesystem callers should also bound reads **before** allocating the input;
the CLI uses `Read::take(MAX_INPUT_BYTES + 1)`.

## Checks versus outstanding shared contracts

Every successful compilation exposes an explicit list of checks, separate source
status occurrence counts, asset reference/unassigned-site counts, and `false`
flags for source/approval/presentation verification.

**Independent initial interface-reference validation is blocked by the shared
contract at base `d66c900`.** `GameContent` has no interface-definition registry.
Current validation checks ID syntax, duplicate initial unlocks, and that guard/
event interface references have an initial unlock or `UnlockInterface` declaration.
It cannot distinguish a misspelled new initial interface ID from an intentional
declaration. The report explicitly sets
`interface_definition_validation_performed = false`. This is not a substitute
for the requested independent registry check or interface-behavior acceptance.
The Director must add a canonical interface ID/definition registry in
`crates/game-types`; this crate must then resolve all initial/guard/effect/event
interface references against it. No alternate shared model was introduced here.

Other precise shared-contract follow-ups:

- `RecipeDefinition` has inputs/outputs and target objects, but **no reusable
  tool requirements**. Gathering tools are validated; recipe tool checking is
  explicitly reported as not performed. Do not encode an unconsumed hammer as a
  consumed ingredient to hide this gap.
- `ProgressTransition.event/target`, `allowed_actions`, and spawn `facing` need
  shared typed/matching conventions. The compiler's current conventions above
  must agree with the simulator; this crate does not redefine simulation.
- Initial hitpoint/prayer skill bindings, run-energy units/maxima, reusable
  spell/prayer definitions, and object placement layers/rotated footprints are
  absent. Their source-specific behavior cannot be validated or invented here.
  Recognizing `cast`/`set_prayer` as intent names does not validate those mechanics.
- The model has no explicit tutorial completed-stage set or required/optional
  progression-node markers. Terminal tutorial stages are currently defined
  structurally, and every declared graph node is treated as required.

These gaps prevent claiming every requested M1 shared-contract validation is
finished, even when the independently useful compiler/loader tests pass. Missing
mandatory represented data is an error; absent shared fields are visible blockers,
not fabricated source rules. The component does not certify a production pack.

`AssetManifest { identity, assets }` is only an identifiable **projection of IDs**,
not a replacement for the asset workers’ canonical manifests. Optional absent
assets do not block independently valid headless mechanics. Missing concrete
terrain, models, UI, animations, audio, source captures, approvals, performance,
and RuneLite evidence remain separate M1 presentation/acceptance blockers.

## Reproducible validation

From the workspace root:

```sh
cargo fmt -p clubscape-content -- --check
cargo test -p clubscape-content
cargo clippy -p clubscape-content --all-targets -- -D warnings
cargo build -p clubscape-content --lib --target wasm32-unknown-unknown
cargo clippy -p clubscape-content --lib --target wasm32-unknown-unknown -- -D warnings
```

Tests exercise the actual compiler, strict JSON parser, binary loader, and native
CLI. They include positive synthetic definitions, hundreds of invalid mutations,
source policies, all header truncations, valid-checksum invalid payloads,
duplicate keys/cells/source IDs, unsafe quantities/XP/graphs, deep owned-tree
rejection, fake enormous length prefixes, round-trip determinism, and output
preservation. CLI test files live in the crate's ignored `test-output/` directory
and are removed by the tests; no system temporary directory is used.

Native tests and WASM code generation are compiler/toolchain evidence only, not
gameplay, visual/audio, browser, performance, or RuneLite acceptance.
