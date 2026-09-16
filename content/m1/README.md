# Source-bound M1 content, schema 4

**The actual content/artifact-4 candidate strictly compiles; bounded water-filling
conformance is still blocked on item-on-only facility authorization.** All prior source selectors
and the conditional vital enum are retained. There are zero active
`SourceBinding::Unresolved` values and six proof-scoped inactive dependencies,
not an old decoder/asset blocker. Source-solid contact and fresh manual-drop
clocks pass. The new source recipe does not fabricate a visible sink operation
to bypass the remaining seam. See the
[water-fill handoff](../../research/water-fill/README.md),
[`contract-gaps.json`](../../research/m1-bindings/contract-gaps.json) and the
current [`status.json`](../../research/m1-bindings/status.json), plus the
[UI4 candidate and remaining control work](../../research/interface-contracts/verification.json).

The preserved current-profile **baseline** is `m1.source-backed.v4.bdbf8a3788b842be`, raw artifact
`5b3ba5f108ed3fec8f8b5f7f49b429c059e21b6f192ec99a0569616e08330059`.
The new candidate's exact revision/hash is in `manifest.json`; the independently
derived legacy5e+water target is in `legacy5e-water/manifest.json`. No migration
or actual continuation is admitted. Persisted state/runtime remain version1,
with explicit UI state version1.
An existing world must keep its exact artifact unless the operator performs
the documented fenced `migrate-ui --from <old-raw-sha256>` upgrade; do not
silently repin it or recreate missing UI history. Fresh journey candidates
use ordinary account creation, not seeded checkpoints.

This candidate adds semantic All amounts, partial recovery, original level-up
bindings, the actual instance-template observer, server-owned music unlock
history and qualified source actor-animation rules. Bank layout2 explicitly
stores All preferences. Audio histories distinguish tracked-from-creation from
legacy-untracked; unknown history is not an empty or fully unlocked account.
Native variable491 exposes only its verified bit mask, not a fabricated whole
word. Existing5e/6f worlds remain pinned until an explicit fenced migration.
The original clips listed in `animation-requirements.json`, normal-grave
Bank-All permission and dough/Empty motion remain separate source/presentation
requirements, not claims of complete M1 acceptance.

## Build and verify

From the worktree root, with the documented existing Python/Rust toolchains:

```sh
python3 tools/m1-content/validate.py --repeat
```

That command runs every gate twice, records exact hashes and exits nonzero on
native failures; repeatable failure is **not** a pass. Individual commands:

```sh
python3 tools/m1-content/build.py
python3 tools/m1-content/check.py
python3 tools/m1-content/verify_assets.py
python3 tools/m1-content/verify_runtime_bindings.py
python3 -m unittest discover -s tools/m1-content -p 'test_*.py' -q
python3 tools/m1-content/verify_routes.py
python3 tools/m1-content/verify_state_oracles.py
```

The build uses committed inputs, then invokes the **actual** `clubscape-content`
Runtime compiler. No fixture mode, permissive validator or field-stripping is
used. `check.py` invokes the real strict parser/compiler and artifact loader,
constructs the actual engine, validates fresh creation/lifecycle/read-only APIs,
executes the first source appearance request and runs isolated native probes.
`game-content.json.gz` is the source-format product; `game-content.csc.gz` is the
compressed compiled artifact. The manifest records their exact versions and
compressed/uncompressed hashes. Gzip timestamps/filenames are normalized.

| Content | Count |
| --- | ---: |
| Items / reciprocal ordinary note pairs | 122 / 55 |
| Skills / normal equipment slots | 24 / 11 |
| NPC / object definitions | 26 / 4,837 |
| Runtime spawns: NPC / object / item | 166 / 360 / 10 |
| Recipes / dialogues / interfaces / shops | 14 / 21 / 31 / 1 |
| Typed counters / grants / entitlements | 134 / 18 / 13 |
| Physical door transforms / source door groups | 68 / 49 |
| Complete combined collision selections / mixed-leaf states | 136 / 38 |
| Actor-specific tutorial traversal definitions | 9 |
| Transit pairs / typed travel definitions | 11 / 26 |
| Combat styles / spells / projectiles / prayers | 27 / 2 / 2 / 1 |
| Runtime navigation regions / explicit cells | 29 / 46,358 |
| Full source regions / explicit cells | 61 / 999,424 |
| Individually retained source object placements | 151,019 |
| Tutorial states / source edges, all bound | 71 / 73 |
| Cook states / source edges, all bound | 10 / 22 |
| Death source states / source edges | 4 / 6 |

