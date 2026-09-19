# Hybrid Game Design

This file defines how Club Penguin content fits into OSRS. It is a product/content reference, not the server architecture or milestone plan.

Most concrete mappings in this file are `PROPOSED` design defaults. Only entries recorded as `REQUIRED` or `APPROVED` in [PRODUCT_DECISIONS.md](PRODUCT_DECISIONS.md) are binding. Preserve OSRS content identity when a mapping remains unresolved.

## Governing principle

The game is OSRS first:

- Gielinor remains the primary world.
- OSRS skills, combat, quests, items, progression, transport, banks, shops, equipment, trading, and interfaces remain foundational.
- Club Penguin changes the player identity, art direction, tone, social activities, special locations, minigames, events, and additional content.
- New Club Penguin-themed systems integrate through existing OSRS concepts whenever that mapping is natural.
- An explicit extension is preferable to a misleading reskin when no OSRS concept fits.

## The penguin player

The penguin is the actual player body, not a cosmetic NPC layered over a human.

- Appearance, chatheads, shadows, hitsplats, overheads, movement, combat, skilling, emotes, cutscenes, and minigames use the penguin.
- Preserve the ordinary OSRS gameplay footprint and pathing unless a recorded mechanic requires otherwise.
- Equipment retains OSRS identities, stats, requirements, effects, actions, and variants.
- Worn models are redesigned to read correctly on a short, wide penguin body.
- Temporary minigame uniforms, vehicles, or animation sets must restore the prior appearance and equipment exactly.
- All players use one canonical penguin skeleton, body proportions, equipment envelope, sockets, seams, collision footprint, and animation basis.

## The OSRS editor edits the penguin

Retain the familiar player-editor interaction and protocol shape where practical. The underlying choices become penguin kits and palettes.

| Existing appearance concept | ClubScape interpretation |
|---|---|
| Body type / sex | Canonical penguin presentation set; never a human body or a different skeleton/proportion set |
| Head kit | Head and crest shape |
| Jaw kit | Beak, face marking, or base face accessory |
| Torso kit | Body and belly pattern |
| Arms kit | Flipper shape |
| Hands kit | Flipper-tip treatment or accessory base |
| Legs kit | Lower-body or garment base |
| Feet kit | Penguin foot shape |
| Hair color | Feather/body color |
| Torso color | Belly or primary apparel color |
| Legs color | Secondary apparel/accent color |
| Feet color | Foot color |
| Skin color | Beak/face color |

This mapping must be implemented consistently across kit definitions, palettes, labels, preview scripts, serialization, player composition, cache IDs, RuneLite API exposure, and WASM rendering. Stock labels need not remain if they would make the UI confusing.

Editor options may change body color, approved markings and facial treatments, and modular head, beak, belly, flipper, or foot parts only when they remain inside the canonical fit envelope and preserve every equipment seam and socket. Clothing is equipped inventory content, not a second appearance inventory hidden inside the editor.

## OSRS item completeness on a penguin

Every OSRS item has two independent completion dimensions:

1. **Semantic:** name, variants, inventory/world model, actions, stackability, slot, requirements, bonuses, effects, charges, degradation, transformation, set interactions, creation, sources, shops, drops, quests, death, trade, and Ironman behavior.
2. **Penguin visual:** reviewed worn model, sockets, hidden body parts, recolors, animation interaction, clipping, chathead effects, and matching RuneLite/WASM output.

Non-wearables normally retain OSRS visual identity. Wearables require adaptation rather than replacement with generic Club Penguin clothing.

| Slot | Penguin-specific concerns |
|---|---|
| Head | Beak clearance, crest, hoods, full masks, chatheads |
| Cape/back | Short torso, flipper motion, alpha/priority, run clipping |
| Neck | Belly placement and torso-armor occlusion |
| Weapon | Flipper socket, reach, two-handed stance, projectile/effect origin |
| Shield | Wide-body clearance and attack/block animation |
| Torso | Belly volume, flipper seams, hidden base parts |
| Hands | Gloves become flipper coverings while preserving identity |
| Legs | Robe/trouser silhouette and locomotion clipping |
| Feet | Original-item readability on webbed feet |

Use parameterized families—metal armor, robes, capes, boots, masks, bows, staves, shields, and melee weapons—while retaining per-item exceptions.

## Club Penguin content categories

Club Penguin-derived rewards are new ClubScape items with non-conflicting IDs and complete OSRS-style definitions. The category mappings below are proposed defaults unless promoted in the decision register.

