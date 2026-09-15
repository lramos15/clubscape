# Protocol simulator

`clubscape-sim` uses the generated account/game Protobuf protocol and ordinary
loopback HTTP RPCs. It never opens PostgreSQL, supplies RNG, sets character
state, grants items, or submits tutorial-result events.

The current journey requires canonical content/artifact4 with `game.ui.v1`/
`game.observer.v1` and a complete `WorldSnapshot.ui` version1 projection. State
and runtime stay version1. It always uses the manifest's actual current hash in
a fresh owned candidate; it does not upgrade or repin historical test worlds.

## Commands

Existing account-only checks remain independent:

```sh
cargo run --quiet -p clubscape-sim -- account-lifecycle \
  --url http://127.0.0.1:4010
```

The named product scenario is:

```sh
cargo run --quiet -p clubscape-sim -- scenario m1_fresh_account \
  --url http://127.0.0.1:4010 \
  --report .local/evidence/m1-fresh-account.json \
  --expected-server-build <exact-server-hello-build-revision> \
  --recovery-control-dir .local/owned-journey-control
```

Run from the repository/worktree root. The scenario's source root defaults to
the current directory. Evidence/control paths must be relative project paths
without symlinks or traversal. Credentials are generated in memory and never
included in public reports or traces. The controlled orchestrator explicitly
enables the private blocker capsule described below; without that option they
remain in memory. Origins are loopback-only; proxies and redirects are disabled.

**Prefer the [controlled orchestrator](../journey-tests/README.md).** A manually
invoked simulator cannot restart an arbitrary server. Without its owned
orchestrator handshake, the required restart checkpoint fails explicitly.
The normal account service defaults are not changed.

### Private blocker capsule

The owner may pass `--private-checkpoint-file` only with its recovery-control
directory. The output must be
`.local/journey-runs/<16-hex-run-id>/control/private-client-checkpoint.json`.
On a handled blocker after an actual source character has been observed, the
simulator writes this new file with mode0600 under an existing mode0700 directory.
Existing files, symlinks, public output paths and oversized capsules are refused.
Only its hash, size and capture/failure status enter the public report.

The **private** capsule holds the synthetic login/password/token, the last
observed actor/source state, original onboarding/reward receipts, and exact
generated-Protobuf bytes for the latest attempted world input or sequence probe.
It also retains the original request UUID and authorization for account/lifecycle
writes, including an uncertain logout or rejoin. Being logged out does not
discard an otherwise observed source character's recovery information.
Receiving an error or an acknowledgment is recorded without authorizing a retry.

