# Parallel Execution

Updated: 2026-09-17

This file defines how ClubScape milestones are decomposed and executed by many agents without duplicating work, racing on shared files, or integrating against unstable contracts. It governs coordination rather than product behavior.

There is no fixed agent limit. Useful concurrency is the number of ready tasks with independent ownership, stable inputs, and independent validation. Add workers when the ready frontier grows; reduce concurrency when shared contracts, repository contention, integration throughput, or test capacity become the constraint.

## Binding coordination choices

- Store the canonical task graph as one version-controlled YAML file per task under `project-state/tasks/`.
- Validate task files with a checked-in schema and CI.
- Use a milestone integration branch. Task branches merge into it after review and validation; the reviewed milestone result is then promoted to `main`.
- Freeze and version shared contracts by integration wave.
- Represent a breaking contract change as its own task. Identify affected work, invalidate stale evidence, and requeue or supersede dependent tasks explicitly.
- Size work as coherent, independently testable merge units with narrow ownership and a clean integration boundary.
- Give every task one independent reviewer. Security-, economy-, protocol-, persistence-, migration-, authentication-, and release-critical work also requires a qualified specialist review.
- Give stateful parallel tasks isolated databases, ports, storage prefixes, fixtures, accounts, and runtime namespaces.
- Do not use a shared database, chat transcript, or agent memory as the only record of task state.

## Required project-state layout

Create these paths when implementation begins:

```text
project-state/
  ACTIVE_MILESTONE.md
  tasks/
    schema.json
    <task-id>.yaml
  contracts/
    <milestone-id>/
      manifest.yaml
      ...versioned contracts...
  integration/
    <milestone-id>.md
  evidence/
    <task-id>/
      manifest.yaml
      ...logs, reports, and capture references...
```

`ACTIVE_MILESTONE.md` is the human-readable scope authority. Task YAML files are the executable dependency graph. Contract manifests identify the exact interfaces each task targets. Evidence manifests point to reproducible outputs without forcing large generated artifacts into Git.

Generated summaries or dashboards may be built from these files, but they are projections and must not become a second authority.

## Roles

| Role | Responsibility |
|---|---|
| Milestone coordinator | Maintains the ready frontier, assigns tasks, prevents duplicate claims, and stops admission at the milestone boundary |
| Contract owner | Owns one shared contract during a wave and evaluates requested changes |
| Task implementer | Changes only assigned paths, runs required validation, and produces the declared outputs/evidence |
| Independent reviewer | Checks behavior, tests, scope, evidence, and contract compliance without relying on the implementer's summary |
| Specialist reviewer | Performs the additional domain review required for sensitive security, economy, protocol, persistence, migration, authentication, or release work |
| Integrator | Owns the milestone integration branch, shared-hotspot changes, merge order, and post-merge regression results |
| Validation worker | Runs cross-task, cross-client, performance, security, economy, or journey suites against integrated work |

One agent may hold several roles across different tasks when capacity is limited. Every task must be reviewed by someone other than its implementer. Sensitive tasks require an additional qualified specialist who is distinct from both the implementer and the primary reviewer.

The coordinator assigns a task before a worker starts. Workers do not race to self-claim an unassigned YAML record. Assignment is committed to the milestone integration branch or otherwise made authoritative by the coordinator before implementation begins.

## Task record

Every task is one YAML file. Use a stable semantic task ID; renaming a file does not change the ID.

