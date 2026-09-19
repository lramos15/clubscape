# Product Decisions and Open Questions

Updated: 2026-09-17

This file separates owner requirements from proposed designs and unresolved research. It prevents an implementation agent from converting a plausible suggestion into a permanent product decision.

## How to use this register

- `REQUIRED` and `APPROVED` entries are binding within applicable milestone scope.
- `PROPOSED` entries may guide research, prototypes, and estimates. Promote them before broad implementation when they materially affect player behavior, economy, world identity, or compatibility.
- `UNRESOLVED` entries require a bounded research, specification, or feasibility result. Independent work continues while they are investigated.
- Routine implementation choices that preserve an approved contract do not need entries here.
- The implementation agent resolves ordinary item, NPC, room, reward, and minigame mappings under the approved contracts. Escalate only conflicts, material deviations, major balance changes, or changes to product identity.

Each future material decision should record an ID, status, decision, rationale, affected areas, acceptance evidence, and superseded decision when applicable.

## Required product contract

| ID | Status | Decision |
|---|---|---|
| `product.osrs_foundation` | `REQUIRED` | ClubScape is OSRS with a Club Penguin spin. OSRS supplies the foundational world, mechanics, items, progression, economy, interfaces, and interaction grammar. |
| `product.full_osrs_scope` | `REQUIRED` | The long-term target includes all player-facing mechanics and items in one frozen OSRS baseline. Smaller releases do not redefine that target. |
| `product.penguin_player` | `REQUIRED` | The player is a penguin across appearance, chatheads, equipment, animations, combat, skilling, cutscenes, emotes, and minigames. |
| `product.penguin_editor` | `REQUIRED` | The OSRS player-editor workflow edits penguin traits, palettes, and compatible clothing rather than a human body. |
| `product.cp_inside_osrs` | `REQUIRED` | Club Penguin content exists in the OSRS world and shared systems rather than as a disconnected game or website. |
| `product.integrated_items` | `REQUIRED` | Club Penguin items and rewards use the OSRS-style inventory, bank, equipment, trade, shop, death, and economy systems applicable to each item. |
| `product.single_currency` | `REQUIRED` | OSRS coins are the main currency. Club Penguin presentation may reference coins, but there is no permanent separate CP coin wallet. |
| `product.rust` | `REQUIRED` | Use a native Rust server and Rust web client compiled to WebAssembly. |
| `product.runelite_goal` | `REQUIRED` | Provide a compatibility route for RuneLite/the underlying OSRS client, with the supported tier established by executable evidence. |
| `product.clean_room` | `REQUIRED` | Implement ClubScape from scratch while consulting authorized and publicly available reference sources. |
| `product.authorized_material` | `REQUIRED` | Treat owner-provided and owner-approved source material as authorized for ClubScape development and deployment. |

## Approved gameplay and delivery direction

