# ClubScape Agent Entry Point

You are the lead implementation agent for ClubScape. Continue from the repository's verified state and complete the active milestone. Inspect, implement, test, integrate, repair, and document the work. Plans and scaffolding count only when the milestone explicitly requires them.

Do not attempt the entire game in one execution. Finish the active milestone, preserve a durable checkpoint, report the evidence, and stop before beginning a later milestone.

## 1. Read these inputs first

Read:

1. `references/README.md`
2. `references/PRODUCT_DECISIONS.md`
3. `references/FROZEN_BASELINE.md`
4. The task-specific reference files selected by `references/README.md`
5. Existing repository instructions and canonical files under `spec/`
6. `project-state/ACTIVE_MILESTONE.md`, if present

Use this precedence when instructions conflict:

1. The project owner's current request
2. The active milestone contract
3. Canonical specifications under `spec/`
4. Required and approved entries in `references/PRODUCT_DECISIONS.md`
5. Technical reference files under `references/`
6. Existing repository conventions and task records
7. Proposed defaults and unresolved ideas

Do not treat a research suggestion as an approved product decision. Record a conflict or unresolved assumption when the sources cannot be reconciled safely.

Resolve ordinary item, NPC, room, reward, character-role, and minigame mappings from the approved contracts and record them in content specifications. Do not repeatedly ask the owner to approve routine mappings. Escalate only conflicts, material deviations, major balance changes, monetization, or changes to product identity.

## 2. Product contract

ClubScape is Old School RuneScape with a Club Penguin spin.

Required product decisions:

- Implement the player-facing mechanics and content of one frozen OSRS baseline.
- Preserve OSRS gameplay behavior unless an approved adaptation says otherwise.
- Players are penguins in appearance creation, chatheads, equipment, animation, combat, skilling, cutscenes, and minigames.
- Existing OSRS NPCs retain their established species. Club Penguin characters are penguins; converting an existing OSRS NPC requires a targeted approved adaptation.
- The OSRS player editor edits penguin traits, palettes, and compatible clothing.
- Use one canonical penguin skeleton, body proportions, equipment envelope, sockets, seams, collision footprint, and animation basis. The editor may change body color, approved markings/facial options, and compatible modular parts within that envelope, but it does not create proportion or skeleton variants. Clothing comes through the equipment system.
- Club Penguin supplies social identity, characters, props, locations, puffles, parties, missions, stamps, Card-Jitsu, minigames, and themed rewards selected in the content contract.
- Club Penguin content exists within the OSRS world and shared game systems.
- Expand the frozen baseline's existing Iceberg and penguin region into the primary Club Penguin social hub. Preserve its OSRS geometry, Cold War storyline, NPCs, quest states, travel, agility-course behavior, and other baseline content; add the new district and venues without replacing or breaking them.
- Make the added Iceberg social district publicly reachable without completing Cold War. Preserve every baseline quest gate for original Iceberg areas, scenes, NPC interactions, routes, shortcuts, and rewards; the public route must not bypass or expose gated Cold War state.
- Club Penguin items and rewards use the ordinary OSRS inventory, bank, equipment, trade, death, shop, market, and economy rules that apply to each item.
- Use OSRS coins as the main currency. Do not create a permanent separate Club Penguin coin balance.
- Preserve individual OSRS items and variants. Penguin equipment art adapts the worn presentation without replacing item identity or behavior.
- Run one authoritative server for the browser client and RuneLite compatibility route.
- The frozen OSRS revision defines the canonical art and interface style. New Club Penguin-derived assets must look as though they belong in that exact revision.
- The browser and RuneLite routes use the same canonical geometry, colors/textures, animations, UI art, lighting assumptions, and visual composition. Do not create separate high- and low-detail art profiles.
- Include the persistent live game and its persistent account modes in the OSRS denominator. Track Leagues, Deadman seasons, temporary events, removed/historical content, inaccessible variants, debug data, and unused records outside parity scope.
- Do not impose an OSRS membership gate. All implemented content is available without a paid subscription or members-world entitlement, while normal gameplay requirements still apply.
- Preserve the frozen baseline's Wilderness, PvP, skull, protection, death, gravestone, reclaim, and item-loss behavior.
- Do not design real-money purchases, subscriptions, paid power, premium currency, or other monetization. A future funding model requires a separate owner decision.

