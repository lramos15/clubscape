# Exact saved52 water continuation

**Only code and synthetic fixtures are currently admitted.** Implementation
validation never inspects, hashes, copies, restores or validates an actual
checkpoint, nor runs databases, servers, gameplay, source generation,
browser/GPU or RuneLite. It does not renew the exhausted operator allowance.
Saved-execution and acceptance flags remain false.

## Separate execution gate

`saved_water.py` does not call `resume.verify_checkpoint` or `resume.execute`.
It is a thin launcher for `saved_water_gate.py`; the runtime imports that same
canonical module so the admission/reservation dataclasses have one identity.
Synthetic actual-script tests cover this handoff, not just imported helpers.
Their old tutorial/dying/mainland/accepted-Cook guards and same-source behavior
are unchanged. There is no generic cross-worktree selector, alternate output
directory, `--skip-build`, fixture-execution switch or actual-checkpoint
`--validate-only` shortcut.

A future Director admission must be committed at the **canonical Git root** as
`milestones/evidence/m1-saved-water-execution.json`. Both its commit and SHA-256
are explicit arguments; the current canonical file must equal those committed
bytes. The separate review is pinned identically at
`milestones/evidence/m1-saved-water-continuation-review.json`.

`verify_admission` enforces this schema:

| Field | Required value |
| --- | --- |
| `schema_version`, `kind`, `status` | `1`, `m1_saved52_water_execution`, `admitted` |
| `authority`, `milestone` | `director`, `m1-starter-journey` |
| `saved_execution_admitted`, `checkpoint_access_admitted` | Explicitly true **only in the separately issued execution record** |
| `automatic_resume`, `milestone_accepted`, `later_milestone_authorized` | False |
| `canonical_git_root` | Exact absolute parent of the repository's common `.git` directory |
| `executor` | Exact `id`, absolute `root`, `worktree`, `branch`, clean committed `code_revision` |
| `start`, `target` | Exact objects from `expected_start(canonical)` and `expected_target(canonical)` |
| `bounds` | Exact `BOUNDS` object; no extra attempts or replacement paths |
| `journal`, `output` | Exact absolute executor paths for the fixed names below |
| `independent_review` | `path`, `revision`, `sha256`; passed `m1_saved_water_continuation_code_review` for the executor HEAD, a different named reviewer, and `saved_execution_admitted: false` |
| `public_inputs` | SHA-256 map with exactly the `PUBLIC_INPUTS` keys: owner approval, frontier, preparation, integration/preflight/root qualification, delivery index/inventory and public report |
| `binaries` | Exact `migrator`, `server`, `simulator` objects, each with fixed `path` and reviewed `sha256` |

The executor is `.worktrees/m1-saved-water-continuation` on
`task/m1-saved-water-continuation`. The original source root is canonical
`.worktrees/m1-headless-journey`, with checkpoint
`.local/journey-checkpoints/52f32d29b5dc4f1a`. Those names are compared lexically
before admission; no protected path is statted or resolved to validate a denial.
After admission, existing no-symlink containment and owner-only modes apply.
Generic `project_path` containment is not loosened.

The permanent journal is executor
`.local/saved-water-continuation-01/authority-v1.json`. Its parent and journal
are exclusively created with owner-only permissions and fsync. Failures retain
that reservation. Run IDs cannot be selected to renew it. Original reads,
restore, migration and each native invocation have exclusive phase records.

Reviewed executables must already exist at:

```text
.local/saved-water-target/debug/clubscape-server
.local/saved-water-target/debug/clubscape-journey-server
.local/saved-water-target/debug/clubscape-sim
```

The orchestrator never builds, downloads, packages or regenerates anything.
`clubscape-server` supplies the existing production migration CLI;
`clubscape-journey-server` supplies the existing production Service launcher
with owned SIGTERM shutdown. The simulator re-enters the same public gate and
claims its one native slot before reading its copied capsule. Its executable,
b2 manifest, control directory and outputs must match the exact executor/run.

The eventual command, **not authorized by this document**, is:

```sh
python3 tools/journey-tests/saved_water.py \
  --admission-revision <separately-committed-execution-record-revision> \
  --admission-sha256 <exact-execution-record-sha256> \
  --executor-id <explicitly-admitted-executor-id>
```

