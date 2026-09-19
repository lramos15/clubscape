# Milestone Roadmap

Updated: 2026-09-17

This is sequencing guidance. It does not authorize an agent to start a later milestone. The active execution is defined in `project-state/ACTIVE_MILESTONE.md` and stops when that milestone passes or becomes fully blocked.

## Milestone contract

Every milestone should state:

```text
id
goal
player-visible journey
deliverables
non_goals
dependencies
required references and specifications
approved adaptations
allowed path ownership
functional acceptance criteria
visual/audio acceptance criteria
performance/security/reliability criteria
validation commands
evidence outputs
known blockers
task graph root
milestone integration branch and integration owner
contract manifest and freeze commit
integration waves and parallel workstreams
shared hotspots and owners
reserved semantic/numeric namespaces
review capacity and required independent-review classes
task-environment allocation policy and integrated test environment
```

Internal checkpoints may divide work and allow parallel progress. They do not redefine milestone completion. Follow [PARALLEL_EXECUTION.md](PARALLEL_EXECUTION.md): freeze shared contracts by wave, create one validated YAML record per task, assign disjoint ownership, and converge through the milestone integration branch.

## 1. Starter journey

Player-visible target:

1. Create a real account and log in.
2. Create/customize a persistent penguin through the adapted player editor.
3. Complete the full frozen-baseline Tutorial Island.
4. Arrive legitimately in Lumbridge.
5. Use the required inventory, equipment, skills, bank, shop, combat, chat, minimap, menus, music, and sound.
6. Complete Cook's Assistant through legitimate acquisition and quest progression.
7. Recover correctly through logout, reconnect, server restart, and source-defined death/recovery behavior.

Required delivery paths: authoritative server, headless simulation, and Rust/WASM browser client. RuneLite feasibility proceeds as a bounded parallel track and reports its demonstrated tier.

Presentation target: selected stock-OSRS interface/world references with the approved penguin and ClubScape identity changes. Establish the comparison captures and tolerances before acceptance. Measure the 60 FPS target at 1920x1080 on named modern integrated-graphics hardware in supported Chrome and Edge builds using WebGPU.

## 2. First public Club Penguin vertical slice

Prove one polished crossover and satisfy the Club Penguin content portion of the first-public-release gate before bulk CP production:

1. Enter and use the public Iceberg social district without bypassing Cold War gates.
2. Adopt, care for, summon, dismiss, and persist at least one puffle.
3. Play one selected faithful minigame using real controls and server-authoritative scoring.
4. Receive its specified capped gold payout and any approved modest skill XP exactly once.
5. Spend legitimately held gold through a small rotating/archived catalog, equip one CP clothing item through the ordinary equipment interface, and bank or unequip it normally.
6. Use the integrated player card and earn/display at least one pin from a real accomplishment.
7. Complete and inspect at least one functional stamp-book page with authoritative criteria and evidence-based migration behavior.
8. Persist the district, puffle, gold, item, equipment, score, XP, pin, player-card, and stamp state through reconnect and restart.
9. Verify that no unintended Quest Points, skill XP, functional reward, requirement bypass, or duplicate settlement occurred.

This milestone establishes templates for later minigames, catalogs, stamps/ranks, and cosmetic equipment. It does not complete those families.

The first public release requires both Milestones 1 and 2 plus deployment, monitoring, backups, rollback, recovery, security, load, accessibility, supported-client, and disclosure gates. Completing a development milestone alone does not authorize a public launch.

## 3. Mining progression milestone

Demonstrate legitimate Mining progression from level 1 to 99 through verified baseline methods and acquisition paths. Cover rocks, ores, pickaxes, formulas, depletion/respawn, locations, guild/content, quests, rewards, achievements, animations, audio, economy effects, browser support, and the applicable RuneLite tier.

This validates large-scale content production and progression. It does not count as complete Mining parity while baseline methods or dependencies remain unverified.

## 4. Core skill and gameplay network

Suggested order, adjusted when dependency evidence supports a change:

1. Woodcutting
2. Fishing
3. Cooking
4. Smithing
5. Melee combat depth
6. Banking and shops depth
7. Quests plus shared mission machinery
8. Equipment families
9. Magic and Ranged
10. Remaining gathering/production network
11. Slayer

Tutorial Island mechanics already exist from Milestone 1. These milestones deepen full baseline coverage and legitimate progression paths.

## 5. Economy, housing, and social depth

- bosses and reward loops
- Grand Exchange and controlled low-population liquidity
- Construction and igloo presentation
- friends, ignore, clans, chat, privacy, and social hubs
- puffles and selected CP social systems
- catalogs and ordinary cosmetic equipment at production scale

Order economy work according to the dependencies needed for legitimate item acquisition and source/sink simulation.

## 6. Activities and completion systems

- OSRS minigames
- selected CP minigames
- Card-Jitsu, Fire, Water, and Snow
- achievements, diaries, collection logs, and stamps
- PvP
- raids and endgame
- selected CP missions and parties

Each family needs a named inventory. Representative examples do not complete a family.

## 7. Release and full-target closure

For each release candidate, validate the approved release scope, deployment, monitoring, backups, rollback, recovery, security, load, economy, supported clients, accessibility commitments, and disclosed limitations.

Full completion requires every included frozen-baseline and approved CP entry to be verified. Later OSRS updates remain expansion work unless the baseline is deliberately changed.

## Roadmap rules

- Reorder milestones when dependencies or evidence make another sequence materially safer or faster.
- Build the milestone task graph and workstream matrix before broad fan-out; do not confuse an unbounded checklist with parallel-ready work.
- Serialize contract foundations and shared hotspots, then use all safe concurrency exposed by the ready frontier rather than a fixed agent count.
- Do not use the roadmap to start work beyond the active milestone.
- Do not mark a family complete from one example, imported definitions, or a level-99 route.
- Preserve blocked and deferred work in the full denominator.
- Update this file when sequencing changes; update `PRODUCT_DECISIONS.md` when product scope or behavior changes.