The complete target is defined by the frozen OSRS baseline plus all content from the original browser Club Penguin through its 2017 shutdown. Club Penguin Island is outside the parity target. An early release may expose a smaller verified subset while keeping both full targets visible.

Preserve OSRS names, quests, characters, maps, and stories by default. Add or adapt Club Penguin content through recorded decisions. A broad world or story retheme requires an approved decision entry.

All source material provided or approved by the owner is authorized for this project. Treat asset availability and technical compatibility as engineering questions rather than recurring permission prompts.

## 3. Fixed technology direction

- Rust server and authoritative simulation
- Rust client core compiled to WebAssembly
- WebGPU through `wgpu` for the primary browser renderer
- TypeScript for the browser shell, account pages, settings, routing, and browser integrations
- PostgreSQL for durable relational state
- Redis only for demonstrated ephemeral or caching needs
- Docker for local services
- Blender for custom 3D assets
- Java isolated to RuneLite integration
- No Unity, Unreal, C#, Java, or Python as the primary game runtime

Keep authoritative simulation independent of rendering. Movement, inventory, equipment, skills, combat, quests, shops, drops, instances, and economy must run headlessly.

Use a monorepo and data-driven content with strongly validated schemas, stable semantic IDs, explicit numeric compatibility mappings, migrations, and deterministic content/asset builds.

## 4. Active milestone

Read `project-state/ACTIVE_MILESTONE.md`. It must state:

- milestone ID and goal
- deliverables and non-goals
- dependencies and known blockers
- acceptance criteria
- validation commands
- required reference/spec files
- allowed repository paths or ownership boundaries
- approved adaptations used by the milestone
- task graph root and milestone integration branch
- integration owner, contract manifest, and contract freeze commit
- integration waves, parallel workstreams, shared hotspots, and their owners
- reserved semantic/numeric namespaces and independent-review requirements
- task-environment allocation policy and integrated test environment

If the repository is empty and no active milestone exists, create the initial milestone contract for the starter journey below and proceed with its first executable work. If substantial work already exists, audit it and continue from verified state.

The initial product milestone is:

1. Real ClubScape account sign-up and login.
2. A persistent penguin character created through the penguin-adapted OSRS player editor.
3. Complete frozen-baseline Tutorial Island using shared gameplay systems.
4. Legitimate transition to Lumbridge.
5. The required nearby world, bank, shop, inventory, equipment, skills, combat, audio, and interface behavior.
6. Full Cook's Assistant with legitimate item acquisition, quest state, and rewards.
7. Logout, reconnect, server restart, and source-defined death/recovery validation.
8. A browser client with the selected OSRS interface layout and approved penguin adaptations.
9. A headless path against the same authoritative server/simulation.
10. Approved visual/audio reference cases and measured 60 FPS at a 1920x1080 browser viewport on named modern integrated-graphics hardware in desktop Chrome and Edge with WebGPU.

Use internal checkpoints inside this milestone so presentation, account/persistence, tutorial mechanics, world content, and quest work can advance independently. Do not claim milestone completion until the complete journey passes its acceptance criteria.

The first public release requires this starter milestone plus one polished Club Penguin vertical slice: the public Iceberg district, puffle adoption and care, one faithful minigame, a small catalog, player cards and pins, and at least one stamp page. It also requires the release, operations, security, recovery, performance, accessibility, and supported-client gates in the roadmap. This does not authorize starting that later scope before the active milestone allows it.

## 5. Execution loop

At the start of each execution:

1. Inspect the repository, git state, existing specs, task ledger, CI, and executable tests.
2. Determine the active milestone and its verified progress.
3. Identify contradictions, missing prerequisites, and stale assumptions that affect the current work.
4. Create or update the smallest specifications and task graph needed for the milestone.
5. Select independent tasks with clear path ownership and acceptance criteria.
6. Implement tasks, validate them locally, review them, and integrate only passing work.
7. Convert failures into diagnosed tasks with reproducible evidence.
8. Update milestone, parity, decision, and coverage records as facts change.
9. Run post-integration regression tests.
10. Stop when the milestone passes or no executable unblocked work remains.

Do useful implementation work during the execution. Avoid spending the entire run expanding plans, orchestration infrastructure, or research inventories.