| Club Penguin category | ClubScape integration |
|---|---|
| Clothing | Equipment or cosmetics in normal inventory, bank, trade, market, death, and equipment systems |
| Pins | Server-authoritative, untradeable accomplishment collectibles shown in the pin collection, player card, Collection Log views, and igloo/POH displays; no equipment slot or bank space |
| Backgrounds/player-card art | Profile, POH display, collection reward, or UI cosmetic; no combat effect unless explicitly designed |
| Puffles | Permanent account-bound followers with adoption, summon, dismiss, care, safe return/reclaim, housing, and collection treatment; the companion itself is not tradeable inventory |
| Puffle equipment | Pet cosmetic or interaction item; ordinary item rules still apply |
| Furniture | Construction/POH object kits, flat-pack-like items, or world decorations with explicit placement rules |
| Food/drinks | Consumables using OSRS eating/drinking delays, healing, boosts, inventory, shops, and production rules |
| Tools | Skilling items, minigame tools, or cosmetic overrides with explicit requirements and effects |
| Card-Jitsu cards | Ordinary cards are tradeable collectible items; specially earned progression cards are account-bound, and deck use does not consume either type |
| Stamps | Account achievements that may unlock integrated items; do not replace diaries, quests, or collection log silently |
| Party items | Seasonal drops, shops, event rewards, cosmetics, emotes, or POH decorations |
| Vehicles | Temporary activity state, transport interface, mount-like visual, or world object—not a separate avatar account |
| Coins | Settle as OSRS coins, item `995`; Club Penguin presentation may be used without a second permanent currency |

Every new item must declare tradability, Grand Exchange eligibility, shop values, high/low alchemy values, death behavior, reclaim behavior, sources, sinks, rarity, requirements, equipment slot, bonuses/effects, collection-log treatment, and Ironman rules.

### Catalogs and availability

Use rotating featured catalogs to preserve seasonal discovery, themed presentation, and a changing storefront. After a featured window ends, ordinary catalog clothing moves into a permanent archive rather than becoming permanently unobtainable. Ordinary catalog clothing is tradeable by default and uses OSRS coins, shops, bank, trade, market, death, and reclaim rules.

Quest, activity, Card-Jitsu rank, stamp, and live-event rewards do not enter the archive automatically. Their definitions control binding, replay availability, and exclusivity. The archive is an ordinary-clothing availability mechanism, not a way to bypass earned progression or scheduled-event reward rules.

## One integrated economy

- There is one main coin economy. Club Penguin coin rewards mint OSRS coins (`995`) unless a deliberately scoped token is necessary for a specific reward shop.
- A scoped minigame token is an OSRS item with bank, trade, death, source, sink, and reclaim rules—not a hidden second wallet.
- Prefer existing OSRS items when the reward naturally maps. Ice Fishing should yield real fish IDs and Fishing XP.
- New themed items may be tradeable, consumable, equippable, displayed, or untradeable according to their individual designs.
- Untradeable items still participate in the economy through sources, opportunity cost, storage, loss/reclaim, requirements, and sinks.
- Reward rates must be compared with OSRS money-making, XP, supplies, clues, pets, collection log, and equipment progression.
- Rebalance Club Penguin prices and payouts around the OSRS economy while preserving relative Club Penguin rarity and prestige; do not copy raw Club Penguin coin values directly.
- Do not let low-risk minigames create unlimited skilling resources or coins.
- Treat Club Penguin-derived rewards as cosmetic, collectible, social, or ordinary tradeable goods by default. Any combat, skilling, traversal, storage, loot, requirement, or progression effect requires an individually approved balance decision.
- Reward settlement must be server-authoritative and exactly once across retries, disconnects, and reconnects.

## World integration

Club Penguin content should make Gielinor feel playfully transformed rather than replaced.

### Locations

Expand the frozen baseline's existing Iceberg and penguin region into the primary Club Penguin settlement and social hub, then distribute suitable Club Penguin venues, rooms, and activity entrances across existing OSRS regions. The Iceberg hub is an additive extension, not a replacement: preserve the baseline geometry, Cold War storyline, NPCs, quest states, travel, agility-course behavior, and other existing content. Record the expanded hub boundaries and every distributed mapping in the content inventory. The hub is not a disconnected second game.

The added social district is publicly reachable without Cold War completion through a new transport route or landing point. It must remain topologically and logically outside every original quest-gated area. Baseline Iceberg scenes, NPC interactions, routes, shortcuts, objects, and rewards continue to check their original quest state; new travel must never land inside or beyond a gated boundary. A player who has not started Cold War can use the Club Penguin hub without seeing or advancing protected quest content.

