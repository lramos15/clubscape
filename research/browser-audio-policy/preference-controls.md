# Native client preference/control calibration

This closes the bounded preference-control component, not the native All
Settings presentation or a played M1 journey. It adds no source asset,
approval, waveform, gain curve or tolerance. The approved pack remains
`b62e19704e17d3d3e4e819f803ef49ba7cc54034ae407184b423427c65d9674d`;
the nine-input supplement remains
`840aef91bac9a1fd042bdb1c3662ff92a378e279f48335108730e168af550d91`.

## Original execution and bounds

`native-preference-controls.json` records twelve fixed controlled fixture
cases and their command sequences, repeated identically in two fresh JVMs.
It pins every JAR, probe source, selected native script, original varbit
definition, relevant music row, enum and read-only cache container.
Injected1.12.38 SHA256 is
`7fdedf1194261cc5b99faa35e0d2b4e45b6d56665402ccbde7f3aa6207c3f947`;
the original runtime is240 and cache2695. There is no new gamepack retrieval.

The actual original script interpreter runs9255,9292,9302..9321 and the
source-created9296/9297 numbered menu callback. Original option-sync scripts
7109/2475/3643/3644 apply the changed varps to the actual native preferences/
mixer. The transport is original `vp/va` index and group decoding over
`BindingCache`'s validated **read-only** file handles. Source archives,
interpreted instructions, varbit decoders, mixer and music request methods
are not patched.

An empty original scene supplies the native varp-change dependencies; no
game loop, gameplay progression, account, credential, personal preference or
hardware audio device is opened. `FixtureVarcs` isolates persisted reads and
`FixturePreferenceWrites` records rather than executes native save IO.
The runner rejects original interpreter error logs even if native script
dispatch swallowed an exception. Earlier host-initialization failures are
not counted as passing policy evidence.

The existing `bounds.json` is hash-bound before the browser comparisons:
native integer difference0, original source byte difference0,20ms known
source-cycle timing/transition tolerance,0.25dB gain bound and0 additional
clipping. No numeric threshold was widened.

| Controlled case | Independent expected/native result |
| --- | --- |
| First-use zero memory | Current and remembered master/music/effects/area become100/20/45/25; synchronized native channel mixers9/18/8. |
| Restore genuine37/21/66/83 memory | Exact same percentages restored; native mixers3/7/10. |
| Mute positive37/21/66/83 | Current0/0/0/0, memory37/21/66/83, synchronized mixers0/0/0. |
| Area0 Skip | Non-primary and primary callbacks enqueue no click; music unchanged. |
| Single2 Skip | Same disabled result. |
| Shuffle1 Skip | Non-primary enqueues0; primary enqueues original2266/repeat1/delay0. The callback itself does not pick a song. |
| Background requests during a jingle | Re-requesting remembered62 preserves zero parameters. Different76 then327 replace remembered selection/parameters, retaining active152 and [0,60,60,0]. |
| Muted Shuffle request | Native mixer0 prevents music selection consumption; independently enabled effects may still queue the bound click. |
| Saved Playlist1 | Empty sentinels, first-hole insertion, noncompacting removal and slot100 round-trip through original scripts. |
| Saved Playlist2 | Same independent operations in its separate native range. |
| Saved Playlist3 | Same independent operations in its separate native range. |
| Actual source-generated numbered menu | Choosing Playlist1 from Area0/current slot0 with19736 clear produces native mode1/current slot1. |

The raw sidebar callback's varp updates and subsequent option synchronization
are recorded separately. Reading constructor effective255/127/127 and calling
that the first-use Unmute fallback would be incorrect.

## Exact source storage and qualified rules

Remembered percentages use14817 (varp3797 bits0..7),12426 (3109 bits0..7),
12427 (3109 bits8..14),12428 (3109 bits15..21). Original9255 substitutes
100/20/45/25 when memory is nonpositive; the detached client schema accepts
only the valid percentage domain0..100 and rejects corrupt negatives.
Slider handlers9232/9238/9244/9250 do not update those memory fields.

