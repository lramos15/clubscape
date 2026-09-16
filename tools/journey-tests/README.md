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

**Current runner contract: canonical content/artifact4, state/runtime1, UIstate1.**
The current fresh candidate is revision `m1.source-backed.v4.3ff4292b311453cc`,
raw `5e0aa8a28851752ae0b8b0a08c635c6c8d2979f509f3a3e564ee74e2abfc8e6f`,
compressed `7d49e8f85b7b229bb0fc9f6f3cb9665aaf9d287f6a873ac42701e3d77cc6e69f`.
The versioned generated UI/observer contract and manifest/strict compiler are
authoritative. Existing 6f/bb99/df3 worlds are never silently upgraded or reseeded.
The historical 6f departure operation's unknown outcome is not retried or
reconstructed; this source revision requires a fresh normal candidate.

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

Packaging verifies both hashes of the manifest-selected committed archive,
decompresses those exact bytes into the isolated GameRoot, and requires the
strict inspector to report the same raw artifact hash. It never reads
`tools/m1-content/.local/compiler/m1.csc`; running the source checker alone need
not refresh that separate compiler output.

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
   diagnostics. On a source blocker, capture the private checkpoint below after
   reaping the client/server but before removing PostgreSQL. Remove the exact
   owned container, transient credentials and isolated GameRoot regardless of
   backup success. No gameplay SQL mutation is executed.

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

## Private blocker checkpoints

For future controlled source runs, the orchestrator explicitly enables the
simulator's private blocker capsule. After a handled scenario failure, the capsule
contains the actual synthetic account credentials, original operation UUIDs and
sequences, generated-Protobuf intents, last observations and recovery receipts.
It is **not** a reconstructed character or an expected-state seed. A missing or
failed capsule cannot be promoted to a recoverable checkpoint.

After the simulator and owned server are reaped, the owner verifies the exact
container ID/name/labels, database endpoint/password-file identity, the expected
single source world/actor and absence of other database clients. Read-only queries
bind the actual world/account/actor,
source artifact, complete persisted world/actor hashes, private RNG-key identity,
game/lifecycle journal hashes and selected actual receipts. A custom-format
`pg_dump` captures the entire isolated database. `pg_restore --file=/dev/null`
then reads the complete archive for integrity **without connecting to a database
or executing a restore**. Before/after private identities must match, and the
database must contain at least the last acknowledged sequence/revision/tick.
Neither receipt presence nor absence causes a retry or changes the unknown-write
policy.

The retained directory is `.local/journey-checkpoints/<run-id>/`, with owned
mode0700 directories and mode0600 files. It includes `world.pgcustom`, the original
byte/hash-verified GameRoot, private client/service/configuration data, the
original PostgreSQL password file, complete scenario report/trace, actual private
identity results and a private file-hash inventory. Only required service
configuration keys are captured, not unrelated host environment secrets.
These files contain authentication and RNG material: **never commit, attach or
publish their contents**. Command output/diagnostics are bounded and private;
public errors disclose only a phase and sanitized failure code.

The aggregate report's `private_checkpoint` and the checkpoint's
`availability.json` contain only sanitized availability/hash metadata.
An available archive is not itself a validated restore, authorized resume, or
journey pass. A full scenario success is preservable only when all required
segments and all70 source transitions passed; it records
`captured_full_journey_passed` separately from the archive's own scope.
Failed/incomplete capture is explicit and never claims a recoverable
checkpoint; partial files remain private. Cleanup still reaps/removes only the
owned services. Existing checkpoint directories are never overwritten.

There is **no automatic restore**. The separately invoked `resume.py` requires
explicit authorization, a checkpoint path and its previously observed archive
SHA-256. It verifies every private inventory entry and the current artifact,
refuses unknown input/control outcomes, and restores only into a newly created,
ownership-checked empty PostgreSQL database. The complete private world, actor,
RNG and journal identities must match before the real server starts. Normal
login/join, source-state comparisons and original-grant deduplication precede
continuation. A same-artifact binary fix does not authorize a content repin,
sequence reset, grant replay or progress reconstruction.

```sh
python3 tools/journey-tests/resume.py \
  --checkpoint .local/journey-checkpoints/<run-id> \
  --expected-archive-sha256 <previously-observed-exact-sha256> \
  --report .local/evidence/<unique-resume-report>.json
```

