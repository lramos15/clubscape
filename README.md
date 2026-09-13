# ClubScape

**M1 is in progress, not accepted.** The approved scope is the complete
Tutorial Island-to-Lumbridge/Cook's Assistant journey in
[`prompt.md` Section 30](prompt.md). There is no playable browser client or
accepted visual/audio slice yet. Account infrastructure is not a reduced
vertical slice.

The current [blocked checkpoint](docs/checkpoints/2026-09-13-m1-blocked.md)
identifies the verified infrastructure revision, exact results, remaining
source/gameplay/presentation work, owner gates and safe resume boundary.

Read [AGENTS.md](AGENTS.md), the [machine setup](docs/machines/sparky.md) and
[active milestone](spec/milestone-01.md) before working. The fixed source
identity is OSRS build 240/cache 2695; see
[reference status](spec/reference-baseline.md) and the
[capture checklist](research/reference-capture-checklist.md). Online source
availability does not waive owner reference-pack or presentation approval.

`just reference-fetch` retrieves the small hash-pinned original source indexes;
`just reference-runtime-fetch` acquires the pinned original client/API artifacts.
`just reference-runtime-inspect` disassembles the build-ID accessor without
running that client. These are reproducible source inputs, not screenshots,
RuneLite compatibility or an approved presentation pack.

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
Protobuf API. It explicitly reports that gameplay/character initialization is
unavailable. No production TLS, public deployment or browser signup is claimed.

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
| `just milestone-status` | Explicit pending/blocked acceptance gates |
| `just milestone-check` | Fails unless every M1 gate, matching evidence/build and owner approval is present |

Integration uses a uniquely named, CPU/memory-bounded PostgreSQL container with
a random loopback port and generated credentials, then cleans it up. It never
uses the development database or an inherited `DATABASE_URL`. Reports under
`.local/evidence/` identify their tested revision and remain distinct from
milestone evidence. CI currently covers these same infrastructure checks,
not GPU, browser, audio, gameplay, owner or RuneLite acceptance.

Durable task/gate state is in [`milestones/`](milestones/); revisioned handoffs
are in [`docs/checkpoints/`](docs/checkpoints/). Work remains on isolated
branches until passing checks, independent review and owner integration.
No later milestone or release is implicitly authorized.
