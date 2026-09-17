# M1 intermediate checkpoint: source inputs and account foundations

**M1 remains incomplete and unaccepted.** This checkpoint does not authorize
Mining 1-99, the Club Penguin loop, any later milestone, or a release.

## Preserved work

- Execution branch: `milestone/01-starter-journey`. Scope checkpoint `7a207d1`;
  architecture/protocol checkpoint `6615c60`. `main` is untouched.
- Reference identity fixed to OSRS build 240/cache 2695. Exact source metadata,
  pinned wiki revisions and material mechanic gaps are in `research/`.
- Downloaded and independently size/SHA-256-verified the original master and
  map indexes; they are retained under `research/inputs/osrs-240/`.
- The owner's acquisition instruction is in
  `milestones/reference-acquisition-decision.json`. It directs online source
  acquisition, not approval of an unseen pack. No integrated-GPU environment
  was supplied.
- Checked the suggested gamepack archive at
  `2e92962075cfbc0df227cc783c37633d5e92f297`: it ends at build 235. Did not
  downgrade the frozen target or treat its farewell-image JAR as a renderer.
- Located and hash-verified original RuneLite 1.12.38 injected-client/API
  artifacts. Offline JDK 17 bytecode inspection found opaque build ID
  `33653951311.245`; it does not prove numeric OSRS revision compatibility.
  No reference client was executed and no source capture/compatibility claim
  is made. Retrieval/inspection is reproducible from `reference-runtime.json`.
- Real PostgreSQL account/session service implemented on isolated
  `task/m1-accounts`, commit `be4cd7282732ac4b48f351cc20ad4b4ce8f15627`.
  It creates no character and explicitly reports unavailable gameplay.
  Integration into the execution branch awaits the reviewed repair below.
- Local service/test tooling, source integrity checks, CI configuration,
  CODEOWNERS and fail-closed checkpoint validation are implemented. These
  are infrastructure, not accepted M1 content.

## Executed evidence and repair

- `cargo test -p clubscape-protocol -p clubscape-sim`: seven protocol and one
  native-client unit check passed.
- `cargo check -p clubscape-protocol --target wasm32-unknown-unknown --locked`:
  passed. This is protocol compatibility, not a WASM renderer.
- Account implementer: 15 server unit checks, formatting and Clippy passed.
- Parent isolated PostgreSQL/HTTP runner: six actual integration tests passed,
  including real process restart, expiry/revocation, concurrent uniqueness,
  session limits, malformed inputs and database transport failures.
- Independent native account lifecycle client passed against the real binary.
  The first runner attempt exposed fixture contamination: the final adversarial
  test deliberately removed a table. The runner now gives the independent
  client a fresh, separate disposable database after the adversarial suite.
  No failed product check was hidden, no player database was reset, and a
  regression covers the isolation rule.
- `python3 tools/reference_inputs.py fetch` and `verify`: both pinned source
  index files verified. Repeated fetch reused matching bytes rather than
  downloading or replacing them.
- `python3 tools/reference_runtime.py fetch` and `inspect`: two original
  artifacts verified and inspected without executing the client.
- Independent account review found one medium issue: acquired but stalled
  PostgreSQL connections could leave operations and graceful shutdown pending
  indefinitely. M1-DB-DEADLINES is actively repairing client-side deadlines and
  adding a real withheld-response regression before integration.
- Reports under `.local/evidence/` are local foundation evidence, never
  milestone acceptance. No hosted CI, browser, graphics, audio or integrated-GPU
  milestone evidence has run.

## Resume boundary

Finish M1-DB-DEADLINES, independently review that bounded repair, integrate only
passing code and rerun the combined workspace and real-server checks. Preserve
the reviewed revision and exact evidence in the next checkpoint.

The complete normal-account tutorial state/arrival branch, source capture pack,
early browser presentation benchmark, full shared gameplay/quest systems,
browser fresh-account journey, audio, target integrated-GPU measurements and
owner presentation acceptance are still outstanding. Source acquisition is
possible and has begun; the pack is not assembled or approved. Do not call
these remaining implementation tasks completed or blame all of them on
external approval.

Continue only within M1. A future approved execution must resume this milestone,
not start its successors. The canonical task/gate records retain the full
Section 30 acceptance denominator.

## Resources and accounting

Only explicitly reserved workers ran; visible peak concurrency was three
including the Director. The reference researcher and first reviewer are parked;
the existing account worker has a fresh admission for the single repair.
No factory or automatically continuing worker/schedule was started.

A session-store snapshot reported 58 GPT-6 Astra API calls, 4,293,227 input and
169,982 output tokens, and 3,255,209 ms of recorded active duration. Its `cost`
field was 58 in the store's billing-multiplier accounting, **not a dollar or
AI-credit total**. This is an intermediate session snapshot, not final or
verified project-wide/descendant accounting. No owner spending cap was imposed.

All integration containers/processes from completed runs were stopped and their
temporary databases/secrets removed. Downloaded reference artifacts remain in
ignored `.local/reference-runtime/` for reproducible follow-up. No unrelated
service, system graphics setting, external account or production world changed.