Supported continuation boundaries are deliberately narrow: a verified
post-onboarding tutorial prefix, or the recorded post-goblin-kill/pre-pickup
client refusal. Other boundaries, including the later dying-state query failure,
are refused before starting services. They need a source-safe continuation
adapter after the corresponding backend fix. Historical trace bytes and source
checkpoints remain an exact prefix, not recreated game events; the same account,
actor and world continue. Required restart evidence records its original
invocation rather than pretending the restored database is the old container.
The previously removed 6f/5e tmpfs worlds remain unavailable; this mechanism
cannot recreate them. Only actual preserved database archives are restorable.

[`evidence/private-checkpoint-machinery-c0f3961.json`](evidence/private-checkpoint-machinery-c0f3961.json)
records the parked implementation and its machinery checks. It does not claim
that a real private database archive already exists.

The later authorized shop continuation exercised real private capture and two
explicit restores. Its latest private checkpoint is the genuine dying-state
frontier, not a completed Death's Office/Cook journey; see the current evidence
entry below.

## Explicit dying observation only

`--observe-dying` is a separate, non-gameplay mode of `resume.py`, pinned to
Director authorization `bd33c742bad7be4ab244e1402cc840d89d3e427b`. It is not
permission to continue the full journey or to use main's newer content.

```sh
python3 tools/journey-tests/resume.py --observe-dying \
  --checkpoint .local/journey-checkpoints/7fa370e14c9b421e \
  --expected-archive-sha256 d2e586a51f78ed36ab30443f0297b1273e984957ae5b8f78c73b86be53ca16e1 \
  --report .local/evidence/<unique-observation-report>.json
```

The code must be committed first. This authority reserves only one actual
invocation, enforced by an exclusive local authorization record. The existing
empty-owned-database restore and complete pre-start private identity comparison
are unchanged. Observation builds use a private workspace/lockfile mirror so
the tracked root `Cargo.lock` is never updated.

Native preflight decodes the original control through generated Protobuf. A
failed control is admitted only if it is the specifically identified read-only
`PollWorld`409; failed/unknown mutating controls remain refused. The actual7fa
capsule retained a successful `JoinWorld` control, not the failed poll's wire
bytes: that fact is recorded honestly, while its original public poll failure
is retained unchanged. No fabricated replacement control is written.

The client may issue only Hello, Login, CurrentAccount, JoinWorld and read-only
PollWorld. Every WorldInput path, including duplicate probes and UI input, is
blocked. HP0 is valid for the source-death observation; ordinary gameplay safety
checks are unchanged. The historical public tick1613/HP1 remains distinct from
private saved tick1616/HP0, and no healthy-state substitute is used on a failed
observation. The original acknowledged gameplay request is carried, never replayed.

Stop after the first successful public poll, at most180seconds after server
readiness. Report the actual exposed HP, UI, style, action, instance and death
facts; the protocol has no standalone life-phase enum, so private phase checks
must not be relabeled as public observations. Normal lifecycle/style repair and
source tick/death progression after startup are reported separately from exact
pre-start identity.

A successful `observed` stop is not a full-journey pass and is checkpointed just
like a failure. Fresh private capture must succeed before this bounded task can
be complete. All services are cleaned after capture; no Office dialogue, grave
claim, Cook action, walking or further gameplay is authorized by this mode.

The single authorized invocation is complete:
[`evidence/dying-restore-observe-c84c66094aaa4526.json`](evidence/dying-restore-observe-c84c66094aaa4526.json).
Committed adapter `c3e7c52` restored the exact private state before startup and
issued only Hello/Login/CurrentAccount/JoinWorld/PollWorld. All five returned200;
there were **zero WorldInputs** and no gameplay receipt changes. The first public
join at1621 and poll at1622 already showed HP10, unarmed style and the source
Office location/instance/active-death ID. Private inspection separately confirms
natural progression from saved1616/HP0/Dying to1622/FirstDeathOffice; no public
Dying or Respawning frame is invented.

