# Architecture and initial shared contracts

## Boundaries

The fleet continuation adds the shared [game contracts](game-contracts.md).
They preserve these account boundaries while defining the source-driven
simulation/content and authoritative persistence extensions needed for M1.

Use a Cargo workspace with Rust 2024, pinned by `rust-toolchain.toml` and
`Cargo.lock`. The first independent infrastructure increment consists of:

- `crates/protocol`: schema-generated Protobuf messages, protocol constants and
  input validation that can compile for native and WASM clients.
- `crates/server`: Tokio/Axum service, PostgreSQL migrations and authoritative
  account/session operations. No client or graphics dependencies.
- `tools/simulator`: a native protocol client exercising the same HTTP endpoint
  a browser will use. An account lifecycle check is not a gameplay simulator or
  evidence of the complete tutorial.
- `tools`: minimal local-service, validation and milestone-checkpoint commands.
  Do not build WaddleWorks before a measured coordination need exists.

Later in this same milestone, source-verified `content-runtime` and `simulation`
crates must own reusable mechanics and deterministic action ordering.
Rust/WASM client-core, renderer, audio and input layers consume authoritative
events without duplicating server rules. Their presentation implementation
requires the approved reference pack. No placeholder world is part of this
infrastructure increment.

## Account versus character

An account is a real, persistent authentication identity. Creating it must not
fabricate a reference-defined character. Until the frozen normal-account
initial state and tutorial content are available, the server explicitly
advertises gameplay as unavailable and character initialization as incomplete.
This is an unpassed M1 requirement, not an approved reduction of the journey.

The account service stores a UUID account ID, a case-normalized login identifier
and an Argon2id password hash. The login identifier is ClubScape account data,
not an OSRS display-name or character-name adaptation. Initially accept 3-20
ASCII letters, digits and underscores; case is insensitive. Passwords are
15-128 UTF-8 bytes at registration and are not trimmed or case-normalized.

Opaque, random, 256-bit bearer sessions last 30 minutes. Only their SHA-256
digests are persisted. Logout revokes the presented session durably. Login
permits at most five active sessions per account, pruning older sessions under
an account row lock. Registering an existing normalized identifier must fail
atomically; retries cannot create duplicate identities. Passwords, raw tokens
and database credentials must never appear in logs or source control.

## Durability and extension points

PostgreSQL is the sole durable store; no in-memory or file-backed success
fallback is allowed. Apply embedded, versioned migrations before serving.
Readiness must check the database. Session expiry uses database time so process
restarts do not extend validity. Normal shutdown closes the listener and pool.

Authentication reads and mutations have no gameplay-side effects. Future
character initialization must be a transactional, idempotent operation tied to
a verified starting-state content revision, not a seed executed at signup.
Future acknowledged game mutations require durable command IDs/sequence
validation and committed inventory/XP/quest changes before acknowledgment.
Existing authentication request IDs are correlation IDs, not a claim that this
future replay-protection boundary exists.

Start one loopback-bound process and one isolated PostgreSQL service. Production
TLS termination, account recovery, game-world transport, distributed services,
and production deployment must not be advertised as implemented. No Redis or
Kubernetes is needed for the independent infrastructure increment.
