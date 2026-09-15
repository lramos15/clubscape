# Live-world server

The optional server adapter runs the actual source engine over PostgreSQL.
It supports **content/artifact 4, persisted state 1, runtime 1, UI state/view 1**. Lifecycle,
contextual views and quotes use the engine APIs in
[`game-contracts.md`](game-contracts.md#live-lifecycle-and-read-only-projection-boundary);
the former missing-API interlocks are removed. This backend is not UI,
presentation, real-product journey, performance or M1 acceptance evidence.

## Configuration and launch

Without `CLUBSCAPE_GAME_ROOT`, the service remains account-only. Setting it
requires a complete game directory. A blank setting, missing/invalid artifact,
hash mismatch, required unresolved binding, invalid restored state or failed
world lease prevents startup; none selects an account-only or fixture fallback.
Game and web environment roots are parsed independently: a headless game
service does not require `CLUBSCAPE_WEB_ROOT`, and an invalid configured game
root cannot be ignored merely because no web root is present.

```sh
# DATABASE_URL uses the separately managed PostgreSQL service.
CLUBSCAPE_GAME_ROOT=/absolute/path/to/game \
CLUBSCAPE_BIND=127.0.0.1:4010 \
cargo run --locked -p clubscape-server
```

`Config::with_game_root(path)` is the equivalent library configuration.
Optional `CLUBSCAPE_WEB_ROOT` serves the independently hash-validated UI bundle;
game/web routes cannot overlap. Binding remains literal loopback only.

The private `clubscape-game.json` descriptor is:

```json
{
  "schema_version": 1,
  "world_id": "00000000-0000-4000-8000-000000000001",
  "artifact": "world.csc",
  "sha256": "<64 lowercase SHA-256 characters>",
  "content_manifest_path": "/content/manifest.json",
  "assets": {
    "asset.example.scene": "/assets/scene.glb"
  },
  "readiness_profile": {
    "id": "ordinary_normal_f2p",
    "excluded_items": [
      "item.ensouled_goblin_head",
      "item.milk.bottomless_bucket"
    ]
  }
}
```

This illustrates configuration, not a product pack. `world_id` must be stable
and non-nil. The private descriptor is bounded to512KiB, accommodating the
full canonical M1 asset-ID map without trimming required source inputs; its
20000-entry limit remains in force. This does not raise client packet/response
or public bundle limits. The artifact is reloaded with the strict compiler's `Runtime`
policy, including revalidation and rebuilt indexes. Referenced assets and the
public manifest must be declared in `clubscape-game-assets.json`, using the
existing `{schema_version, files:[{url,path,sha256,content_type}]}` bundle format.
Only `/content/` and `/assets/` game routes are allowed. Files are bounded,
regular, hash checked and cannot traverse symlinks/parent paths. Static world
cells/assets stream separately, never in each actor snapshot. Existing
same-origin/CSP/no-wildcard-CORS/GET/HEAD behavior is retained.

The exact artifact hash and a private OS-generated PRF key are pinned in
`game_worlds`; a changed artifact cannot silently reuse that world. Keys never
enter public state, URLs or logs. Domain-separated HMAC-SHA256 counter draws
use world and operation/tick identity with bounded unbiased rejection sampling.
Clients do not submit random values, and retrying a committed operation does
not draw again.

## Source-aware readiness, not missing-value substitution

Omitting `readiness_profile` requires every reported binding to be resolved.
The explicitly selected `ordinary_normal_f2p` profile requires declared
`world_members = false` and computes conservative proofs over the actual
validated artifact:

* A conditional-stack alternative can remain unresolved only when it is
  explicitly excluded and has no initial/container, ground-spawn, positive
  shop stock, grant, recipe, reconciliation, transformation or non-members
  loot introduction. Only a literal source `MembersWorld` requirement (including
  a conjunction containing it) removes a loot pool through the false-membership
  proof; arbitrary guards are not reimplemented.
* A gather respawn binding can remain unresolved only when the source depletion
  rule is exactly constant zero. Cadence, relocation and other bindings remain
  independently required.
* Office overflow can remain unresolved only when the total defined ordinary
  item-key universe fits the declared Office capacity and possible
  introductions cannot produce charged/conditional/per-instance entries.

Every unresolved path must match a computed proof; there is no arbitrary
allowlist, count-based success condition, fabricated respawn or item-rule
coercion. The definitions remain unresolved in the compiled report. Logs retain
the selected profile and exact proved-inactive paths. Changed acquisition,
membership, capacity or item-instance assumptions invalidate the proof.
Restored state and every engine/lifecycle mutation are also checked against the
profile, including banks, ground items, shops, projectiles and recovery records.

Thus the six proof-scoped inactive alternatives described in the source
resolution inventory need not disable ordinary M1, but an unresolved active
input still prevents runtime readiness. Source evidence and owner approvals
are not authenticated by compilation or by these reachability/capacity proofs.
The actual regenerated v4 product and its public asset bundle remain launch
prerequisites; synthetic fixtures are not substitutes.

The UI4 source uses all four immutable publication layers: base, content-v2,
potions and `assets/manifests/osrs/cache2695-consumables-published.json`.
The fourth manifest is pinned to SHA-256
`2340ea5e3e6eeeeecdbda5de7f371f3ff2eb57576f4c783009344a1c348dbede`.
Original items229/230/1919/1920 and models561/2548/2747/8234 are published and
bound; `content/m1/manifest.json` records `asset_closure_passed=true` with an
empty missing list. No replacement output or original asset is substituted.
Build/check/asset verification continue to reject any missing or mismatched
required input. Asset closure and source/UI component checks do not establish
the full source journey, browser/audio presentation or owner M1 acceptance.

## Transactions, time and connection lifecycle

One bounded coordinator owns a fenced world lease. At each 600 ms boundary it
commits exactly one storage-owned clock advance, derives verified transport/auth
facts inside that transaction, applies engine lifecycle reconciliation, obtains
`engine.tick_context`, and calls `engine.process_advanced_tick_with_context`.
It never calls `engine.tick` inside a storage callback, decrements reserved
metadata, or marks all persisted actors online.

After due work, at most eight queued intents are attempted in receive order,
with one new attempt per actor per tick. Both channel and pending queue hold at
most 64 requests; overflow is structured HTTP 429. Missed timer intervals delay
instead of bursting. Requests have a nine-second coordinator reply bound inside
the existing ten-second RPC bound. Each DB group, including commit and owned
release, retains `database::run`'s five-second limit and cancellation/discard.

UI source mutations use the same boundary and `ui_request_requires_tick`
classification as engine admission. Preference changes, document/reward
continuations, read-only modal selection and public chat use the same bounded
queue and command journal without consuming an action phase. They cannot make
movement, combat or production run faster.

Verified transport connections are those actually joined to this coordinator
whose exact account-token digest and exclusive game lease remain live.
The 30-second heartbeat lease is transport ownership, **not** a combat/logout/
grave policy. Auth revocation yields `AuthenticationRevoked`; lease/transport
loss yields `TransportLost`. The engine decides whether the body is
Disconnecting or safely Offline. Disconnecting combat/projectiles and source
life phases remain vulnerable and continue processing; one disconnected actor
does not freeze the world or allow offline gathering.

At process restart there are initially no verified live connections. The
engine's `CoordinatorRestart` reconciliation preserves acknowledged state and
pending combat rather than waiting for every saved actor. Legacy untracked
input clocks can be migrated from the already authoritative
`last_action_tick`, not from a poll or a new invented timestamp. Actual join/
rejoin uses `apply_lifecycle`; repeated rejoin of an already-connected actor
does not reset idle. Accepted real intents report `Activity` transactionally.
Polls, quotes, heartbeat refreshes and duplicate callbacks never do.

Game and lifecycle mutations validate auth/ownership under locks, then commit
state, changed character/world revisions and actor-routed results before ack.
Gameplay command sequences remain positive and contiguous. Operation UUIDs
and canonical intent hashes provide restart-safe deduplication, independent of
auth/game-lease renewal. `expected_character_revision` is an observation hint:
future observations fail; source ticks may have advanced since an older
observation. Source mechanics always evaluate current state.

`GameStore::apply_session_lifecycle` atomically joins/leaves the exclusive lease
or performs account logout together with the source transition. `LeaveWorld`
and `Logout` do not consume gameplay sequences. Migration
`0004_game_lifecycle.sql` journals their request identity/intent and bounded
receipt. Reused IDs with changed actions/authorization conflict, and a stale
leave cannot delete a replacement session. A non-owner account token can log
out without disconnecting the other token's game session.

Requested logout uses the source engine's combat/projectile/life/travel rules;
rejection leaves auth/session/world state intact. `WorldInput.request_logout`
remains a sequenced source intent; its retained lease permits outcome recovery
until explicit leave, auth logout or expiry. This does not implicitly rejoin
an already-offline actor. Account-only auth behavior is unchanged.

`control_world` handles idempotent owner-only startup/shutdown reconciliation
without changing tick or sequence. `commit_live_tick` provides verified
connection facts to source processing. `session_snapshot` combines live auth,
optional ownership heartbeat and a consistent view snapshot. Existing legacy
storage APIs remain compatible.

## Public protocol and views

Ready game servers advertise `game.observer.v1`. `Player.running` field30 and
visible-player `Entity.running` field20 are optional for older peers but always
populated by the new server. They report actual current-tick steps, not a run
checkbox or queued-path prediction. `movement_tick` fields31/21 are decimal
strings and preserve the final run step even if activity is now idle or energy
has reached zero; subsequent nonmoving ticks report false/no movement tick.

`Player.action` field32 and `Entity.action` field22 contain the typed
`ActorAction` observer. The engine records exact activity/method/recipe/target/
style/spell identity and stable action-instance/cycle timing inside the existing
transaction. Queries and reconnects do not restart that identity. Observer
metadata is additive, bounded runtime state, not a client-write API, source
gameplay rule, RNG draw or cadence change.
`nextActionTick` describes the source gameplay phase, not a replacement for
original sequence frame lengths/client-cycle timing. The renderer uses those
validated sequence definitions for playback.

The server uses explicit bound animation references first. For the exact
canonical M1 baseline, `research/interface-contracts/actor-observer-bindings.json`
maps named original RuneLite animation constants to the actual method/recipe;
range and fire cooking are distinct recipe selections. It never searches
nearby scenery. Unknown numeric animations retain the precise source
recipe/action/spell/style identity in the compatibility `animation` string and
typed action; they are not assigned an unrelated sequence. The renderer/shell
must consume that identity and must not reinstate adjacency fallbacks.

`WorldSnapshot.dynamic_objects` field13 stays unchanged and complete. The shell
forwards it as typed `WorldView.dynamicObjects`, including canonical `objectId`,
instance, state, doorOpen and quarterTurns. It may attach numeric `sourceId`
only by lookup in the validated definition catalogue, never by parsing IDs or
creating client-owned door state. Renderer handles, HUD/audio APIs and source
world/stock ownership remain unchanged.

`CreateCharacter` copies the source normal-account initial definition exactly.
Creation has no grants or signup-triggered progress. Creation options must be
empty; appearance confirmation and experience selection are real subsequent
source intents. Source runtime defaults/style initialization comes from the
engine's creation API, once only. Retry never overwrites inventory, XP or gates.

`game.ui.v1` is advertised only for a ready UI-enabled content profile. An M1
consumer must require that capability and `snapshot.ui.version == 1`, then
consume the complete typed projection. Absent `ui` means an unsupported older
profile, not empty successful UI data. The exact Rust/TypeScript DTOs and
request names are in the shared gameplay UI contract. The shell maps its flat
`GameplayUiIntent` into Protobuf `GameplayUiRequest`; the internal engine
representation is `GameIntent::Ui { request }`. Renderer/audio ABIs do not change.

Production menus may have no world target. Preserve Protobuf
`UiProduction.target` absence as `WorldView.ui.production.target = null`;
do not construct an empty `WorldTarget` message or invent a facility. Inventory
`UseItem` retains its selected item/target slots in trusted typed menu state,
and `ProductionSelect` continues to send only the menu ID, recipe, quantity
and explicit mode through the ordinary durable sequence/operation journal.

Bank callers echo `ui.bank.revision` in
`GameplayUiRequest.expected_bank_revision`. Invalid/missing preconditions fail;
stale revisions fail transactionally without consuming a sequence. A duplicate
operation with the same intent/sequence recovers its committed receipt before
this mutable-state precondition is checked. The existing
`WorldInput.expected_character_revision` remains an observation hint.

Views distinguish the selected tab from contextual/modal state and expose
target-bound production, entitlement rewards/continuations, native bank
metadata, exact per-actor abilities/bonuses/weight, recovery/coffer controls,
original documents and routed public chat. Public chat messages have stable
IDs independent of each recipient's event ID; repeat snapshots/history must
not duplicate rendered lines. Other players' private UI state is never sent.

### Explicit UI content upgrade

Stop the existing coordinator and configure a complete target v4 game directory,
keeping its stable world UUID. Run the operator-only command with the exact
old raw artifact SHA-256, not its compressed-file hash:

```sh
CLUBSCAPE_GAME_ROOT=/absolute/path/to/complete-v4-game \
cargo run --locked -p clubscape-server -- migrate-ui --from <old-raw-sha256>
```

`migrate_game_ui(config, old_hash)` is the library equivalent. The operation
acquires exclusive fencing, preserves source state/claims/sequence/tick/private
RNG and old receipts, initializes only absent legacy UI metadata, updates the
artifact/content pin and writes one audit row. It is idempotent and refuses an
active owner, wrong old pin or missing UI history in an already-versioned world.
Normal startup never performs this content swap. UI bank metadata is derived
from real old bank slots; old reward/chat history is not invented. Migration
failure/timeout does not claim rollback; retrying the same exact target checks
the committed pin/audit state.

Additive intents include `ProduceSelected` with explicit `Single`/`MakeX`
(including Make-X-of-one), `OpenGrave` and `OpenDeathOffice`. A generic
`OpenInterface` cannot manufacture contextual grave/bank/shop/Office access.

`PollWorld.quote` optionally requests a typed read-only bank deposit/withdraw,
shop buy/sell, or recovery quote. The server calls the engine's immutable
planners directly. Quotes consume no gameplay sequence, activity clock, RNG,
items or progress. Bank/shop quoted partial quantities and prices come from
the same planners as execution. Recovery quotes explicitly cover the full
selected quantities, not a future capacity guarantee.

Snapshots use `context_view`/`dialogue_view`/`bank_view`/`shop_view`/
`recovery_view`, `target_view`, `interaction_options`, `ground_item_views` and
`presence_view`. The server neither executes speculative intents nor copies
source guard, price, morph, fee, range or contextual-permission rules.
Typed responses include eligible dialogue choices, authorized own bank data,
shop stock/unit prices, quoted totals/partial denials, owned recovery layouts/
fees/active ticks, instance/temporary-object/transform state and actual pickup/
interaction permissions. Other players' inventory, bank, quests, XP, reward
ledgers, RNG and recovery contents are never broadcast. Safely offline actors
are omitted; vulnerable disconnecting actors remain visible.

Each response contains the complete local player, visible ground list and
dynamic-object list. The 32-tile same-plane/same-instance interest window is a
network limit, not a substitute gameplay reach rule. Entity deltas require a
known session/revision baseline; otherwise `full_snapshot` replaces the entity
set. Removals are explicit. Byte/entity/event bounds remain 256 KiB, 2048 and
256; oversized views fail rather than silently truncate authoritative state.

Actor association is persisted alongside each `GameEvent`. Stable IDs hash
world UUID, committed revision, ordinal and recipient; a changed recipient
fails receipt integrity. The server never guesses routing from event targets.
Events are published only after commit and filtered by recipient. History is
bounded to 256 batches/1024 events/four MiB; baselines to 128/eight MiB.
Missing history is explicit, and a fresh join/rejoin establishes an event floor
without obsolete notifications/audio. Input responses may repeat recent IDs;
client-core deduplicates them. Original request-ID spelling is echoed exactly.

Timeouts and lost acknowledgments never claim rollback. A command retry uses
the same UUID/sequence/intent to recover its journal receipt without a callback.
Unknown tick outcomes are reconciled by locked state/receipt reads; unresolved
outcomes stop further work rather than blindly rerun source effects.

## Lifetime, acceptance and verification

Health/capability reporting reflects actual configured coordinator readiness
and failures. `CurrentAccount.character_initialized` comes from authenticated
DB state, even if the service is later started account-only. Existing optional
web-bundle security behavior is retained.

Shutdown owns/cancels the coordinator and its supervision task, rejects queued
work, commits source transport-loss reconciliation and releases the fenced
world lease before closing the pool. The two lifecycle/release DB groups retain
five-second bounds; coordinator join is bounded to twelve seconds plus a
one-second abort join. Cleanup/loop failures return `ServeError`; no ticker is
detached. The normal account-only shutdown bounds remain unchanged.

Run native server/protocol/client-core and relevant engine/compiler/shared-type/
simulation tests, formatting and all-target Clippy. The parent isolated runner
selects all ignored server suites, runs real PostgreSQL/HTTP lifecycle,
concurrent-player/privacy, source-context/quote, combat-disconnect and recovery
tests, then an independent account client on a fresh DB and removes its owned
container. The synthetic test loader is compile-time test-only: no production
environment variable, RPC or constructor enables fixture mode.

Passing this integration does not mark source product, complete Tutorial
Island/Cook's Assistant, UI/presentation/audio, browser performance, RuneLite
or clinical/owner acceptance complete.
