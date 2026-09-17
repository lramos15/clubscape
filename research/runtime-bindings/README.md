# Exact M1 runtime bindings

**112/112 paths have dispositions; 104 can be bound now.** This task changes
only `research/runtime-bindings/**` and `tools/runtime-bindings/**`. It does not
edit product definitions, old snapshots, approvals, assets, or server code.

| Disposition | Paths |
| --- | ---: |
| Explicit published fact/data/calculator model | 5 |
| Usable, source-supported reversible inference | 98 |
| Known fact unblocked by parent fix `7150464` | 1 |
| Inactive/alternative-only dependency | 6 |
| Narrow parent enum work | 1 |
| Material live-loot choice | 1 |

Repeated fields account for most paths: these are **29 rule groups**, not 112
independent unknown mechanics. Public reconstruction code is deliberately
labeled inference rather than Jagex server evidence. A `bound` inference is
usable engineering data; it is not a verified observation or owner approval.

## Artifacts

`resolutions.json` is keyed by the **exact original dotted/bracket paths**.
Each entry includes its unambiguous token pointer, input fingerprint, typed
proposed value/replacement, classification, precise sources/hashes, rationale,
scope and acceptance impact. Seven coupled changes are included.

`oracles.json` contains independently authored numeric and edge-case
requirements. `death-values.json` audits all 116 represented item IDs.
`profile-resolutions.json` addresses existing arrival/default candidates without
pretending they were observed. `sources.json` identifies 77 actual snapshots:
56 pinned wiki revisions, 20 public-code files and one public price feed.
The mutable feed's **original bytes** are retained compressed under `inputs/`;
normal reproduction must not refresh it. Wiki prose/code research caches stay
ignored and are reproducible by immutable URL/revision/hash.

`validation-result.json` and `typecheck-result.json` record actual checks,
explicitly **not an executed game journey**.

## Important resolutions

