# Native2266 control-input readiness

This is the new bounded admission
`ebabe42b0750d0134d9a9d1b99dd1c4a87314dc0`, based on
`66c6d873603b423f856549363ca6acc4f282c564`. It addresses the distinct cold-input
cause, not another attempt under the exhausted earlier three-run budget.
The original failed reports and ready-cue scheduling repair remain intact.

## Lifecycle and admission

`createAudio(ClientAssets, report)` keeps its existing asynchronous ABI. After
validating the frozen catalog and creating its real AudioContext, it prepares
**only** original index4/group2266 using the existing hash-checked transport,
decoder and signal validation. Its promise does not resolve to a usable
handle until this input is ready. Preparation failure rejects initialization,
reports the original failure and disposes the cache, graph and context; there
is no silent ready handle or automatic fallback/retry.

`SourceControlInput` exposes pending/ready/failed/disposed state. Actual
snapshots include optional `controlInput`; traces include
`control_input_pending`, `control_input_ready`, `control_input_failed` and
`control_input_disposed`. Pending readiness is an input-initialization state,
not an accepted control awaiting playback. An attempted admission without a
ready input throws before creating a source deadline.

The decoder's2266 buffer is retained in the existing96MiB cache. It occupies
11,024 decoded bytes and survives ordinary music/SFX LRU eviction,
disconnect/re-entry and character changes. It is public source input, not
player history; disposal releases it. Retention does not preserve transient
voices, controls, source-scene values, player preferences or authorization.

Control admission uses the already verified buffer synchronously. The normal
source-cycle service runs in a microtask after the synchronous control
operation, preserving its volume/mute ordering and the existing
ready-reservation behavior. There is no fetch/decode wait after a click has
been admitted, no moved `requestedAt`/`dueAt`, no delayed UI persistence queue,
and no blanket music/effect preload.

The required cue's pending/failed state is established at the real factory
lifecycle, before shell/UI binding. No additional app/composition API or
ownership change is required. `unlock()` still invokes the actual
`AudioContext.resume()` in the trusted handler **before any await**. A decoded
input is not autoplay permission: factory resolution leaves `unlocked=false`,
`pendingGesture=true`, zero voices and zero source queue entries.

2266 remains only the source-bound native control cue. It is not added to
arbitrary clicks. Other effects/music stay demand loaded. FIFO50, source
countdown/grace, delay2/call3, eating/call4, weighted silence, original PCM,
offsets, jingle policy and actual source authority boundaries are unchanged.

## Timing and deterministic regressions

Decoded notices now separate:

* `fetchAndVerifyMs`: same-origin fetch, bounded byte reading and SHA checking
  together; this is **not** asserted to be network time alone.
* `decodeMs`: time awaiting the actual decoder.
* `signalValidationMs`: decoded-format/finite/peak checks and cache insertion.

The interval from the decoded trace to `control_input_ready` records promise
completion separately. These measurements do not retroactively explain the
old62.53ms interval as a network failure.

The deterministic suite verifies shared in-flight preparation, explicit
pending/failed admission, no automatic failed-input retry, disposal during
preparation, a delayed required decode keeping the factory unresolved, corrupt
bytes rejected before decode, missing HTTP input, decoder rejection and full
failed-initialization cleanup. It also verifies that no other playable input
is requested during readiness, actual resume is invoked synchronously, and
the ready reservation retains its native gain update, zero-volume
cancellation, queue and re-entry/reset behavior.

The whole-client fixture retains all21 source/UI/restart/device-loss checks,
six native storage/control contracts and all18 real2266 dispatches. It now
requires the initial2266 decoded and ready traces **before the first actual
enqueue**, while factory initialization has no voices and no unlock claim.
Both six-call timing comparisons, the unchanged20.045351473922903ms limit,
native loading retries1..10, actual start-call overrun check and original
error checks remain in force.

Post-gain AnalyserNode comparisons stay qualified: they compare actual output
to the submitted decoded buffer on naturally completed voices. Explicitly
cancelled voices remain recorded separately. These are not new independent
original-server timing or physical-speaker captures.

## Committed build and two-run allowance

The implementation must be committed before the normal production build:

```sh
pnpm --dir web exec tsc --noEmit
node --test web/audio/*.test.ts

CLUBSCAPE_WEB_OUTPUT=.local/web-builds/audio-control-readiness \
CLUBSCAPE_CLIENT_MANIFEST=.local/audio-composed-source/content/manifest.json \
CLUBSCAPE_CLIENT_ASSET_ROOT=.local/audio-composed-source \
CLUBSCAPE_CONTENT_OWNER=game pnpm --dir web build
```

That is the normal protocol + renderer + typecheck + Vite + delivery path,
not a diagnostic/bundler-only substitute. Cargo uses this worktree's default
target; no target from another worktree/source profile is shared.

The source remains the exact contained bundle `.local/audio-composed-source`,
world `e550d3ed-37aa-487e-b67a-e0fa11bd540f`, source artifact
`5b3ba5f108ed3fec8f8b5f7f49b429c059e21b6f192ec99a0569616e08330059`.
No old world is repinned. Each of the two newly authorized invocations uses
its own fresh browser profile/context and evidence directory:

```sh
CLUBSCAPE_WEB_OUTPUT=.local/web-builds/audio-control-readiness \
CLUBSCAPE_BROWSER_EXECUTABLE=/home/lramos15/.cache/ms-playwright/chromium-1243/chrome-linux-arm64/chrome \
CLUBSCAPE_BROWSER_EVIDENCE=.local/audio-readiness-evidence/cold-1 \
CLUBSCAPE_RECORDED_CAMERA=tutorial-starting-house \
  pnpm --dir web test:isolated --game-root .local/audio-composed-source
```

The second invocation changes only the evidence directory to `cold-2`.
There is no additional pre-repair/standalone browser run, test sleep, special
game-UI teardown, removed cue, changed tolerance or automatic third attempt.
Both outcomes must be retained. A result is not accepted merely because an
intermediate attempt passed.

No absent source scene, unlock/history, varp or committed gameplay event is
invented. All275 original/additive files and1,307 frozen hashes remain
unchanged. Browser output stays locally muted; no speaker, Mac/Edge, frozen
workload, full journey, owner/M1 or later-milestone acceptance is implied.
