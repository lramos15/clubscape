# ClubScape Reference Pack

Updated: 2026-09-17  
Purpose: source, version, hybrid-content, and asset reference for implementation agents  
Architecture and milestone planning intentionally live elsewhere

## Product contract

ClubScape is **Old School RuneScape with a Club Penguin spin**.

- OSRS supplies the world, mechanics, combat, skills, quests, interfaces, NPCs, objects, items, equipment rules, progression, and economy.
- The intended scope is **all OSRS mechanics and all OSRS items**, not a small OSRS-inspired subset.
- Every player is a **penguin**. This includes appearance creation, chatheads, equipment, animations, combat, skilling, emotes, cutscenes, and minigames.
- The OSRS player editor remains the customization workflow, but its selections edit penguin traits, palettes, and penguin-compatible clothing.
- Club Penguin supplies the social tone, visual language, characters, props, activities, parties, puffles, minigames, and original rewards.
- Club Penguin content exists inside Gielinor. It is not a separate Flash island, disconnected arcade, second inventory, or second economy.
- Club Penguin-themed items and rewards are first-class items in the OSRS economy.
- Existing Club Penguin art is evidence for newly authored OSRS-style 3D art, not a substitute for it.

The selected implementation is a native Rust server, a Rust web client compiled to WebAssembly, and a compatibility shim for RuneLite/the underlying OSRS client.

## Fidelity rule

When the source games differ:

1. Preserve the OSRS rule, item identity, actions, requirements, stats, effects, progression relationship, and world interaction unless an explicit ClubScape decision changes it.
2. Express the result through a penguin body and Club Penguin-inspired art, presentation, humor, or writing.
3. Integrate Club Penguin activities through explicit OSRS-world locations, requirements, rewards, and state transitions.
4. Record intentional differences. A reskin is not permission to silently alter gameplay.

Examples:

- A dragon scimitar remains the dragon scimitar with its OSRS behavior, but its worn mesh and animations fit a penguin flipper.
- Ice Fishing can preserve its recognizable Club Penguin loop while using actual OSRS Fishing XP, fish, equipment, inventory, and reward rules.
- A puffle can become a follower/pet with OSRS-style ownership, summoning, dismissal, banking or reclaim behavior rather than a separate Club Penguin pet system.

## Files and reading order

| File | Load when working on |
|---|---|
| [PRODUCT_DECISIONS.md](PRODUCT_DECISIONS.md) | Determining what the owner required, what has been approved, what remains proposed, and which questions are unresolved |
| [FROZEN_BASELINE.md](FROZEN_BASELINE.md) | Any OSRS protocol, cache, RuneLite, visual-parity, scope, or coverage work; this is the immutable baseline identity unless the owner approves migration |
| [OSRS_REFERENCES.md](OSRS_REFERENCES.md) | Protocol, RuneLite compatibility, caches, definitions, mechanics, server patterns, or OSRS research |
| [CLUB_PENGUIN_REFERENCES.md](CLUB_PENGUIN_REFERENCES.md) | Club Penguin behavior, media, eras, minigames, servers, or browser implementations |
| [HYBRID_GAME_DESIGN.md](HYBRID_GAME_DESIGN.md) | Penguin player/editor, Club Penguin content in Gielinor, items, economy, puffles, parties, quests, and minigames |
| [BLENDER_ASSET_GUIDE.md](BLENDER_ASSET_GUIDE.md) | Penguin bodies, equipment fitting, environments, animation, Blender, export, or visual validation |
| [RUNELITE_COMPATIBILITY.md](RUNELITE_COMPATIBILITY.md) | Testing the shim, client/cache delivery, penguin appearance, interfaces, events, menu actions, and plugin support |
| [MILESTONE_ROADMAP.md](MILESTONE_ROADMAP.md) | Proposing the next bounded milestone after the current execution has stopped |
| [PARALLEL_EXECUTION.md](PARALLEL_EXECUTION.md) | Decomposing an active milestone, creating task records, freezing contracts, assigning concurrent work, reviewing, or integrating branches |
| [VERSION_AND_COVERAGE_TRACKING.md](VERSION_AND_COVERAGE_TRACKING.md) | Current revision, repository/cache pins, source confidence, upgrade diffs, or completeness ledgers |

Agents should always load this file, `PRODUCT_DECISIONS.md`, and `FROZEN_BASELINE.md`, then load only the specialist documents needed for the task. Coordinators, integrators, reviewers, and agents receiving parallel implementation work must also load `PARALLEL_EXECUTION.md`. This README defines the product identity; the decisions file determines whether a design statement is binding; the baseline file fixes the OSRS artifact family and visual target.

