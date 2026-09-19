# Frozen OSRS Baseline

Frozen: 2026-09-17 UTC  
Baseline content date: 2026-09-16  
Status: `APPROVED` baseline identity; artifact hashes and injected-client identity still require capture before ingestion

This file is the human-readable authority for the OSRS baseline ClubScape copies. Do not advance these pins merely because a newer OSRS update, cache, RuneLite release, or reference repository appears. A baseline change requires an owner-approved decision and a measured migration plan.

## Frozen identities

| Component | Frozen identity | Evidence and remaining capture |
|---|---|---|
| OSRS protocol | Revision `240` | Confirmed by the 2026-09-16 live cache and matching RuneLite updates |
| Live cache | OpenRS2 archive id `2710`, `oldschool/live/en`, build `240`, timestamp `2026-09-16 10:30:13` | Archive reports 25/25 archives and 117,455/117,455 groups; download and record SHA-256 before ingestion |
| Weekly cache mirror | `abextm/osrs-cache` release `2026-09-16-rev240` | Full commit `ec6c640cad14b78b05b74c25e07c2603b085fd14`; record each consumed release-asset hash |
| RuneLite stable source | Release `1.12.39` | Release commit `67d51a4a4a945e6e3f60c75cbc7dd859d441ff04` |
| RuneLite baseline data update | `2026-09-16-rev240` | GameVals commit `0edc8a6bc4a0755d2c1e44de04b88adc8fea10c4`; legacy-ID update `e98cda9d134f5b523bf473d47b581ba7c7107c1f` |
| CS2 scripts | `2026-09-16-rev240` | Full commit `baf8a24853e230d024b627f2513cd475bfbd58ba` |
| Default visual target | Frozen revision's standard OSRS presentation | No HD replacement assets or GPU-specific appearance in parity screenshots; user-installed visual plugins are outside the canonical comparison profile |

## Scope locked to this baseline

Included:

- the persistent live OSRS game represented by the frozen cache and client;
- persistent account modes and their player-facing rules;
- the baseline's ordinary interfaces, items, quests, skills, activities, Wilderness, PvP, death, gravestone, reclaim, and item-loss behavior;
- F2P and traditionally members-only content, without a ClubScape membership entitlement gate.

Tracked outside the OSRS parity denominator:

- Leagues and Deadman seasons;
- temporary OSRS events;
- removed or historical content not present in the persistent frozen game;
- inaccessible variants, debug data, and unused records;
- later OSRS releases until an approved baseline migration.

This exclusion does not remove classic Club Penguin parties from the separate Club Penguin target.

## Cross-client visual rule

The browser and RuneLite routes must render the same canonical asset and interface design:

- geometry and model composition;
- face colors and textures;
- animations and timing;
- UI art and layout for the selected state;
- lighting assumptions, camera composition, and scene identity.

Define screenshot tolerances for unavoidable rasterizer, antialiasing, operating-system, and font-rendering differences. Do not create separate web and RuneLite art-quality tiers. Treat any required visual substitution as a compatibility blocker or owner-approved exception.

## Required completion before implementation import

The baseline identity is frozen even while these capture tasks remain open:

1. Download the selected OpenRS2 and mirror artifacts and record SHA-256 values.
2. Resolve and hash the exact injected OSRS client artifact used by the selected RuneLite route.
3. Pin matching RSProt and RSProx commits/modules and validate revision-240 fixtures.
4. Capture the default visual-reference corpus in named locations, interfaces, animations, and lighting states.
5. Store the machine-readable equivalent under the implementation repository's canonical baseline manifest.

Failure to complete a capture task blocks claims based on that artifact; it does not silently authorize a newer baseline.

## Verification sources

- [OpenRS2 archive cache 2710](https://archive.openrs2.org/caches/runescape/2710)
- [`abextm/osrs-cache` release `2026-09-16-rev240`](https://github.com/abextm/osrs-cache/releases/tag/2026-09-16-rev240)
- [RuneLite release commit `67d51a4`](https://github.com/runelite/runelite/commit/67d51a4a4a945e6e3f60c75cbc7dd859d441ff04)
- [RuneLite GameVals update `0edc8a6`](https://github.com/runelite/runelite/commit/0edc8a6bc4a0755d2c1e44de04b88adc8fea10c4)
- [CS2 scripts update `baf8a24`](https://github.com/runelite/cs2-scripts/commit/baf8a24853e230d024b627f2513cd475bfbd58ba)
