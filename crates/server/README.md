# Account service (M1-ACCOUNTS)

This is account/session infrastructure, **not accepted M1 gameplay**. It creates
no character, world, tutorial progress, content, UI, or assets. The frozen API is
[`account.proto`](../protocol/proto/account.proto); `hello` and account snapshots
explicitly report the protocol's gameplay-unavailable reason.

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

## Validation

From the workspace root:

```sh
cargo fmt --all -- --check
cargo test --quiet -p clubscape-server
cargo clippy --quiet -p clubscape-server --all-targets -- -D warnings
```

`tests/postgres.rs` is explicitly ignored in normal tests. Its real HTTP socket
and PostgreSQL tests require the parent runner's uniquely named ephemeral
PostgreSQL 16 container and random loopback port. Set
`CLUBSCAPE_TEST_DATABASE_URL` to its **`clubscape_m1_test`** database. Missing
configuration, a non-loopback database host or another database name is an
error, never a skip. The suite resets only its owned account/migration tables,
and never targets a public service.

```sh
cargo test --quiet -p clubscape-server --test postgres --locked -- --ignored --test-threads=1
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
