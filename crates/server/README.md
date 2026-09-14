# Account service (M1-ACCOUNTS)

This is account/session infrastructure, **not accepted M1 gameplay**. It creates
no character, world, tutorial progress, content, UI, or assets. The frozen API is
[`account.proto`](../protocol/proto/account.proto); `hello` and account snapshots
explicitly report the protocol's gameplay-unavailable reason.

The public `game_storage` library module additionally provides real PostgreSQL
world/character/session persistence for a future authoritative runtime. The
running account service does **not** call it to create characters or run gameplay.
Game RPCs remain unavailable and `game.v1` is not advertised. Storage tests are
not source-content, gameplay, presentation, performance or milestone acceptance.

## Running locally

Use the pinned workspace toolchain and a separately managed PostgreSQL database.
The `clubscape-server` binary requires `DATABASE_URL`, including an explicit
database name. Do not put credentials on command lines, in source control or in
logs; use the parent project's local-service tooling to generate protected local
configuration. The service does not start or stop Docker.

- `CLUBSCAPE_BIND`: default `127.0.0.1:4010`; literal loopback IPv4/IPv6 only.
  Port `0` is supported and the actual address appears in the JSON `listening`
  event. No public plaintext bind, trusted forwarding headers or CORS policy is
  provided. Production TLS/proxy deployment is not implemented.
- Transient TCP accept failures are logged and retried without discarding the
  listener. Other accept failures, including descriptor/buffer exhaustion, use
  a one-second backoff. Shutdown remains selectable during either retry mode.
- `CLUBSCAPE_BUILD_REVISION`: optional exact revision string, returned unchanged
  by `hello`. When absent it is `unversioned-development`, not an inferred commit.
  Blank, control-character or greater-than-256-byte values fail configuration.
- `GET /healthz` executes a database query; only a successful query is ready.
- `POST /v1/rpc` requires exactly one `application/x-protobuf` content type.
  Requests, including streamed/malformed bodies, are limited to 16 KiB and body
  receipt to ten seconds. Errors are Protobuf results with real HTTP failure
  statuses and a server error UUID; valid correlation UUIDs are echoed. When a
  correlation ID cannot be recovered or is invalid, the response ID is empty.

Startup applies embedded, versioned PostgreSQL migrations before binding.
Invalid configuration, failed migrations, failed database access and occupied
ports fail closed. Ctrl-C and SIGTERM stop acceptance, drain HTTP work and close
the database pool within the deadlines below.

The library's `Config::new`, `Service::bind`, `Service::local_addr` and
`Service::serve(shutdown_future)` support in-process service ownership without
global configuration changes. `serve` owns all HTTP/1 connection tasks and closes
its pool after shutdown; it does not leave detached Axum connection tasks running.

## Deadlines and uncertain outcomes

Deadlines use Tokio's monotonic client-side clock, not PostgreSQL responses.
The server-side five-second statement/lock timeouts remain defense in depth:
they cannot protect a client from an open connection that withholds responses.

| Wait | Client-side limit |
| --- | --- |
| Startup database acquisition, including connect/authentication/setup | 5 seconds |
| Entire embedded migration run, including migration locks | 5 seconds |
| Startup dummy-password initialization | 5 seconds |
| Each database operation group, including acquisition and response receipt | 5 seconds total |
| Complete account command after body validation, including password work | 10 seconds total |
| Returning a successful connection to the pool, including its release ping | 2 seconds, inside its operation's remaining deadline |
| Pool close, including startup-failure cleanup | 2 seconds |
| HTTP graceful draining after shutdown signal | 6 seconds |
| Joining force-aborted HTTP tasks after drain expiry | 1 second |

Login's credential lookup and entire session-issuance transaction are separate
database groups; the account lock, pruning, insert **and commit** share the latter
group's single five-second deadline. Readiness, registration, account lookup and
logout each use one group. Request-body receipt still has its separate ten-second
limit. On shutdown, connection acceptance stops immediately; remaining HTTP work
is force-aborted after six seconds, followed by at most one second of joining and
two seconds of pool cleanup (nine seconds total for these async shutdown phases).
A forced drain/cleanup failure is logged and returns `ServeError`/nonzero process
status, not a claim of clean draining.

Runtime database/command/release deadlines return HTTP 503 with an `UNAVAILABLE`
Protobuf error and a fresh error UUID. Valid request UUIDs remain correlated with
sanitized JSON diagnostics containing the operation, phase and deadline. These
timeouts do **not** claim a mutation rolled back: a write or commit may already
have reached PostgreSQL while its acknowledgment was withheld. Write responses
explicitly report an unknown outcome; these client-deadline responses supply no
`Retry-After` and never automatically retry a write. Reconcile with authoritative
account/session state before taking another action. Correlation UUIDs are not
idempotency keys.
A forced HTTP disconnect can likewise leave a write's outcome unknown.

