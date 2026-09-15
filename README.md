# ClubScape

**M1 is in progress, not accepted.** The approved scope is the complete
Tutorial Island-to-Lumbridge/Cook's Assistant journey in
[`prompt.md` Section 30](prompt.md). There is no playable browser client or
accepted visual/audio slice yet. Account infrastructure is not a reduced
vertical slice.

The current [native runtime checkpoint](milestones/evidence/m1-native-runtime3.json)
records the strictly compiled source content, authoritative backend and
persistence results. The [task ledger](milestones/m1-tasks.json) tracks active
review repairs and browser/journey integration. The earlier
[account-only checkpoint](docs/checkpoints/2026-09-13-m1-blocked.md) is historical.

Read [AGENTS.md](AGENTS.md), the [machine setup](docs/machines/sparky.md) and
[active milestone](spec/milestone-01.md) before working. The fixed source
identity is OSRS build 240/cache 2695; see
[reference status](spec/reference-baseline.md) and the
[capture checklist](research/reference-capture-checklist.md). Online source
availability does not waive owner reference-pack or presentation approval.
The exact [reference pack v1.3.0 is owner-approved](milestones/approvals/m1-reference-pack-v1.3.0.json);
final candidate visual/audio acceptance is still outstanding.
Its hash-bound art/interface context documents retain historical pre-approval
text. The [current browser boundary](spec/browser-implementation.md) and external
approval record govern implementation; frozen source inputs must not be edited
to update status prose.

`just reference-fetch` retrieves the small hash-pinned original source indexes;
`just reference-runtime-fetch` acquires the pinned original client/API artifacts.
`just reference-runtime-inspect` disassembles the build-ID accessor without
running that client. These are reproducible source inputs, not screenshots,
RuneLite compatibility or candidate presentation acceptance.

## Local account infrastructure

Prerequisites: Git, the pinned Rust toolchain, Python 3, Docker/Compose and
`just`. `just doctor` reports the current host; bootstrap does not install
global tools or modify unrelated services.

```sh
just bootstrap
just server
```

Bootstrap generates a mode-0600 password under ignored `.local/`, starts only
the project PostgreSQL service on `127.0.0.1:55432`, and builds/checks the account
workspace. Account migrations apply when the server starts. It does **not**
seed game progress, compile missing content or claim a WebGPU renderer exists.
Existing database data and credentials are preserved across restarts.

The server binds `127.0.0.1:4010`. It supports real account creation, password
authentication and durable expiring/revocable sessions through a generated
Protobuf API. Without `CLUBSCAPE_GAME_ROOT`, it explicitly runs account-only.
A configured game root enables strict content/artifact3 loading, a fenced
600ms authoritative world, persistent normal-source character creation,
gameplay requests, contextual views and source-safe disconnect/reconnect.
See [game configuration and wire contracts](spec/game-networking.md).
Invalid configured content does not fall back to an account-only or seeded
world. No production TLS, public deployment or real browser journey is claimed.

In another terminal:

```sh
just sim
just milestone-status
```

`just sim` creates a synthetic account against the real local service, exercises
login/logout/relogin and leaves no active session. It is **not** the full
headless gameplay simulator. See [API](spec/client-api.md) and
[security boundaries](spec/security.md).
Local synthetic-client traffic ignores environment HTTP proxies and does not
follow redirects to other services.

Stop the server with Ctrl-C and use `just db-stop` for the project database.
Neither command deletes persistent account data. Never delete its volume or
replace its credential file to hide a migration/authentication failure.

## Validation and checkpoints

| Command | Scope |
| --- | --- |
| `just lint` | Rust format/Clippy and checkpoint structure |
| `just test-fast` | Rust unit and checkpoint/tooling regression checks |
| `just test-integration` | Real HTTP/PostgreSQL checks in an isolated disposable database, plus independent account client |
| `just check-wasm` | Shared protocol WASM compilation, not a browser renderer |
| `just content-build` | Generate and strictly compile the canonical source M1 artifact |
| `just content-check` | Source/native probes, routes/oracles and deterministic repeatability |
| `just asset-check` | Original published source and audio manifest integrity, not renderer/playback acceptance |
| `just milestone-status` | Explicit pending/blocked acceptance gates |
| `just milestone-check` | Fails unless every M1 gate, matching evidence/build and owner approval is present |

Integration uses a uniquely named, CPU/memory-bounded PostgreSQL container with
a random loopback port and generated credentials, then cleans it up. It never
uses the development database or an inherited `DATABASE_URL`. Reports under
`.local/evidence/` identify their tested revision and remain distinct from
milestone evidence. CI currently covers these same infrastructure checks,
not GPU, browser, owner or RuneLite acceptance. Canonical source/native
conformance is separately recorded in `research/m1-bindings/`; actual WebAudio
component evidence is under `tools/browser-audio-tests/`. Neither component
fixture substitutes for the complete legitimate player journey.

Durable task/gate state is in [`milestones/`](milestones/); revisioned handoffs
are in [`docs/checkpoints/`](docs/checkpoints/). Work remains on isolated
branches until passing checks, independent review and owner integration.
No later milestone or release is implicitly authorized.