Ask the owner only when a decision changes product direction, baseline/scope, an approved adaptation, the active milestone, RuneLite strategy, significant irreversible cost, or credentials/accounts. Choose normal engineering defaults autonomously.

## 6. Parallel agents

Follow `references/PARALLEL_EXECUTION.md`. Store one schema-validated YAML record per task under `project-state/tasks/`, freeze and version shared contracts by integration wave, and merge task branches through the milestone integration branch. Size tasks as coherent, independently testable merge units with narrow ownership and clean integration boundaries. Use parallel workers for ready tasks with isolated ownership, stable inputs, isolated stateful test environments, and independent validation. Scale concurrency according to the ready frontier, available runtime/provider capacity, review and integration throughput, repository contention, dependency structure, and test capacity. Queue work that cannot be executed safely in parallel.

The owner's preferred routing is GPT-6 Astra for most tasks, Opus 5 for bounded/easy tasks, and Claude Fable 5.1 sparingly for the hardest work. Apply those names only when the active runtime actually exposes them. Otherwise use a capable general model for most work, a lower-cost model for bounded mechanical tasks, and the strongest available model for rare high-risk reasoning. Continue with the closest available model when a preferred name is unavailable. Model selection must not block the milestone.

Before assigning a task, define:

- task ID and goal
- dependencies
- relevant specs and references
- allowed and forbidden paths
- expected output
- validation commands
- acceptance criteria
- reserved content-ID namespace, if applicable
- required evidence and approved adaptation IDs
- exact contract versions and integration wave
- canonical task-record path, isolated test-environment allocation, review tier, and assigned reviewers

Assign the task in the canonical ledger before the worker begins; workers do not race to self-claim work. Use isolated task branches/worktrees for concurrent modifications. Serialize shared schemas, migrations, generated files, registries, milestone fixtures, and core contracts under named owners. A breaking contract change is its own task and must identify and requeue or supersede affected work. Keep communication durable through files, task records, tests, and review results.

Use the task-state and integration rules in `references/PARALLEL_EXECUTION.md`. Only the coordinator assigns work, all required independent reviewers must approve before review passes, and only the integrator marks work integrated. Do not target an arbitrary worker count or manufacture subdivisions when the milestone has a smaller safe ready frontier.

Require one independent reviewer for every task. Require an additional qualified specialist review for security-, economy-, protocol-, persistence-, migration-, authentication-, destructive-, and release-critical work. Use task-specific databases, ports, storage prefixes, fixtures, accounts, and runtime namespaces for stateful validation; immutable pinned reference artifacts may be shared read-only.

Build WaddleWorks only as measured coordination needs justify it. Begin with a durable task ledger and ordinary development tooling.

## 7. Research and baseline use

Follow `references/VERSION_AND_COVERAGE_TRACKING.md` for baseline identity and source tracking.

The OSRS baseline is already frozen in `references/FROZEN_BASELINE.md`. At implementation kickoff:

- Materialize and hash the selected cache, RuneLite/client, scripts, protocol, and source/data artifacts without advancing their frozen identities.
- Enumerate the persistent live game, interfaces, and persistent account modes inside the denominator. Explicitly list excluded seasonal/temporary, removed/historical, inaccessible, debug, and unused content outside it.
- Keep later OSRS updates in an expansion backlog until the baseline changes through an approved decision.

After freezing the baseline, do not repeatedly refresh every upstream source during ordinary implementation. Research only unresolved behavior that affects the current task. Preserve concise source notes for non-obvious mechanics, conflicts, imported data, and deliberate adaptations.

Use evidence labels from `references/README.md`. Never present a repository implementation, wiki statement, decoded definition, or guess as independently verified live behavior.

## 8. Full-content accounting

Build machine-readable coverage registries from the selected baseline and approved Club Penguin inventory.

Each required entry records:

- source game, baseline/era, category, and source ID
- stable ClubScape ID and numeric compatibility mappings
- required behavior and supporting evidence
- dependencies
- behavior completion and presentation completion separately
- approved adaptation IDs
- implementation and verification state
- executable validation evidence

Use orthogonal fields rather than one overloaded status:

```text
implementation: not_started | in_progress | implemented | blocked | deferred
verification: unverified | partially_verified | verified
fidelity: exact | presentation_adaptation | behavior_adaptation | original_content
```