Use these integration patterns:

| Pattern | Use |
|---|---|
| Adapted room inside an OSRS settlement | Pizza Parlor in or near the Cooking Guild; Dance Club in a city entertainment district |
| Themed sub-area | Dojo in a mountain region; ski village in a snow region; puffle habitat near an appropriate biome |
| Instanced activity | Sled races, tactical Card-Jitsu Snow, missions, or bespoke-camera arcade games |
| Seasonal overlay | Parties, decorations, temporary NPCs, quests, shops, and activities added to existing towns |

Club Penguin room silhouettes, signs, colors, and props can remain recognizable, but terrain scale, collision, entrances, pathing, clickboxes, object actions, and camera behavior must follow the OSRS world.

### Igloos and Construction

There is one player home rather than parallel POH and igloo systems. The igloo is ClubScape's presentation of the OSRS Player-Owned House and shares its Construction level, requirements, costs, rooms, hotspots, object behavior, storage, portals, servants, persistence, permissions, and visiting rules. Club Penguin furniture and decorating extend the same placement and economy contracts.

### Characters

Recognizable Club Penguin characters can become penguin NPCs with OSRS-style definitions, dialogue, quests, shops, combat or non-combat roles, schedules, and examine text. Their exact locations and roles require content decisions.

Possible roles:

- Rockhopper as a traveling merchant, sailor, transport contact, or quest NPC.
- Gary as an inventor tied to quests, construction, crafting, or instanced gadgets.
- Aunt Arctic as a newspaper, rumor, tutorial, or social-event contact.
- Sensei as the Card-Jitsu progression master.
- Rookie as comic quest support whose mistakes interact with actual world state.
- Herbert and Klutzy as quest, mission, or event antagonists.

### Puffles

Puffles use OSRS follower and housing grammar while preserving the full care identity:

- ownership and variant ID;
- individual name and personality state;
- adoption and igloo residence;
- food, rest, play, cleanliness, and other source-supported care needs;
- direct care interactions and visible state feedback;
- summon/dismiss/follow/teleport behavior;
- reclaim and loss rules;
- movement and obstacle behavior;
- interaction/emote animations;
- participation in compatible original puffle activities and minigames;
- no combat, skilling, storage, traversal, loot, or progression advantage without individual approval;
- neglected puffles may become unhappy or inactive and return to the player's igloo until cared for;
- ownership is permanent: neglect never deletes the puffle or requires re-adoption;
- normal visibility and synchronization in both clients.

Adoption may charge ordinary OSRS coins as an explicit gold sink. Once adopted, the puffle is a permanent account-bound companion: it cannot be traded, sold, dropped, lost on death, or transferred to another account. Puffle food, care supplies, equipment, and cosmetics remain ordinary tradeable items by default, with individual source, sink, death, reclaim, and Ironman rules.

### Quests and missions

Preserve recognizable classic mission characters, mysteries, puzzles, and outcomes while adapting travel, locations, items, and dialogue enough to fit coherently inside Gielinor. PSA/EPF-style missions are first-class quests using the shared journal, quest engine, map markers, requirement panels, dialogue, state machine, puzzle, instancing, inventory, cutscene, and reward machinery. The journal may filter them as Penguin Missions, but they are not a separate progression silo.

Assign each mission unified Quest Points using OSRS-style length, difficulty, and requirement conventions. Explicit OSRS quest prerequisites remain intact. Inventory every total-Quest-Point consumer and rebalance it deliberately; generic Quest Point gates count the unified total, while a specifically required quest cannot be replaced by unrelated Penguin missions. The Quest Point Cape ultimately requires all persistent OSRS quests and Penguin missions in the supported release. Coverage reporting keeps baseline OSRS quests and Penguin missions distinguishable even though the player has one Quest Point total.

### Parties and holidays

Run classic Club Penguin parties on a calendar as scheduled world events with decorations, temporary objects, NPC dialogue, emotes, shops, activities, collection entries, and small event stories. Provide a controlled archive/replay route so every required classic party remains testable and eventually accessible outside its live window. Archive replay supports stories, activities, collection progress, and appropriate stamps; repeatable economic rewards must be capped or disabled, and selected live-event rewards may remain exclusive to the scheduled window. Specify the exact rules per party and reward. Avoid permanently duplicating OSRS holiday rewards unless the new reward is clearly distinct.

### Stamps