## Decision labels

Reference files contain facts, requirements, proposals, and unknowns. Apply these labels when a statement could affect implementation:

| Label | Meaning | Agent action |
|---|---|---|
| `REQUIRED` | Explicit owner requirement or fixed project constraint | Implement within the active milestone when applicable |
| `APPROVED` | Specific design choice accepted for implementation | Treat as binding until superseded |
| `PROPOSED` | Recommended design that has not been accepted as a product decision | Use for planning and prototypes; do not silently freeze it |
| `UNRESOLVED` | Evidence or product direction is incomplete | Run a bounded investigation and record the result |
| `VERIFIED` | Reproduced or established against a named baseline/artifact | May support implementation and tests within that scope |

Technical research files may describe recommended use without repeating a label on every sentence. When a recommendation changes player-facing behavior, economy, scope, world identity, or client support, `PRODUCT_DECISIONS.md` is authoritative.

## Evidence classes

Never treat all references as equivalent.

| Evidence class | Answers | Does not establish |
|---|---|---|
| Current wire/client | What the selected live revision sends, stores, and renders | Complete gameplay rules |
| Cache/data inventory | Which client-visible definitions, assets, and IDs exist | Server logic, formulas, or completion |
| Modern mechanics implementation | How another current-ish server implemented a behavior | That its behavior exactly matches live OSRS |
| Engine/content pattern | How mature projects organize scripting, transactions, combat, persistence, and tools | Modern revision accuracy |
| Historical behavior | How older RuneScape versions solved comparable systems | That modern OSRS retained the rule |
| Community documentation | Player-facing behavior and change history | Hidden implementation details |
| Direct observation | Reproducible behavior seen on a specific client/cache/revision | Unobserved branches or internal design |
| ClubScape decision | An intentional rule for the hybrid game | A claim about either source game |

## Confidence labels

Attach one of these labels to research conclusions and generated specifications:

| Label | Meaning |
|---|---|
| `confirmed-current` | Supported by the selected cache/protocol artifact or an official current source |
| `observed-current` | Reproduced directly on the selected live/client baseline |
| `community-documented` | Supported by OSRS Wiki, Club Penguin archives, or comparable community documentation |
| `source-implemented` | Present in a reference repository but not independently verified |
| `historical` | Accurate only for a named historical revision or era |
| `inferred` | Best explanation from incomplete or conflicting evidence |
| `clubscape-decision` | Deliberate hybrid behavior authored for this game |

A repository's popularity does not increase a conclusion's confidence label. Stars help prioritize inspection; they do not prove accuracy.

## Source precedence

For OSRS work:

1. Start with the exact pinned cache and protocol/client artifacts for encoded facts.
2. Use OSRS DB to enumerate definitions and seed coverage.
3. Use official OSRS updates and OSRS Wiki for rules and change history.
4. Use RuneLite, RSProt, RSProx, modern server implementations, and permitted observation to reconcile runtime behavior.
5. Use Void, Lost City, and other historical engines for implementation patterns after labeling their eras.

For Club Penguin work:

1. Name the client family and era before comparing behavior.
2. Prefer preserved media/direct observation for the original loop and presentation.
3. Use Houdini, Snowflake, Yukon, and other implementations as hypotheses and engineering references.
4. Record the ClubScape adaptation separately from claims about the original game.

When sources conflict, retain the conflict. Do not silently choose whichever value is easiest to parse.

## Core instructions for every implementation agent

1. Treat ClubScape as OSRS with a Club Penguin spin, not a 50/50 merger.
2. Keep players penguins in every system and client.
3. Preserve all OSRS item and mechanic semantics unless a difference is explicitly recorded.
4. Track semantic completion separately from penguin visual completion.
5. Use the source appropriate to the evidence class; no repository is complete truth.
6. Build original 3D assets that match the frozen OSRS revision and validate the same canonical visual design in both clients.
7. Integrate Club Penguin items and rewards into the single OSRS economy.
8. Make minigames authoritative server activities with exact-once rewards.
9. Require both RuneLite and WASM validation for client-visible work.
10. Never claim completion from decoded definitions, repository README claims, or revision compatibility alone.

## Research boundary

The repositories in this pack were inspected but not all built or independently parity-tested. Dates, configurations, README claims, generated reports, and author checklists are evidence—not warranties. Exact operational pins and coverage status belong in [VERSION_AND_COVERAGE_TRACKING.md](VERSION_AND_COVERAGE_TRACKING.md).
