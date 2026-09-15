# Controlled real M1 protocol journey

This runner owns one bounded PostgreSQL container, one server process at a time
and one generated-Protobuf simulator. It does not manage global services, touch
live players, reset gameplay tables, invoke per-player agents, or use fixture
content as product evidence.

Read `AGENTS.md`, `docs/machines/sparky.md`, `spec/game-networking.md` and the
server README first. The machine guide is a dated host snapshot.

## Execute

From the repository/worktree root:

```sh
python3 tools/journey-tests/run.py \
  --report .local/evidence/m1-headless-journey.json
```

Prefer the fully integrated product GameRoot when it is available:

```sh
python3 tools/journey-tests/run.py \
  --game-root .local/product-game-root \
  --report .local/evidence/m1-headless-journey.json
```

`--game-root` and all output paths are relative to the current worktree. The
GameRoot must use the real `clubscape-game.json` and
`clubscape-game-assets.json` contracts, a strict Runtime compiled artifact, and
its actual hash-pinned assets. No test-fixture switch exists in this script.
Builds use project-local `.local/journey-target` and `.local/journey-build-work`.
`--skip-build` explicitly uses existing binaries; their SHA-256 identities are
still recorded and are not proof that they were rebuilt from the HEAD label.

Without `--game-root`, the script obtains the **exact required asset-ID set from
the real strict Runtime compiler**, validates the canonical publication chain,
and constructs an isolated source-only GameRoot. Original definition-collection
shards remain byte-for-byte original; their existing asset-ID keys select their
records. Other assets use hash-matched original published outputs. Every compiler
reference must be mapped; an absent output fails rather than becoming a
placeholder. Payload files are deduplicated by hash and use short loopback asset
URLs. This is not a rendered browser bundle or visual-fidelity claim.

The generated descriptor explicitly selects the documented
`ordinary_normal_f2p` readiness profile and its two excluded, unacquired rare
alternatives. The real server—not the packager—must prove all six inactive
bindings. Startup still enforces descriptor size, artifact/source readiness,
asset membership and engine construction. Compact JSON does not remove or relax
any startup check.

## Isolation and sequence

1. Build the real account/game server and simulator.
2. Start a uniquely named PostgreSQL 16 container on a random **127.0.0.1**
   port, with CPU, memory and PID bounds. Credentials are in a mode-0600 file
   under a mode-0700 project run directory. The container database and socket
   directories use bounded memory mounts; host temporary directories are not
   used.
3. Run the original real account lifecycle against an account-only server.
   Separately run the named journey as a **negative readiness test** against
   that server. A nonzero gameplay-unavailable result is required and never
   counted as journey progress.
4. Stop only that owned server process. Start the normal product server using
   the same isolated database and the supplied/constructed GameRoot. A startup
   error remains the first product blocker.
5. If genuinely ready, execute `clubscape-sim scenario m1_fresh_account`.
   Its onboarding/post-quest checkpoint requests trigger replacement of only
   the orchestrator's own `Popen` server process, against the same PostgreSQL
   data. The binary, descriptor and artifact identities must not change.
6. Persist the aggregate report, actual simulator report/trace and server
   diagnostics. Stop/reap owned processes and remove the exact owned container,
   credentials and isolated GameRoot. No gameplay SQL is executed.

The default restart is graceful. `--restart-mode crash` sends SIGKILL only to
the owned process and waits 35 seconds for the existing fenced lease, without
editing lease state. Crash-restart coverage is reported only when actually run.
The default journey deadline is 5,400 seconds, configurable within 30–7,200.
Every individual action and readiness/cleanup wait is also bounded.

## Restart handshake

This is private orchestration, **not a game API**:

* Simulator writes `control/restart-{onboarding|after_quest}.request.json`
  atomically with a request UUID and public checkpoint state. No password or
  authentication token is written.