Only verified entries count as verified coverage. Blocked and deferred entries remain in the full denominator. Report verified/required counts and unresolved records, not percentages alone.

Cache or database presence establishes discovery, not complete gameplay. Legitimate player acquisition and progression paths are part of completion.

## 9. Gameplay and economy rules

Treat the server as authoritative. Validate location, collision, timing, action state, requirements, ownership, inventory capacity, targets, transactions, trades, drops, and world state. Protocol secrecy is not a security mechanism.

Preserve selected-baseline behavior for tick timing, action ordering, movement, collision, combat, XP, skills, inventory, equipment, banking, shops, trading, death, quests, activities, and rewards unless an approved behavior adaptation applies.

Keep genuine real-player group requirements. Low population does not authorize NPC stand-ins, solo conversions, increased rewards, or easier requirements. Make source-defined solo progression discoverable and state unavailable group content honestly.

The economy must work at low population through controlled, explicit NPC liquidity. Player orders take priority when compatible. NPC markets require eligibility, price inputs, stock/funding budgets, accounting, and conversion-cycle tests.

Club Penguin minigame payouts and catalog spending use ordinary gold and authoritative transactions. Define payout formulas, caps, eligible outcomes, retries, disconnects, inventory behavior, sources, sinks, and Ironman/account-mode behavior. Never trust a client-submitted score to mint value.

Rebalance Club Penguin prices, payouts, sources, and sinks for the OSRS economy while preserving relative Club Penguin rarity and prestige. Treat raw Club Penguin coin values as reference evidence rather than values to copy directly.

Club Penguin-derived clothing, puffle supplies, stamps, and most activity rewards are cosmetic, collectible, social, or ordinary tradeable goods by default. Adopted puffles themselves are permanent account-bound companions. Any reward that changes combat power, skilling efficiency, traversal, storage, loot, requirements, or progression needs an individually approved balance decision. Integration into the OSRS economy does not itself imply gameplay power.

## 10. Club Penguin content

Follow `references/HYBRID_GAME_DESIGN.md`, treating entries marked proposed as proposals until promoted in `references/PRODUCT_DECISIONS.md`.

The long-term Club Penguin target is the complete original browser game through its 2017 shutdown, including:

- puffles
- igloos and decorating integrated with Construction/housing
- clothing and catalogs
- emotes and selected parties
- named minigames
- Card-Jitsu, Fire, Water, and Snow
- stamps and stamp-book interface
- classic missions

Before bulk production, enumerate the complete target as a named inventory with source era and variants, then select milestone subsets from it. When content changed across 2005–2017, use the latest stable original-browser version as the normal-world default and preserve materially distinct earlier variants through parties, archives, missions, or instances. Club Penguin Island is excluded unless a later decision approves a specific element as inspiration. One example does not complete a family.

Preserve each Club Penguin minigame's recognizable core rules, controls, timing, scoring, and win/failure loop by default. Integrate its entrance, world placement, accounts, persistence, multiplayer, rewards, economy, security, interfaces, and presentation into ClubScape. Record material gameplay deviations per minigame.

A minigame may award modest OSRS skill XP only when its actual play directly exercises that skill. Keep its XP rate below comparable dedicated OSRS training by default, simulate its progression and economy effects, and record the exact skill, formula, caps, eligibility, and Ironman behavior in the activity specification.

Implement the full puffle-care identity: adoption, ownership, individual personality and variant, needs such as food/rest/play/cleanliness, direct interactions, follower behavior, igloo residence, and participation in compatible original activities. Puffles remain non-power companions unless an individual functional effect is approved.

Adoption creates a permanent account-bound puffle and may charge ordinary OSRS coins as a gold sink. A puffle cannot be traded, sold, dropped, lost on death, or transferred between accounts. Puffle food, care supplies, equipment, and cosmetics are ordinary tradeable items by default unless their individual definitions say otherwise.

Use rotating featured Club Penguin catalogs for discovery and seasonal presentation, backed by a permanent archive containing ordinary catalog clothing after its featured window. Ordinary catalog clothing is tradeable by default. Quest, activity, rank, stamp, and live-event rewards remain account-bound or availability-limited according to their own definitions and do not enter the archive automatically.

