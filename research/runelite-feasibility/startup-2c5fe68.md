# Shared startup blockers resolved: 2c5fe68

The explicit 2026-09-15T01:01:22.811-04:00 authorization was integrated:
2c5fe68b9a6e2e99bcbf3543174f5540be02e95f →
`4953c4ddcdf87773ffab008d22090ff56b02024d`.

**The earlier nested-environment and `game_file_size` failures are historical,
not current blockers.** This update does not establish live RuneLite acceptance.

## Current startup contract and checks

The shared server now parses `CLUBSCAPE_GAME_ROOT` independently of optional
`CLUBSCAPE_WEB_ROOT`. Standalone headless/RuneLite configuration does not need
a fake web bundle. Empty/control/non-Unicode game settings still fail closed.

Only the private game-descriptor limit changes to **524,288 bytes (512KiB)**.
The unchanged current `game-3a74cbe` descriptor is283,075 bytes and fits. All
other source/profile/hash/path/asset-membership/public-view and protocol/gameplay
guards remain unchanged.

Executed focused checks:

* Shared server `config::tests`:5 pass, including independent roots and invalid
  configured-game handling with no web root.
* Shared
  `canonical_sized_game_descriptors_load_but_the_private_byte_limit_is_enforced`:
  1 pass.272,433/406,574/524,288-byte valid fixture descriptors load;
  524,289 bytes reject rather than falling back.
* Rebuilt strict compiler probe loads the actual pinned `df3e…b3d` artifact and
  reports5010 asset IDs /267,049 minimum asset-map bytes.
* Owned checks pass:9 Java codec/projection,35 shop identity/retry,6 Python,
  five shared wire/JSON/v1-hash goldens, strict Clippy and rustfmt.

The descriptor-boundary test is a **synthetic loader regression**, not a new
real game-world/database/RuneLite integration. No live attempt or normal account
was created by these checks.

The packer now reads the actual shared `MAX_GAME_DESCRIPTOR_BYTES` declaration
instead of emitting an obsolete256KiB interlock. The Rust inspection probe no
longer embeds a duplicate historical server limit; it reports compiler facts
only. Current capacity is paired with the actual shared-source declaration.

## Exact candidate and preserved state

```text
artifact: df3e2a452c100ecd94d2abc68e5cb1556f58090700474f547e7fd36de6682b3d
revision: m1.source-backed.v3.0e506f3dab24bbe0
root:     runelite/compatibility/artifacts/game-3a74cbe
catalog:  runelite/compatibility/artifacts/game-3a74cbe/adapter-catalog.json
```

The descriptor, world UUID, artifact, catalog and all previous evidence remain
unchanged. No world was repacked/repointed and no acknowledged progress/rewards
were reseeded. The previous probe binary is retained separately before rebuild.
Complete actor/shop/contact/private-clock fixes remain present. No external
source account/login, peer message, nested agent, or graphics change was used.

## Remaining experiment boundary

The instruction explicitly retains the **same experiment bounds**. The frozen
ledger already contains three conservatively counted invocations, including the
early harness failure. This repair does not reset that ledger or silently admit
a fourth run.

The known shared startup blockers are closed. The exact original-runtime /
authoritative normal creation / live scene and penguin / real event /
genuine plugin-gain tuple is still **UNVERIFIED** because it was not executed
after this fix. No support tier, M1/presentation acceptance, desktop deferral,
or replacement native client is claimed.

**Next action:** explicitly renew/extend live-attempt admission before testing
real normal-account signup/join/first-XP on the unchanged pinned current candidate.
Keep that run's exact artifact through reconnect/restart. Do not cite the old
startup defects or resolved engine findings as current interlocks.

Current machine-readable closure:
[`startup-2c5fe68.json`](startup-2c5fe68.json).
Actual commands/results:
`startup-regressions-2c5fe68.json`,
`service-probe-build-2c5fe68.json`,
`strict-compiler-capacity-2c5fe68.json`,
`validation-2c5fe68.json`.
