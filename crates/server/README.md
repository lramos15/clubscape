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
ports fail closed. Database acquisition and SQL statements have five-second
bounds. Ctrl-C and SIGTERM drain HTTP work and close the database pool.

The library's `Config::new`, `Service::bind`, `Service::local_addr` and
`Service::serve(shutdown_future)` support in-process service ownership without
global configuration changes. `serve` closes its pool after graceful shutdown.

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
HTTP/Protobuf/authentication, actual-peer limits, unavailable database transport
and startup failures. Test credentials are generated in memory and no services
are intentionally left running. Passing unit tests does not imply these ignored
tests ran. Neither evidence class establishes browser signup, a character,
gameplay, visual fidelity, performance acceptance or RuneLite compatibility.
