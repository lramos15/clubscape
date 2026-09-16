# Composed2266 timing: ready-cue repair, cold-input blocker

This is the bounded admission `0b1a75c`, executed only in
`.worktrees/m1-audio-composed` on `fix/m1-audio-composed`, based on
`2f3704d7132e02e15bf5c306549abcd3ccd21962`. The previous `dbeb1ac`
repair is already integrated and was not repicked, replaced or discarded.
The original audio worktrees and all historical source/calibration reports are
unchanged.

**The task remains blocked.** Three post-repair composed invocations were
used, and no fourth was attempted. The ready-cue scheduling repair works on
the measured coarse device, but the final run exposed a genuine first-use
fetch/hash/decode readiness failure. That failure and the original timing
threshold remain visible.

## Root cause independently reproduced

The Director's retained source-browser run completed21 production UI/source
checks and six native audio/storage contracts, then failed its audio gate:
2266 `source-control/7` was22.49433106575949ms late and
`source-control/19` was21.859410430838942ms late. The limit is still
20.045351473922903ms. Its original result/failure/log were copied unchanged
into `.local/audio-composed-evidence/director-before-*`.

An instrumented **whole current client** reproduction kept the existing game
UI alive and again failed on2266, this time `source-control/20` at
22.49433106575949ms. Its actual AudioContext reported:

| Property | Observed value |
| --- | --- |
| Source sample rate | 22050Hz |
| Base latency | 42.675736961451244ms |
| Output latency | 128ms |
| Common published cursor increments | 896 or1024 frames,40.635 or46.440ms |
| Existing logical-cycle lookahead | 20ms, unchanged |

The5ms callbacks continued executing while `currentTime` stayed at
1.4512471655328798s. The cue deadline was1.4751927437641725s, outside the
20ms logical window. The next cursor publication jumped directly to
1.497687074829932s. API duration was below a millisecond. This is not the
earlier10ms-window case, a missing scene, or slow preference persistence.

## Surgical runtime change

`submitReadyEffects` submits **only decoded, zero-delay, non-positional**
one-shots to the native WebAudio renderer at their **existing `dueAt`** when
that deadline is within the device's base latency plus one128-frame quantum.
It does not advance, remove, reprioritize or dispatch the logical source
queue. The original queue later consumes the same voice rather than starting
another. `effect_submitted` explicitly distinguishes preparation from the
logical `effect_dispatched` receipt.

The source-cycle horizon remains capped at20ms. Queue capacity50, tombstone
retention, delay2/call3, eating/call4, the negative loading grace, stable-event
deduplication and all source waveform/offset/weight rules are unchanged.
Delayed and positional sounds still use their existing source-cycle path;
this change does not guess future positions or scene authority.

Before logical dispatch, a submitted future voice follows the actual channel
volume and can be cancelled by a zero volume, reset, disconnect, character
change or action cancellation. Playback uses the actual context cursor at
native start; a missed deadline cannot be backdated. Already-submitted finite
repeat buffers are reused instead of being allocated a second time.

No new public UI/audio ABI is required. Only `web/audio/**` and the narrowly
related `web/app/tests/real-player-audio.ts` changed.

## Validation and the unclosed path

| Invocation | Result |
| --- | --- |
| Instrumented before |21 source checks and six storage/control contracts reached; one22.494331ms native overrun reproduced. |
| Post-repair1 |Stopped at a newly added whole-PCM assertion. That assertion did not distinguish explicit reset/logout `stop()` calls from natural completion. No timing success is inferred from this incomplete run. |
| Post-repair2 |Whole21-check current source client passed, with all18 native dispatches, both six-call comparisons and zero timing/loading failures. The same42.68ms device still advanced by896/1024 frames. Fourteen naturally completed live2266 waveforms matched exactly, with zero clipping. |
| Post-repair3, final |All18 dispatches were present. Seventeen ready cues had0ms lateness. The cold first cue had `AUDIO_LOADING_LATE` and `AUDIO_TIMING_LATE`, a60ms overrun and four processing attempts. The run failed and is not accepted. |

The final first cue was enqueued at source time0.5979138321995465s with
unchanged deadline0.6295691609977325s. Its `decoded` notification arrived at
0.6849886621315193s, **62.53000020980835ms of wall time after enqueue**.
Native loading retries dispatched it on processing call4 at
0.6895691609977326s,60ms beyond the original deadline. At that point no honest
scheduling reservation could repair the missed onset.