This output is not a database backup, a source-state setter or a resume command.
The owner must pair it with the real PostgreSQL archive and matched private
identities before cleanup. All future recovery requires explicit authorization
and actual receipt/state reconciliation; an original operation's UUID, sequence
and intent must not be replaced. See the
[private checkpoint contract](../journey-tests/README.md#private-blocker-checkpoints).

## What the plan executes

* Real registration, login, empty-option `CreateCharacter`, and `JoinWorld`.
  Appearance confirmation and the `experience.brand_new` source choice are
  separate sequenced gameplay inputs, not creation-time state seeding.
* All 71 source tutorial states along the 70-edge successful normal-account
  path. The source graph has 73 edges; its two departure backouts and interrupted
  teleport cycle are not falsely reported as covered. Unknown stages or an
  unexpected transition fail immediately.
* Actual instructor dialogue choices, locked/unlocked interfaces, source doors,
  ladders, gathering, firemaking, cooking (success **or burn**), smelting,
  smithing, equipment, melee/ranged kills, prayer activation, Wind Strike, and
  completed Home Teleport. The brand-new arrival is the source-bound Jon branch,
  not a forced castle spawn.
* Source Lumbridge copper with the legitimately acquired bronze pickaxe,
  inventory slot movement, equipment, bank deposit/withdrawal, finite shop
  purchase/sale, credited goblin combat and real loot pickup.
* An item-losing death from actual goblin retaliation. Normal settings disable
  auto-retaliation and supply piles; no HP setter exists. Death's actual private
  Office, introduction, all three topics, portal, grave and authorized recovery
  IDs/fees must work. A missing recovery view/request is a blocker.
* Actual kitchen pot, cellar bucket, dairy cow, west-coop egg, wheat and the
  three-floor mill (hopper, controls, bin). The selected milk → egg → flour
  partial-delivery order and the final Cook reward are checked separately.
  Other five delivery permutations are not execution evidence from this account.
  Starter supplies are banked first, so they cannot substitute for acquisition.
* A real post-reward Cook-o-matic recipe using another legitimate mill/sink
  acquisition. Unsupported source bucket/sink use fails, rather than inferring
  range permission from a completed-quest label.
* During onboarding and after the quest: quiescent checkpoint, HTTP connection
  replacement/rejoin, logout/token revocation/login, owned-server restart,
  original-operation replay and invalid-sequence rejection. Acknowledged source
  state must survive without replaying grants or rewards.

## Authority and oracles

[`tests/scenarios/m1_fresh_account.json`](../../tests/scenarios/m1_fresh_account.json)
contains independently authored expectations and references the frozen
`research/journey-rules` contracts. Numeric XP, grants, partial quest states and
reward assertions are not generated by the game implementation. Source
coordinates retain their original **inference** classifications.

Canonical content is read only to select legitimate targets/recipes and plan
routes through source collision. The client submits bounded walk waypoints and
actually opens source doors. It never applies its hypothetical open-door map to
the server, trusts a missing cell, or sends a teleport to solve missing geometry.
The server still evaluates clipping, permissions, state, outcomes and time.

Ordinary actions use generated Rust messages. Additive `open_grave`/
`open_death_office` inputs are resolved only through the **actual generated
Protobuf descriptor**. There are no guessed wire tags. An absent generated
request is an error; generic `OpenInterface` is not substituted for recovery.

With the integrated artifact4 protocol, source facility interactions open the
actual target-bound `UiProduction`, then generated `GameplayUiRequest.production`
selects its exact menu/recipe and explicit `Single` mode. Inventory-only dough
uses real item-on-item input and preserves the absent world target. The normal
tinderbox/logs and raw-shrimp/temporary-fire controls execute their exact source
item-use semantics; when no menu is opened, the executed recipe/target observer
must match. No generic `Produce` shortcut is used to skip anvil UI progression.
Commerce selects the bucket row from the live authorized `ShopView`, obtains a
real read-only quote, verifies source item/quantity/price, and submits the public
purchase/sale. Quotes must preserve progression and input sequence. Public
presence and bank/shop/recovery/quote contexts are retained in the trace.
Actual reward/document continuations are processed by their typed IDs, never by
auto-accepting an unknown confirmation. Typed bank withdrawals use the current
entry identity and `GameplayUiRequest.expected_bank_revision` copied from the
actual decimal `ui.bank.revision`; duplicates retain that original intent.

Every **new buy and buy-quote** carries `ShopBuy.expected_item` from the
displayed row's canonical `item` string. It is never the numeric cache/source ID
or a client price. The exact same selected row/identity payload is quoted and
submitted. Missing, duplicate, ambiguous or out-of-stock selections fail
explicitly. An original retry receipt retains its original operation ID,
sequence, row and optional identity; a refreshed view cannot rewrite it.

On a definite buy conflict or rejected buy quote, the scenario refreshes the
authoritative shop view read-only, records the original expected identity and
current view, and stops for a new explicit selection. It does not silently
choose a replacement item, remove the identity field, or resubmit an uncertain
operation under another ID. Historical `None` wire/intent compatibility belongs
to the unchanged server/protocol journal contract, not a new-client fallback.

Polls wait 600 ms. Gathering/combat/ignition waits and retries are bounded and
observed; success probabilities are never modified or statistically certified by
one run. Writes with uncertain transport outcomes are not retried under new
operation IDs. Explicit duplicate probes reuse the original operation ID,
sequence and action; the current authenticated world session may change.

Input responses can report a gap against the server's old join-event cursor
even while this client continuously polls. Such an already successful input is
**not replayed**. The server returns a chronological suffix, discarding only
the oldest events. If the exact last locally observed event (identity **and**
payload) occurs once in that suffix, it proves coverage of later events. The
runner preserves the wire-gap flag and records this overlap proof.

Without that proof, it requests a read-only `PollWorld` from the last actually
observed revision. It continues only when the response explicitly has no gap,
its history floor covers that cursor, its revision/tick cover the acknowledgment,
and its next sequence agrees. Real history loss remains an error. Poll gaps are
never silently ignored or replaced with inferred events.

Moving NPC approaches prefer the actual evaluated interaction menu over the
client's conservative geometry candidates. Approach replanning has a 12-attempt
limit and a 240-source-tick guard between bounded walk plans; rejected interactions
have a 32-attempt bound with a 600-source-tick guard, and reopened
dialogue choices a sixteen-attempt bound with a 600-source-tick pursuit guard.
Retargeting stays on the same source NPC
and choice. A definite conflict is reconciled through actual snapshots and
unchanged sequence; an invalidated open dialogue may be closed only with a real
acknowledged `CloseInterface`. All denials and target observations are retained.
Unknown HTTP503/write outcomes are not treated as movement races.
If a mobile NPC moves away and back between observations, the exact same action
may be re-approached within the existing bound when both actual public views
permit it and the rejected command did not consume a sequence. The trace labels
that as current permission evidence, not as an observed position change.

Reports include exact source/artifact/protocol hashes, the server's build
identifier, input sequence/operation IDs, source expected-vs-actual checkpoints,
inventory/equipment/XP/quest/stage/position and authorized bank observations.
Private ledgers/counters are not exposed or invented. Regenerating vitals and
source UI-session closure are distinguished from persisted progression.

## Result semantics

The JSON report and adjacent `.trace.jsonl` are persistent and flushed at each
checkpoint. `segments` distinguishes passed work from unchecked work.
`full_journey_passed` becomes true only after **every** required segment and both
owned restarts. Any blocked action exits nonzero with stage, sequence, tick,
reason and last authoritative state.

These are authoritative-journey checks, **not UI, presentation, audio,
performance, RuneLite, source-capture approval, or M1 self-acceptance**.
`milestone_accepted` always remains false. Synthetic client machinery tests and
the real account-only negative scenario never count as product gameplay.

## Focused validation

Read `docs/machines/sparky.md` and verify the host first.

```sh
mkdir -p .local/build-work .local/journey-target
export TMPDIR="$PWD/.local/build-work"
export CARGO_TARGET_DIR="$PWD/.local/journey-target"
cargo fmt --package clubscape-sim --check
cargo test --quiet --package clubscape-sim --package clubscape-protocol
cargo clippy --quiet --package clubscape-sim --all-targets -- -D warnings
cargo test --quiet --package clubscape-sim --all-features
cargo clippy --quiet --package clubscape-sim --all-features --all-targets -- -D warnings
python3 -m unittest discover -s tools/journey-tests -p 'test_*.py' -q
python3 tools/journey-tests/run.py
```

The additional dependencies belong to this crate. Integrators should resolve
their normal workspace lockfile; this task does not commit the root lockfile.

The optional `journey-server` feature adds a thin real-`Service` process launcher
for the orchestrator's explicit `--server-entrypoint public-api` mode. It tests
the documented `Config::with_game_root` constructor without altering the
production server binary or certifying its environment parsing.