* The owner verifies its current origin and checkpoint, stops its own process,
  starts/health-checks the replacement and atomically writes the matching
  `.ack.json` with the actual old/new PIDs and loopback origin.
* Simulator validates the acknowledgment and uses ordinary Hello/JoinWorld,
  authoritative snapshot comparison and durable-operation replay. It cannot
  instruct the orchestrator to kill an arbitrary PID or run a command.

Game/account writes occur only through public RPCs. The same isolated
PostgreSQL data is retained across both required restarts; the only database
creation/initialization is the container's ordinary startup and the real
server's migrations.

## Evidence and blockers

The aggregate JSON differentiates infrastructure checks, negative readiness
checks, product startup, passed journey segments and unchecked segments.
The detailed [simulator documentation](../simulator/README.md) defines the
source path, oracles and limitations.

[`evidence/base89f3386.json`](evidence/base89f3386.json) records the actual
28 Rust checks, 9 orchestration checks, both real account/readiness runs, exact
binary/content/protocol identities, first failures and verified cleanup.
**Zero product tutorial edges and zero required gameplay restarts passed.**

### Historical base89f3386 failures

At base `89f3386`, the committed product pack was artifact/schema 2, while the
engine requires artifact/schema 3. The existing network adapter also still
documents missing lifecycle/guarded views, and its generated `game.proto` lacks
`OpenGrave`/`OpenDeathOffice` and a public recovery-entry/fee view. These are
integration blockers, not permission to remove readiness checks, fabricate a
server, or mark the source journey passed. Regenerated content and the final
public adapter must be integrated before rerunning.

The source sink is used for an actual post-reward Cook-o-matic recipe. If the
product pack exposes its geometry but no legitimate bucket-fill item-use rule,
that exact source action fails. A completed quest label alone does not become
a successful range-reward demonstration.

The first real base-`89f3386` run found an earlier configuration defect:
`Config::from_env` reads `CLUBSCAPE_GAME_ROOT` only inside its
`CLUBSCAPE_WEB_ROOT` branch (`crates/server/src/config.rs:58–65`). Setting the
documented game-only environment therefore advertises only account/session
capabilities. The scenario fails at actual Protobuf Hello, and database health
alone is **not** reported as product readiness.

A second, explicitly labeled entry point tests the documented public constructor:

```sh
python3 tools/journey-tests/run.py --server-entrypoint public-api \
  --report .local/evidence/m1-headless-public-api.json
```

This builds the optional `clubscape-journey-server` launcher in the simulator
crate. It calls `Config::with_game_root`, `Service::bind` and `Service::serve`;
all HTTP/account/game, storage, clock, RNG, engine and strict startup logic are
the real unmodified server library. It installs no callbacks, fake server or
fixture content. This lets product artifact validation be tested independently
of the binary environment-parsing defect; it does **not** claim that defect is
fixed. The default remains the actual `clubscape-server` binary.

### Authorized final backend/content integration

Parent commits `1ea2e6539668a57600ddb7b8988785ba608ef61b` and
`e8dfa2df63e7694ac1b52b865dbe9478504646d6` were explicitly authorized for this
worktree. Their local cherry-picks are `1d2f6d6` and `bcc0de9`. The updated backend
implements lifecycle/contextual views and quotes, and exposes `ProduceSelected`,
`OpenGrave` and `OpenDeathOffice`. Its documentation describes independent
game-only configuration, but the actual `Config::from_env` implementation still
nests GameRoot under WebRoot: the real binary probe still stops at Hello.
The real source pack is artifact 3 with zero active unresolved bindings and six
proof-scoped inactive bindings. Historical artifact-2 evidence is not a claim
about this new revision.

The launcher additionally supports `inspect-artifact <project-artifact>` for
packaging. It calls `clubscape_content::load_compiled(..., Runtime)` and emits
the actual compiler reference set; it does not instantiate or seed characters.
Both production entry points retain the same real server startup checks.