```yaml
schema_version: 1
id: m02-puffle-persistence
milestone_id: m02-first-cp-slice
title: Persist adopted puffle identity and care state
state: ready
priority: normal

goal: >-
  Persist one adopted puffle and its care state across logout and server restart.
non_goals:
  - Puffle minigame behavior
  - Additional puffle variants

dependencies:
  - m02-contract-player-companion-v1
dependents: []
blocked_by: []

assigned_to: null
review_tier: sensitive
reviewers:
  primary: null
  specialists:
    - persistence
integration_owner: milestone-integrator
integration_wave: implementation-1

contract_versions:
  player_companion: 1
  persistence_event: 2
approved_adaptation_ids:
  - puffles.full_care
  - puffles.account_bound_economy

owned_paths:
  - crates/persistence/src/puffles/
  - crates/world/src/puffles/state.rs
read_only_paths:
  - crates/protocol/
  - content/schemas/
forbidden_paths:
  - migrations/
  - content/generated/

reserved_ids:
  semantic_namespaces:
    - puffle.cp.base.*
  numeric_ranges: []

test_environment:
  isolation: ephemeral
  database_namespace: task-m02-puffle-persistence
  port_namespace: task-m02-puffle-persistence
  storage_prefix: task/m02-puffle-persistence/
  fixture_namespace: task-m02-puffle-persistence
  cleanup_after_evidence: true

inputs:
  - project-state/contracts/m02-first-cp-slice/player-companion-v1.yaml
outputs:
  - persistent puffle repository implementation
  - restart/reconnect tests

validation:
  - just test-fast
  - just test-integration puffle_persistence
acceptance:
  - Adoption is exactly once.
  - Care state survives reconnect and restart.
  - The puffle cannot be traded, dropped, or lost on death.

evidence:
  manifest: project-state/evidence/m02-puffle-persistence/manifest.yaml
  required:
    - test-results
    - migration-or-schema-note

failure_notes: []
supersedes: []
superseded_by: null
```

Omit fields only when the checked-in schema marks them optional. Do not use prose task lists as a substitute for records with dependencies, ownership, validation, and evidence.

## Task sizing

Create tasks as testable merge units:

- one coherent outcome that advances the player journey or a required enabling contract;
- narrow owned paths with little or no overlap with another active task;
- explicit inputs and output contracts;
- validation that can run without waiting for unrelated work;
- a merge that can be reviewed, reverted, and diagnosed independently;
- completion within one integration wave unless the task is itself a named long-running investigation.

Split a task when its outputs have independent contracts, ownership, or validation. Keep work together when splitting would make several agents edit the same file, require unrecorded coordination, or produce pieces that cannot be validated independently.

Do not optimize for the number of workers. Tiny file-edit tasks inflate handoffs, reviews, and merges without increasing useful concurrency. Broad subsystem assignments spanning several waves accumulate drift and hide blockers. The coordinator may resize or supersede a proposed task before assignment; after work begins, scope changes require a ledger update and review.

## Task state machine

Use these states:

| State | Meaning |
|---|---|
| `proposed` | Useful work identified but not yet sufficiently specified |
| `blocked` | A named dependency, decision, credential, contract, or environment prevents execution |
| `ready` | Inputs, ownership, contracts, validation, and acceptance criteria are complete |
| `assigned` | Coordinator has reserved the task for one worker |
| `active` | Worker is implementing within the assignment |
| `review` | Declared outputs and evidence exist; independent review is pending |
| `passed` | Task-level review and validation passed against the declared contract versions |
| `integrated` | Work is present on the milestone integration branch and post-merge checks passed |
| `failed` | Attempt failed with reproducible evidence and a diagnosed next action |
| `superseded` | Another task or contract version replaces this work |
| `cancelled` | Work is no longer in the active milestone and the reason is recorded |

Normal flow is `proposed -> blocked|ready -> assigned -> active -> review -> passed -> integrated`.

Only the coordinator changes assignment state. Only reviewers mark task review as passed. Only the integrator marks a task integrated. A worker discovering out-of-scope work creates a proposed follow-up task rather than silently expanding ownership.

## Ready-frontier scheduling

At each scheduling pass:

1. Load the active milestone, task files, contract manifest, and current integration record.
2. Recompute which tasks have all dependencies integrated or explicitly satisfied.
3. Exclude tasks whose owned paths overlap an active task unless both tasks declare a reviewed sharing rule.
4. Prioritize critical-path contract and integration tasks, then independent player-visible work, then optional optimizations.
5. Assign one worker to each selected task and record the assignment before work begins.
6. Keep enough capacity for review, integration, and failure diagnosis; implementers alone do not create throughput.
7. Recompute the frontier after every integration, failure, contract change, or newly discovered blocker.

