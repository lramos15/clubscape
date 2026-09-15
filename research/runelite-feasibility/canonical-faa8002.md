# Authorized canonical refresh: faa8002

The 2026-09-14T23:21:13.756-04:00 instruction explicitly authorized cherry-picking
`faa800286cfeeed4f3bec0b924d1fb01c65b97d9`. It is integrated here as
`c841f6a6dcd4a1a2b3e7a9e121c62849140c8634`, after the already-present
backend1ea2e65/content-e8dfa2d prerequisites. The upstream original fix was
9bff13e9260cc185d22cf6b3db5335ac35320cb4.

**Canonical source readiness is updated. Real RuneLite integration remains
blocked and unverified; no support tier or desktop deferral is approved.**

## Current source, not the obsolete native interlock

The packed raw artifact matches the supplied and manifest-recorded SHA256:

```text
a200ca08c80f6a3fc812fe02ca95f52325e183084f83b12e8d470a1f37cc6d62
```

Content/artifact versions remain3. The current manifest declares zero active
unresolved bindings and six proved-inactive alternatives. The canonical source
conformance record marks solid-range Single1/Make-X-one3 contact and the fresh
manual-drop offline clock as **resolved**, with the new explicit played-time/
ground-clock selection. Those former failures are **not** held as interlocks.
No unknown legacy playtime was coerced into zero.

The published component validation supplied with the instruction is not being
relabeled as this worker's live RuneLite, full journey or presentation evidence.
This worker rebuilt the unchanged shared-service probe and loaded the current
artifact through the actual strict compiler API.

## Independent startup blockers still apply

The authorized commit does not change either previously diagnosed surface:

| Surface | Current SHA256, identical to the prior reproduced failure |
|---|---|
| `crates/server/src/config.rs` | `ea9c8764da422f6f59c334fcb1d72c2da718d9944950fb4ec451f7c055049926` |
| `crates/server/src/game_service/content.rs` | `9c914d88b9d97a8dba841ff78625cd6c4c49faa48862f7f504c07b9e8b3c09c9` |

1. `CLUBSCAPE_GAME_ROOT` parsing remains nested inside optional
   `CLUBSCAPE_WEB_ROOT` parsing. The earlier real game-only launch therefore
   reached account-only Hello, not an authoritative game world.
2. The current compiler still reports5010 required source assets. Their
   theoretical minimum JSON map is267,049 bytes; the actual current descriptor
   is283,075 bytes, exceeding the unchanged262,144-byte loader bound. This
   current compiler result is in
   [`strict-compiler-capacity-faa8002.json`](strict-compiler-capacity-faa8002.json).
   The earlier public-API service launch reproduced `game_content/game_file_size`;
   a fourth identical failing launch is unnecessary and was not performed.

Director action remains independent game-root parsing plus sufficient bounded
descriptor capacity (for example1MiB), with full-source startup/configuration
tests and every hash/path/membership/readiness guard retained. Pending
shop/actor/style issues were not seeded around or treated as completed.

## Owned integration preparation and preserved history

The current game root and adapter catalog were built separately:

```text
runelite/compatibility/artifacts/game-faa8002/
runelite/compatibility/artifacts/game-faa8002/adapter-catalog.json
```

Packaging now refuses to overwrite an old artifact, report, or catalog when
the canonical revision changes. The runner accepts explicit `--game-root` and
`--catalog` selections, refuses obsolete canonical inputs or mismatched
catalog revisions, and the Java transport checks actual created/joined content
revisions against the selected original catalog.

The original42 research files, including the three-invocation ledger and all
previous assessment/validation records, remain byte-identical. The prior probe
binary and adapter classes were retained under ignored artifact paths before
rebuilding. The new reports are separately suffixed `-faa8002`.

Reproduction commands (build/static checks only):

```sh
JDK17=/home/lramos15/.local/share/jdks/temurin-17.0.20.1+1
python3 tools/runelite-compatibility/pack.py \
  --output runelite/compatibility/artifacts/game-faa8002 \
  --catalog runelite/compatibility/artifacts/game-faa8002/adapter-catalog.json \
  --report research/runelite-feasibility/source-pack-faa8002.json
python3 tools/runelite-compatibility/build-service.py \
  --report research/runelite-feasibility/service-probe-build-faa8002.json
runelite/compatibility/artifacts/rust-target/debug/clubscape-runelite-service-probe \
  --inspect-artifact runelite/compatibility/artifacts/game-faa8002/world.csc
python3 tools/runelite-compatibility/build.py --java-home "$JDK17" \
  --report research/runelite-feasibility/build-faa8002.json \
  --history research/runelite-feasibility/build-history-faa8002.json
python3 tools/runelite-compatibility/validate.py --java-home "$JDK17" \
  --report research/runelite-feasibility/validation-faa8002.json
```

Java compilation,9 synthetic codec/projection checks,5 Python harness checks,
the owned Rust probe build, Clippy with warnings denied, and rustfmt checks pass.
No new original-runtime graphical run, database, live socket attempt, account,
XP event, or plugin gain is claimed by this refresh.

The original three-invocation budget is not reset by a source update. Correct
the two shared startup surfaces and establish renewed explicit live bounds
before admitting actual signup/join/first-XP integration against this selected
current pack. Browser/headless M1 work remains independent.

Machine-readable current status and exact evidence references:
[`canonical-faa8002.json`](canonical-faa8002.json).