`manifest.json` is authoritative if generation changes counts.

## Applied source policies and approved loot

Canonical generation now runs the exact source applicator **before serialization**.
All104 available replacements and all seven coupled updates are consumed.
The parent's already-bound Wind Strike value is preserved semantically and by its
audited fingerprint; eight other accepted input differences are exact asset-locator
refreshes, not permission to ignore changed source targets. The 98 source-supported
reversible inferences remain **inference**, never observations or owner approvals.
See `research/m1-bindings/application-result.json`; the original
`research/runtime-bindings` source evidence is not rewritten.

Departure removes the documented **noncurrency** tutorial types/note variants
from inventory, equipment and bank, then grants the 18-kind noncurrency provisions
once. Cleanup, provision grant, reciprocal entitlements, completed transport and
stage acknowledgement belong to the same authoritative transaction. Banked,
withdrawn and dropped coins stay where owned; expired money is not recreated.
There is no second25-coin reward or bank reset.

Owner approval594a4fd adds one independent1/16 goblin energy-potion event with
uniform1/2/3/4 doses. The canonical type lowers that distribution to a separate
64-way pool:60 no-potion outcomes and one outcome for each source dose. It uses a
fresh draw independent of the unchanged128-weight primary pool. This is
**approved_adaptation**, not verified OSRS odds. Original item3010 and note3011
are decoded from verified cache bytes; guide-price123, not actively traded
wikiPrice108, extends the fixed death-value table. The3-dose item is also included
in the source death-supply classification.

The source tertiary candidates are retained separately; the potion approval does
not certify full-target clue-family/ownership eligibility.
Invalid, uncredited and replayed NPC lives must not resolve loot twice.

## What is now represented

Source skilling curves retain unclamped endpoints including `+1`: copper/tin
**101/351/256**, shrimp **49/257/256**, with exact level domains and round-nearest
interpolation. Typed cadence distinguishes single/first/repeat/menu delays,
per-tool ownership and bounded respawn distributions. Five cooking/facility
variants preserve success/burn outputs, XP and source range guards; firemaking
creates an owned temporary fire, retains the ground log on failure, and uses
the source cardinal step order. No stochastic activity is replaced by guaranteed
success.

Every tutorial edge retains its authoritative source hook in content4. Real dialogue
choices, source actor targets, UI contexts, production method/outcome, credited
kill method, valid Wind Strike hit/splash and completed source travel are checked.
There is no client stage-advance command or v1-disabled group of 36 edges.
Interacting is not interchangeable with succeeding, and a generic hit/kill or
invalidated cast cannot award Learning the Ropes.

Supply-recovery entries are normalized after the immutable source-binding
application. An eligible primary lesson remains the sole entry; otherwise one
guarded recovery menu offers the original eligible grant choices. Replacement
choices require an actually missing item or an unsatisfied source top-up count
over the declared inventory/equipment scope. Multiple missing grants cannot
create multiple matching entries, and a present net no longer blocks the next
Survival Expert lesson. Choice/grant identities, quantities, stage gates and
capacity behavior remain unchanged. This is an explicit source-supported
dialogue-routing interpretation; the original source records are not rewritten.
`test_dialogue_entries.py` and the native `dialogue_entries` example verify the
lesson, lost-tool, equipped-tool, full-inventory and departure boundaries.