The observation stopped in3.989seconds after readiness. A fresh mode0600
checkpoint was captured at `.local/journey-checkpoints/c84c66094aaa4526`,
archive SHA `a7cd1774144525c29efc694b5550f71a0e04052324bb59c3ab4fbccbe2d8b4af`,
private-inventory SHA
`996a199f2d47d83add36ee9de22802774cb9aaecd06e9fd0c0ea2d1a0e394f6f`.
The original7fa archive and trace prefix remain exact; all owned services are
cleaned. The authorization has been consumed. Further Office/grave/Cook or
post-quest gameplay requires a separate director review and admission.

## Authorized remaining mainland gameplay

Director authorization `db74895cd5d9f109d292eea20f07f5c2a57e3343` permits a
separate `--continue-mainland` invocation, not reuse of the consumed observation
authorization:

```sh
python3 tools/journey-tests/resume.py --continue-mainland \
  --checkpoint .local/journey-checkpoints/c84c66094aaa4526 \
  --expected-archive-sha256 a7cd1774144525c29efc694b5550f71a0e04052324bb59c3ab4fbccbe2d8b4af \
  --report .local/evidence/<unique-mainland-report>.json
```

Only this exact accepted observed-success checkpoint is admitted. The reader
preserves its `observed` status and original capsule/trace, verifies the
successful typed PollWorld control and exact pre-start private identity, and
keeps321 historical source checks separate from the three observation checks.
Adapter code must be committed before the one actual restore. The existing
5400-second and input bounds remain in effect.

Continuation starts at the existing source Office/death record. The recovery
conservation baseline is the actual pre-death public frame retained in the
historical trace, not a recreated inventory or a new death. Source topics,
portal, grave opening and selected opaque recovery IDs use ordinary public
operations. No ordinary-grave Bank-All, extra kill, retention replay, forced
arrival or guessed motion is introduced. Cook acquisition/rewards and the
existing post-quest recovery/replay plan follow only if these source operations
actually pass.

Full success now captures the actual private client/world checkpoint too,
including after legitimate logout. All source segments must be passed before
that status is eligible; a missing or failed full-success capture makes the
overall result blocked rather than deleting the only completed frontier and
claiming completion. Source blockers are preserved with the same owned cleanup
and privacy rules. Any next restore or later milestone requires new authority.

The single authorized continuation executed from committed adapter `5856343`.
[Run907d84b817974f55](evidence/office-grave-pot-907d84b817974f55.json) completed the
existing Office introduction, all three topics, portal, actual grave opening and
recovery of17 opaque entries at zero source fee. The original20 owned item kinds,
24 skill XP values, quest states and sword equipment matched their real
pre-death conservation baseline. This account now has336 source checks and the
same70 tutorial transitions; its three observation checks remain separate.

Cook's Assistant acceptance acknowledged at sequence355/tick1886 after legitimate
banking left the inventory empty. The first ingredient-container acquisition then
blocked **before a pickup input**: source pot `spawn.pot.3209.3214.p0` is on
unwalkable cell `(3209,3214,0)` with movement mask255, while the engine's
`ground_access` requires the actor to occupy the item's exact tile. No declared
door/transform collision override unblocks that cell. The runner did not
no-clip, relocate the pot, bypass pickup reach or substitute a banked pot.
The source/backend owner must supply legitimate access for this source location;
Cook ingredients/rewards and post-quest recovery remain unexecuted.

Protected checkpoint `.local/journey-checkpoints/907d84b817974f55` is retained:
archive SHA `4f3cbf8148ef6d9334da8cbffeb4a405924018ba603cdf4ac9aebc997b53fa0d`,
private-inventory SHA
`b3c7ee44921e67a182d3a017d818d5953beacfcabef5213906b0f08a7b28da40`.
All original checkpoint/trace hashes remain exact, all owned services are
cleaned, and this one-restore admission is consumed. The full headless journey
is still blocked, not accepted or automatically retried.

## Director accepted-Cook continuation

The unused `a913bb7` allowance now has a Director-only assignment recorded at
`2250455`; it retains the **same one-attempt key**, not another restore budget.
The exact source-ground repair is adopted as `9cab1e9` without changing source5e
or any protected checkpoint. The narrow `--continue-cook` reader verifies the
original907 archive/private inventory/identity, successful typed JoinWorld and
acknowledged sequence355 `DialogueChoice` for Cook acceptance. All eight completed
segments must remain passed and the three remaining Cook/postquest segments
unchecked. The retained active-death pointer is not permission to replay the
already completed Office/grave flow.

