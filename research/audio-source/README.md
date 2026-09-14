# M1 original audio source inputs

This is an **offline source-audio preparation record**, not an approved
reference pack, source-game recording, or implemented ClubScape audio system.
Source selection and owner gates are unchanged.

## Consume

The canonical output registry is
[`assets/manifests/osrs/audio-runtime.json`](../../assets/manifests/osrs/audio-runtime.json).
It identifies the actual FLAC files, original inputs, hashes, sample format,
duration, signal levels, native loops, export gain and exact PCM relationships.
[`tools/audio-import`](../../tools/audio-import/README.md) contains reproduction
commands, locked dependencies, tests, limitations and retained notices.

| Evidence | Status |
| --- | --- |
| Original cache/MIDI/patch/sample/SFX inputs | Hash-verified |
| Actual original-runtime music/SFX decoding | Executed; native PCM measured |
| Browser-playable file packaging | Lossless FLAC; actual decoded-signal checks |
| Original source-session recordings | **Not obtained** |
| Browser file decoder | Separate `browser-decode.json`; not audible playback |
| Exact live triggers, mute, volume, looping/transitions | **Not accepted** |
| Source reference-pack/ClubScape presentation approval | **Not granted** |

Actual playable output after the binding follow-up: **264 files,
1472.961950096s,55,197,689 bytes**: five music tracks935.123174603s;
30 jingles207.004852603s;229 audible SFX330.833922890s.
The follow-up corrected native percussion-bank initialization, changing26
musical payloads, and added six identified source effects. All223 pre-existing
SFX hashes and the original weighted silence are preserved.
See [`bindings-README.md`](bindings-README.md) for the exact native/public
evidence, validation and still-unclosed per-action/per-quest selectors.

## Required music

| Scope | Original index6 group | Track |
| --- | ---: | --- |
| Title/startup | **0** | Scape Main |
| Tutorial Island surface | 62 | Newbie Melody |
| Tutorial Island dungeon | 144 | Scape Cave |
| Lumbridge | 76 | Harmony |
| Lumbridge farms | 2 | Autumn Voyage |

Scape Main is the **current name-hash group0**, not the older wiki seed16.
Each file contains a complete native pass and1s release. Exact durations and
integer-clock end frames are in the manifest; no seamless-loop or source
playlist-boundary claim is made.

## Source maps and important distinctions

`source-map.json` contains the actual original sequence sound events, including:

* Net fishing621 →2603 at frame4.
* Bronze mining625 →3220 at frame11.
* Tinder733 →2597 at frames8/10.
* Bronze chopping879 →2735 at frame3.
* Smithing898 →3790/3791 at frames5/7/9/11.

Frame durations, weights, repetitions and original object sound/fade fields are
retained. Source placements are not moved. Native cycle sums are descriptive;
exact source packet/frame-entry/wall-clock offsets still need observation.

**Do not remove source silence2411.** The original5ms effect decodes to110 zero
samples and is the74-weight alternative in sequence13612/frame1. It has no
playable output and is not counted as one. Renormalizing the remaining choices,
or replacing it with a nonzero tone, would be wrong.

Thirty named quest/low-level skill/combat/death jingles are prepared as source
inputs. Which quest jingle plays for Learning the Ropes/Cook's Assistant, and
the precedence against level-up jingles, remain source-session observations.
The normal Smithing jingle is explicitly a comparison input, not a claim it
plays in the current leveling rules.

`references.json` separates exact current cache relationships, RuneLite symbols,
wiki jingle cache IDs, and independent reference candidates. In particular:

* Wind Strike220/221, milk372, equipment2238 and player-hit variants have
  independently named source inputs, not observed server-trigger timing.
* Smelting2725 is a **candidate**, not a verified current bronze-smelting
  binding. It is not automatically assigned to the journey action.
* Current sound-enabled eating12526 contains2393 at frame1. The silent
  motion-equivalent829 remains distinct; its ordinary-food server selection
  is not inferred. Original goblin469/472/471 and tutorial-rat713/711 were
  identified against independent recordings, with recording age and
  current-trigger limitations explicitly retained.
* Ordinary shortbow, rat attack, current smelting and per-quest selectors
  are not promoted from weak/candidate evidence. Do not use a generic sound,
  player grunt for a goblin, or UI2266 everywhere.
* RSMod's custom654xx/655xx parameters were absent from the selected original
  NPC/item definitions; they were **not** promoted into original cache bindings.

## Next acceptance work

An authorized source-session observation must settle the remaining sound
identities/variants, trigger/packet offsets, music repeat and region-transition
policy, source mixer/volume settings and positional/fade behavior. Owner
reference-pack approval still precedes ClubScape presentation implementation.
After that, real browser playback must test gestures/autoplay, audible output,
volume/mute, loops, transitions and reconnects without duplicate sources.

These are separate from successful extraction, file decoding, signal analysis,
source-byte identity, gameplay, visual fidelity and RuneLite compatibility.