## Source and historical identity

The sole transition is
`5e0aa8a28851752ae0b8b0a08c635c6c8d2979f509f3a3e564ee74e2abfc8e6f`
to
`b2a1be20a0e6c3e1968f6f7610198ce38212f196c005539b4ac5013d10ebc650`.
Native loading explicitly selects `content/m1/legacy5e-water/manifest.json`,
verifies compressed/raw artifact hashes and checks that its JSON differs from
the public legacy source only by the water recipe and revision. Current adb,
regenerated 1bc and newer UI/audio/actor metadata are rejected. Ordinary modes
retain their canonical source selection.

The immutable public delivery is canonical
`.local/saved-water-delivery-01/game-root`, descriptor
`24fec6e152cf5e5a899248bfe302fc12f6f080b71d43f9c0052264f1d9fcbc6e`,
assets manifest
`95d7afc9d2740299c6e5091b547dbcfbc1b7b2c6f5614eca0b2251adc60abf08`.
Its committed inventory binds 40 files / 14,296,055 bytes; its four-file index
starts `a7b09591`. It is verified/copied, never regenerated. Packaging and earlier
operator evidence do not establish saved52 readiness or execution.

The reader verifies the original 58-file inventory and public archive/private
identity/capsule/service hashes. Its scenario must equal the committed public
frontier, not the executor's current default manifest. Fixed public facts include
sequence 499, observation tick 3280 / revision 3794, tile 3208,3216,p0, owned
flour-pot and bucket, completed Cook, 3000 Cooking XP tenths / level 4, two quest
points, 377 historical source checks, 70 tutorial transitions and three separate
observations. A later private quiescent tick is not invented from that public
observation. The actual typed acknowledgment/control is decoded, not guessed.

Complete restored private identity must equal the captured identity before
migration. Original tutorial, reward, Office/grave and ingredient progression
are never re-entered. Reports distinguish active b2 from original 5e
`saved_water_history` / `source_transition`. Original trace bytes remain a
prefix and completed segment records are unchanged. Fresh phase checkpoints
copy every original inventory file, inventory and availability byte-for-byte
under `lineage/`; history is not relabeled as work performed on b2.

## Proposed one-attempt phases and bounds

| Phase | Bound and required proof |
| --- | --- |
| Public gate/reservation | One committed exact admission/review; no protected I/O before exclusive reservation |
| Delivery/original read | Only named immutable delivery/checkpoint; bounded existing readers |
| Native preflight | One invocation, at most 60 seconds; zero RPCs or WorldInputs |
| Owned empty restore | One pinned-image database; exact ID/name/labels, empty public schema; one `pg_restore --single-transaction`, at most 300 seconds |
| Restored identity | Complete original private equality, no service started or other clients |
| Production migration | One `clubscape-server migrate-ui --from <exact-5e>` using b2, at most 35 seconds |
| Migration oracle | Quiescent read-only snapshots; only exact pin/revision, one audit and one replacement fence may change |
| Native remainder | One invocation, at most 600 seconds and 128 **new** WorldInputs including four replays; historical 603 submissions are not charged again |
| Service lifecycle | At most two source starts, one same-target graceful postquest restart; existing 35-second readiness and bounded owned shutdown |
| Preservation/cleanup | 960 seconds reserved inside the 2400-second overall bound; dump/integrity commands at most 300 seconds each, other private DB commands at most 90 seconds |

Database startup retains the pinned PostgreSQL image, random loopback-only port,
two CPUs, 768 MiB memory, 256 PIDs and bounded tmpfs. Operations bind the exact
owned container ID, never a supplied database URL. Missing tools do not authorize
installation or lock changes. Builds, generation, operator preflights,
browser/GPU and RuneLite invocations have zero execution allowance.

The overall clock begins before public admission verification. Setup and native
work use the original deadline minus the 960-second preservation reservation;
shutdown, capture and cleanup use the original overall deadline, never a fresh
window. Database creation, readiness, identity reads, server readiness and
shutdown inherit the remaining phase budget. Legacy helper callers retain their
previous default limits.

