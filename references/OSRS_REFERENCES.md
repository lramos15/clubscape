# OSRS References

Updated: 2026-09-17  
Snapshot window: 2026-09-14 through 2026-09-17

This file ranks references for a clean-room Rust server, Rust/WASM client, RuneLite compatibility shim, complete OSRS content coverage, and OSRS-style custom assets. See [VERSION_AND_COVERAGE_TRACKING.md](VERSION_AND_COVERAGE_TRACKING.md) for the compact pin manifest and refresh procedure.

## Current baseline

Frozen baseline selected from the newest verified stable evidence on 2026-09-17:

- OSRS protocol revision: **240**
- OpenRS2 live cache: **archive/cache id 2710**, dated **2026-09-16 10:30:13**, build **240**
- `abextm/osrs-cache`: release **`2026-09-16-rev240`**, commit **`ec6c640cad14b78b05b74c25e07c2603b085fd14`**
- RuneLite stable release: **`1.12.39`**, release commit **`67d51a4a4a945e6e3f60c75cbc7dd859d441ff04`**; injected-client artifact still needs resolution
- RuneLite GameVals: **`2026-09-16-rev240`**, commit **`0edc8a6bc4a0755d2c1e44de04b88adc8fea10c4`**
- RuneLite CS2 scripts: **`2026-09-16-rev240`**, commit **`baf8a24853e230d024b627f2513cd475bfbd58ba`**
- RSProt advertised module: **`osrs-240-api:1.0.0-ALPHA-20260912`**

This is archive-backed evidence, not a live Jagex handshake. Multiple weekly caches can share revision 240, so the revision number is not a complete content identity.

## Tier 1: current wire, client, scripts, and cache