The ordinary runner excludes nonmutating polls from its private control journal.
Its actual last recorded control is the successful empty JoinWorld request,
not a poll. The first Director preflight correctly stopped before reserving an
attempt or restoring a database when the initial reader expected a poll; that
setup failure is retained. The corrected reader accepts only the actual typed
successful join, with no failed-control exception or expanded restore budget.

```sh
python3 tools/journey-tests/resume.py --continue-cook \
  --checkpoint .local/journey-checkpoints/907d84b817974f55 \
  --expected-archive-sha256 4f3cbf8148ef6d9334da8cbffeb4a405924018ba603cdf4ac9aebc997b53fa0d \
  --report .local/evidence/m1-cook-a913-director.json
```

This command is admitted only while the a913 attempt record is absent.
The committed adapter performs a zero-network native preflight before reserving
that key, restoring one fresh owned database and checking exact private identity.
It continues the existing ingredient/partial-delivery/reward/range plan without
repeating bank emptying, acceptance, tutorial, combat or recovery. Postquest
logout/reconnect/restart/known-reward replay and protected success-or-blocker
capture remain mandatory. The private build target stays worktree-local.
No actual continuation or milestone acceptance is implied by reader validation.

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

The authorized profile `5104c464…` and exact-state collision-cache repair
`eda966a9…` are integrated locally as `cfa2b5f` and `6809ace`. Historical
first-door deadline reports above are retained, not automatically applied to
the repaired candidate. A fresh real source crossing is required to close that
specific blocker. No source content, actor permissions, collision masks, RNG,
600 ms cadence or five-second deadline is changed by the simulator.

[`evidence/door-cache-6809ace.json`](evidence/door-cache-6809ace.json) records the
actual repaired production path: source door Open sequence29, crossing
sequence30, and the real `starting_exit -> survival_greeting` transition at
tick94, with continued world processing through tick176. SQL
`m1-dynamic-collision-performance` is done for this deadline blocker only.
The later moving-NPC dialogue stop is separate and does not invalidate the
measured crossing or certify the rest of M1.

[`evidence/fishing-access-6809ace.json`](evidence/fishing-access-6809ace.json)
records the subsequent fresh source candidate: eight acknowledged tutorial
transitions, the actual one-net grant and inventory lesson, and a **new**
`Net` reach denial for the original stationary fishing NPC3317 at3099,3090.
The client now intersects shared definition access lists with the selected
spawn's footprint/reach and checks alternative declared faces. Four real
in-reach source land tiles, including cardinal west/south, receive evaluated
`OUT_OF_REACH`; no catch/XP is fabricated. The remaining candidate tile had no
route in the client's canonical map and was not teleported to.

Source/code evidence points to `touch_edge` requiring movement through the
water NPC's blocked target cell while NPC `solid_footprint` remains false.
The engine owner must address declared non-walking-resource contact without
clearing water/wall clipping or moving the source fishing spot. The cache
repair remains verified; this is not the historical tick deadline.

### Actual stationary catch and onboarding recovery

The authorized declared-contact repair `2828643a…` is integrated as `dcec4a5`,
without changing the pinned `df3e…82b3d` artifact, source water, NPC placement,
odds or cadence. The real normal-account run admitted Net from cardinal west,
observed Gathering with no initial reward, then actually caught one raw shrimp
and earned 100 Fishing XP-tenths before transitioning to `skills_open`.
SQL `m1-nonwalking-contact` is done for this live result, not only admission.

Both independent current candidates also **passed onboarding recovery**:
HTTP transport rejoin, logout/token revocation/login, replacement of the owned
server against the same isolated PostgreSQL data and exact artifact, original
net-grant operation replay without new value, and stale/future sequence
rejection. [`evidence/nonwalking-catch-dcec4a5.json`](evidence/nonwalking-catch-dcec4a5.json)
records actual items, XP, ticks, sequences, PIDs and identities.