A separate watchdog caps the owned native process group even while the parent
is blocked handling a restart. It kills that group at the native deadline and
reports an explicit timeout; leaving the monitor on an error also stops the
owned group. A missing new capsule after forced termination is a capture failure
that retains the database, not permission to substitute historical client state.
The failing pre-repair deadline evidence and its actual-process regression are
preserved separately from the original passing code-only suites.

The oracle compares the complete persisted world/actor fields, owned items,
bank/equipment, RNG-key digest, last tick receipt, UI/audio history, account/auth/
game sessions, complete command/lifecycle journal hashes and schema migrations.
Only b2 pin/revision, world/character revision +1, lease fence +1 with ownership
released and exactly one correctly bound audit are allowed. Tick, RNG and old
receipts remain unchanged. This verifies the existing fenced transaction; it is
not a replacement migration or gameplay SQL mutation.

Only then may the service start. Hello/Login/Join plus public-player/sequence
comparison are lifecycle-only entry. Existing source contact planning and one
UseItem fill the owned bucket without a fabricated menu or wider reach. Existing
flour makes dough, then one normal range recipe preserves source success **or
burn** with matching XP. No reacquisition or forced outcome exists.

Recovery uses lifecycle operations and four replays of the **new acknowledged
water operation**, before/after transport reconnect, logout/login and same-target
restart. It never replays Cook's already-proved reward or uses the old generic
sequence-probe/bank-travel recovery path.

## Failure and preservation

Submitted restore/migration outcomes remain `unknown` until their exact
postconditions are proved. Timeout, transport error, unexpected state, bad audit,
capture failure and cleanup failure are not rollback claims or retry permission.
Only owned processes are stopped and reaped.

Both successful and failed executions attempt a fresh protected custom-format
archive, read-only before/after comparison, integrity check, original lineage,
current evidence and actual client capsule before owned cleanup. Existing
`private_checkpoint.py` writer helpers enforce private modes and durability.
Missing/invalid post-native capsules are explicit capture failures, not
successful historical fallbacks.

A pre-gameplay failure snapshot may contain only the original 5e observation
alongside the **actual current database archive** and explicit migration outcome,
labeled `original5e_capsule_only_no_new_client_capture`. Unknown migrations are
not recast as old-source or target-source success. This phase-checkpoint kind
does not satisfy the original saved52 reader or a generic resume admission.

**Capture failure forbids database/credential removal.** Partial capture, fixed
journal, run files and a blocked report remain for Director reconciliation.
Cleanup failure also retains credentials/evidence and cannot claim success.
No exit automatically resumes. Proved remainders use `continued` /
`saved_water_remainder_passed`, never full-journey or milestone acceptance.

## Code-only verification and blockers

Python fixtures use fresh `synthetic-saved-water-*` directories in this
worktree's `.local`, synthetic archives and mocked subprocess/database/service
results. Native fixtures decode synthetic capsules and exercise real public
source selection/contact planning without a service or gameplay submission.

Focused commands, from this worktree:

```sh
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=tools/journey-tests timeout 120 \
  python3 -m unittest test_saved_water test_saved_water_runtime \
  test_private_checkpoint test_resume test_run test_cook_continue \
  test_mainland_continue test_dying_observe

CARGO_BUILD_JOBS=4 CARGO_TARGET_DIR="$PWD/.local/saved-water-target" timeout 600 \
  cargo test --locked --offline --quiet -p clubscape-sim --bin clubscape-sim

CARGO_BUILD_JOBS=4 CARGO_TARGET_DIR="$PWD/.local/saved-water-target" timeout 600 \
  cargo clippy --locked --offline --quiet -p clubscape-sim \
  --features journey-server --all-targets -- -D warnings

CARGO_BUILD_JOBS=4 CARGO_TARGET_DIR="$PWD/.local/saved-water-target" timeout 600 \
  cargo build --locked --offline --quiet -p clubscape-server -p clubscape-sim \
  --features clubscape-sim/journey-server
```

Director independent review, exact reviewed binary/source pins, a named executor
and a separate public execution admission remain required. These tests perform
no actual saved read/restore/migration, sink contact, cooking, postquest recovery
or fresh real checkpoint capture. Browser, presentation, audio, performance,
RuneLite and owner acceptance are separate gates. Stop for review; do not run
the eventual execution command automatically.
