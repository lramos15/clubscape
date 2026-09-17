# Complete actor fix and revision-only source refresh

Applied the 2026-09-15T00:50:50.849-04:00 authorization in order:

| Authorized commit | Integrated commit |
|---|---|
|62a003c6a8d87c42cd46643509ff3757ae541355|fe87989 (actor cadence/presence part1)|
|e1076a87c25e8aafaa5478cc64d7a10c54ce7e46|5150ead (complete effective-context routing)|
|3a74cbe5911e699fd30e51cdedf944e4616f86b5|32167ca (source refresh)|

The complete pair was applied before building or testing; neither the partial
actor repair alone nor parentc58f was selected. Both prior shop fixes and
faa8002's private ground-clock/playtime/contact changes remain integrated.

The source refresh required resolving two hunks of fleet bookkeeping in its
authorized `milestones/m1-tasks.json` patch. Resolution preserved this branch's
existing RuneLite reservation, applied the supplied completed-review evidence,
and did not broaden worker ownership or change milestone acceptance.

## Exact current source

```text
raw artifact: df3e2a452c100ecd94d2abc68e5cb1556f58090700474f547e7fd36de6682b3d
revision:     m1.source-backed.v3.0e506f3dab24bbe0
```

A structural comparison of the prior and current decoded source JSON proves
that **only `/revision` differs**. The artifact and current manifest hashes match;
content/artifact versions remain3, with zero active unresolved bindings and six
inactive proofs. Approved source/reference assets are untouched.

The current candidate is packaged separately:

```text
runelite/compatibility/artifacts/game-3a74cbe/
runelite/compatibility/artifacts/game-3a74cbe/adapter-catalog.json
```

Its world UUID differs from the preserved a200 pack's UUID. No existing world
was repointed, no acknowledged progress was reset, and no account was reseeded.
The older game roots, catalog files, binaries, reports and invocation ledger
remain intact for diagnosis. The runner rejects selecting a200 as the *current*
candidate; an already-running process is not repacked or retargeted. Any future
restart/reconnect within one admitted run must keep that run's exact artifact.

## Current executable checks, not acceptance

* Rebuilt actual strict compiler probe loads the new `df3e…b3d` artifact.
*15 actor and18 shop review tests pass, with zero ignored tests.
*9 generated-codec/projection and35 shop selection/retry checks pass.
*5 Python harness checks and five shared wire/JSON/v1-hash goldens pass.
* Owned service-probe build, strict Clippy and rustfmt pass.

Exact commands/results: `native-reviews-3a74cbe.json`,
`service-probe-build-3a74cbe.json`, `strict-compiler-capacity-3a74cbe.json` and
`validation-3a74cbe.json`. The supplied broader component pass counts are not
relabeled as this worker's live RuneLite, whole-M1 or presentation evidence.

## Independent real-integration blockers remain

The complete engine findings are closed; none is retained as an obsolete
interlock. However, neither actor repair nor the source refresh changes:

1. `crates/server/src/config.rs`: game-root environment parsing is still nested
   inside optional web-root parsing.
2. `crates/server/src/game_service/content.rs`: the private descriptor bound is
   still262,144 bytes.

The **current strict compiler** reports5010 required asset IDs. Their theoretical
minimum JSON map remains267,049 bytes, and the actual new descriptor is283,075
bytes. Thus the new publication still cannot pass the same unchanged size guard
previously reproduced through actual `Service::bind`; a new failed launch would
not provide a distinct diagnostic.

No new database, socket/RuneLite launch, normal account, live shop transaction,
XP callback or generic-plugin gain was created here. The three-invocation
allocation remains exhausted and unchanged. Compatibility remains **UNVERIFIED**,
with no support tier, owner desktop deferral, or automatic later milestone.

Next action: Director fixes the two independent startup surfaces with
fail-closed/full-source tests, and establishes renewed explicit live bounds.
Only then admit real creation/join/first-XP integration on the distinct current
candidate. Do not swap it into a pinned in-flight world or seed around failures.

## Reproduce current packaging/static checks

```sh
python3 tools/runelite-compatibility/pack.py \
  --output runelite/compatibility/artifacts/game-3a74cbe \
  --catalog runelite/compatibility/artifacts/game-3a74cbe/adapter-catalog.json \
  --report research/runelite-feasibility/source-pack-3a74cbe.json
python3 tools/runelite-compatibility/build-service.py \
  --report research/runelite-feasibility/service-probe-build-3a74cbe.json
runelite/compatibility/artifacts/rust-target/debug/clubscape-runelite-service-probe \
  --inspect-artifact runelite/compatibility/artifacts/game-3a74cbe/world.csc
python3 tools/runelite-compatibility/validate.py \
  --java-home /home/lramos15/.local/share/jdks/temurin-17.0.20.1+1 \
  --report research/runelite-feasibility/validation-3a74cbe.json
```

Keep using the authoritative shop field4 schema and immutable displayed item
identity adaptation from [`shop-af75e17.md`](shop-af75e17.md).