The new blocker occurs after the Skills tab advances to `survival_tools`.
Approaching the Survival Expert makes actual `PollWorld` return HTTP500
`game_source_view` (`InvalidContent`). The canonical primary
`transition.tutorial.survival_tools` entry overlaps `finish.net` or `replace.net`
after the net lesson; the engine correctly refuses multiple matching entries.
No first-entry fallback, inventory/flag workaround or stage skip is used.
[`evidence/survival-tools-entry-dcec4a5.json`](evidence/survival-tools-entry-dcec4a5.json)
contains the two normal-account reproductions, exact failed poll IDs and source
guard/code evidence. Each reached ten source transitions and one verified
onboarding restart; later gameplay and post-quest recovery remain unpassed.

### Actual exclusive tools lesson and longer event history

Authorized generator `2f775f79…` and validated product `435e27c5…` are integrated
as `4677324`/`f2b1870`. A **fresh** normal candidate uses revision
`m1.source-backed.v3.41555bd46c074d62` and raw artifact
`bb99ad96add9669fd49c4162832fbd8e9cb151997a4e4c4682b33b1f1882e2e1`;
no existing df3 world was repinned.

[`evidence/exclusive-tools-f2b1870.json`](evidence/exclusive-tools-f2b1870.json)
records the actual `woodcutting_firemaking_intro` choice at sequence35/tick162:
exactly one axe and tinderbox, net/shrimp retained, stage `cut_logs`. SQL
`m1-tutorial-dialogue-entries` is done separately. That account then performed
real log cutting/fire/cooking, Chef production and Quest Guide/journal steps,
reaching24 acknowledged transitions before a conservative client event-gap stop.

Long-running input replies use the server's original join event floor.
The runner reconciles a successful input's stale-floor gap by proving exact
last-observed-event overlap in the server's chronological event suffix, or by
one real read-only poll from its last observed revision requiring explicit
no-gap continuity and matching acknowledgment sequence. The original wire flag
and the chosen proof are recorded; no missing event is generated. It never
resends that input, ignores a genuine gap, or counts an older stopped run as a
complete journey. The overlap path also avoids introducing an unnecessary poll
between a moving NPC's acknowledged dialogue open and the player's real choice.

[`evidence/anvil-open-f2b1870.json`](evidence/anvil-open-f2b1870.json) records the
latest actual candidate: **31 source transitions**, 162 source checks, real
onboarding recovery and uninterrupted evidence through Chef production, tin/
copper mining, bronze smelting and the hammer grant. Thirty-one input suffix
overlap proofs preserved event continuity without replaying a successful input.

The new blocker is `anvil_open`: source anvil2097 `Smith` is acknowledged at
sequence102/tick500 but emits only “Select a permitted source recipe.” and
`interacted`. No contextual `interface.smithing` opening is presented, so its
source `interface_opened` transition does not fire through tick540. That
interface is contextual, and generic `OpenInterface` cannot authorize it.
The required actual production-menu boundary belongs to the upcoming integration,
not an injected event, direct dagger-production bypass, or a fabricated stage.
All old tools-dialogue/water/collision failures remain historical; none is
reported as the current blocker.

### Canonical4 real contextual production and departure boundary

The authorized shared `d153…`, consumables `82e0a41…`, and ordered UI/backend
sequence through `78fcec42226e17d715f422f8e9cc2be8a1fb8762` are integrated.
Generated conflicts were resolved to that **final worker content4**, not old
artifacts. Milestone records and the root lockfile were preserved unchanged;
only owned runner, tests and evidence are changed by the follow-up implementation.

The runner requires `game.ui.v1`, `game.observer.v1` and `snapshot.ui.version=1`,
and records complete UI, action and dynamic-object observations. World facility
menus are opened by the actual source action; production uses the displayed
menu ID, exact recipe and explicit mode. Inventory dough is opened by actual
flour-pot/water-bucket `UseItem`; its target stays null. Source rewards and
documents use their actual typed continuation IDs. UI bank withdrawals echo the
actual decimal `ui.bank.revision` and current entry ID, without rewriting retry
intents. Ordinary deposit and other retained source APIs keep their real
context/ownership checks; no generic interface command creates bank access.

For the early shrimp lesson, generic fire `Cook` currently requests a cooking
menu before that interface unlocks; its denial is retained as a separate
diagnostic, not called successful. The actual raw-shrimp-on-fire inventory
control executes the source recipe directly, with an exact observed
recipe/temporary-target identity. This is not a direct recipe/stage setter.
Anvil production does require the actual menu and selection.