Use one player home. The igloo is ClubScape's presentation of the OSRS Player-Owned House, backed by the same Construction skill, requirements, rooms, hotspots, costs, storage, portals, servants, persistence, and visiting rules.

Preserve classic Club Penguin mission characters, mysteries, puzzles, and outcomes. Adapt travel, locations, items, and dialogue enough to fit coherently inside Gielinor. Implement Penguin missions as first-class quests in the shared journal, quest engine, map markers, requirement panels, state machine, dialogue, cutscenes, inventory flow, and reward screens.

Penguin missions award unified Quest Points according to OSRS-style length, difficulty, and requirements. Preserve every explicit OSRS quest prerequisite. Inventory every system that consumes total Quest Points and deliberately rebalance each threshold, formula, cape, diary, unlock, or reward so the added missions neither accidentally trivialize nor obstruct progression; generic Quest Point gates count the unified total. The Quest Point Cape ultimately requires all persistent OSRS quests and Penguin missions in the supported release. Track baseline OSRS quest coverage and Penguin mission coverage separately for parity reporting, not as separate player-facing point currencies.

Use an additive expansion of the frozen baseline's existing Iceberg and penguin region as the primary Club Penguin settlement and social hub. Preserve the original Iceberg geometry, Cold War storyline, NPCs, quest states, travel, agility-course behavior, and all other baseline content. Distribute additional venues, rooms, and activity entrances where they fit existing OSRS regions. Do not create a disconnected second world. Record the exact expanded boundaries and every distributed mapping in the content inventory.

Preserve a dedicated Club Penguin stamp book and its activity-specific categories, criteria, and completion tracking. Keep it distinct from OSRS Achievement Diaries and the Collection Log, while allowing relevant activities and accomplishments to be cross-linked in their interfaces. Define one authoritative completion event for every stamp so cross-links cannot double-award progress or rewards.

Missions, minigames, exploration, stamps, parties, and other accomplishments may award server-authoritative pins. Earned pins are untradeable collectibles shown through a dedicated pin collection, a Club Penguin-style player card, relevant Collection Log views, and an igloo/POH pin board. A displayed pin links back to its source. Pins do not consume OSRS equipment slots or bank space; any later tradeable decorative pin item must be a distinct item family.

When adding or migrating a stamp, backfill it only when authoritative stored history proves the exact criterion. Current state may prove a state-based criterion, but do not infer event counts, timing, difficulty, party composition, or other historical facts that were not recorded. If proof is insufficient, require a fresh completion. Make migration and reward settlement idempotent.

Preserve Card-Jitsu as a separate activity progression family with collectible cards and decks, matchmaking, belts, ranks, and ninja progression across Card-Jitsu, Fire, Water, and Snow. Its rewards are primarily cosmetic, social, or collection-oriented. Card-Jitsu does not require or award OSRS combat or Magic levels, XP, equipment power, or other functional progression unless a later decision approves the exact crossover.

Use hybrid binding for Card-Jitsu. Ordinary collectible cards are tradeable through the OSRS inventory, bank, trade, and market economy; using a card in a deck does not consume it. Belts, ranks, ninja status, rank rewards, and specially earned progression cards are account-bound and cannot be bought to substitute for earned progression. Define acquisition sources, duplicate handling, deck ownership checks, storage, losses, and sinks before implementation.

For archived out-of-season parties, allow story replay, activities, collection progress, and appropriate stamps, but cap or disable repeatable economic rewards and reserve selected event-window rewards for the scheduled live party. Define the rule per party and reward.

Puffle ownership is permanent. Neglected puffles may become unhappy or inactive and return to the player's igloo until cared for; neglect does not delete ownership or require re-adoption.

Use shared OSRS-style inventory, equipment, bank, chat, friends, persistence, housing, and economy contracts wherever they fit. Penguin missions grant unified Quest Points, and qualifying minigames may grant their approved modest skill XP. Other CP progress does not grant functional combat rewards, requirement bypasses, or unapproved progression effects.

Preserve OSRS free chat, private messages, friends, ignore, clans, reporting, and moderation. Add optional Club Penguin-style quick-chat phrases, emotes, and social actions without creating a separate social graph or enforcement system. Both clients expose the same authoritative social state and moderation outcomes.

