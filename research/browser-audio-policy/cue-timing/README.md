# Composed native cue timing repair

This is the separately admitted `M1-AUDIO-CUE-TIMING` task, not a promotion of
the historical64bd325 component or its41-browser result. All earlier audio
reports,275 original/additive files, the approved pack and its1,307 input
hashes remain unchanged.

## Recorded failure and bounded reproduction

The shell's original handoff is
`.worktrees/m1-browser-shell/.local/evidence/audio-preferences-handoff.json`.
Its retained `audio-preferences-final/result.json` reports18 dispatches,
11 overruns and maximum26.4399092970522ms against the unchanged
20.045351473922903ms limit. Both coordinated and direct-native paths exceed
that limit. The later renderer handoff also reports9 audio failures; its
source/account/restart checks do not make that gate pass.

`tools/browser-audio-tests/cue-timing.mjs` executes the **unchanged**
`checkPlayerAudioPreferences` function from shell commit
`35e12584681fd45fa1671805f26adc64feae511a`. That is the exact audio/storage
fixture invoked by the composed source-browser harness after GPU teardown.
It retains all six storage/entry assertions, actual localStorage, real mouse
activation, source data, native APIs and its two six-call timing comparisons.
It does not fabricate a scene, music unlock history or a gameplay event.

The before side serves the existing immutable
`.local/web-ui4-6f-bd4a693e` delivery and the existing
`.local/source-ui4-6fdb60e4` content, checking their actual hashes. The after
side builds only a diagnostic bridge from the same immutable shell code plus
the owned audio changes, using the existing Vite8 toolchain. Its build
artifact identifies the changed code honestly; the historical bd4 artifact
is not rewritten or reused as the candidate's identity. No UI type error,
renderer result, database/account check or full production build is silently
converted into success.

The first diagnostic omitted the shell's COOP/COEP headers. Its apparent pass
is retained as **`before-unisolated-diagnostic.json`**, explicitly not the
composed reproduction. The corrected driver uses the source server's actual
COOP `same-origin`, COEP `require-corp` and CSP, the original headful Chrome153
configuration and2560x1440 Xvfb screen. It does not change global sound,
graphics, CPU or sandbox settings.

`before.json` reproduces the failing boundary:18 real cue dispatches, a
23.854875283446653ms overrun and timing/loading feedback. The recorded
coordinated path overruns; its direct-native member does not overrun on this
particular run. The original shell's independently failing direct-native
samples remain valid historical evidence and are not relabeled.

## Root cause and repair

The old scheduler used a fixed10ms lookahead. The real context reports
10.70294784580499ms base latency and commonly advances by256 source frames
(11.609977324263039ms), larger than that lookahead.

Immediately before the reproduced overrun, the context clock was
1.0274829931972789s and the cue's recorded native deadline was
1.0384580498866214s: **10.9750566893425ms ahead**, just outside the old window.
The next service callback was delayed24.21ms; it observed the context at
1.062312925170068s and therefore dispatched23.85ms late. API calls were well
below a millisecond. Blaming preference persistence or changing its test
cadence would not repair this scheduling boundary.

The audio factory now:

* Covers the device base latency plus one128-frame render quantum, bounded
  between the previous10ms minimum and **one20ms source client cycle**.
  On this context the window is16.50793650793651ms, which covers the recorded
  deadline before the delayed callback.
* Services a selected cue when its actual decode completes, rather than
  waiting unnecessarily for another timer poll.
* Does not consume an undecoded *future* dispatch boundary as if loading had
  already missed it. Actual due cycles still run, report missing data and
  retain the native negative-countdown/-10 expiry rules.
* Reads the real context cursor immediately before SFX dispatch. Trace
  `requestedAt` and `sourceCycleAt` expose the input and logical-cycle anchors;
  `when`, `dueAt` and `lateMs` are not replaced by fabricated on-time values.

The source deadline formula,50-entry FIFO, dispatched tombstone, delay2 on
call3, eating on call4, source offsets, original PCM, weighted silence,
volume, jingle and gameplay attribution policies are unchanged. The clock
tests additionally check every sample phase within a20ms source cycle against
the independent one/three/four-call onset bounds. The change is scheduling
headroom, not a slower test, a new source delay or a blanket asset warmup.

## Measurements and qualification

`after.json` records the actual repaired composed audio/storage fixture:
18 dispatches, both six-call comparisons, no timing/loading failures and
zero dispatch overruns. All native dispatch lateness values are0ms.

The driver passively records actual `AudioBufferSourceNode.start` calls,
their real context cursor, gains and end callbacks. It adds **no live
monitoring AudioNode**, avoiding a possible observer-dependent scheduling
path. After the live context closes, actual played buffers and gains are
rendered in `OfflineAudioContext` and checked against the pinned cue's first
nonzero sample. This is actual PCM rendering plus real enabled-graph playback,
not only decoding a file.

The18 PCM comparisons have at most one sample of onset rounding, maximum
absolute signal error1.26e-14 and zero additional clipping. Maximum absolute
one-cycle onset error is13.650793650793931ms, also within the unchanged bound
**independently of the runtime's `dueAt`**. The actual native start-call
cursor is considered as well, so scheduling a source in the past cannot
masquerade as an on-time trace.

The final independently rerun after member is `after-final.json`: all18
dispatches again have0ms lateness and no failures; its maximum independent
absolute onset error is16.19047619047633ms, maximum PCM error1.21e-14 and
additional clipping0. `component-full.json` passes all41 original browser
checks under the real isolation headers. Eating still dispatches on call4
(84.3991ms source start; one sample waveform-onset error), and all four full
music timer boundaries have0ms scheduled error. See `validation.json` for
exact hashes and qualifications.

The component harness now uses the same isolation headers; omission of those
headers was independently demonstrated by the initial diagnostic. The new
`--report-file` option writes this task's regression results separately,
without overwriting historical41-case evidence.

```sh
pnpm --dir web exec tsc --noEmit
node --test web/audio/*.test.ts

# Single bounded matrix, using the same recorded shell cases:
xvfb-run --auto-servernum --server-args='-screen 0 2560x1440x24 -nolisten tcp' \
  node tools/browser-audio-tests/cue-timing.mjs before \
  --report-file research/browser-audio-policy/cue-timing/before-rerun.json
xvfb-run --auto-servernum --server-args='-screen 0 2560x1440x24 -nolisten tcp' \
  node tools/browser-audio-tests/cue-timing.mjs after \
  --report-file research/browser-audio-policy/cue-timing/after-rerun.json

node tools/browser-audio-tests/run.mjs --mute-output \
  --report-file research/browser-audio-policy/cue-timing/component-full.json
PYTHONDONTWRITEBYTECODE=1 python3 tools/reference-pack/validate.py --require-complete
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tools/reference-pack -p 'test_*.py' -q
```

The before command is expected to fail on an observed overrun, but a
nondeterministic scheduling miss is not manufactured if the host happens to
service every callback promptly. The recorded failure remains the reference.
Only the after command is a repair check. Both are loopback-only and preserve
the source delivery and original test function. Browser output is muted for
shared-host privacy.
The driver refuses to overwrite an existing timing report; choose a new
owned report path on subsequent reruns. The45 audio unit tests include the
recorded failure as an immutable scheduler regression fixture.

This rechecks the **actual composed audio/storage fixture**, not the entire
source signup/restart/renderer journey. Real production UI preference routing,
source unlock/history/varp/scene authority and all existing committed Cook
event requirements remain separate. No speaker perception, Mac/Edge,
RuneLite compatibility or M1 acceptance is asserted.