[`evidence/ui4-anvil-1e9c7be.json`](evidence/ui4-anvil-1e9c7be.json)
records source `Smith` sequence105/tick498 opening interface312/target-bound
menu `ui.4` and advancing `anvil_open -> smith_dagger` **without** a dagger or
XP grant. Actual typed selection sequence106 then consumes the bronze bar,
produces one dagger and awards125XP-tenths at tick505.

[`evidence/ui4-departure-1e9c7be.json`](evidence/ui4-departure-1e9c7be.json)
records **66 acknowledged source transitions**,263 source checks, real melee/
ranged kills, the source bank's25coins, prayer/magic and Learning the Ropes1QP
with its actual reward continuation, plus onboarding recovery on the same
isolated database/artifact. It also records the new exact source blocker:
`offer_mainland` sequence181 receives HTTP500 `game_source_view`; the source
`departure_confirmation` stage has three simultaneously eligible **primary**
Magic Instructor entries. No first-entry fallback, forced Home Teleport,
unknown-write replay or completed-departure claim is made.

### Timed UI travel interruption integration

Authorized `0575fe0051b649adbd19b194b8521f328a54aad2` is applied as `9291564`
at a safe boundary with no active owned candidate. The complete five-file
repair includes only the existing flate2 test dependency edge, not new package
versions. Timed UI dispatch now invokes the existing source
`interrupt_travel(AnotherAction)` before the action; read-only UI keeps its
noninterrupting behavior and refusals remain transactional.

[`evidence/ui-travel-9291564.json`](evidence/ui-travel-9291564.json)
records five targeted travel regressions, 48 client/protocol tests, strict
Clippy/formatting and the unchanged `6fdb…c1b3` artifact. No new service, world
repin or unknown-operation retry was needed. The independent Magic Instructor
confirmation-entry blocker and prior actual66-edge evidence remain intact;
these component passes are not a completed journey or new live Home Teleport
acceptance.

### Actual 5e departure, mainland and shop-entry blocker

Authorized generator `bacbdf1` and product `8778fb3` are integrated as
`c41c1ee` and `1c7a449`; the timed-UI travel repair remains present.
The packager verified the committed archive and actual raw `5e0a...c8e6f`
bytes, not a stale compiler-output filename. The old 6f account and uncertain
sequence181 were neither retried nor reconstructed.

[`evidence/departure-mainland-shop-1c7a449.json`](evidence/departure-mainland-shop-1c7a449.json)
records fresh normal run `19cea04e47564728`: **70/70 acknowledged tutorial
transitions and 285 source checks**. The real confirmation menu contains the
original three choices. `offer_mainland` acknowledges at tick966; the actual
normal confirmation at992 and Home Teleport at993 lead to Lumbridge at1017.
The source18-kind kit, earned XP and1QP match their source checkpoints.
Onboarding reconnect/logout/login/same-database-and-artifact restart/replays
also pass in this same account. Mainland copper adds one ore and175XP-tenths
using the legitimately acquired bronze pickaxe. Inventory movement, the real
castle/bank route, copper deposit/typed withdrawal, and10-coin withdrawal pass;
the bank retains15coins and one copper ore.

The next blocker is **not** departure or shop-row identity. At tick1244, with
next sequence245 and a mainland actor adjacent to `spawn.shopkeeper`, the real
evaluated target view allows `Talk-to` but denies `Trade` with
`REQUIREMENT_NOT_MET`. No Trade mutation is submitted. Both source keepers use
`OpenShop(interface.shop)`; its query/execution requires an already unlocked
contextual interface, but the actual progressed actor lacks `interface.shop`
and the canonical pack has no effect unlocking it. Both entries have empty
`before_open`, and that hook runs after the locked-interface check. The
information-only shop dialogue supplies no alternative trade choice.

The source/backend owner must provide the legitimate contextual shop-entry
unlock/admission bridge. The runner does not invent an unlock, use generic
`OpenInterface`, bypass context with a direct buy, or skip the shop. The combined
inventory/equipment/bank/shop segment therefore remains unchecked despite its
passed inventory/bank substeps. Goblin/death/Cook/post-quest recovery remain
unexecuted. Source-service shutdown and all owned resource cleanup passed;
this is qualified journey progress, not a full journey or milestone pass.