Re-create recognizable Club Penguin musical themes and sound identities within the frozen OSRS revision's audio style and technical constraints. Use original audio as reference rather than inserting it unchanged. Browser and RuneLite routes must present the same canonical music, ambience, cues, and sound-effect identity.

## 11. Browser client and interfaces

The browser is a complete game client. Implement the selected OSRS entry sequence, viewport, game frame, minimap, chat, tabs, menus, panels, input behavior, disabled/error states, and activity interfaces required by the active milestone.

The Rust/WASM client owns protocol state, scene/entity state, camera, animation, viewport rendering, interpolation, and world interaction. TypeScript owns browser/account shell concerns without duplicating authoritative gameplay.

Stream assets by region/content need. Record supported browsers, inputs, viewport range, and required capabilities. Give clear feedback when required capabilities are unavailable.

For each required interface family, maintain:

- traceable visual references
- deterministic screenshot states
- control/state matrix
- interaction tests against authoritative state
- visual comparison tolerances established before acceptance

Regression screenshots against ClubScape detect changes; fidelity screenshots compare against the selected source references. Neither replaces functional tests.

## 12. RuneLite compatibility

Follow `references/RUNELITE_COMPATIBILITY.md`.

RuneLite is a required complete alternative gameplay client for the same accounts, world, mechanics, quests, items, progression, and Club Penguin activities as the browser client. Reach that target through demonstrated compatibility tiers; do not describe an API-shaped shim, successful compilation, mock screenshot, or Creator's Kit scene as working compatibility.

Run bounded spikes against named RuneLite/client/cache builds. Record executable evidence for launch, connection, scene rendering, penguin appearance, game ticks, state/events, menu actions, and at least one generic plugin.

Keep the authoritative server independent of RuneLite assumptions. Browser/server work continues while compatibility gaps are investigated. The supported default configurations of the browser and RuneLite clients must use the same canonical asset geometry, colors/textures, animations, UI art, lighting assumptions, camera composition, and visual identity, within defined cross-renderer screenshot tolerances. Platform rasterization differences may exist; intentional detail tiers or visual substitutions require an owner-approved exception. A strategy change, browser-only activity, or desktop deferral requires a documented feasibility review and owner decision.

## 13. Asset pipeline

Follow `references/BLENDER_ASSET_GUIDE.md`.

Reuse suitable authorized OSRS assets for unchanged OSRS content. Create original Club Penguin-derived assets for penguins, puffles, clothing, props, locations, missions, and minigames that match the frozen OSRS revision's geometry density, proportions, silhouettes, face colors/textures, shading, animation cadence, camera-distance readability, and visual rhythm. Preserve OSRS item identity when adapting worn equipment to penguin anatomy.

Keep canonical authored sources target-independent and generate deterministic outputs that render the same visual design in both the web and RuneLite routes. Treat any required client-specific simplification or substitution as a compatibility blocker or owner-approved exception, not the default pipeline.

The canonical asset source is the authored source file plus semantic manifest. Generate browser and RuneLite/cache outputs deterministically. Keep target-specific metadata explicit, including face color, textures, alpha, priorities, skin groups, recolors, animation links, pivots, sockets, bounds, and IDs.

Before bulk asset production, prove the validation corpus in Blender, decoded target output, the real browser renderer, and the selected RuneLite route. Treat preliminary triangle ranges and content mappings as proposed values until measured.

Every used/generated asset records its source path, input hash, relevant tool version, import/generation steps, modifications, output hashes, and client validation state.

## 14. Architecture and repository shape

Prefer a Cargo workspace with focused crates for protocol, shared types, simulation, world, movement, pathfinding, combat, skills, quests, economy, social systems, server, client core, rendering, audio, input, and WASM delivery. Let actual coupling and compile boundaries determine final crate count.

Keep content under data directories organized by category and compiled into optimized runtime data. Avoid thousands of bespoke Rust classes for ordinary content. A new content entry using existing mechanics should not require an engine edit.

Use stable semantic IDs such as:

```text
item.osrs.pickaxe.rune
item.cp.clothing.sailor_hat
npc.osrs.goblin.basic
mission.cp.operation.blackout
```

Map selected-baseline numeric IDs explicitly for RuneLite/client/cache compatibility. Reserve namespaces before parallel content work.

The repository should support consistent commands such as:

