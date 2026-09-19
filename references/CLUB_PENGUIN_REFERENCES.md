# Club Penguin References

Updated: 2026-09-15

Club Penguin has no single equivalent to an OSRS protocol revision. Every conclusion must name the client family/era, media set, server commit, and relevant room, item, mission, or minigame.

## Behavior and content implementations

| Repository | Stars at snapshot | Latest inspected commit | Inspected SHA | Primary use |
|---|---:|---|---|---|
| [solero/houdini](https://github.com/solero/houdini) | 417 | 2026-09-14 | `45021fa838ede1e1526858ea079bd13d1b01d5b9` | Broad room, social, inventory, puffle, moderation, and minigame behavior hypotheses |
| [nhaar/Waddle-Forever](https://github.com/nhaar/Waddle-Forever) | 170 | 2026-09-07 | `7a578e7544e7bfd95ffb2a92033e02a86e262475` | Preserved parties, events, date selection, and local behavior observation |
| [solero/wand](https://github.com/solero/wand) | 128 | 2026-09-14 | `fdeec704748ca6c75509eab3661c2c5fdb2615ad` | Pinned composition of Houdini, Dash, media, and Snowflake |
| [Times-0/Timeline](https://github.com/Times-0/Timeline) | 73 | 2020-10-12 | Not captured | Older AS2/AS3 server comparison |
| [Lekuruu/snowflake](https://github.com/Lekuruu/snowflake) | 35 | 2026-09-14 | `089411b895dd3cd9b19b1771dc8c046bcd7748e3` | Card-Jitsu Snow rules, sessions, powers, rewards, and Tusk battle hypotheses |
| [solero/dash](https://github.com/solero/dash) | 29 | 2025-11-16 | `89ccac34883088f176c594ae55e0e93a29dac265` | Web/bootstrap services surrounding Houdini |
| [wizguin/mammoth](https://github.com/wizguin/mammoth) | 12 | 2024-10-17 | Not captured | Pre-CPIP early-era behavior only |

### Recommended use

- Inspect Houdini first for broad system coverage. Its implementation is a hypothesis until compared with preserved behavior or independent documentation.
- Use Snowflake first for Card-Jitsu Snow-specific state and Houdini second for integration context.
- Use Waddle Forever when researching historical parties, event dates, or observable client behavior.
- Use Wand as a reproducible composition, not as independent gameplay evidence.
- Label conclusions `observed`, `source-implemented`, `historical`, `inferred`, or `clubscape-decision`.

## Wand composition and media inputs

The inspected Wand `.gitmodules` declares:

| Component | Location | Treatment |
|---|---|---|
| Houdini | `github.com/solero/houdini` | Behavior/server submodule |
| Dash | `github.com/solero/dash` | Web/bootstrap submodule |
| Snowflake | `github.com/Lekuruu/snowflake` | Card-Jitsu Snow submodule |
| Legacy media | `git.solero.me/solero/legacy-media` | Independently tracked media input |
| Vanilla media | `git.solero.me/solero/vanilla-media` | Independently tracked media input |

For a Wand snapshot, record the Wand parent SHA and the five recorded submodule SHAs. A child repository's current head is not the same composition. Hash each consumed SWF, sprite sheet, audio file, room asset, or configuration file separately.

## Browser and minigame implementations

| Repository | Stars at snapshot | Latest inspected commit | Inspected SHA | Assessment |
|---|---:|---|---|---|
| [wizguin/yukon](https://github.com/wizguin/yukon) | 129 | 2025-08-23 | `2f47b90137d168d484ce934b58543cadf0f94920` | Phaser 3/Socket.IO room, avatar, UI, and asset reference |
| [wizguin/yukon-server](https://github.com/wizguin/yukon-server) | 53 | 2025-08-23 | `fead5f7849e97a866e07d39213ff41b970497f4e` | Matching backend comparison |
| [Ep8Script/Club_Penguin_Minigames](https://github.com/Ep8Script/Club_Penguin_Minigames) | 34 | 2020-05-13 | Not captured | Secondary HTML5 minigame comparison |
| [project-flipper/ClubPenguin](https://github.com/project-flipper/ClubPenguin) | 3 | 2025-04-06 | Not captured | Phaser reconstruction with its own protocol |
| [project-flipper/Island](https://github.com/project-flipper/Island) | 2 | 2025-03-30 | Not captured | Matching backend; not Disney's Club Penguin Island game |

Yukon demonstrates non-Flash room/avatar presentation. It does not make Phaser, Socket.IO, or its message design appropriate for ClubScape. Study client and server together, then express the observed state model in the project's Rust protocol.

## Flash observation and asset inspection

| Repository | Stars at snapshot | Latest inspected commit | Inspected SHA | Role |
|---|---:|---|---|---|
| [ruffle-rs/ruffle](https://github.com/ruffle-rs/ruffle) | 18,538 | 2026-09-14 | `9525d0fb8773a0d1868940918385e66360927a07` | Rust/WASM runtime for observing preserved behavior; not the new client |
| [jindrapetrik/jpexs-decompiler](https://github.com/jindrapetrik/jpexs-decompiler) | 5,871 | 2026-09-14 | `b2ef2d3d3f44291a4bdfa899c986bc39a89fa6aa` | Inspect symbols, timelines, transforms, labels, scripts, audio, and embedded media |
| [abarichello/cp-swf](https://github.com/abarichello/cp-swf) | 47 | 2023-11-22 | Not captured | Year-organized archive viewer; media comes from an external submodule |
| [solero/canon](https://github.com/solero/canon) | 7 | 2020-10-27 | Not captured | Specialized minigame compression reference |

Running an SWF in Ruffle is observation, not clean-room implementation. A useful observation record contains:

```text
game_or_room
club_penguin_era
media_filename + sha256
ruffle_or_jpexs_version + full_sha
starting_state
inputs_with_timestamps
observed_outputs
screenshots_or_capture_hashes
unknowns_and_alternate_branches
```

## Community documentation

Use community documentation to find names, eras, room histories, item catalogs, party dates, stamp requirements, mission steps, and minigame variants. The [Club Penguin Wiki](https://clubpenguin.fandom.com/wiki/Club_Penguin_Wiki) is a discovery and cross-reference source rather than hidden implementation truth.

For content that changed across 2005–2017, identify the latest stable original-browser version for the normal-world default and separately catalog materially different earlier variants for parties, archives, missions, or instances. Record dates/eras and source artifacts rather than silently combining incompatible versions.

For consequential mechanics:

1. Record the exact page and revision/timestamp.
2. Locate preserved media or a second independent source.
3. Separate original behavior from later private-server behavior.
4. Write the final ClubScape rule in [HYBRID_GAME_DESIGN.md](HYBRID_GAME_DESIGN.md) or the relevant content specification.

## Era separation

Do not combine these without an explicit adaptation decision:

| Era/family | Typical differences to expect |
|---|---|
| Early/AS2 | Simpler avatar, rooms, games, inventory, and protocol behavior |
| CPIP/AS3 | Different client structure, features, UI, assets, stamps, and minigame revisions |
| Later web/private implementations | Recreated or altered behavior, missing media, modern service layers |
| Club Penguin Island | Outside the required parity target; use only when a later decision approves a specific element as inspiration |

An activity name may persist across eras while scoring, rewards, controls, art, difficulty, or stamps change. Store variants rather than overwriting them.

## Subsystem routing

| Question | First reference | Cross-check |
|---|---|---|
| Broad social/inventory/puffle behavior | Houdini | Preserved client, Wiki, Waddle Forever |
| Party/event recreation | Waddle Forever | Preserved media and dates |
| Card-Jitsu Snow | Snowflake | Houdini, preserved observation |
| Browser room/avatar presentation | Yukon + Yukon Server | Preserved client, ClubScape WASM constraints |
| Symbols/timelines/audio | JPEXS | Ruffle observation |
| Input timing/game loop | Preserved SWF in Ruffle | Server implementation and independent docs |
| Media composition | Wand parent/submodules | Individual child repositories and artifact hashes |
| Original item/room catalog | Wiki + media | Houdini definitions and screenshots |

## Known limitations

- No mature, active, complete Club Penguin-specific Rust client/server pair was found.
- Private-server implementations often intentionally differ from original Club Penguin.
- Media availability does not itself establish permission to redistribute it.
- A complete minigame list does not imply that complete, verified rules have been recovered.
- ClubScape should preserve recognizable loops without importing a second incompatible economy or avatar system.
