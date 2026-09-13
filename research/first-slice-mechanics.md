# M1 reference findings, 2026-09-13

Read-only M1-REF research identified OSRS build **240**, OpenRS2 cache **2695**,
dated **2026-09-08T10:30:08.897945Z**. The exact URLs, retrieved-index hashes,
tooling revision and pinned wiki revisions are in
[`first-slice-sources.json`](first-slice-sources.json). This is a fixed reference
identity, not a complete behavior inventory or an approved presentation pack.

The dated official update aligns with the cache date but does not independently
prove its build number or all server-side hotfixes. Archive group completeness
must not be counted as live-content or implementation coverage.

## Verified reference facts

The pinned Skills/Hitpoints pages describe 24 skills: Hitpoints starts at level
10 with 1,154 XP; other skills start at level 1 with zero XP. Initial total level
33 is a calculation from those facts, not a directly observed fresh account.
Pre-first-dialogue inventory, equipment and flags remain unresolved.

The current Tutorial Island quest is **Learning the Ropes**. The pinned source
records its quest conversion on 2025-10-22, renaming on 2026-02-11 and a
one-quest-point reward. Implementing a remembered historical tutorial would
miss current baseline requirements.

| Stage | Source-supported required mechanics |
| --- | --- |
| Gielinor Guide | Appearance and experience selection, guide dialogue, Settings, gated exit |
| Survival Expert | Net, inventory, shrimp fishing, Skills, axe/tinderbox, logs/fire/cooked shrimp |
| Master Chef | Flour/water, dough, range-baked bread, replacement ingredients, run controls |
| Quest Guide | Journal unlock and dialogue before ladder access |
| Mining Instructor | Bronze pickaxe, tin/copper, furnace/bar, hammer/anvil/bronze dagger |
| Combat Instructor | Equipment/stats, dagger, sword/shield, melee rat, bow/50 arrows, ranged rat |
| Bank / Account Guide | Bank then poll-booth inspection, account-management interface and dialogue |
| Brother Brace | Prayer interface and gated instruction; prayer/bone/altar behavior |
| Magic / departure | Filtered spellbook, five air/mind runes, Wind Strike on chicken, real Lumbridge Home Teleport |

The current transcript includes numerous early-action, range, full-inventory,
equipment, rat-pen and repeat-action guards. The table is a dependency outline,
**not** a complete executable state graph. Source lines are in pinned
`Transcript:Learning_the_Ropes` revision 15309360, ranges 4-947.

Tutorial sources describe trainable-skill level-3 caps, no Hitpoints training,
nonfatal tutorial combat, restricted chat/trade and departure item cleanup.
Learning the Ropes lists departure tools/gear/food, **25 arrows**, runes
(25 air, 15 mind, 6 water, 4 earth, 2 body), bucket/pot and 25 banked coins.
Do not flatten these into a seeded initial inventory: tutorial grant counts,
bank initialization, slots, losses/deposits and departure retention need
specific verified transitions.

Arrival differs with experience/Adventure Paths selection: one route reaches
Adventurer Jon near the pub, another the castle area. The exact selected normal
account branch, arrival tile and camera must be captured, not assumed.

There is a source conflict: the older Tutorial Island guide retains music/emote
instruction descriptions absent from the current detailed transcript; that
transcript also marks missing departure text. These gaps block claiming a
complete current tutorial, not independently verified account infrastructure.

## Cook's Assistant

Pinned quest, dialogue and journal references require an egg, bucket of milk
and pot of flour. They support partial deliveries and bringing ingredients
before quest start; journal states distinguish missing, held and delivered
ingredients.

Legitimate dependencies include the castle kitchen/cellar pot/bucket spawns or
general-store purchases (one/two coins respectively), chickens, dairy cow,
wheat and the multi-floor mill hopper, controls and flour bin. Guide markers
include Cook 3208,3214; coop 3181,3288; dairy field 3177,3315; wheat 3163,3289.
These are **guide markers, not certified object coordinates**.

Rewards are one quest point, 300 Cooking XP and Cook-o-matic 100 access. The
source does not list a coin reward. Neither a dialogue-only quest nor granting
ingredients in tests can establish legitimate completion.

## Mining and death

The pinned bronze pickaxe reference identifies item 1265, acquired during
tutorial mining and among departure provisions. Copper rocks require Mining 1
and award 17.5 XP on success. Success timing/formulas, exact reachable deposit,
shop stock/repricing and the selected goblin variant remain to be pinned.

The current grave reference describes the first Death's Office dependency and
a 15-minute grave timer that pauses during logout, extended idling and grave
inspection. It also records July 2026 inventory-order recovery changes.
Do not implement the obsolete ground-pile-only model. Full first-death flow,
retention selection and recovery edge cases still require source evidence.

## Asset and audio retrieval

The source-inspected RuneLite decoder pin is
`ac79ed8bd8926bec7bf172aa291574b4d944b0e7`, not a ClubScape compatibility result.
OpenRS2 individual group downloads are JS5 containers without their two-byte
version trailer. The current map index is unnamed protocol 7, revision
1788780617, flags 4, 2,937 groups; do not assume historical named/encrypted maps.
`RegionLoader` maps group ID to map square, file 0 to terrain and file 1 to
locations. Five seed group IDs in the source manifest are not complete bounds.

| Input | Cache archive / definition dependencies |
| --- | --- |
| Terrain / placements | 5; map-square group, files 0/1 |
| Definitions | 2: underlay 1, overlay 4, object 6, NPC 9, item 10, sequence 12, spot animation 13 |
| Models / textures / sprites | 7 / 9 / 8 |
| Animations | 0 frames, 1 skeletons, 22 newer animation data, sequence definitions |
| Interfaces / title | 3 / archive 10 `title.jpg` |
| Fonts | 13; native `p11_full`, `p12_full`, `b12_full` and matched glyph data |
| Music / jingles / samples / patches | 6 / 11 / 14 / 15 |
| Sound effects | 4 |

Decoder paths relative to the pinned repository's cache module include
`src/main/java/net/runelite/cache/{IndexType,ConfigType,FontManager}.java`,
`region/RegionLoader.java`, `definitions/loaders/SequenceLoader.java` and
the cache tests `TitleDumper`, `TrackDumperTest`, `SoundEffectsDumperTest`.
The last is ignored upstream; its source is not evidence of working runtime
sound. Preserve imported upstream notices and do not apply RuneLite's code
license to Jagex assets.

Pinned music references identify cache IDs: Scape Main 16, Newbie Melody 62,
Scape Cave 144, Harmony 76 and Autumn Voyage 2. UI Resizable-Classic does not
choose Classic music mode: area/shuffle/single selection and the entire route's
playlist/transitions remain unresolved. RuneLite sound-ID seeds include tree
chop 2735, mining tink 3220, cooking whoosh 2577 and fire whoosh 2596; names/IDs
alone do not establish trigger timing.

Public media observations included a starting-house image (2026-07-07,
1200x800, SHA-1 `d414217a337642b57e82563d0bc94fadcf5d78ec`), Newbie Melody
recording (2026-05-29, 222.609705 seconds, SHA-1
`1f89e1de34e161ad0650fc00504ef851e801e0e9`) and an older Cook reward-scroll
image (2020-11-14, 488x320, SHA-1
`3f2f964894e4db7196a11f98acfd2578dae1e85e`). These are retrieval aids, not
matched build-240, 1920x1080 acceptance captures.

See the [capture checklist](reference-capture-checklist.md) for missing
observations and the bounded owner reference-pack gate. Source availability
does not establish successful conversion, rendering, audio or milestone
acceptance.