The explicit real-Service run found a separate hard deployment blocker:
the complete **5,010-reference** compact descriptor is **272,433 bytes**, larger
than `game_service/content.rs`'s **262,144-byte** descriptor limit. Actual startup
returns `game_content / game_file_size`. All references are backed by validated
originals: 4,981 use unchanged original definition collections, and 35 original
payload files cover the whole compiler-required set. Nothing is missing or
replaced with a placeholder. The parent must increase the private descriptor
bound; public RPC budgets and source-readiness checks must remain intact.
Neither this failure nor the stale environment parser is a gameplay pass.
[`evidence/artifact3-bcc0de9.json`](evidence/artifact3-bcc0de9.json) records the
actual two runs, 29 Rust and 10 orchestration checks, complete genuine mapping,
exact identities, startup errors and cleanup. No source character or required
gameplay restart was reached.

All four engine review findings are integrated in the latest candidate below;
none is a client exception or a continuing engine blocker. The simulator uses generated explicit Single
production requests and live guarded shop-row identities/quotes, and records
public presence, bank, shop, recovery and quote contexts. No failed source action
is transformed into a pass.

### Authorized native-ready source repair

Parent `faa800286cfeeed4f3bec0b924d1fb01c65b97d9` (original fix `9bff13e9260cc185d22cf6b3db5335ac35320cb4`)
was authorized and cherry-picked as `0984877`. The current source/native
conformance status has zero active executor failures: original solid-range
contact, Single/Make-X timing and owner-online/manual-drop/playtime clocks are
repaired. The exact current raw artifact is
`a200ca08c80f6a3fc812fe02ca95f52325e183084f83b12e8d470a1f37cc6d62`.
Private playtime is never seeded or reset by this client.

The runner does not use the old failure inventory or the manifest's aggregate
`runtime_ready` Boolean as an interlock. It loads the current hash-matched
artifact and asks the real server to apply the documented source-readiness
profile. The fresh isolated run still fails **earlier**, at the unchanged private
descriptor-size gate (272,433 bytes versus 262,144); the independent GameRoot
environment parser also remains nested in the actual code. Neither native
source fix is being reported as a continuing failure.

[`evidence/native-ready-0984877.json`](evidence/native-ready-0984877.json)
records this actual new-artifact run, 29 Rust/10 orchestration checks, 11 real
account checks, current identities, exact `game_file_size` error and complete
cleanup. No product character, tutorial edge or required gameplay restart passed.

### Authorized shop identity/capacity ABI

Parent `3310032cc6621f41d395103a35e81b2cfd87e1ae` then
`af75e17b98eeb94adbb9ef6d1066b3ec805a866a` were explicitly authorized and applied
as `d44cde2` then `d35f4fc`. They reclaim empty extra shop rows and bind purchases/
quotes to the optional canonical expected item without changing old `None`
bytes, fixed catalogue indices, restock phase or historical receipt hashes.
The canonical `a200ca08…6d62` artifact and prior native clock/contact repair are
preserved.

New simulator buy and quote requests always carry the displayed row's canonical
`expected_item`, through the existing generated `ShopBuy` field 4 and unchanged
action/quote envelope tags. They never supply a numeric source ID or price.
Focused client tests cover buy/quote encoding, ambiguous rows and preservation
of the original retry intent after a row is reused. Conflicts refresh the real
view read-only and stop; the client never silently retargets or downgrades to an
identity-less request. These client tests are not an executed shop/M1 journey.
[`evidence/shop-identity-d35f4fc.json`](evidence/shop-identity-d35f4fc.json)
records 35 client/protocol Rust checks, 2 historical server-intent checks,
10 orchestration checks and a fresh real-Service run with the preserved canonical
artifact. The actual run still stops at the independent private descriptor-size
gate, before any product shop trade or character creation; cleanup passed.

### Complete actor pair and final revision refresh