Do not target a predetermined worker count. A milestone with four safe ready tasks should use four implementation lanes, not manufacture extra subdivisions. A milestone with dozens of independent content entries may use many more when review and integration capacity can keep up.

## Isolated task environments

Stateful validation runs in a task-specific environment by default. Allocate unique:

- database, schema, or database-name namespace;
- TCP/UDP ports and service names;
- object-storage bucket or prefix;
- cache, queue, topic, and lock namespace;
- test accounts, character IDs, world/instance IDs, and deterministic RNG seeds;
- temporary directories, browser profiles, logs, screenshots, and capture paths.

The task record declares the allocation. Repository tooling should create, reset, and tear down the environment deterministically. Preserve required logs and evidence manifests before cleanup.

Read-only immutable caches and pinned reference artifacts may be shared. A mutable service may be shared only when the test proves isolation through namespacing and the task record identifies the remaining contention. Never point parallel task validation at production, shared personal accounts, or an unpartitioned development database.

Integration and milestone validation run against a separately named integrated environment built from the milestone integration commit. Passing in a task environment is necessary but does not replace post-merge validation.

## Path and identifier ownership

Every active task classifies paths as:

- `owned`: the task may modify them;
- `read_only`: the task may inspect but not modify them;
- `forbidden`: the task must not touch them;
- `shared_hotspot`: modification requires an integration-owned task.

Default shared hotspots include:

- Cargo workspace manifests and dependency policy;
- protocol and authoritative simulation contracts;
- database migrations and schema history;
- content schemas and validators;
- numeric ID registries and allocation files;
- generated content/assets and their manifests;
- authentication, authorization, security policy, and secrets handling;
- milestone-wide test fixtures and golden captures;
- release/deployment definitions.

Reserve semantic namespaces and numeric ranges before parallel content production. A reservation gives a task permission to allocate inside the range; it does not authorize changing the registry. The integrator owns registry updates and collision checks.

## Contract freeze and change protocol

Each integration wave has a contract manifest containing:

```yaml
milestone_id: m02-first-cp-slice
wave: implementation-1
contracts:
  protocol_events: 3
  player_state: 2
  content_schema: 4
  asset_manifest: 1
  client_capabilities: 2
frozen_at_commit: <full-commit-sha>
owners:
  protocol_events: <task-or-role>
```

Tasks declare the exact contract versions they consume. Compatible additions may be accepted by the contract owner and included in the next manifest revision. A breaking change requires a dedicated change task that records:

- the reason and rejected alternatives;
- old and new contract versions;
- affected active, passed, and integrated tasks;
- required migrations and compatibility adapters;
- invalidated tests, captures, or evidence;
- requeue, supersession, and merge order;
- rollback procedure.

Do not ask every worker to chase a moving contract. Complete or stop the affected wave, integrate the contract change, regenerate the ready frontier, and resume against the new manifest.

## Branch and integration model

Use these logical branch roles; exact safe names may follow repository conventions:

```text
main
milestone/<milestone-id>/integration
task/<task-id>
```

- Task branches start from the recorded milestone integration commit.
- Each task uses an isolated worktree when it modifies files.
- Workers do not push directly to `main` or merge their own work into the integration branch.
- Before review, update the task branch against the integration branch when required and rerun affected validation.
- The integrator merges passing work in dependency order and owns conflict resolution. Semantic conflicts return to a task or contract owner; the integrator does not guess behavior during a textual merge.
- Post-merge targeted tests run after every integration. Broader suites run at the wave gates.
- Promotion from the milestone integration branch to `main` requires milestone-level review and the active acceptance gates. Partial task success is not a milestone release.

An integrator handoff must update the integration record with the current branch/commit, queued tasks, known conflicts, failing tests, contract versions, and next safe action.

## Integration waves

Every milestone uses explicit waves, adapted to its needs:

| Wave | Purpose | Concurrency |
|---|---|---|
| 0. Evidence and decomposition | Resolve unknowns, write specs, identify shared hotspots and task DAG | High for independent research; low for final synthesis |
| 1. Contract foundation | Implement/freeze schemas, protocols, state, IDs, fixtures, and asset interfaces | Limited and deliberately serialized by contract |
| 2. Independent implementation | Build server, client, content, assets, and tests against frozen versions | Highest safe concurrency |
| 3. Subsystem integration | Merge related tasks and run subsystem journeys | Moderate; integration lanes are serialized per hotspot |
| 4. Cross-client/system validation | End-to-end, RuneLite/WASM, visual/audio, security, performance, and economy tests | Parallel by validation class against one build |
| 5. Closeout | Repair failures, update ledgers/evidence, review, and promote or stop | Low, controlled admission |

A milestone may repeat waves 1–4. Record each repetition rather than treating the entire milestone as one long mutable phase.

## Review and evidence gates

A task can enter `passed` only when:

- changes stay within owned paths or have an approved ownership amendment;
- declared contracts and adaptation IDs match the implementation;
- validation commands pass or unavailable results are explicitly blocking;
- source and behavior conclusions have traceable evidence;
- failure, retry, reconnect, migration, and duplicate-settlement behavior are covered when applicable;
- client-visible work has the required browser/RuneLite and visual/audio evidence;
- one independent primary reviewer approves it;
- sensitive security, economy, protocol, persistence, migration, authentication, destructive, and release-critical work receives its required specialist review.

An integrated task can be reopened when a later merge breaks its acceptance tests or a contract change invalidates its evidence. Never retain `integrated` as a ceremonial status when the integrated build no longer passes.

## Milestone contract additions

Every `ACTIVE_MILESTONE.md` must include:

```text
task_graph_root
milestone_integration_branch
integration_owner
contract_manifest_path
contract_freeze_commit
integration_waves
parallel_workstreams
shared_hotspots and owners
reserved semantic/numeric namespaces
review capacity and required independent-review classes
task-environment allocation policy and integration environment
integration and regression commands
```

Each parallel workstream declares its goal, entry dependencies, output contracts, owned paths, shared hotspots, validation, integration wave, and convergence test. A milestone is not ready for broad fan-out until these fields exist and its first contract foundation tasks are ready or integrated.

## Default decomposition by roadmap milestone

These are workstream templates, not permission to start a milestone or fixed task counts.

### Milestone 1: starter journey

Serialize the initial protocol, simulation, persistence, content-schema, ID, and asset contracts. Then fan out:

- account/session/authentication;
- deterministic tick and shared simulation primitives;
- persistence and reconnect/restart behavior;
- browser shell, WebGPU renderer, input, UI frame, and audio foundation;
- canonical penguin rig, editor assets, animations, and chatheads;
- Tutorial Island areas and individual tutors/mechanics;
- Lumbridge world slice, shops, bank, NPCs, and combat;
- Cook's Assistant content and quest-state tests;
- RuneLite feasibility spikes;
- fixtures, browser journeys, visual/audio comparisons, security, and performance.

Converge first on login-to-character-creation, then Tutorial Island, then Lumbridge/Cook's Assistant, and finally the complete restart/death/client acceptance journey.

### Milestone 2: first public Club Penguin vertical slice

After player-state, activity/reward, item/economy, profile/collection, world-access, and client-capability contracts freeze, fan out:

- Iceberg district, transport, collision, and Cold War gate protection;
- puffle adoption, care, follower behavior, persistence, and assets;
- one faithful minigame simulation, interface, networking, scoring, and audio;
- catalog rotation/archive, shop definitions, clothing item, economy simulation, and equipment art;
- stamp criteria, migration, and stamp-book UI;
- pin award, player card, Collection Log view, and igloo display;
- browser implementation and RuneLite-equivalent delivery;
- social/moderation behavior needed by the slice;
- cross-client journey, security, economy, visual/audio, performance, and recovery validation.

Converge on the complete public-district-to-puffle/minigame/reward/catalog/pin/stamp journey, not on isolated demonstrations.

### Milestone 3: Mining progression

Freeze action-cycle, resource-node, XP, inventory, equipment, drop, world-placement, and content schemas. Fan out research and implementation by:

