# RuneLite Compatibility

Updated: 2026-09-17

RuneLite support is a required project goal. The delivery mechanism and achievable support tier remain unresolved until demonstrated against named builds. This file prevents the project from treating a shim-shaped design or compile success as compatibility proof.

## Required outcome

The browser and RuneLite routes connect to the same authoritative ClubScape account and world state. The server remains independent of RuneLite internals.

The approved long-term target is a complete alternative gameplay client: RuneLite must ultimately access the same world, mechanics, quests, items, progression, and Club Penguin activities as the browser client. Its supported default configuration must also present the same canonical geometry, colors/textures, animations, UI art, lighting assumptions, camera composition, and visual identity as the browser route. Platform rasterization differences are measured through cross-renderer screenshot tolerances; intentional reductions, substitutions, or separate quality tiers require an owner-approved exception.

The initial compatibility demonstration must show a real runtime that:

1. Launches through the selected RuneLite/client route.
2. Connects and authenticates with the ClubScape server.
3. Renders the selected slice scene and penguin player from live state.
4. Receives game ticks, local player, NPC, object, ground-item, skill, inventory, equipment, animation, chat, and game-state updates needed by the demonstrated experience.
5. Sends a real menu/world action that the authoritative server validates.
6. Runs at least one named generic overlay or tracker against live state.

Compilation, mocks, disconnected screenshots, Creator's Kit previews, API facades without a rendering client, and browser screenshots inside a RuneLite-shaped window do not satisfy this demonstration.

## Compatibility tiers

| Tier | Required behavior | Evidence |
|---|---|---|
| 0: feasibility | Named client/RuneLite builds launch and reach a controlled integration point | Reproducible launch log and version manifest |
| 1: connected slice | Live login, scene, penguin, ticks, state, menu action, and one generic plugin | Executable end-to-end demonstration and captures |
| 2: generic plugins | A named support list of generic overlays/trackers works | Automated compatibility suite per plugin/build |
| 3: semantic OSRS plugins | Selected OSRS-aware plugins work through stable ID/semantic mappings | Named plugin tests and documented exceptions |
| 4: complete gameplay and visuals | All required OSRS and Club Penguin gameplay is accessible and the canonical visual corpus passes cross-client tolerances | Full coverage registry, end-to-end suites, visual corpus, support matrix, and regression policy |

Do not state “RuneLite compatible” without the tier, runtime version, underlying client/cache identity, supported plugin versions, and known limitations.

## Delivery approaches to test

These are investigation paths, not assumed solutions:

| Approach | Current state | Central question |
|---|---|---|
| Upstream RuneLite plus stock underlying client/cache | `UNRESOLVED` and unlikely to display all custom content unchanged | Can custom penguin/content data be delivered without changing the client/cache route? |
| Upstream RuneLite around a compatible custom underlying client/cache | `UNRESOLVED` | Can RuneLite's loading/injection/API expectations be met with a maintained target artifact? |
| Upstream RuneLite plus a ClubScape plugin/bridge | `UNRESOLVED` | How much scene, model, event, menu, and interface behavior can a plugin safely supply? |
| Minimal maintained RuneLite/client fork | `UNRESOLVED` | What changes are required, and what is their ongoing rebasing cost? |
| External shim translating ClubScape state to expected protocol/client state | `UNRESOLVED` | Which differences can be translated without changing rendering/cache/interface behavior? |

Test the cheapest decisive experiment for each approach. Retain failed results and diagnostics so agents do not repeat the same attempt without new evidence.

## Capability matrix

Every row needs one of: `not_tested`, `partial`, `demonstrated`, `blocked`, `requires_change`, or `deferred`.