```text
just bootstrap
just dev
just server
just web
just test
just test-fast
just test-integration
just test-browser
just test-runelite
just content-build
just asset-build
just sim
```

Bootstrap blank supported machines with project-local or containerized dependencies where practical. Never store credentials in source control.

## 15. Testing and delivery gates

Test at the appropriate layers:

- unit tests for formulas and local state transitions
- property/invariant tests for ownership, quantities, transactions, state graphs, and migrations
- integration tests with real server components
- headless scenarios for player journeys and progression
- browser tests for real account/session/UI flows
- RuneLite launch/state/plugin tests for supported tiers
- visual comparisons against selected source references
- audio trigger/playback tests in the running client
- adversarial tests for impossible actions, replay, races, duplication, malformed inputs, and downgrade/capability abuse
- load and economy simulations in controlled environments

Every fixed defect needs a regression test when the test would prevent recurrence meaningfully.

A feature is complete when its required implementation, content, legitimate player path, server validation, tests, client behavior, presentation, source/adaptation records, migration behavior, and relevant performance/security evidence pass. A placeholder, mocked path, imported definition, screenshot, or one training route does not complete a category.

CI should run formatting, compilation, lints, schema/content validation, ID uniqueness, unit tests, dependency checks, path ownership, and affected subsystem tests. Expensive suites may run in suitable test environments, but unavailable results remain visible.

## 16. Git and integration

- Do not push directly to main.
- Use isolated branches/worktrees for modifying tasks.
- Restrict core protocol, security, authentication, persistence, simulation, and economy areas to stricter review.
- Require independent review and CI before integration.
- Do not merge solely on an LLM's textual review.
- Preserve unrelated user changes.
- Bound retries and retain failure diagnostics.

## 17. Observability, recovery, and deployment

Add structured logs, metrics, traces, error IDs, session correlation, tick timing, DB latency, packet/rejection rates, client capability/build IDs, economy flows, and asset/content build identities as the active milestone requires.

Design durable transaction boundaries so acknowledged inventory, trade, market, reward, and purchase changes survive crashes and cannot duplicate on retry.

Start deployment simply with containers, PostgreSQL, and object storage/CDN as needed. Add multi-world, region-process, load-balancing, or Kubernetes complexity only after measurements justify it.

## 18. Completion and stopping rule

Milestone completion and full-game completion are different claims.

When the active milestone passes:

1. Stop new work admissions.
2. Park or cancel unfinished workers so they cannot continue into later scope automatically.
3. Preserve the reviewed build and evidence.
4. Report completed scope, commands/results, visual/audio/performance evidence, remaining blockers, known limitations, and proposed next milestone.
5. Stop. Wait for a later execution before implementing the next milestone.

Full-game completion requires verified coverage of the frozen OSRS denominator, the approved Club Penguin inventory, supported clients, deployment, monitoring, backups, recovery, security, performance, and all required assets/interfaces. Deferred or blocked entries remain visible.

Future milestone ordering is planning guidance rather than authorization. Consult `references/MILESTONE_ROADMAP.md` when proposing the next milestone.

## 19. First actions

For an empty repository:

1. Create the active milestone record, per-task YAML schema, milestone integration record, and first contract manifest following `references/PARALLEL_EXECUTION.md`.
2. Establish the selected reference baseline and initial inclusion rules.
3. Create minimal canonical product, architecture, content, interface, art, testing, and RuneLite feasibility specs.
4. Bootstrap the Cargo workspace, browser shell, local services, task commands, minimal CI, and durable task ledger.
5. Define shared simulation, protocol, persistence, stable-ID, content-schema, and asset-manifest contracts.
6. Build an early real-renderer benchmark for login/loading, a representative Tutorial Island/Lumbridge scene, the penguin, one unchanged OSRS object/NPC, HUD, and audio playback.
7. Build real account/session/persistence and shared headless/browser gameplay in dependency order.
8. Complete Tutorial Island, Lumbridge dependencies, and Cook's Assistant.
9. Run the milestone's functional, visual, audio, persistence, security, and performance gates.
10. Report and stop.

For an existing repository, inspect and validate before creating or replacing systems. Continue from working code and reopen failed milestone requirements.

At every stage, favor verified playable progress. Improve tooling and orchestration when a measured delivery problem justifies the work.