### Actual shop, goblin and dying-state frontier

[`evidence/shop-goblin-dying-8ab279a.json`](evidence/shop-goblin-dying-8ab279a.json)
records the complete same-account provenance, source-oracle checks, exact
operations, public trace hashes and sanitized private-checkpoint availability.

The authorized binary-only shop repair `9d67cef` is applied as `8ab279a`; the
artifact remains raw `5e0a...c8e6f`. A fresh normal account's first run
`98fe26d99054484d` hit sixteen bounded Brother Brace reach refusals. It produced
the first real private archive. Explicit restoration in `c8cea29df6fc42a5`
preserved that exact world/account/RNG/journal state and the chronological trace.
Bounded alternate-contact walks, rather than repeated selection from the same
failed face, completed the remaining tutorial. No source odds, clipping or ticks
were changed.

Actual `Trade` sequence280/tick1251 opened the source shop without a permanent
`interface.shop` unlock. A canonical `item.bucket` quote and buy cost2coins;
the real sale returned0coins. Copper, inventory/bank/shop and a credited
5HP goblin kill with200Attack-XP-tenths passed. The runner then exposed its own
pickup-ordering error: it checked `can_take` before walking to the drop. The fix
walks normally and rechecks the same ground ID, stack, position and permission;
it never retargets or grants loot.

The second explicit restoration, `7fa370e14c9b421e`, took the original
`ground.engine.8` bones without repeating the kill. This same account now has
**70/70 tutorial transitions and321 source checks**, with seven complete
segments passed. Actual goblin retaliation then reduced HP to zero. The next
`PollWorld` returned HTTP409/error
`688aab5a-a58b-4a57-bb48-ec9dcdd1391a` instead of an observable dying-state view.
Last public state: tick1613/revision1924/next302, HP1. Sanitized inspection of the
actual preserved server data confirms tick1616/revision1928, HP0, `life=dying`,
an active death record, and incomplete arrival; it is not a successful public
Office/grave view.

The current private archive is
`.local/journey-checkpoints/7fa370e14c9b421e/world.pgcustom`,
SHA-256 `d2e586a51f78ed36ab30443f0297b1273e984957ae5b8f78c73b86be53ca16e1`.
It and its private identity/configuration companions must not be published.
All owned services are cleaned. The remaining blocker is authoritative
world/UI query observability during legitimate dying/respawn progression;
Office/grave/Cook/post-quest recovery remain unchecked.

The subsequent bounded
[read-only dying diagnostic](evidence/dying-ui-style-7fa370e14c9b421e.json)
isolates the exact saved-state failure: no equipment remains, but the selected
style is still `style.sword.bronze.stab.accurate` and the sword is in the grave.
Immutable `ui_view` calls `equipped_weapon` for that stale style and propagates
`RequirementNotMet`; presence, actor-observer, context and ground-item queries
succeed. No world input, tick, restore, healing or style/equipment change was
performed. This diagnosis adds no journey progress and does not authorize
continuation of the preserved dying checkpoint.

The authorized four-file repair `634cff9` is integrated as `8897211`:
equipment retention refreshes derived style, and trusted lifecycle reconciliation
handles preexisting dying/respawning/first-Office records. It does not mutate a
query or the frozen checkpoint. The
[integration record](evidence/dying-repair-8897211.json) separates four native
regressions and client compatibility checks from an actual resumed journey.
The checkpoint, exact pre-start identity comparison and narrow resume boundaries
remain unchanged. Wait for the director's PostgreSQL confirmation and explicit
source-safe dying-resume adapter authorization before starting a service or
continuing gameplay.

The Director has now confirmed the actual-state diagnosis and PostgreSQL
prerequisites. The current
[bounded authorization](../../milestones/evidence/m1-dying-observation-authorization.json)
admits a tested, committed observation-only adapter and one real restored
observation with normal lifecycle/ticks. Preserve the original archive and
trace, stop before player gameplay inputs, and capture a fresh private checkpoint
before cleanup. Full Office/grave/Cook continuation remains separately gated.

```sh
python3 -m unittest discover -s tools/journey-tests -p 'test_*.py' -q
```