| Capability | Current status | Proof required |
|---|---|---|
| Launch selected RuneLite and client artifact | `not_tested` | Reproducible launch command/log |
| Connect/authenticate to ClubScape | `not_tested` | Real account/session handshake |
| Load selected base cache | `not_tested` | Cache identity and rendered known scene |
| Deliver custom cache entries | `not_tested` | Custom model/config visible from live game state |
| Render penguin player composition | `not_tested` | Live idle/walk/turn capture in selected scene |
| Render adapted OSRS equipment | `not_tested` | Weapon, shield, cape, helmet, robe, and two-handed corpus |
| Use penguin player editor | `not_tested` | Working choices, preview, serialization, reconnect persistence |
| Render custom NPCs/objects/items | `not_tested` | One of each with correct IDs/actions |
| Preserve OSRS interfaces | `not_tested` | Selected interface captures and interactions |
| Add custom minigame interfaces | `not_tested` | One authoritative minigame UI/state demonstration |
| Expose ticks/events/callbacks | `not_tested` | Automated listener assertions |
| Translate menu actions | `not_tested` | Live validated world/inventory action |
| Support generic plugins | `not_tested` | Named plugin matrix |
| Support OSRS-ID-aware plugins | `not_tested` | Stable mappings and named plugin tests |
| Handle reconnect/version mismatch | `not_tested` | Controlled failure/recovery tests |

Update this table only from executable evidence.

## Bounded feasibility sequence

### Spike A: launch and inspect

- Select exact RuneLite, underlying client, and cache artifacts.
- Document the client-loading, injection, API, cache, scene, and networking boundaries.
- Launch under automation and capture logs/version identities.

Exit condition: a reproducible launch/instrumentation environment or a diagnosed blocker.

### Spike B: live state bridge

- Connect to a minimal ClubScape server endpoint.
- Establish login/game state and game ticks.
- Expose local player, one NPC, one object, one ground item, skills, inventory, and chat.
- Send and validate one menu action.

Exit condition: live state round trip with protocol fixtures and logs.

### Spike C: custom visual content

- Render the base penguin and one editor choice.
- Render one adapted weapon plus one non-wearable custom item/object.
- Record cache/client changes required for each.

Exit condition: real runtime captures tied to the selected artifacts.

### Spike D: plugin proof

- Run one generic tracker/overlay.
- Verify callbacks/events against known server fixtures.
- Publish the initial support matrix.

Exit condition: Tier 1 evidence or a feasibility report showing the remaining gap and viable alternatives.

## Feasibility report

If Tier 1 is not reached within the bounded milestone, report:

- exact artifacts and approaches attempted;
- commands, patches, fixtures, logs, and captures;
- which launch, rendering, cache, state, event, menu, and plugin requirements worked;
- diagnosed failures and missing prerequisites;
- maintenance risk of each viable approach;
- estimated effort and effect on browser delivery;
- recommendation to continue, change approach, or defer the desktop route.

A deadline alone does not prove impossibility. A staged delay may still be reasonable when executable evidence shows disproportionate cost or unavailable prerequisites, but it does not change the complete-client target. A browser-only activity, strategy change, or deferral requires an owner decision in `PRODUCT_DECISIONS.md`.

## Relevant references

- [runelite/runelite](https://github.com/runelite/runelite): client API, injection, cache, events, rendering, and plugin runtime
- [runelite/plugin-hub](https://github.com/runelite/plugin-hub): discovery of external plugin sources and version markers
- [blurite/rsprot](https://github.com/blurite/rsprot): current protocol/state-update reference
- [blurite/rsprox](https://github.com/blurite/rsprox): capture, decoding, replay, and comparison
- [openrs2/openrs2](https://github.com/openrs2/openrs2): client/cache artifact identity and archives
- [runelite/cs2-scripts](https://github.com/runelite/cs2-scripts): interface and appearance-script research
- [ScreteMonge/creators-kit](https://github.com/ScreteMonge/creators-kit): interactive model/scene prototyping only

See [OSRS_REFERENCES.md](OSRS_REFERENCES.md) and [VERSION_AND_COVERAGE_TRACKING.md](VERSION_AND_COVERAGE_TRACKING.md) for snapshot details.