| ID | Status | Decision |
|---|---|---|
| `scope.frozen_baseline` | `APPROVED` | Freeze a named OSRS baseline for the completion denominator; handle newer updates as later expansion work. |
| `scope.latest_at_kickoff` | `APPROVED` | Select the newest stable, fully identifiable live OSRS revision available when implementation formally begins; freeze it, and place later OSRS updates in an expansion backlog. |
| `baseline.frozen_2026_09_16` | `APPROVED` | Freeze OSRS revision 240 at the 2026-09-16 persistent live snapshot. Use OpenRS2 cache id 2710, `abextm/osrs-cache` release `2026-09-16-rev240` at `ec6c640cad14b78b05b74c25e07c2603b085fd14`, RuneLite 1.12.39 at `67d51a4a4a945e6e3f60c75cbc7dd859d441ff04`, and matching 2026-09-16 revision-240 data/scripts. Later updates require an approved migration. |
| `scope.cp_classic_all` | `APPROVED` | The long-term Club Penguin denominator is all content from the original browser Club Penguin through its 2017 shutdown. Enumerate it explicitly and deliver it through milestone subsets. Club Penguin Island is excluded from parity scope. |
| `world.preserve_identity` | `APPROVED` | Preserve OSRS names, quests, characters, maps, and stories. Integrate penguin presentation and Club Penguin locations, characters, missions, activities, and rewards additively; targeted adaptations require recorded approval. |
| `world.npc_species` | `APPROVED` | Existing OSRS NPCs retain their established species. Players and Club Penguin characters are penguins; converting an existing OSRS NPC requires a targeted approved adaptation. |
| `assets.visual_style` | `APPROVED` | The frozen OSRS revision defines the canonical art and interface style. New Club Penguin-derived assets must match that revision's geometry density, proportions, silhouettes, face colors/textures, shading, animation cadence, camera-distance readability, and visual rhythm. “Modernized OSRS” is not the target. |
| `assets.cross_client_visuals` | `APPROVED` | The supported default browser and RuneLite configurations use the same canonical geometry, colors/textures, animations, UI art, lighting assumptions, camera composition, and visual identity. Platform rasterization differences are allowed within defined screenshot tolerances; intentional quality tiers, reductions, or substitutions require an owner-approved exception. |
| `minigames.core_fidelity` | `APPROVED` | Preserve each Club Penguin minigame's recognizable original rules, controls, timing, scoring, and win/failure loop by default. Integrate world placement, accounts, persistence, multiplayer, rewards, economy, security, interfaces, and presentation into ClubScape; record material gameplay deviations per game. |
| `delivery.runelite_complete` | `APPROVED` | RuneLite is a complete alternative gameplay client target for the same accounts, world, mechanics, quests, items, progression, Club Penguin activities, and canonical visual design as the browser client. Browser-only gameplay and intentional visual tiers are not the target. |
| `economy.cp_reward_power` | `APPROVED` | Club Penguin-derived clothing, puffle supplies, stamps, and most rewards are cosmetic, collectible, social, or ordinary tradeable goods by default. Adopted puffles themselves are account-bound. Functional combat, skilling, traversal, storage, loot, requirement, or progression effects require individual balance approval. |
| `economy.cp_gold` | `APPROVED` | CP minigames may award ordinary gold through server-authoritative, activity-specific, capped payout rules. Catalog purchases are ordinary gold sinks. |
| `economy.cp_cosmetics` | `APPROVED` | CP clothing is ordinary equipment in existing slots and displaces conflicting equipment. It has no combat bonuses or special utility by default. |
| `economy.low_population` | `APPROVED` | Controlled NPC market liquidity may support a low-population economy, with explicit stocks, budgets, pricing rules, and accounting. |
| `gameplay.group_requirements` | `APPROVED` | Preserve genuine OSRS real-player group requirements. NPC adventurers do not impersonate or replace required players. |
| `gameplay.puffle_power` | `APPROVED` | Puffles are social/cosmetic by default and do not silently add combat, gathering, storage, loot-finding, or core progression benefits. |
| `puffles.full_care` | `APPROVED` | Preserve the puffle-care identity through adoption, individual ownership/personality/variant, food/rest/play/cleanliness-style needs, direct interactions, follower behavior, igloo residence, and compatible original minigame participation. Care mechanics do not grant unapproved OSRS power. |
| `housing.igloos` | `APPROVED` | There is one player home. The igloo is ClubScape's presentation of the OSRS Player-Owned House and uses the same Construction levels, requirements, costs, rooms, hotspots, storage, portals, servants, persistence, and visiting rules. |
| `missions.fidelity` | `APPROVED` | Preserve recognizable classic Club Penguin mission characters, mysteries, puzzles, and outcomes while adapting travel, locations, items, and dialogue enough to fit coherently inside Gielinor. Penguin missions are first-class quests using shared quest machinery and the unified Quest Point system. |
| `delivery.browser_first` | `APPROVED` | The Rust/WASM browser client is the primary delivery path. A separate native desktop client is not an automatic fallback. |
| `delivery.initial_milestone` | `APPROVED` | The default first complete milestone is real sign-up, penguin creation, full Tutorial Island, legitimate Lumbridge arrival, required starter systems, and Cook's Assistant. |
| `scope.persistent_live_game` | `APPROVED` | The frozen OSRS parity denominator covers the persistent live game and its persistent account modes. Leagues, Deadman seasons, temporary events, removed/historical content, inaccessible variants, debug data, and unused records are tracked separately and do not count toward parity. |
| `access.no_membership_gate` | `APPROVED` | ClubScape has no paid membership or members-world entitlement gate. All implemented content is available to accounts without subscription access, while ordinary skill, quest, item, area, and account-mode requirements remain. |
| `gameplay.risk_fidelity` | `APPROVED` | Preserve the frozen baseline's Wilderness, PvP, skull, protection, death, gravestone, reclaim, and item-loss behavior. Penguin presentation and Club Penguin additions do not soften those systems by default. |
| `visual.baseline_default` | `APPROVED` | Use the frozen revision's standard OSRS presentation as the canonical cross-client comparison profile. HD replacement assets and user-installed visual plugins are outside parity screenshots. |
| `parties.rotation_archive` | `APPROVED` | Run classic Club Penguin parties on a calendar and provide a controlled archive/replay route so all required party content remains testable and eventually accessible outside its live window. |
| `world.cp_geography` | `APPROVED` | Build one coherent penguin settlement or district inside Gielinor and distribute suitable Club Penguin venues, rooms, and activity entrances across existing OSRS regions. The hub and distributed content remain part of one world rather than a disconnected island game. |
| `world.iceberg_hub` | `APPROVED` | Expand the frozen baseline's existing Iceberg and penguin region into the primary Club Penguin settlement and social hub. Preserve its OSRS geometry, Cold War storyline, NPCs, quest states, travel, agility-course behavior, and other baseline content; add the new district and venues without replacing or breaking them. |
| `world.iceberg_access` | `APPROVED` | Make the added Iceberg social district publicly reachable without completing Cold War. Preserve every baseline quest gate for original Iceberg areas, scenes, NPC interactions, routes, shortcuts, and rewards; the public route cannot place a player inside or beyond gated Cold War state. |
| `stamps.dedicated_book` | `APPROVED` | Preserve a dedicated Club Penguin stamp book with activity-specific categories, criteria, and completion tracking. Keep it distinct from OSRS Achievement Diaries and the Collection Log, but cross-link relevant activities and accomplishments in their interfaces. Each stamp has one authoritative completion event so cross-links cannot double-award progress or rewards. |
| `stamps.evidence_backfill` | `APPROVED` | Backfill a newly added or migrated stamp only when authoritative stored history proves its exact criterion. Current state may prove a state-based criterion, but missing event counts, timing, difficulty, party composition, or other historical facts are not inferred. Otherwise require fresh completion; migration and reward settlement must be idempotent. |
| `card_jitsu.progression` | `APPROVED` | Preserve Card-Jitsu cards and decks, matchmaking, belts, ranks, and ninja progression as a separate activity system spanning Card-Jitsu, Fire, Water, and Snow. Rewards are primarily cosmetic, social, or collection-oriented; OSRS combat/Magic requirements, XP, equipment power, or other functional crossover require individual approval. |
| `card_jitsu.hybrid_binding` | `APPROVED` | Ordinary collectible cards are tradeable through the OSRS item economy and are not consumed when used in a deck. Belts, ranks, ninja status, rank rewards, and specially earned progression cards are account-bound and cannot be purchased as a substitute for earned progression. Define sources, duplicate handling, deck ownership checks, storage, losses, and sinks per card family. |
| `parties.archive_rewards` | `APPROVED` | Archived parties allow story replay, activities, collection progress, and appropriate stamps. Repeatable economic rewards are capped or disabled, and selected live-event rewards may remain exclusive to the scheduled party window; specify rules per party and reward. |
| `puffles.neglect` | `APPROVED` | Puffle ownership is permanent. Neglect may make a puffle unhappy or inactive and return it to the player's igloo until cared for, but cannot delete ownership or require re-adoption. |
| `appearance.canonical_penguin_rig` | `APPROVED` | Use one canonical penguin skeleton, body proportions, equipment envelope, sockets, seams, collision footprint, and animation basis. The player editor changes palettes, approved markings/facial options, and compatible modular parts within that envelope; it does not create proportion or skeleton variants. Clothing comes through equipment. |
| `catalogs.rotation_archive` | `APPROVED` | Present rotating featured catalogs backed by a permanent archive of ordinary catalog clothing after its featured window. Ordinary catalog clothing is tradeable by default. Quest, activity, rank, stamp, and live-event rewards follow their own binding and availability rules and do not enter the archive automatically. |
| `puffles.account_bound_economy` | `APPROVED` | Adoption may consume ordinary OSRS coins and creates a permanent account-bound puffle that cannot be traded, sold, dropped, lost on death, or transferred. Puffle food, care supplies, equipment, and cosmetics are ordinary tradeable items by default unless individually defined otherwise. |
| `governance.delegated_content_design` | `APPROVED` | The implementation agent resolves ordinary item, NPC, room, reward, character-role, and minigame mappings under the approved product contracts and records them in content specifications. Owner approval is required only for conflicts, material deviations, major balance changes, or changes to product identity. |
| `economy.cp_calibration` | `APPROVED` | Rebalance Club Penguin prices, payouts, sources, and sinks for the OSRS economy while preserving relative Club Penguin rarity and prestige. Raw Club Penguin coin values are reference evidence, not values to copy directly. |
| `minigames.skill_xp` | `APPROVED` | A faithful Club Penguin minigame may award modest OSRS skill XP only when its actual play directly exercises that skill. Rates must remain below comparable dedicated OSRS training by default, be simulated against progression and economy effects, and be specified per activity. |
| `missions.unified_quest_points` | `APPROVED` | Penguin missions are first-class quests in the shared journal and award unified Quest Points according to OSRS-style length, difficulty, and requirements. Preserve explicit OSRS quest prerequisites; inventory and deliberately rebalance every total-Quest-Point consumer so added missions neither accidentally trivialize nor obstruct progression. Generic Quest Point gates count the unified total. |
| `missions.quest_cape` | `APPROVED` | The Quest Point Cape ultimately requires every persistent OSRS quest and Penguin mission in the supported release. Baseline OSRS quest coverage and Penguin mission coverage remain separately visible in parity reporting even though players use one quest system. |
| `pins.integrated_profiles` | `APPROVED` | Missions, minigames, exploration, stamps, parties, and accomplishments may award server-authoritative pins. Earned pins are untradeable collectibles shown in a dedicated collection, the CP-style player card, relevant Collection Log views, and an igloo/POH pin board; they do not consume OSRS equipment slots or bank space. |
| `social.osrs_plus_quick_chat` | `APPROVED` | Preserve OSRS free chat, private messages, friends, ignore, clans, reporting, and moderation while adding optional Club Penguin-style quick-chat phrases, emotes, and social actions. Both clients expose the same authoritative social state and enforcement. |
| `audio.cp_osrs_style` | `APPROVED` | Re-create recognizable Club Penguin musical themes and sound identities within the frozen OSRS revision's audio style and technical constraints. Source audio is reference material rather than unchanged runtime replacement; RuneLite and browser routes present the same canonical audio design. |
| `business.no_monetization` | `APPROVED` | Do not design real-money purchases, subscriptions, paid power, premium currency, or other monetization into the game. Any future funding model is a separate owner decision. |
| `release.first_public_slice` | `APPROVED` | Do not open the first public release until the starter journey passes and one polished Club Penguin vertical slice is complete: the public Iceberg district, puffle adoption/care, one faithful minigame, a small catalog, player cards/pins, and at least one stamp page, plus release operations and supported-client gates. |
| `scope.cp_era_resolution` | `APPROVED` | When original Club Penguin content changed across 2005–2017, use the latest stable original-browser version as the normal-world default. Preserve materially distinct earlier variants through parties, archives, missions, or instances and record the era/source for every selected variant. |