Cancelled or failed database operations detach and drop their physical
connections rather than leaving SQLx's unbounded release/rollback response wait
running in a background task. Successful release pings are explicitly owned and
timed. The eight-connection pool has no background minimum-connection maintenance
or idle/lifetime reaper; connections are checked on acquisition and closed on
shutdown. Password-worker permits remain owned by their blocking jobs even when
the requesting future times out or is aborted; fixed-cost hashing finishes before
its permit is returned.

## Authentication boundaries

The canonical protocol validates login names and password byte lengths.
Registration stores a normalized unique login name and independently salted
Argon2id v19 hash (`m=19456`, `t=2`, `p=1`, 32-byte output), but does not log in.
Passwords preserve whitespace, Unicode bytes and case. Stored hash parameters
are checked before verification to bound work even for corrupted stored data.
Password workers use `spawn_blocking`; at most four are admitted, with excess
admission returning 429. Unknown users still undergo dummy-hash verification.

An actual socket peer can make 20 decoded login/register attempts in a sliding
minute, including invalid inputs, unsuccessful credentials and conflicting names.
Input validation and admission both precede account/session operations. Limiter
cardinality is capped at 1024 live peer IPs, without evicting existing limits.
Expired entries are reclaimed on admission; exhaustion returns `Retry-After`
and the matching protocol delay. Forwarding headers never influence identity.

Tokens are 32 OS-random bytes encoded as 43 canonical URL-safe, unpadded Base64
characters. Only SHA-256 of those ASCII token bytes is stored. Sessions expire
30 minutes after PostgreSQL issuance time. Login locks the account row before
account-scoped expired/old-session pruning and insertion, allowing at most five
active sessions. Logout durably deletes only the presented valid session.
Protected identity always comes from that session, never request identity data.

Responses use `Cache-Control: no-store`. JSON logs contain event types,
correlation/error IDs, status, duration and sanitized dependency type/SQLSTATE;
not credentials, bodies, authorization, tokens or connection URLs. The binary
intentionally uses a fixed application-only log filter rather than enabling
dependency tracing through `RUST_LOG`.

## Server-side game repository

`clubscape_server::game_storage::GameStore` is a public adapter over `PgPool`.
`new(pool)` does not change pool configuration or create game state. Call
`migrate()` to run the same embedded migrations as `Service::bind`, or use an
already migrated service pool. Cloning a store shares its pool and world-owner
UUID. Constructing a new store creates a fresh owner UUID. `close(self)` closes
the entire shared pool, including other clones, with the existing two-second
cleanup bound; only the pool owner should call it.

| API | Contract |
| --- | --- |
| `initialize_world(world_id, initial)` | Install a validated source-derived dynamic `WorldState`, with no characters and tick/revision zero. Repeating the same world/content/schema returns stored state without resetting it; a different content revision conflicts. |
| `load_world(world_id)` | Server-only complete dynamic snapshot, including the latest tick receipt. This is not a client broadcast format. |
| `acquire_world_lease`, `renew_world_lease`, `release_world_lease` | Fenced ownership for authoritative world writes, described below. |
| `create_character(lease, authentication, SourceCharacter)` | Create exactly once per account, or return its existing character unchanged. Creation in another world conflicts. Never called by registration. |
| `load_character(world_id, authentication)` | Read only the live token's account's character in that world, or `None`; never initializes a missing character. |
| `join_session`, `read_session`, `heartbeat_session`, `leave_session` | Exclusive player-session infrastructure bound to the actual account token digest. |
| `commit_command(lease, access, GameCommand, callback)` | Atomically apply trusted mechanics, update world/character versions and store the exact result under the operation ID/sequence. |
| `commit_tick(lease, expected_tick, callback)` | Server scheduler only: atomically advance one tick, update changed character versions, and retain the latest committed tick receipt. No tick RPC is exposed. |

`AuthTokenDigest::from_token` reuses the account code's canonical token parsing
and SHA-256 implementation. `from_digest` supports the existing server auth
adapter. Neither constructor authenticates the caller: **each protected
transaction checks the live `account_sessions` row**. Digests have redacted
`Debug` output, and are not serializable. `SessionAccess` accepts world/actor/
session selectors but no account ID; account ownership comes only from the live
token. Do not expose the trusted source/state/callback arguments as HTTP inputs.

`SourceCharacter` must come from upstream content/reference/appearance
validation. It carries the content revision, shared `InitialStateDefinition`
and a supported appearance selection. Storage checks provenance presence,
schema, structural limits and revision matching; it cannot verify the sources
or appearance branches itself. All progression, location, inventory, equipment,
bank, skills, vitals, energy, flags, quests and interface fields are copied from
that definition. Only identity/bookkeeping is supplied by storage: a generated
stable actor ID, the account login name as display name, idle activity, no open
dialogue, the current world tick as last-action bookkeeping, and sequence zero.
There is no default or boosted character, no tutorial completion or invented
grant, and no fallback for missing/invalid content.