The newly added unconditional `processingCalls === 1` audit was also wrong
for a cold cue: the original queue permits loading retries. It was corrected
to allow the original1..10 processing attempts while retaining the existing
loading/timing failures. A unit regression pins this exact four-attempt,
60ms-late case. No composed rerun followed that audit correction because the
three-run ceiling was reached; the final failed outcome is preserved.

The fixture records native source `start`/`stop`/`ended` calls, timer callback
and AudioContext times, long tasks and original runtime traces. A passive
per-voice AnalyserNode captures actual post-gain2266 PCM on natural completion;
it does not replace the independently connected production destination or
use an AudioWorklet to change the device cadence. The measured base latency
and896/1024-frame cursor steps remained the same. Explicitly stopped/truncated
waveforms are retained separately, not made whole by waiting longer.
The final diagnostic also contains14 complete uninterrupted waveforms with
zero sample error and zero clipping; their correct PCM does not excuse the
first cue's late onset.

TypeScript and48 audio units pass, including the45 inherited cases and three
new coarse-clock/retry regressions. The unchanged strict pack validator
passes1,307 hashes and all85 reference-pack tests. The275 playable files,
source manifests, frozen thresholds, canonical world and renderer inputs are
unchanged. Cleanup asserts the fixture context closed, no voices/queue/cache
or pending loads, and its storage key removed. The full wrapper reports its
owned server, PostgreSQL, Xvfb/browser and scratch resources cleaned.

## Exact candidate and commands

The source bundle is a byte-identical contained copy at
`.local/audio-composed-source`; its world remains
`e550d3ed-37aa-487e-b67a-e0fa11bd540f` and source artifact remains
`5b3ba5f108ed3fec8f8b5f7f49b429c059e21b6f192ec99a0569616e08330059`.
The renderer input remains
`78fed3ca6b1549b329d6a48a47b35b68e912303b7d4698bb4cc0a8bdd2c34455`.
The old63b21e build was preserved. The final fresh output
`.local/web-builds/audio-composed-final` has build artifact
`311a088d30282b0e09348db29ee299f1347376c13013e9133831e712973e64f1`.
It uses the exact existing protocol/renderer WASM and glue, not rebuilt or
edited native components.

```sh
pnpm --dir web exec tsc --noEmit
node --test web/audio/*.test.ts

CLUBSCAPE_WEB_OUTPUT=.local/web-builds/audio-composed-final \
  pnpm --dir web exec vite build
CLUBSCAPE_WEB_OUTPUT=.local/web-builds/audio-composed-final \
CLUBSCAPE_CLIENT_MANIFEST=.local/audio-composed-source/content/manifest.json \
CLUBSCAPE_CLIENT_ASSET_ROOT=.local/audio-composed-source \
CLUBSCAPE_CONTENT_OWNER=game node tools/web-build/deliver.ts

# Recorded final invocation; do not rerun under this exhausted admission:
CLUBSCAPE_WEB_OUTPUT=.local/web-builds/audio-composed-final \
CLUBSCAPE_BROWSER_EXECUTABLE=/home/lramos15/.cache/ms-playwright/chromium-1243/chrome-linux-arm64/chrome \
CLUBSCAPE_BROWSER_EVIDENCE=.local/audio-composed-evidence/after-3-final \
CLUBSCAPE_RECORDED_CAMERA=tutorial-starting-house \
  pnpm --dir web test:isolated --game-root .local/audio-composed-source
```

## Actionable remaining blocker

The first-use2266 clip must be decoded before its first admitted control
deadline. A follow-on repair should prepare **that one source-bound active UI
control input** at its real native preference-binding/gesture-unlock lifecycle,
with explicit readiness/load failures, not insert test waits, preload unrelated
music/assets, delay the logical cue or suppress its error. The62.53ms
enqueue-to-decoded interval is measured; its network versus decoder/callback
breakdown is not yet isolated. Any follow-on change needs a fresh contained
build and newly authorized composed validation budget.

No source unlock/history/scene/varp/event authority was invented. Missing
spatial data still reports explicitly. No global sound/CPU/GPU/sandbox flags,
extra agents, speaker perception, Mac/Edge, full journey, frozen performance
or owner/M1 acceptance is asserted.