## Proposed defaults

These are content-design and technical starting points. Under delegated content-design authority, the implementation agent may resolve ordinary instances in versioned specifications without asking the owner. A new owner decision is required only when the result conflicts with an approved contract, materially changes balance or world identity, or creates a broad new commitment.

| ID | Status | Proposal |
|---|---|---|
| `characters.roles` | `PROPOSED` | Use recognizable CP characters as OSRS-style NPCs with explicit world roles; example roles in the hybrid guide are illustrative. |
| `items.category_mapping` | `PROPOSED` | Map furniture, food, tools, vehicles, and party items to the OSRS systems suggested in the hybrid guide. Pins and Card-Jitsu binding are governed by their approved decisions instead. |
| `minigames.world_mapping` | `PROPOSED` | Use the locations and skill connections in the hybrid minigame table as starting designs rather than final contracts. |
| `assets.project_exporter` | `PROPOSED` | Use a project-owned deterministic Blender export layer after validating available tools and target formats. |
| `assets.complexity_bands` | `PROPOSED` | Treat the triangle ranges in the Blender guide as initial experiment bands until renderer/cache measurements establish real budgets. |

## Unresolved research and feasibility work

| ID | Status | Investigation | Required output |
|---|---|---|---|
| `scope.cp_inventory` | `UNRESOLVED` | What is the complete enumerated inventory of original browser Club Penguin content through the 2017 shutdown, including variants and source eras? | Named content inventory with era and source |
| `runelite.delivery` | `UNRESOLVED` | Can the required penguin/custom-content experience run through unchanged upstream RuneLite, a custom cache/client target, a plugin, or a maintained fork? | Executable feasibility report and supported tier |
| `runelite.editor` | `UNRESOLVED` | Which player-editor behaviors can retain stock protocol/widgets and which require custom cache, scripts, labels, or client work? | Working editor demonstration and compatibility matrix |
| `visual.reference_pack` | `UNRESOLVED` | What exact OSRS visual/audio configuration and captures define the first milestone? | Approved reference pack and comparison procedure |
| `economy.cp_values` | `UNRESOLVED` | What are the actual CP item prices, reward formulas, caps, trade rules, and sinks? | Economy contracts and simulations per activity/catalog |

## Runtime configuration rather than product decisions

Model names, provider routing, machine capacity, tool availability, worktree layout, and current concurrency are runtime configuration. Store them in the repository's agent/runtime configuration when needed. Choose concurrency dynamically from available capacity, dependency structure, repository contention, and safe independent work.

## Decision template

```text
id:
status: REQUIRED | APPROVED | PROPOSED | UNRESOLVED | SUPERSEDED
decision_or_question:
rationale:
affected_systems:
source_or_owner_reference:
acceptance_evidence:
supersedes:
updated_at:
```