**Projectiles:** [Hit delay, revision 15319469](https://oldschool.runescape.wiki/w/Hit_delay?oldid=15319469)
documents bow `1 + floor((d+3)/6)` and Magic `1 + floor((d+1)/3)`.
Player-to-NPC processing adds one tick. The typed representation is base 2,
nearest-ties-up `d/6` or `d/3`. No extra whole-tick warmup is added; flight is
never zero. Distances 1-10 are checked against the published table. Measure
source same-edge Chebyshev distance, not an NPC center. Impact rechecking
means original identity/life/presence, **not** rerolling accuracy or rechecking
the old range/LOS. Do not double-add processing order. Client-cycle visual
launch offsets are a separate renderer concern.

**Transit/cooking:** contemporary RSMod handlers support immediate accepted
spiral-stair transit and one-tick direct ladder transit. A ladder's
choice-dialogue branch has an extra phase; do not apply direct-op timing to
the whole dialogue. Office portal phase is an explicit one-tick inference,
not a Home Teleport animation. Single cooking and Make-X stay distinct:
single 1, queued first 3 and repeat 4 under the stated discrete-action
convention; first bread/single shrimp are localized family inferences.

**NPCs:** incoming-damage calculator code retains the NPC's zero-inclusive
damage distribution, including a zero-max-hit tutorial chicken. Do not apply
the player's minimum-successful-1 rule. Walking consumes one legal tile per
tick; an idle wander decision is separate. Current radii remain explicit
anchors/candidates, not newly verified radii. Tutorial chicken/rat respawn
25/30 ticks inherit their ordinary species references and remain inferences.

**Shops:** choose linear stock-delta pricing, supported by the article's worked
example and contemporary shop code, not the wiki calculator's conflicting
division branch. This is an engineering interpretation, not an owner-level
redesign. Restock checks `world_tick % interval == 0`; purchasing again does
not restart the timer. Keep world epoch/processed-boundary state across restart.
Tests use 500-value items so integer rounding cannot conceal a wrong formula.

**Ground items:** automatic ammunition uses the documented automatic-drop
200-tick lifetime / 100-tick private period as an origin-specific inference;
manual inventory Drop is different (normally 300/100, with fresh-account
restrictions). Ashes last about 300 ticks after the fire ends; immediate
visibility is explicitly inferred. Death supplies are never public and have
character/world-switch persistence and offline-paused active time.
The current `GroundItemPolicy` values alone do not encode those clock/scope
semantics: the parent must honor them in the source-origin runtime, not use
a wall-clock expiry that deletes logged-out recovery supplies.

**Death prices:** the real RuneLite endpoint is `item/prices.js`, not the
nonexistent `prices.json` probe. Its `price` is the Jagex guide; `wikiPrice`
is actively traded pricing and is not used. The snapshot is
`69ecf1c861f20f8854848d1a9f309b34d8c7a69e338bd92db6a8ecddc0e77589`.
Resolve note aliases, calculate high alchemy, use explicit currency-1 and the
documented untradeable-value rule. Missing non-GE items are recorded, not
given fabricated GE quotes. Rare untradeable exception assumptions remain
visible. The finite provider is labeled `fixed_source_table`.

**Departure conserves money:** the source bank already has 25 coins at its
first opening. That is not a second coin reward on departure. The new
inference removes documented tutorial **noncurrency** types/note variants,
then grants the 18-kind provisions once. It leaves banked, withdrawn and
dropped coins where they are; expired money is not recreated. It therefore
supersedes the earlier *provisional choice* to replace every container and
set bank coins to 25, without rewriting that historical record. Noncurrency
bank/equipment cleanup and inventory kit placement remain explicit
inferences, not a captured departure dump. Cleanup, grant, both entitlements,
transport and stage acknowledgement must commit atomically.

**Defaults:** the official
[2025-10-22 update](https://oldschool.runescape.wiki/w/Update:Grid_Master_Rewards%2C_Poll_%26_New_Player_Improvements?oldid=15010818)
explicitly enables grave auto-equip for **all** new profiles. Replace the old
false candidate. Supply-piles-on is a labeled inherited-default inference;
the setting remains configurable. The update also verifies Quest Guide
tracking and lists audio/control defaults. Its ordinary Modern-layout
default does **not** override Section 30's explicit Resizable-Classic target.
Arrival areas/experience branches remain correctly separated; exact retained
tiles and cameras are not claimed observed, nor made into an account-use
prerequisite.

## What still needs parent action

**One precise enum gap:** the corroborating current-level rule raises current
to the new base only if it equaled the old base; drained/boosted values stay
unchanged. At base 10 -> 11, current 5/10/15 becomes 5/11/15.
None of the three existing vital policies expresses all three cases. Add
`raise_if_at_old_base_otherwise_preserve` (or an equivalent conditional type).
This is a source-supported inference, not a claimed Jagex observation.
Do not read this policy on Mining/Cooking-only level gains and block Cook's
Assistant unnecessarily.

**One material loot choice:** the official 2026-08-19 update says goblins
receive **1-4-dose** energy potions at a fairly common rate. That overrides
the energy-potion article's 1-3 summary, but neither supplies numeric odds.
The old primary table already sums to 128. Keep this exact gate, or explicitly
accept the supplied provisional independent 1/16 event with equal dose
weights. The latter is **not applied or verified**. Dose 3 (source item 3010)
and source clue/tertiary eligibility also need their proper content identities.
Do not substitute an empty pool, guaranteed coins, or a members-only condition
for ordinary F2P potion drops. No external account is requested.

**Six inactive fields are not six journey blockers:** two rare conditional
stacking alternatives are not acquired by the ordinary route; three fishing
respawn fields cannot execute when depletion is exactly 0; Office overflow
cannot execute with at most 116 ordinary merged item keys in a 120-entry store.
Keep their definitions/full-target requirements. Require the unresolved
policy only when its stated branch actually becomes reachable. The Office
proof is invalid if the item universe/instance-merge rules change.

## Exact application and checks

Run from the reserved worktree/project root before product integration:

```sh
python3 tools/runtime-bindings/validate.py --self-test
python3 tools/runtime-bindings/apply.py
CARGO_TARGET_DIR=tools/runtime-bindings/.local/target \
  cargo run --offline --quiet \
  --manifest-path tools/runtime-bindings/typecheck/Cargo.toml -- \
  research/runtime-bindings/.local/game-content.candidate.json
```

The overlay writes **only an owned candidate**, applies all 104 replacements
and seven coupled changes, and verifies each old target. If only unrelated
input bytes changed, `--allow-unrelated-input-changes` still requires every
target/coupled fingerprint to match. It never promotes the eight remaining
dispositions or overwrites canonical product content.

The parent should wire this overlay/equivalent resolutions into its content
generator **before** JSON/compiled-artifact serialization, preserve inference
provenance, resolve the enum/loot items above, and run the real authoritative
journey. Applying only departure cleanup without its coupled grant is invalid.
The strict compiler accepted the candidate in Runtime mode and decoded the
known Wind Strike numeric-key table; exactly eight explicitly classified
unresolved fields remain.

To reproduce all raw evidence and the hash-bound source check:

```sh
python3 tools/runtime-bindings/sources.py --restore
python3 tools/runtime-bindings/build.py
python3 tools/runtime-bindings/validate.py --sources --self-test --report
```

No source-account creation/login, approval-record change, test-fixture
compilation mode, or gameplay/presentation acceptance is involved.