Preserve a dedicated Club Penguin stamp book with activity-specific categories, criteria, difficulty, completion state, and rewards. It remains distinct from OSRS Achievement Diaries and the Collection Log, while relevant activities and accomplishments may link between their interfaces. Each stamp must have one server-authoritative completion event and stable identifier; cross-links are alternate views of that event, not separate awards.

When a stamp definition is added or migrated, award prior completion only from authoritative evidence. Durable event history can prove event-based criteria; current account state can prove genuinely state-based criteria. Do not infer missing counts, timing, score, difficulty, party composition, or other historical conditions. Where the exact criterion cannot be proven, require a fresh completion. Store a migration version and settlement key so reevaluation cannot duplicate a stamp or its reward.

### Pins and player cards

Missions, minigames, exploration, stamps, parties, and other accomplishments may award pins. Earned pins are server-authoritative and untradeable. They live in a dedicated pin collection rather than inventory or bank space, appear in relevant Collection Log views, can be displayed on a Club Penguin-style player card, and can be placed on an igloo/POH pin board. Displayed pins link to their source activity or requirement and never consume an OSRS equipment slot.

The player card is the social presentation of existing authoritative account state, not a second character sheet. It may show the penguin's current outfit, selected background, displayed pins, and approved social or completion details without exposing private data or replacing OSRS equipment, stats, examine behavior, or moderation controls.

### Social and chat

Preserve OSRS free chat, private messages, friends, ignore, clans, reporting, moderation, and privacy behavior. Add optional Club Penguin-style quick-chat phrases, emotes, and social actions on top of the same authoritative social graph and enforcement system. RuneLite and the browser expose equivalent capabilities and moderation outcomes.

### Era resolution

When original browser Club Penguin rooms, activities, items, interfaces, or rules changed during 2005–2017, use the latest stable original-browser version as the normal-world default. Preserve materially distinct earlier versions through parties, archives, missions, or instances. Record the selected era, source artifacts, differences, and availability route in the content inventory.

### Card-Jitsu

Card-Jitsu is a separate activity progression family spanning the original game, Fire, Water, and Snow. Preserve collectible cards and deck construction, matchmaking, belts, ranks, ninja progression, and each variant's recognizable rules. Sensei and the Dojo anchor the progression in the world.

Rewards are primarily cosmetic, social, or collection-oriented. Card-Jitsu does not require or award OSRS combat or Magic levels, XP, equipment power, skilling efficiency, or requirement bypasses unless an individually approved crossover defines the exact effect. Cards, acquisition sources, ownership, deck rules, and trading behavior still require explicit economy contracts.

Use hybrid binding. Ordinary collectible cards are tradeable using OSRS inventory, bank, trade, and market rules, and playing them in a deck does not consume them. Belts, ranks, ninja status, rank rewards, and specially earned progression cards are account-bound. Buying ordinary cards may broaden deck choices but cannot buy ranks or satisfy earned progression gates. Each card family still needs explicit sources, duplicate behavior, ownership and deck-validation rules, storage and loss behavior, and economic sinks.

## Minigame adaptation rules

- Preserve the recognizable input loop, pacing, scoring idea, failure condition, and audiovisual identity where practical.
- Connect entry, exit, inventory, XP, rewards, requirements, and progression to OSRS.
- Award OSRS skill XP only where actual minigame play directly exercises that skill. Use a specified, simulated rate below comparable dedicated OSRS training by default; do not add thematic XP merely because a loose association exists.
- Bespoke cameras, boards, lanes, rhythm prompts, or vehicle controls are allowed. Not every game needs ordinary tile walking.
- The Rust server owns legal inputs, RNG, simulation, score, completion, and rewards.
- RuneLite and WASM must present the same authoritative state even if their rendering implementations differ.
- Each activity must define reconnect, timeout, abandon, spectator, party, and duplicate-settlement behavior.
- Re-create recognizable Club Penguin music and sound identities within the frozen OSRS revision's audio style and constraints; do not insert source audio unchanged as the default runtime asset.

## Minigame catalog and recommended role

The approved default is faithful core gameplay: preserve each activity's recognizable original inputs, rules, timing, scoring, and win/failure loop. ClubScape integration adds authoritative simulation, world placement, accounts, persistence, multiplayer, rewards, economy rules, security, and target-specific presentation. Material deviations require a per-minigame decision.

This catalog remains a set of proposed placements and system connections, not a claim that every original rule has been recovered. The complete classic roster must be enumerated, and each game's recovered source behavior and adaptations must be specified before bulk implementation.