Primary tutorial entries with the same source question and guard are one
choice menu, not competing entry candidates. The Magic Instructor's departure
question retains all three original decline, Ironman-information and normal
confirmation choices, including their order, guards and transition effects.
Only normal confirmation authorizes Home Teleport; no menu choice performs
departure or awards provisions. Incompatible text, duplicate choices or
explicit node continuations fail normalization instead of gaining arbitrary
entry priority. `test_departure_entries.py` and the native `departure_entries`
example cover all four offer/confirmation branches and wrong-stage refusal.
This lowering runs after the unchanged104+7 source application.

Ordered grants, line satisfaction and durable entitlements implement partial
Vannaka supplies and missing-only/top-up recovery. The normal inventory remains
empty at creation. **25 bank coins are granted once before first presentation**,
not repeatedly seeded or placed in inventory. Quest rewards use `Once` and
source run-energy restoration. Canonical full energy is **10,000**, not 100.

The mill uses character counters for hopper grain and flour units 0–30,
source varbit 5325 and the actual empty/full flour-bin variants. Loading consumes
grain; controls process only a filled hopper; collecting consumes a pot and one
flour unit. Its original three floors and both ladders remain connected. There is
no grain-plus-pot-to-flour shortcut.
The additive `recipe.water.bucket` records the source one-tick, zero-XP
empty-bucket-to-water replacement at sink14868. Its original empty object menu
and geometry are unchanged; current types still lack non-menu item-on facility
authorization. Current actor metadata marks its motion unverified, while the
minimal legacy target preserves its absent actor-authority block.
Every valid morph selector maps to an original equal-clipping variant; invalid
counter values reject. The unreachable runtime fallback names the actual empty
variant, while the original absent fallback remains in the source records.
Ordinary/tutorial bone burial uses `ConsumeOnly`, real input consumption,
4.5 Prayer XP and two ticks, with no fake output item.

Cook's ordinary milk/flour/egg graph accepts precollected ingredients and every
partial-delivery order. Rewards are exactly 1 QP, 300 Cooking XP, source energy
restoration and range permission, never coins. Bottomless milk 33089/33091 and
its 10,000-charge capacity are retained as a charged full-target alternative.
No Brutus/Ides of Milk acquisition is invented or made a new starter requirement.

Death has a private source-chunk instance, a separate walkable **player** arrival,
all three required topics, guarded portal exit, retained-item/valuation policy,
grave/Office fees, clock pauses and storage limits, explicit contextual recovery
interfaces, and source-inferred two-tick Dying/four-tick Respawning phases.
Valuation/restoration are bound. Only the proof-scoped unreachable Office
overflow remains unresolved; it is not free recovery or a tutorial reset.

## Coordinates, doors and stationary NPCs

X increases east, Y north; planes are 0–3 and source scale is 128 units per tile.
Unlisted cells are blocked. Every original scenery placement remains in the full
source bindings, including beyond the navigation envelope. Movement/sight masks,
source placement shape/layer/quarter-turn, bridge-plane data and model transforms
remain separate.

Door states change actual source leaf placement and explicit clipping, without
deleting whole walls or clearing terrain. Every mixed double-leaf combination has
an explicit combined replacement: no last-writer-wins masks. Independent original
object insertion checks all 636 combined-state cells. Open/close roundtrips restore
source cells. Thirty source route segments
connect through those **actual declared open-state masks**, not the v1 door-omission
check map. Source hinge/landing candidates remain inferences, not captured
observations or owner-approved rendering.
Separate actor traversal guards preserve lesson permission even after another
actor opens a shared door; the rat gate distinguishes entering from leaving.
All 28 fixed/experience/respawn travel destinations are explicit and walkable.

Death remains at source map pin **3180,5727**, using `ScriptedActor` navigation and
walkable access tiles. Player arrival is a different, walkable candidate
**3174,5726** inside the private source mapping. Fishing NPC **3317** stays at all
three water coordinates with `NonWalkingResource` policy and real gather
interactions. One published mobile goblin candidate conflicting with source
clipping is retained in the source-only candidate ledger, not moved or exempted
as a stationary combat actor.

## Remaining limits