Native19731 is the four-bit current playlist selection in varp19; the UI's
actual enum2772 contains **All music, Playlist 1, Playlist 2, Playlist 3**,
values0..3. Values4..15 are rejected rather than using an enum fallback label.
Each saved playlist has100 slots:

| Playlist | Varp range | Varbit range |
| --- | --- | --- |
| 1 | 5239..5288 | 19738..19837 |
| 2 | 5289..5338 | 19838..19937 |
| 3 | 5339..5388 | 19938..20037 |

Each varp packs two16-bit entries. Zero is empty. Original9302 stores
table44 column5's first field times100 plus its second field;9303 reverses
that identity through native table indexes. It is not group+1 or row+1.
For the published menu groups, the exact group:stored-ID relationships are
2:117,64:123,327:203,163:215,76:226,62:2700,144:406,145:601.
The title source0 is not manufactured into a music-menu row.

The additional conditional9297 branches are derived from its pinned original
instructions: with19736 clear, nonzero selection changes Area to Shuffle;
Single changes to Shuffle only when an actual current track is absent from
the selected slot. All music restores Area. With19736 set, selection leaves
the current mode/track alone. The runtime retains a same-group playing node,
including during a jingle, instead of forcing a restart to update metadata.

Native9630's pinned same-track guard has an important distinction: Single
always permits its repeated request; native4137 permits repeats outside
Single. The versioned contract exposes `repeatInAreaShuffle`, not a falsely
universal Single stop button. Enum684 independently fixes Modern0/Classic1.
The new client-record defaults combine actual constructor slider values with
empty storage/source-zero preference flags; they are not a claim that an
existing account has empty history. Shell-confirmed absent records and failed
storage reads must remain different conditions.

The original next-row chooser crosses the client/server boundary. Native9292
contains only the primary/Shuffle guard and source click. Direct browser Skip
uses the previously qualified internal duration/no-repeat selection over
declared unlocked/published inputs, followed by the independently executed
native3201/rj.bc transition policy. This is not a measurement of external
server response latency or an invented generic three-song playlist.

## Factory integration and reproduction

`web/audio/preferences.ts` owns strict detached version1 parsing, source
storage helpers, defaults, slot operations and native mode/volume rules.
`web/audio/index.ts` consumes them in the actual `createAudio` runtime.
Direct Skip, numbered selection, native mute and sliders manipulate the real
graph/queue and retain source-prepared choices. Jingles keep last accepted
background parameters; identical/same-group preferences do not restart nodes.
Asynchronous controls carry both character identity and a reset/selection
generation, including the same-character logout/re-entry case.

The exact UI/shell signatures, complete record, synchronous world/binding
order and persistence responsibilities are in `web/audio/README.md`.
`AudioHandle`, shared/app/UI/renderer code, all266 original files and all nine
supplement inputs are unchanged. The schema does not persist unlock grants,
authentication, playhead, device permission or global privacy mute.

```sh
python3 tools/browser-audio-tests/native/preference_controls.py run
pnpm --dir web exec tsc --noEmit
node --test web/audio/audio.test.ts web/audio/native-policy.test.ts web/audio/reward-levels.test.ts web/audio/supplement.test.ts web/audio/preferences.test.ts
node tools/browser-audio-tests/run.mjs --preferences-only --quick --mute-output
node tools/browser-audio-tests/run.mjs --mute-output
PYTHONDONTWRITEBYTECODE=1 python3 tools/reference-pack/validate.py --require-complete
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tools/reference-pack -p 'test_*.py' -q
```

The real-Chrome component uses actual mouse/keyboard activation and an actual
AudioContext/destination graph; all JS evaluation has CDP `userGesture:false`.
`--mute-output` is a **test-browser-only** shared-host privacy flag, explicitly
recorded; it does not substitute a silent AudioContext sink for an enabled
graph or fake its clock. Native starts/ends, post-gain monitoring, source
timers, zero added clipping and complete node cleanup remain measurable.
No speaker perception, Mac/Edge, live gameplay or full M1 acceptance is claimed.