| Repository | Stars at snapshot | Latest inspected activity | Verified target | Use |
|---|---:|---|---|---|
| [runelite/runelite](https://github.com/runelite/runelite) | 5,457 | 2026-09-16 | Stable `1.12.39` at `67d51a4a4a945e6e3f60c75cbc7dd859d441ff04`; injected-client mapping unresolved | Primary client API/events, interfaces, rendering, cache tooling, and shim reference |
| [blurite/rsprox](https://github.com/blurite/rsprox) | 121 | 2026-09-03 | Protocol modules 223–240 | Capture, decode, replay, compare packets, and patch/configure clients |
| [blurite/rsprot](https://github.com/blurite/rsprot) | 75 | 2026-09-14 | OSRS 221–240 | Best modern public packet and state-update reference found |
| [openrs2/openrs2](https://github.com/openrs2/openrs2) | 54 | 2026-09-05 | Multi-version archive/tooling; includes 240 | Cache formats, archive identity, XTEAs, client artifacts, reproducible inputs |
| [abextm/osrs-cache](https://github.com/abextm/osrs-cache) | 25 | Release generated 2026-09-16 | `2026-09-16-rev240` at `ec6c640cad14b78b05b74c25e07c2603b085fd14` | Convenient release-indexed weekly cache artifacts and revision transitions |
| [runelite/cs2-scripts](https://github.com/runelite/cs2-scripts) | 17 | 2026-09-16 | `2026-09-16-rev240` at `baf8a24853e230d024b627f2513cd475bfbd58ba` | Interfaces, scripts, appearance/editor behavior; requires careful source tracking |

### How to use them

- RuneLite is the strongest general client reference, but its public tree is not the complete underlying OSRS engine. Resolve the exact injected-client dependency used by the selected build.
- RSProt should lead packet/state-update implementation. It is labeled alpha, so verify behavior with fixtures and captures.
- RSProx is observation and differential tooling, not gameplay logic.
- OpenRS2 provides durable identities and archive comparison, not hidden server rules.
- `abextm/osrs-cache` makes weekly cache transitions easy to enumerate. Its inspected releases remain revision 240 from August 5 through September 16 after revision 239 on July 15, 22, and 29.
- Decompiled CS2 scripts are separately tracked research inputs; RuneLite's repository terms do not automatically describe every input artifact.

## Tier 1: machine-readable data and Rust cache tooling

| Repository | Stars at snapshot | Activity/version signal | Use | Limitation |
|---|---:|---|---|---|
| [wvanderp/osrs-db](https://github.com/wvanderp/osrs-db) | Recheck | 256 commits inspected; package version embeds cache number | Machine-readable items, NPCs, objects, quests, slot stats, object locations, JSON schemas, and cache number | Aggregates RuneLite, OSRSBox, and OSRS Wiki; validate generated fields |
| [osrs-rs/rs-cache](https://github.com/osrs-rs/rs-cache) | 0 under current organization | 308 commits; crate 0.8.4; included OSRS test cache is 180 | Rust cache reading, definitions, update/checksum data, compression, Huffman, ISAAC, mmap, serde | Experimental, incomplete, read-only, and its bundled cache is old |
| [osrsbox/osrsbox-db](https://github.com/osrsbox/osrsbox-db) | 243 | Last activity observed in 2022 | Historical schema and build-pipeline reference for items, monsters, prayers, icons, models, cache/Wiki aggregation | Stale; do not accept its “up-to-date” wording as current truth |
| [Displee/rs-cache-library](https://github.com/Displee/rs-cache-library) | Recheck | Pin exact head before adoption | Cache read/write/manipulation comparison | Broad format support is not proof of correct custom-model round trips |

Use OSRS DB to seed content ledgers, then reconcile against the exact cache and documented behavior. Language match makes `rs-cache` useful to the Rust implementation, but does not make its decoder authoritative.

## Tier 1 and 2: mechanics and content implementations

| Repository | Stars at snapshot | Latest inspected activity | Target | Assessment |
|---|---:|---|---|---|
| [rsmod/rsmod](https://github.com/rsmod/rsmod) | 178 | 2025-10-01 | RSProt OSRS 233 | Strong modern mechanics and engine reference; not revision-240 packet truth |
| [GregHib/void](https://github.com/GregHib/void) | ~170 | Repository updated 2026-09-15; release 2.9.0 on 2026-07-12 | RuneScape revision 634, January 2011 | High-value server systems, tooling, documentation, and content-authoring reference |
| [AlterRSPS/Alter](https://github.com/AlterRSPS/Alter) | 81 | 2025-10-26 | README 228; config 228.2 | Alternative plugin/content organization and behavior comparison |
| [Guthix/OldScape](https://github.com/Guthix/OldScape) | 52 | 2025-07-06 | README advertises 189 | Historical secondary implementation |
| [OpenRune/OpenRune-Server](https://github.com/OpenRune/OpenRune-Server) | 28 | 2026-09-14 | Config 240.2; RSProt 240 | Most current-revision full-server candidate found, but incomplete |

### Void

Void is a primary **engine/content-system** reference even though it is not modern OSRS truth. Its documentation covers:

- string IDs and definitions;
- event handlers and scripting;
- entities and movement modes;
- inventory transactions and shops;
- queues and task timing;
- combat lifecycle;
- drops, instances, charges, and degradation;
- bots and automated players;
- persistence;
- cache/client builds;
- definition browsing and developer tooling;
- an experimental web-client path.

Its target revision predates OSRS. Do not import modern items, interfaces, packets, bosses, raids, or formulas from it without current evidence. The inspected release was 2.9.0 with abbreviated commit `65d6290`.

### OpenRune completeness warning

Its own generated report listed skills **17/23**, bosses **11/169**, raids **0/4**, and minigames **0/51**. Those are author-defined detection categories rather than an independent audit and do not comprehensively cover quests, transport, interfaces, social systems, or item effects. Revision compatibility is not gameplay completeness.

### Comparative-use rule

RS Mod, Alter, OpenRune, Void, and historical projects may share ideas or ancestry. Agreement between them is not automatically independent confirmation. Convert behavior into a written specification, label the evidence, and test it.

## Browser, RuneLite-plugin, and rendering comparisons

| Repository | Stars at snapshot | Activity signal | Role |
|---|---:|---|---|
| [chsami/Microbot](https://github.com/chsami/Microbot) | 208 | RuneLite 1.12.38 integration inspected | Supplementary RuneLite entity/query/interaction patterns |
| [dennisdev/rs-map-viewer](https://github.com/dennisdev/rs-map-viewer) | 148 | Updated 2026-08-03; explicit OSRS 238 work in May 2026 | Browser map/model decoding and rendering comparison |
| [xrsps/xrsps-typescript](https://github.com/xrsps/xrsps-typescript) | 37 | `osrs-237_2026-03-25`; activity 2026-08-19 | Web client/interface comparison, not the Rust implementation |
| [runelite/plugin-hub](https://github.com/runelite/plugin-hub) | Recheck | Active registry; over 15,000 commits inspected | Discovery index for model, cache, development, and compatibility plugins |
| [117HD/RLHD](https://github.com/117HD/RLHD) | Recheck | Active RuneLite GPU renderer | Optional comparison for scene upload, shaders, materials, lighting, and GPU constraints |

Plugin Hub is a registry rather than implementation authority. Inspect and record the actual external plugin repository. RLHD is useful for rendering research but is not the stock OSRS visual target.

## Lost City split repositories

| Repository | Stars at snapshot | Target signal | Best use |
|---|---:|---|---|
| [LostCityRS/Engine-TS](https://github.com/LostCityRS/Engine-TS) | 35 | Early RS2; branch 274; 1,945 commits inspected | Cycle behavior, data tooling, compatible protocol, hot-reload workflow |
| [LostCityRS/Content](https://github.com/LostCityRS/Content) | 29 | Must match engine branch; 2,205 commits inspected | Script/config packs, maps, models, sprites, textures, and deterministic repacking |
| [LostCityRS/Server](https://github.com/LostCityRS/Server) | 267 | Setup/umbrella repository | Resolving and launching matching engine, content, and client branches |

Lost City explicitly requires matching engine and content branches. Its November 2004 target is historical, but its separation of engine from content and automatic repacking are strong references for large-scale agent-authored content.

## Historical and lower-priority references

| Repository | Stars at snapshot | Activity | Reason to keep |
|---|---:|---|---|
| [2004Scape/Server](https://github.com/2004Scape/Server) | 386 | Archived/migrated; inspected 2025-10-14 | Historical ecosystem and migration trail |
| [open-osrs/runelite](https://github.com/open-osrs/runelite) | 325 | Archived 2022-06-28 | Old RuneLite fork comparison only |
| [runejs/server](https://github.com/runejs/server) | 299 | 2026-04-12 | Active historical RS2 revision-435 engine |
| [apollo-rsps/apollo](https://github.com/apollo-rsps/apollo) | 194 | 2020-05-21 | Server architecture history |
| [jbx5/devious-client](https://github.com/jbx5/devious-client) | 83 | 2024-06-27 | Client history around revision 223 |
| [MeteorLite/meteor-client](https://github.com/MeteorLite/meteor-client) | 40 | 2023-09-29 | Client history around revision 217 |
| [RustCityRS/rs-majula](https://github.com/RustCityRS/rs-majula) | 10 | 2026-08-07 | Rust structure comparison with historical RS2 lineage |

Do not add every historical emulator to the primary reading list. Void and Lost City are promoted because their documentation, content workflow, or ongoing maintenance adds distinct value.

## Non-Git sources required for OSRS completeness

| Source | Use | Evidence rule |
|---|---|---|
| [Old School RuneScape Wiki](https://oldschool.runescape.wiki/) | Items, NPCs, quests, drops, mechanics, activities, update history, and edge cases | Record page revision/timestamp and corroborate critical formulas or ambiguity |
| [Official OSRS news](https://secure.runescape.com/m=news/archive?oldschool=1) | Release dates, intended changes, hotfixes, and new-content deltas | Authoritative for announced intent, not necessarily every resulting implementation detail |
| Permitted direct observation/capture | Timing, UI transitions, packets, state updates, and edge cases | Record client, cache, revision, date, setup, inputs, and expected outputs as fixtures |

No public emulator, cache, or database establishes complete modern OSRS gameplay by itself.

## Subsystem routing

| Question | First references | Cross-checks |
|---|---|---|
| Packet encoding/state update | RSProt | RSProx captures, RuneLite client behavior |
| Login/cache update | RuneLite, OpenRS2 | RSProt, `rs-cache` |
| Definitions and ID inventory | Pinned cache, OSRS DB | RuneLite dumpers, OSRS Wiki |
| Interface/player editor | CS2 scripts, RuneLite | Captures, cache widgets/configs |
| Combat/skills/content engine | RS Mod, OpenRune | Alter, Void, Wiki, observation |
| Transactions/shops/degradation | Void | RS Mod, Alter, current documentation |
| Agent-friendly content packs | Lost City, Void | RS Mod plugins, project-owned schemas |
| Browser scene/model decode | RuneLite, map viewer | Environment Exporter, WASM fixtures |
| RuneLite extension discovery | Plugin Hub | Actual plugin repositories |
| Enhanced renderer research | RLHD | Stock RuneLite and project visual rules |