### Authority, locking and storage limits

Migration `0002_game_state.sql` creates these owned tables:

* `game_worlds`: stable UUID, pinned content revision/schema, monotonic dynamic
  revision/tick, `WorldState` JSONB, latest tick result, and owner/fence/expiry.
* `game_characters`: one account-to-actor/world identity mapping plus revision
  and last committed sequence. **Character state lives only in the world's
  `characters` map**; this table is an index, not a second state authority.
* `game_sessions`: one exclusive lease per account/actor with a generated UUID,
  actual auth token digest, heartbeat and expiry.
* `processed_game_commands`: per-account operation UUID, unique actor sequence,
  versioned canonical intent hash, committed revisions and exact result JSONB.
  The journal has no auth-token or game-session identifier.

The consistent locking order is:

1. One world row `FOR UPDATE`.
2. The authenticated account row `FOR UPDATE`, when applicable.
3. That world's character index rows in actor-ID order `FOR UPDATE`.
4. The account's bounded auth-session rows in digest order `FOR SHARE`.
5. Its exclusive game-session row `FOR UPDATE`, when applicable.
6. The processed-operation row `FOR UPDATE`, when applicable.

World lease operations need only the world lock. Server ticks need the world
and character locks, not a player's token. The preliminary token lookup only
discovers which account to lock; authentication is checked again under locks,
including after waits and before committing a player mutation. Shared live-token
locks serialize with the existing logout `DELETE`. Account-first ordering agrees
with login/pruning, without changing those APIs or requiring a foreign key to
`account_sessions`. Revocation and pruning cannot invalidate a token unnoticed
by a later game transaction. Account/world deletion is restricted while indexed
characters exist, avoiding orphaned JSON authority.

World and character revisions change atomically with the state and command
receipt. Every successful creation, new command and tick increases the world
revision. Each changed character gets its own revision increase; an accepted
command also advances the submitting character's sequence. Callbacks may change
other existing characters for shared mechanics, and their indexes advance in
the same transaction. Session/owner heartbeats do not change gameplay revisions.
Corrupt state, indexes, schemas or journal metadata return sanitized errors;
they never trigger new initial state.

This is an explicit **small-population M1 adapter**, not an MMO scaling claim:
one world serializes its mutations and holds at most 256 characters. Compact
world writes are bounded to 4 MiB, character state to 128 KiB, intents to 4 KiB,
and command/tick receipts to 256 KiB and 1024 events. Collection limits also
bound decoding and iteration. PostgreSQL canonical JSONB text can be larger
than compact JSON; database checks and read projections cap it at twice the
corresponding compact limit. Static content, collision definitions and assets
belong outside this row. A later partitioned/relational adapter must preserve
the same atomic ownership, revision and journal contracts.

### Player leases versus world fencing

Player lease durations range from 1 ms to five minutes, capped by the live auth
token's expiry. Rejoining with the same still-live token and game lease reuses
the session UUID and updates the heartbeat. Another live token's active lease
conflicts. Explicit leave, player-lease expiry, or old-token
revocation/expiry/pruning permits a new UUID and owner. A heartbeat cannot revive
an expired lease; reconnect through `join_session` instead. Repeated leave of an
absent row returns `false`, but a stale selector cannot remove a replacement.

These are infrastructure ownership timers, **not source combat/logout/death/
disconnect mechanics**. Leaving a lease does not change character activity,
award items, cancel combat or apply a gameplay logout rule. The runtime must
enforce those rules before accepting a requested leave and during later ticks.
The account logout API continues to revoke authentication immediately; token
revocation does not silently modify durable gameplay state.

World-owner durations range from 1 ms to 60 seconds, using PostgreSQL time.
A still-active owner can reconnect/renew its fence; another owner must wait for
explicit release or expiry. Each new ownership period increments the fencing
token, even when the same repository reacquires after expiry. Character creation,
commands and ticks validate the active owner/fence while locked and again at
the authoritative write/commit boundary. An old owner cannot write after a
takeover. Player session operations are independently authenticated
infrastructure, not simulation writes, and do not require the world-owner
capability.

`commit_tick` compares the persisted tick to `expected_tick`, increments it once
before invoking the callback, and rejects a stale/gapped expectation rather
than repeating simulation. Retrying the immediately previous tick returns its
stored receipt with `duplicate = true`, even after intervening player commands.
Only the latest tick receipt is retained: older outcomes must be reconciled
from the current world, never resimulated. Multiple ticker instances cannot
double-apply a tick through this adapter. The runtime still owns the source
600 ms cadence and deterministic random draws; faster calls are not permission
to accelerate gameplay.

