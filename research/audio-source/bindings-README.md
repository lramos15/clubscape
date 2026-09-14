# Exact source cue/selector follow-up

**The two whole reference-pack audio identity gates are not fully closed.**
This report does not replace them with a blanket login gate or approve running
browser audio. [`bindings.json`](bindings.json) separates proven native behavior,
identified original signals, historical recording observations and exact
remaining selectors.

## Closed technical questions

* **Current eating cue:** sequence12526 is the same original motion/timing as829,
  with source frame1 sound2393, repeat1, range5, retain0, weight100. Sequence829
  has no frame sound. Never add a second game-triggered2393 when12526 already
  emits it. The source frame-start cycle sum is retained in the binding record.
* **Correct tutorial rat family:** original NPC3313/3314/3315 has standing4932
  and walking4931. Attack/defence/death variants are4933/4934/4935, not legacy
  138/139/141. These current sequences have no embedded sound events.
* **Original cue identities:** current-cache goblin469(attack),472(hit),471(death)
  match two independently labeled public recordings; hit/death normalized
  correlation exceeds0.97. Current rat713(hit) and711(death) match a public
  tutorial recording at0.942/0.822. The recordings'2018/2019/2024 dates are
  explicit, not relabeled as build240 captures. Six missing source files,
  including eating2393, are now losslessly packaged.
* **Native effect queue:** actual opcode3200 pops ID/repeats/delay, gates muted
  and repeat0 requests, and appends to a50-entry FIFO. It drops a new request
  when full. A submitted delay2 is observed as1,0,dispatch(-100),removed across
  four actual `client.ib` calls. These are client-cycle units, not source-server
  attack ticks or calibrated device latency.
* **Native jingle priority:** actual opcode3202/`bl.bp` accepts a group other
  than-1 when music volume is nonzero. Its second integer is unused. Accepted
  requests are last-wins, replacing pending tasks and active music streams while
  retaining the background playlist. There is **no quest-vs-skill ranking**.
  Tests submit both154-then33 and33-then154; the last accepted ID wins. A-1
  sentinel does not erase an earlier accepted request. Native calls request
  zero delay/fade arguments and no MIDI looping.

Original SFX instrument offsets are already in the published waveform.
The native queue's whole20ms leading-delay trim must not be added again when
using that untrimmed file. Original source silence2411 and its74/100 weighted
alternative remain unchanged. Music files still contain one native pass plus1s
release, not an approved padded-file loop.

## Necessary renderer correction

The investigation found a real omission in the earlier offline renderer:
native startup **`dg.ay -> nu.ap(9,128,-27396)`** selects percussion bank128.
The constructor alone leaves that bank at0. This source call is now reproduced
and tested. Re-rendering was necessary, not a repeat of completed work without
cause. All35 musical inputs were checked;26 payloads changed. All223 old SFX
hashes were reused and verified unchanged.

`musical-startup-correction.json` records the exact delta. The source selection
is unchanged. The parent must refresh reference-pack musical hashes when
integrating this commit; this worker changed no pack/spec/milestone files.

## Precise remaining selectors

| Pack gate | Remaining input |
| --- | --- |
| `input.required_effect_bindings` | Ordinary shortbow selection/combination of named2702/2692/2693 draw/release/projectile cues; decisive current tutorial-rat attack710 evidence; ordinary-food server choice of829 plus an explicit cue versus12526; current bronze-smelting2725 callback and event phase; current server delay/variant confirmation for historically identified NPC cues |
| `input.quest_jingle_binding` | Actual server-emitted MIDI_JINGLE group or no-request outcome after valid Learning-the-Ropes Wind Strike and Cook's Assistant rewards, and its emission order relative to skill-level jingles |

899 and3243 have identical frame-length/other sequence settings but **different
frame IDs**, and neither contains a frame sound. An independent furnace2725
name or a plausible/weak recording match is not a verified current899 callback.
No bow/rat-attack candidate is promoted merely because it sounds plausible.

The actual current native parser decoded **9,801 scripts**. None contains
opcode3202. Script0's two-byte `0009` payload is explicitly rejected by that
native parser and is outside the required closure. Quest widget153:0 calls
script118, which calls6816/6817 to format quest-point fields; it does not choose
audio. The original packet receiver reads a numeric group and auxiliary value,
maps65535 to-1, and calls the generic jingle function. It receives no quest ID
or skill class. Thus receiver behavior alone cannot establish a per-quest
server selector or packet ordering.

## Public alternatives actually evaluated

Recent Cook's Assistant and Learning-the-Ropes guides, a recent no-commentary
tutorial, an older game-sounds-only tutorial, and two short ordinary-goblin
recordings were retrieved through public guest access with **no account,
cookies or personal downloader configuration**. Source URLs, authors, upload
dates, downloaded hashes, decoder settings and actual comparison results are
in `binding-public-evidence.json.gz`; whole recordings remain ignored.

The recent quest/tutorial candidate ending intervals did not yield a decisive152/153/154
match under normalized waveform or phase-insensitive spectral comparisons.
Neither154 nor silence is therefore asserted. The independent wiki
ID-labeled154 recording was also decoded and compared, checking that the
template itself is recognizable; recognizing that recording does not select
it for a particular quest.

Native script/sequence/callback inspection, direct original opcode and tick
execution, exact named-ID research, positive-control effect matches, three
quest-jingle alternatives, and multiple identifiable public recordings have
all been tried. The residual is the **specific server selection/emission
boundary**, not “everything requires login.”

## Reproduce

Read the machine guide first. Reuse the already verified source artifacts:

```sh
python3 tools/audio-import/bindings.py audit
python3 tools/audio-import/bindings_report.py

# Only when applying/reproducing the source initialization correction and new cues:
python3 tools/audio-import/bindings.py publish-audio

PYTHONPATH="$PWD/.local/audio-bindings/python" AUDIO_IMPORT_INTEGRATION=1 \
  python3 -m unittest discover -s tools/audio-import -p 'test_*.py' -q
python3 tools/audio-import/audio_import.py validate
```

`--cache`, `--reuse` and `--extracted` override the documented task-artifact
paths. The cue audit uses read-only disk handles and rechecks original cache
hashes. The unchanged general supplemental extractor gets only a private cache
copy. Musical correction uses `--reuse-existing-sfx`; it verifies old hashes
instead of converting old effects again.

Optional public-media analysis dependencies are exact-hash pinned in
`binding-media-requirements.txt` and `binding-analysis-requirements.txt` and
installed only under ignored `.local/audio-bindings/python`. Source recording
research is not a product playback or M1 acceptance result.
