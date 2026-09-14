# Concrete audio selector observations

This follow-up reads the parent's `9740655` audio handoff without modifying the
reference pack. No existing audio file was converted again.

[`selector-observations.json`](selector-observations.json) records new,
hash-bound public source observations and exact residuals. The full comparison
data, recording identities/dates, decoded settings and selected visual-state
timestamps are in `selector-observation-evidence.json.gz`. Whole recordings
remain ignored, not redistributed as runtime assets.

## Positive findings

| Handoff action | Identified original cue | Source-relative event |
| --- | --- | --- |
| Ordinary shortbow841 / sequence426 | **2693**, not name-only2702 | Projectile release, not equipping, drawing or target impact |
| Tutorial rats3313-3315 | **710 attack**,713 hit,711 death | Separate game-triggered attack/reaction/death events; current493x animation family |
| Copper/tin bronze smelt / sequence899 | **2725** | Accepted furnace operation starts; source chat places copper/tin together |
| Normal-account Cook's Assistant | **Jingle152**, not candidate154 | Quest reward scroll opens; Cooking level-up jingle33 follows after scroll dismissal |

Background music had masked the shortbow and rat attack signals in unfiltered
correlation. Applying the same fixed analysis band to the current-cache
template and recording yields **0.994** for2693 and **0.988** for710.
Named alternatives2692/2700/2702 do not match comparably. Furnace2725 aligns
within one22050Hz sample in five disjoint bands;6000-9000Hz reaches**0.782**,
with a negligible off-event alternative. Visual source frames confirm the
associated action boundaries. This does not turn those events into fabricated
animation-frame sounds or add duplicate asset-leading offsets.

The normal-account Cook's Assistant recording identifies152 over five seconds
at**0.981** correlation and visibly shows the quest scroll. Jingle33 appears
with the reward-caused Cooking level4 dialog after the scroll closes.
Jingle34 later accompanies an actual subsequent cooking level5, not a
simultaneous quest reward. A separate Quest Speedrunning recording contains34,
but its speedrun results interface is explicitly excluded from the normal
quest ordering. Native client request semantics remain **last accepted request
wins**, with no quest/skill rank or use of the auxiliary integer.

The source footage is dated: the decisive tutorial observations are from
2024-05-21 and normal Cook's Assistant observation from2017-03-01. They identify
the exact **current-cache payloads** in source-game contexts; they are not
relabeled as build240 recordings. This version qualification remains available
for the owner's source-compatibility review.

## Still not invented

* **Learning the Ropes:** the actual current completion jingle/no-request
  selector and its submission order with any simultaneous skill jingle.
  Whole current-named and post-2025 recordings, upload-filtered recent videos,
  and a bounded September10 stream interval were tested. None supplied a
  decisive completion cue. Some stream portions were already mainland
  gameplay and are not misrepresented as completion evidence.
* **Shrimps315/bread2309:** exact source selection of silent829 plus a
  game-triggered2393 versus sound-enabled12526. The two exact item definitions
  have `Eat` but no selector parameters;12526's source frame1 cue remains
  verified. Tested recordings did not decisively identify literal315/2309
  consumption. Do not fill this with guessed silence or a duplicate cue.

Thus the entire source-input gate is not declared closed. There is no blanket
login requirement, no owner-pack approval and no running-browser/M1 acceptance.

## Reproduce

Use the existing hash-pinned local analysis dependencies, with
`PYTHONPATH="$PWD/.local/audio-bindings/python"`. `public_audio_match.py`
contains the fixed-band and normalized correlation routines.
`record_selector_observations.py` rebuilds the compact report from the retained
source evidence. `LegacyCueProbe.java` exercises the **current original
runtime's** eight-bit compatibility mode solely as a historical-comparison
control; those fingerprints are not published or used to replace16-bit assets.
`FrameRelationProbe.java` verifies that899/3243 are not raw-frame aliases.

The new tests check exact recorded selector IDs, chronology, source hashes,
fixed-filter positive/negative behavior, independent-band alignment, and
preservation of the two residual selectors.