The complete authorized actor pair `62a003c6…` then `e1076a87…` was applied as
`0d31957` then `e844639` **before testing**, followed by source refresh
`3a74cbe5911e699fd30e51cdedf944e4616f86b5` as `2d4c6d4`. Only the director-owned
`milestones/m1-tasks.json` import conflicted; its existing worktree record was
preserved rather than altering acceptance metadata. All source/runtime changes
were applied. No partial actor candidate or parent `c58f` was used.

The current revision is `m1.source-backed.v3.0e506f3dab24bbe0`, raw artifact
`df3e2a452c100ecd94d2abc68e5cb1556f58090700474f547e7fd36de6682b3d`.
An independent decoded-JSON comparison confirmed that only `/revision` changed.
The earlier `a200ca08…` reports remain historical; the new candidate uses a
fresh isolated world and does not repin or reset any acknowledged older world.
Restart identity checks inside a run remain unchanged.

[`evidence/all-reviews-2d4c6d4.json`](evidence/all-reviews-2d4c6d4.json)
records the new candidate's 35 client/protocol tests, 2 legacy-intent tests,
10 orchestration tests, 11 real account checks and exact identities/cleanup.
The fresh real-Service attempt still fails only at the unchanged deployment
descriptor limit (272,433 > 262,144 bytes), before character creation. The actual
GameRoot environment nesting also remains. These are deployment gates, not
reports that any of the four engine repairs failed.

### Verified standalone startup and continued source actions

The authorized startup repair `2c5fe68b…` was applied as `dea24a0`. The real
production binary now parses game-only configuration independently and accepts
the complete canonical descriptor within its private 512 KiB bound. Neither
public RPC/asset limits nor source checks were relaxed.

The first post-fix run exposed an owned packaging error: extensionless payload
storage paths violate the server's existing allowlist. Payloads now use `.bin`
storage paths with the **same original bytes/hashes**, canonical asset IDs and
public URLs. A regression covers this distinction. No asset is omitted or
replaced.

[`evidence/startup-dea24a0.json`](evidence/startup-dea24a0.json) records actual
production startup and source-readiness proofs, `game.v1` Hello, real registration/
login, empty-option character creation/join and acknowledged source appearance/
experience transitions. SQL `m1-game-root-startup` was closed only after that
evidence. Full gameplay acceptance remains separate.

Moving-NPC targeting prefers actual guarded public interaction permissions over
conservative client access candidates. Definite conflicts are reconciled only
for the same mobile NPC and an unchanged sequence, with observed movement or a
typed reach denial. Re-approaches and reopened dialogue choices are bounded.
If that same previously opened speaker moves away and its guarded query itself
rejects, the client may submit a normal `CloseInterface` and re-approach; that
real input must acknowledge and actually close the stale dialogue. Every denied
action/query remains recorded. Unknown transport outcomes are not retried,
source clock/RNG/geometry is not altered, and no speaker/choice/stage is
silently substituted.

The latest full-path attempt and one independent fresh-world reproduction both
reached five acknowledged tutorial transitions and opened the actual starting
door. The following one-tile walk receives HTTP503 when **`game_commit_tick`
exceeds its 5,000 ms storage-group deadline**; the coordinator then stops and
reports an unknown outcome. Neither unknown write was automatically retried.
Resource reaping/removal succeeded, but source-service `clean:false` is recorded
separately and can never become a clean-world/full-journey pass.

[`evidence/first-door-tick-dea24a0.json`](evidence/first-door-tick-dea24a0.json)
contains both exact worlds, actor/input/operation IDs, ticks, errors, binary/
source/protocol hashes, earlier diagnostics, 38 Rust and 12 machinery checks,
and the still-unchecked original scope. The startup task is done; the full
journey is blocked on this actual next source-tick failure, not on the repaired
deployment configuration.

`milestone_accepted`, browser/UI, audio, performance and RuneLite verification
remain false. Unit fixtures below establish client machinery only:

```sh
python3 -m unittest discover -s tools/journey-tests -p 'test_*.py' -q
```
