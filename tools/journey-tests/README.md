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

Without `--game-root`, the script constructs an isolated descriptor from the
committed product artifact and **only hash-matched original published outputs**.
It records any unmapped asset IDs rather than generating substitutes. This
limited assembly is not a replacement for the complete product adapter.
Startup still runs the real server's artifact-version, unresolved-binding,
asset-membership and engine-construction checks, without relaxing any of them.

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

At base `89f3386`, the committed product pack is artifact/schema 2, while the
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

`milestone_accepted`, browser/UI, audio, performance and RuneLite verification
remain false. Unit fixtures below establish client machinery only:

```sh
python3 -m unittest discover -s tools/journey-tests -p 'test_*.py' -q
```