### Command journal and failure semantics

`GameCommand` carries a non-nil operation UUID, a positive contiguous sequence
starting at one, and a typed `GameIntent`. Hashing uses domain-separated SHA-256
of version-1, recursively key-sorted compact intent JSON. It excludes the
token, player-session UUID and world lease, allowing safe authenticated
reconnect/renewal/repository restart.

* A previously committed operation with identical intent and sequence returns
  its exact `CommandReceipt` with `duplicate = true`. No callback runs, even if
  newer commands changed the character. Load the current snapshot separately
  when needed.
* A reused operation UUID with another intent/sequence conflicts. A different
  operation with a stale or gapped sequence also conflicts. PostgreSQL uniqueness
  plus the locked transaction handles concurrent retries and contenders.
* Callback `GameError`, structural/bound failures, ownership/auth failures and
  rejected sequences produce no success receipt. Before commit, such failures
  roll back all state/index/journal mutations and consume no sequence or
  operation ID. A corrected, never-committed operation can therefore retry.
* State, revisions and the full result/events are committed together **before**
  acknowledgment. The journal is not pruned automatically. Compaction would
  need an explicit retention/replay contract.
* A timeout, lost response, release failure or forced disconnect may happen
  **after commit**. Those errors do not assert rollback. The inherited generic
  deadline message conservatively says not to retry automatically; a game-aware
  caller can reconcile/retry the **same** operation UUID, sequence and intent
  under valid renewed ownership to recover the journal result. Never substitute
  a new operation ID or grant items locally to resolve an uncertain outcome.

Callbacks are trusted, deterministic, bounded, synchronous and nonblocking.
They must perform no I/O/external side effects and cannot change actor-map
membership, identities, schema/content, world revision, command sequences or
the repository-selected tick. Return `GameResult<Vec<GameEvent>>`; validation
failures are sanitized by code, not by exposing arbitrary callback text.
Storage does not implement action eligibility, source progression or tick
mechanics. Every database group uses `database::run`, including acquisition,
locks, all writes, commit and owned release; cancellation detaches/discards the
physical connection. As with any async deadline, a callback must not block the
executor to bypass the caller's timing bound.

`GameStorageError` exposes public HTTP status, protocol code, static sanitized
message, error UUID and retry delay. It preserves the account error's UUID and
correlated sanitized dependency logging, adding a `game_storage_failure` event.
It never includes SQL, connection URLs, token values, stored JSON or callback
error text.

## Validation

From the workspace root:

```sh
cargo fmt --all -- --check
cargo test --quiet -p clubscape-server
cargo clippy --quiet -p clubscape-server --all-targets -- -D warnings
```

Database tests in `tests/postgres.rs` and `tests/game_storage.rs` are explicitly
ignored in normal tests. Their real HTTP socket and PostgreSQL tests require
the parent runner's uniquely named ephemeral
PostgreSQL 16 container and random loopback port. Set
`CLUBSCAPE_TEST_DATABASE_URL` to its **`clubscape_m1_test`** database. Missing
configuration, a non-loopback database host or another database name is an
error, never a skip. The suites reset only owned game/account/migration tables
in foreign-key order,
and never targets a public service.

```sh
cargo test --quiet -p clubscape-server --tests --locked -- --ignored --test-threads=1
```

Tests include two actual binary process launches against the same database,
SIGTERM/Ctrl-C shutdown, persistent identities/sessions, PostgreSQL constraints,
bounded concurrent registration/issuance, expiry and scoped pruning, malformed
HTTP/Protobuf/authentication, actual-peer limits, unavailable database transport,
open-socket response withholding (including committed writes with lost
acknowledgments), migration deadlines and bounded/forced shutdown. The proxy
regressions use only local test connections; they do not pause PostgreSQL or
modify other services. Test credentials are generated in memory and no services
are intentionally left running. Passing unit tests does not imply these ignored
tests ran. Neither evidence class establishes browser signup, a character,
gameplay, visual fidelity, performance acceptance or RuneLite compatibility.

The game-storage suite uses actual registered/logged-in accounts, actual token
revocation/pruning, real pools and migrations: no persistence mocks. It covers
source-fixture creation races and retry safety, exclusive/reconnected/expired
leases, wrong ownership, live-token checks, atomic state/event journals,
mismatched/stale/gapped commands, concurrent exact-once retries, callback
rollback, bounded serialization, pool/repository and auth-session renewal,
fenced ticker takeover, and corrupt-state failure. A loopback PostgreSQL proxy
withholds a successful `COMMIT` response; the caller times out in five seconds,
its connection is discarded, and a later fenced retry returns the durable
result without rerunning the mutation. Synthetic initial definitions are marked
`EvidenceStatus::TestFixture` and are not usable as source/gameplay acceptance.