| Club Penguin activity | Recommended ClubScape form | OSRS connections |
|---|---|---|
| Aqua Grabber | Underwater collection instance | Agility, Fishing, oxygen, salvage, transport |
| Astro Barrier | Arcade/POH cabinet or repeatable challenge | Score, waves, collection reward; little forced skill integration |
| Bean Counters | Dock or warehouse job | Carried-object state, timed delivery, coins; optional Agility |
| Bits & Bolts | Invention-themed puzzle activity | Crafting/Smithing requirements or quest puzzle |
| Cart Surfer | Mine-cart course | Agility/Mining hooks, tricks, transport, instanced rails |
| Catchin' Waves | Coastal surfing course | Agility-like activity, tricks, seasonal competition |
| Card-Jitsu | Dojo board game | Deck collection, matchmaking, ranks, cosmetic/equipment rewards |
| Card-Jitsu Fire | Competitive board/route instance | Matchmaking, elemental deck strategy, rank progression |
| Card-Jitsu Water | Competitive lane/board instance | Matchmaking, tile hazards, elemental deck strategy |
| Card-Jitsu Snow | Cooperative tactical instance | Party roles, targeting, powers, encounters, Tusk boss |
| Dance Contest | Rhythm interface in a city club | Difficulty, score, emotes, social competition |
| DJ3K | Music-mixing activity | Music tracks, emotes, score; POH/venue integration |
| Find Four | Tavern/park board object | Casual PvP board game, wagers only if safely designed |
| Hydro Hopper | Boat-towed obstacle course | Agility, tricks, coastal transport/activity |
| Ice Fishing | Fishing spot or Guild activity | Fishing level, equipment, real fish, XP, inventory |
| Ice Hockey | Snow-region team activity | Multiplayer movement, teams, goals, cosmetic rewards |
| Jet Pack Adventure | Gnome/dwarven flight course | Agility, fuel, regions, checkpoints, collectibles |
| Mancala | Tavern/Dojo board object | Casual PvP board game |
| Pizzatron 3000 | Cooking Guild/restaurant workstation | Cooking, ingredients, order accuracy, real food items |
| Puffle Launch | Puffle-focused traversal instance | Pet ownership, launch physics, collectible rewards |
| Puffle Rescue | Rescue puzzle/instance | Agility, hazards, puffle unlocks, quest or activity |
| Pufflescape | Puffle puzzle course | Pet interaction, puzzles, cosmetic rewards |
| Puffle Roundup | Herding activity | Puffle collection/training, timed score, ranch location |
| Sled Racing | Snow-region race instance | Multiplayer start/results, movement, seasonal leaderboards |
| Smoothie Smash | Cooking/restaurant activity | Cooking, fruit/item inputs, orders, combo scoring |
| System Defender | Tower-defense interface or quest activity | Construction/Crafting gadgets, EPF mission line |
| Thin Ice | Arcade puzzle or quest chamber | Grid puzzle, limited moves, collection/stamp goals |
| Treasure Hunt | Two-player board activity | Dig-site/pirate theme, social cooperation, item rewards |

Additional variants and one-off party games should be added to the catalog when their media and rules are identified. Do not force low-information games into production simply to increase a count.

## Per-minigame specification

Every implementation requires:

| Area | Required content |
|---|---|
| Source | Era, SWF/media/build or source commit, hashes, confidence labels |
| Original loop | Inputs, timing, scoring, RNG, rounds, failure/win, player count |
| Adaptation | Preserved, changed, omitted, and newly added behavior |
| World placement | Entrance, NPC/object, area, instance, return point |
| OSRS links | Skills, quests, items, equipment, consumables, requirements |
| State | Lobby, active phases, legal transitions, reconnect, timeout, abandon |
| Presentation | Penguin animations, equipment visibility, camera, UI, audio |
| Rewards | Coins/items/XP, rates, limits, sources/sinks, exact-once settlement |
| Validation | Deterministic simulations plus RuneLite and WASM behavior |

## Hybrid-content decision test

Before adding a Club Penguin feature, answer:

1. Where does it physically or socially belong in Gielinor?
2. Which existing OSRS systems should it use?
3. What recognizable Club Penguin identity must survive?
4. Does it create redundant currency, inventory, progression, or avatars?
5. How does it affect the OSRS economy and Ironman play?
6. Can both clients express it from the same authoritative state?
7. Is the result still recognizably OSRS at ordinary play scale?

If these answers are unclear, the content is not ready for implementation.
