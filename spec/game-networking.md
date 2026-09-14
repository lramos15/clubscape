# Live-world server adapter

This is the server integration contract at the current engine boundary, **not
full M1 or source-content acceptance**. Account-only operation remains the
default. The live adapter executes real compiler-validated content through
`WorldEngine` and PostgreSQL, but the missing presence and public-view APIs
below prevent marking the live-world integration task complete.

## Configuration and source identity

`Config::with_game_root(path)` or `CLUBSCAPE_GAME_ROOT` explicitly configures a
game directory. An absent setting is account-only mode; a blank setting,
missing file, wrong checksum, invalid artifact, unresolved source binding or
failed engine construction is a startup error, never account-only fallback.

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
  }
}
```

The example identifies the configuration shape, not usable product content.
The world UUID must be non-nil and stable. Files are bounded regular files
under the configured directory, without parent traversal or symlinks. The
artifact is reloaded with `clubscape_content::load_compiled(..., Runtime)`;
definitions, source mode, checksums and indexes are not trusted merely because
an artifact was previously compiled. Any reported unresolved binding prevents
startup. The engine receives those immutable, validated definitions.

`clubscape-game-assets.json` uses the existing hash-pinned web-bundle manifest
shape (`schema_version`, `files[]` with `url`, `path`, `sha256`, `content_type`).
Game routes must be under `/content/` or `/assets/`. Every referenced content
asset ID must map to a present, hash-checked file, as must the public manifest.
Static files stream independently from RPC snapshots. Optional
`CLUBSCAPE_WEB_ROOT` remains independent; overlapping game/web URLs fail
startup. Existing same-origin headers, no wildcard CORS, path restrictions,
GET/HEAD behavior and loopback-only binding remain intact. File/hash validation
does not establish mesh, audio or presentation fidelity.

The exact artifact hash and a private 32-byte random key are pinned in
`game_worlds` by migration `0003_world_runtime_identity.sql`. The key is
OS-generated only once, never placed in `WorldState`, public protobuf or logs.
A different artifact cannot silently reuse the same world. HMAC-SHA256
counter draws are domain-separated by world and operation/tick identity, with
bounded unbiased rejection sampling. Retries of the same tick use the same
private draws; clients never supply random values or see the key.

There is no production fixture switch. The ignored server unit integration
tests alone compile a test-only branch that loads `TestFixture` artifacts.
It cannot be selected through environment variables, RPC or a production
`Config` constructor. Those tests use actual PostgreSQL and HTTP.

## Ownership, scheduling and acknowledgments

One bounded coordinator owns each configured world and its fenced lease.
The immutable engine never owns world time. At each 600 ms boundary:

1. `GameStore::commit_routed_tick` advances time once through the shared storage
   transaction, calling `engine.process_advanced_tick`, **not** `engine.tick`.
2. Due source activities commit, including actor-routed events.
3. Up to eight queued intents are attempted in receive order at that tick,
   with at most one new attempt per actor. Their actual effects come only from
   `engine.apply_intent`. Durable duplicates bypass callback execution.

The bounded channel and pending-intent queue each hold at most 64 requests.
Full queues return structured HTTP 429 with a retry delay. A coordinator request
has a nine-second response deadline within the existing ten-second RPC deadline.
Cancelled queued requests are discarded before mutation. A request cancelled
after work began may have an unknown outcome. Missed timer intervals use delay,
never a burst of catch-up ticks; network arrivals cannot advance world time.

Every storage group still uses the existing five-second `database::run` and
cancellation-owned connection discard. Auth tokens and exclusive player leases
are validated inside mutation transactions, including after waiting for locks.
Operation IDs are request UUIDs; intent hashes exclude auth/session/lease IDs.
Sequences are strict, positive and contiguous. `expected_character_revision`
is an observation hint for a sequenced intent, not an arbitrary state-replacement
CAS: future observations fail, while source ticks may have advanced state since
an older observation. The engine always evaluates current authoritative state.
The observation hint does not enter the intent hash.
Authenticated polls and admitted input requests heartbeat only their own
infrastructure lease; ticking itself never keeps disconnected leases alive.
Heartbeat metadata does not award progress or consume a command sequence.

Character creation copies the validated source initial definition and does not
run tutorial transitions. Creation options must be empty. Nonempty legacy
appearance/experience fields are explicitly rejected; callers must submit
`ConfirmAppearance` and `SelectExperience` as source-game intents after joining.
Unsupported engine implementations return unavailability without creating a
fake confirmation, choice, entitlement or progression result.

New `commit_routed_command`, `commit_routed_tick` and `session_snapshot` APIs
return transaction-consistent dynamic state for server projection. Existing
`commit_command`/`commit_tick` APIs and legacy receipts remain compatible.
`routed_events` is an optional/defaulted receipt field with actor membership,
stable-ID and shape checks. A receipt cannot mix legacy flattened events with
routed events. The server never guesses a recipient from an event's target.
IDs are domain-separated hashes of world UUID, committed revision, event ordinal
and recipient actor. They survive replay, remain bounded for long actor IDs,
and detect a receipt whose recipient changed without its identity.

State, revisions and event routing commit before acknowledgment. Command
timeouts stop further world processing rather than blindly rerunning a callback.
A timed-out tick is reconciled through a locked read: a committed next tick
uses its stored receipt; an unconfirmed outcome stops processing. Source errors
are not retried as database failures. Reconnect and process restart preserve
durable state, fences, operation receipts and private random identity.

## Public snapshots and polling

`CurrentAccount.character_initialized` is derived from actual authenticated
database state, including when the service is subsequently started account-only.
`game.v1` and `gameplay_available` describe a currently ready coordinator, not
milestone completion. Health and hello expose stopped/unavailable game state;
ordinary account operations remain independently implemented.

Every world response includes the complete local player and complete visible
ground list. Only the local player's inventory, equipment, skills, quests,
tutorial instruction, settings, experience, prayers and instance are projected.
Private flags, entitlement ledgers, RNG, other players' containers/quests/XP
and item-origin records are never broadcast. Other players expose identity,
appearance and position, not their private character records.

The network interest window is 32 tiles on the same plane and in the same live
instance. It is not a gameplay reach/LOS or source rendering-distance formula.
Explicit temporary-object and object-transform state has an additive typed
`DynamicObject` projection; static cells, assets and complete content never
appear in dynamic snapshots. Missing required definitions are errors.

Entity baselines are bounded to 128 entries/eight MiB across sessions.
An available baseline produces `full_snapshot = false`, changed entities and
explicit removals. A missing baseline produces a complete entity replacement.
The local player, ground list and dynamic-object list are complete in either
case. The wire response stays within 256 KiB, 2048 entities and 256 events;
oversized views fail explicitly rather than silently dropping state.

Event history holds at most 256 revision batches, 1024 events and four MiB.
Only the receiving actor's events are included. Polls use `after_revision`;
input responses may repeat recent post-join event IDs, which client-core
deduplicates. History gaps are explicit through `event_history_gap` and
`event_history_floor_revision`, with complete entity state. Rejoining establishes
a new event floor and sends no historical notifications/audio. A repeated
operation after restart cannot replay an old notification.

## Missing engine boundaries: integration is blocked, not substituted

The current `WorldEngine` public API exposes construction, initial state, intent
application and tick processing, but not the following required integrations:

* **Presence/lifecycle:** an authoritative actor join/rejoin, requested logout,
  auth revocation and transport-loss transition, with declared combat/retaliation/
  interruption timing and online/idle/offline/grave-clock inputs. `ClockPause`
  and `last_active_tick` exist in the shared state, but do not define a complete
  presence policy or provide a public transition API.
* **Guarded public views:** current bank/shop access, stock-sensitive per-unit
  quotes, dialogue text/eligible choices, recovery panels and fees, available
  interaction options, ground-item permissions, and source-variable morph
  resolution. The relevant existing engine authorization helpers are private;
  the server must not duplicate their rules or probe permissions by executing
  speculative mutations.

Until those APIs exist, the adapter has a conservative safety interlock, **not a
replacement presence mechanic**: each persisted actor must have explicitly
rejoined this coordinator, and every tick requires its exact live auth/game
lease. Missing joins pause processing and report unavailability. Loss/revocation
of an active lease fails the world closed before another actor tick; it does
not erase pending combat/activity or award offline gathering. A restart waits
for explicit rejoin without resetting stored time or applying offline catch-up.
This all-actor interlock is not suitable as a final multiplayer presence model.
In particular, a fresh client obeying unavailable capability negotiation cannot
complete ordinary reconnect while this missing-policy interlock is active.

Requested `LeaveWorld`/`RequestLogout`, and account logout with a character in
this world, return a concrete presence-API error rather than a successful
game logout. The account-only service is unchanged. External account-token
revocation still really revokes access and fails the interlock; it is not an
implicit successful gameplay disconnect.

Bank/shop/dialogue opening is refused before mutation when its guarded public
view cannot be produced. Recovery is explicitly unavailable. Existing unsupported
source appearance, experience and mechanics-v2 intents remain engine errors.
The generic snapshot does not leak bank/recovery data or invent quotes.
Source-declared entity action names have `actions_evaluated = false`;
ground `can_take` is not asserted without a permission helper
(`permissions_evaluated = false`). `unavailable_views` names these missing
boundaries rather than presenting them as resolved.

The engine owner must expose these source-owned operations before the server
task can be marked done. Neither an empty registry, a fixture, freezing the
world nor a successful infrastructure test resolves those contracts.

## Shutdown and verification

The service owns the coordinator and its supervision task. Shutdown signals
the coordinator while HTTP drains, cancels pending database futures using the
existing discard semantics, rejects queued work, and attempts fenced lease
release before closing the pool. Coordinator join/release are bounded; forced
abort/release or loop failures return `ServeError`, not clean-world success.
No ticker is detached from `Service::serve`.

Use the parent isolated PostgreSQL runner with this worktree and report
`.local/evidence/m1-live-world-server.json`. It selects all ignored server
binaries, including the test-only compiled-content HTTP integration cases, then
uses a fresh database for the independent account client and removes its owned
container. Normal native checks cover server, protocol and client-core tests
plus strict all-target Clippy. Synthetic fixtures establish integration only;
source product content, complete engine execution, gameplay/presentation and
owner acceptance remain separate.