- formulas, timing, boosts, interrupts, and level rules;
- rocks, depletion, respawn, regions, guilds, and special locations;
- ores, pickaxes, equipment requirements, shops, drops, and creation sources;
- quests, diaries, achievements, and other Mining dependencies;
- animations, sounds, effects, interfaces, and penguin equipment fitting;
- browser/RuneLite events and plugin-visible state;
- economy/progression simulation and level-band journeys;
- parity ledger, fixtures, regression, and level 1–99 acquisition-path validation.

Partition content by stable semantic namespace or region, not by multiple agents editing one central definition file.

### Milestone 4: core skill and gameplay network

Build and freeze reusable action, production, combat, projectile, spell, prayer, requirement, reward, and quest contracts before large fan-out. Then allocate independent skill families, item/equipment families, quest groups, world regions, UI families, asset families, and validation suites. Keep cross-skill production chains as explicit integration tasks rather than hidden dependencies between workers.

### Milestone 5: economy, housing, and social depth

Use separate workstreams for Grand Exchange/player orders, controlled NPC liquidity, shops, trade, Construction/igloos, furniture, social graph/chat/clans/moderation, puffles, catalogs, and their client interfaces. Serialize transaction, persistence, migration, privacy, and abuse-prevention contracts. Run economy, duplication, crash-recovery, and permission tests against the integrated build.

### Milestone 6: activities and completion systems

Freeze activity lifecycle, matchmaking/party, instance, reward settlement, Quest Point, achievement, stamp, collection, and replay/archive contracts. Fan out by individual OSRS activity, CP minigame, Card-Jitsu variant, quest/mission group, diary/collection family, PvP/raid system, party, or asset set. Give each activity its own specification, namespace, state-machine tests, client deliverables, and evidence. Converge through shared matchmaking, rewards, completion systems, and cross-activity economy tests.

### Milestone 7: release and full-target closure

Parallelize validation by deployment, observability, security, privacy/moderation, backups, restore, disaster recovery, load, performance, economy integrity, accessibility, browser compatibility, RuneLite compatibility, visual/audio parity, content coverage, and operational documentation. All lanes test the same immutable release candidate. Serialize release-candidate creation, data migrations, rollback decisions, and promotion.

## Failure and recovery

When a task fails:

1. Preserve the branch, logs, commands, inputs, contract versions, and failing fixture.
2. Mark the task `failed`; do not leave it `active` indefinitely.
3. Diagnose whether the cause is local implementation, missing evidence, environment, dependency, or contract failure.
4. Create a bounded repair task or breaking-contract task with explicit ownership.
5. Recompute dependent task states. Do not allow stale dependents to remain ready.
6. Prefer resuming from preserved work when ownership and contracts remain valid.

Repeated retries without new evidence are not parallel progress.

## Anti-patterns

- Spawning workers before task ownership and contract versions exist.
- Splitting one tightly coupled file among several implementers.
- Letting every worker edit manifests, migrations, schemas, or registries.
- Treating research summaries as implementation tasks without executable acceptance.
- Creating duplicate server, web, and RuneLite rules instead of one authoritative behavior contract.
- Holding large task branches across several integration waves.
- Merging many passing branches at once without post-merge isolation.
- Counting worker activity, commits, or generated definitions as milestone progress.
- Adding orchestration infrastructure before a measured coordination problem justifies it.

## Bootstrap sequence

For an empty repository:

1. Create `ACTIVE_MILESTONE.md`, the task schema, the milestone integration record, and the first contract manifest.
2. Create contract-foundation tasks and identify every shared hotspot.
3. Integrate the minimum stable contracts and freeze the first implementation wave.
4. Generate the first ready frontier and assign independent tasks.
5. Keep review and integration lanes active while implementation runs.
6. Run subsystem convergence tests throughout the wave rather than postponing integration to the end.
7. Close the wave, repair or requeue failures, and freeze the next contract manifest.

Start with these files and ordinary Git/CI tooling. Build WaddleWorks or another coordination service only when measured scheduling, visibility, or scale problems cannot be solved cleanly with the versioned ledger.