The former blanket111-binding block is obsolete. Source-backed projectile,
NPC/transit/cooking timing, stock phase/prices, guide death values, restoration,
repeat-death behavior and currency-conserving departure are now bound. Fresh
grave auto-equip is corrected to the documented true default; supply piles use
the explicitly recorded inherited-default inference. Historical assumptions are
retained, with current resolution provenance rather than rewritten history.

Six residuals are inactive under explicit ordinary-route proof preconditions:
two unacquired rare conditional alternatives, three never-depleted fishing
respawns and Office overflow within the published ordinary-key/capacity proof.
Never require these policies before their reachable branch, and recheck the
proof if acquisitions, item universe or instance/merge rules change.

The declared `raise_if_at_old_base_otherwise_preserve` HP/Prayer policy is bound:
old base10 -> new11 maps current5/10/15 to5/11/15. Mining/Cooking-only gains do
not read that vital policy. No enum gap remains.

The shared tagged numeric-key decoder is now repaired. Wind Strike's known
1/5/9/13 → 2/4/6/8 source table is bound as `MaximumHitFormula::LevelTable`,
not an unresolved field or a constant
max hit.

The completed source closure is now connected through the validated merged
`cache2695-consumables-bundle.json.gz` catalog and all four publication layers.
All **5,266 requested product asset references** resolve, including the formerly
missing 72 item definitions, 68 requested models, six NPC definitions (Cook4626)
and 13 interface groups. Original definitions3010/3011, model2697 and dependent
placeholder19365 are now published. The two additive layers supply 272 original
assets and 1,137 outputs. No requested closure list remains nonempty, and no
model or icon link is fabricated. Later original vial/beer-glass publications
are preserved; water filling creates no new asset or guessed animation link.

`asset-refresh-validation.json` records the exact source/output hashes and the
unchanged original asset/geometry boundary. The audited source application
preserves the71/73 tutorial graph, Cook10/22, geometry and repaired Wind Strike
table while changing only approved/source-resolved targets and coupled values.
It is not full M1 source certification or an executed runtime journey.
The earlier 258-file audio export is explicitly historical: upstream75bfde1
corrected native percussion and added cues before this task. Its exact current
264-file manifest and files are hash-validated separately, without an
unchanged-audio or playback-acceptance claim.
Penguin NPC2063/model21547 is the approved source base in the externally approved
v1.3.0 reference pack; the actual rendered player/equipment still needs acceptance.
Source captures, live mechanics execution, persistence, presentation/audio,
browser performance, owner approval and RuneLite acceptance remain separate.

## Native conformance and integration paths

At the original Lumbridge range, the walkable allowed west-side tile
`3211,3215,0` accepts both selected Single and Make-X-of-one, preserving their
one-tick and three-tick initial deadlines. The near-face contact query preserves
source clipping, intervening walls, access-side masks, distance and plane.
The range is not made walkable or transparent.

Fresh manual drops select the explicit private/300 owner-online-tick policy.
An offline tick moves their absolute expiry from300 to301 without changing the
PlayerDrop origin or quantity. The selected clock is frozen in persisted ground
provenance. Source-supported playtime selection changes tradeable manual drops
at120000 played ticks (20 hours), while untradeables retain private owner-online
clocks and tutorial-stage overrides remain separate. Unknown legacy playtime
requires explicit migration, not a reset. Cross-world transfer is not exposed
by the single-world M1 profile and remains a full-target obligation.
`runtime3-policy.json` retains the source qualification and unobserved live
boundary explicitly; no10QP GE condition is substituted.

The raw compiled file is generated at
`tools/m1-content/.local/compiler/m1.csc`; its committed reproducible gzip is
`content/m1/game-content.csc.gz`. `content/m1/manifest.json` provides the exact
raw hash. Source asset resolution starts at
`assets/manifests/osrs/cache2695-consumables-published.json` and follows all four
publication layers. Original empty vial/beer-glass items, notes and models are
published; their real replacement outcomes are not omitted.
The backend owns `CLUBSCAPE_GAME_ROOT/clubscape-game.json` and
`clubscape-game-assets.json` deployment descriptors; this task does not create a
world UUID, seed state, publish files or claim a working server/browser journey.
