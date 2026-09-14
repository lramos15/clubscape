# `window.__clubscapeBenchmarkV1`

Implement [the exported types](src/protocol.ts) in the real renderer only
after the source-pack checkpoint. This document describes instrumentation,
not permission to implement presentation before that approval.

## Interface

```ts
window.__clubscapeBenchmarkV1 = {
  read(afterFrame: number | null): RenderSnapshot {
    // Return current state and every GPU-completed render frame after afterFrame.
    // A null initial read returns state plus frames: [].
  }
};
```

`read` is synchronous, side-effect-free and uses `performance.now()` from the
same window. It must not drive rendering, synthesize frames or reset counters.
The version-1 observer supports a main-thread WebGPU canvas, including WASM
calling that API. Worker/OffscreenCanvas instrumentation needs an explicitly
reviewed extension; the current harness rejects an unobserved device/canvas
instead of treating a separate adapter probe as renderer evidence.

### Identity and readiness

Return `version: 1`, `clock: "performance.now"`, `application: "clubscape"` and
`backend: "webgpu"`. `ready` becomes true only after the declared scene,
workload and required assets really finish loading.

`identity` must match the configuration exactly:

* `buildId`, `sceneId`, `routeId`, `workloadId`;
* `sourcePackSha256`: digest of the approved source-pack file;
* `benchmarkContractSha256`: digest of canonical JSON for the whole
  `config.contract` object (recursive lexicographic object keys, compact
  JSON, UTF-8). Obtain it using `--print-contract-hash`;
* `assetManifestSha256` and `settingsSha256` from the actual loaded build.

The contract digest covers workloads, pins, settings, hardware declarations,
resizing, input setup and numeric budgets. Changing them after seeing a run
creates a different contract and does not repair that run.

Expose current `viewport: { width, height, deviceScaleFactor }`, loaded
`assets: [{ id, sha256, loaded }]`, actual `entities` by the contract's count
keys, and a `deviceEpoch` that changes on device recreation. Missing assets,
wrong hashes, insufficient workload, changed identities or a device reset
invalidate the window. Merely copying a manifest into these fields is not
loading evidence; review the actual loader/state wiring.

### What counts as one rendered frame

1. The real renderer must encode the declared loaded scene into the capture
   canvas's current WebGPU texture, using actual nonzero draw/primitive counts.
2. Submit those command buffers successfully with `GPUQueue.submit`.
3. Associate that frame with its asynchronous
   `queue.onSubmittedWorkDone()` receipt, registered after submission.
4. **Only when that receipt resolves**, publish its frame record and advance
   `renderedFrames` to the highest contiguous completed frame sequence.

Do not await each receipt before starting the next frame. Keep normal
rendering pipelined and collect receipts asynchronously. Neither a configured
60-FPS limiter, RAF count, timer, empty command buffer, standalone compute
probe, nor a submit-only counter is a completed render frame.

Every `RenderFrame` contains:

| Field | Meaning |
| --- | --- |
| `sequence` | Safe integer, starting at 1; exactly contiguous, never reset within a window |
| `submittedAtMs` | Window `performance.now()` immediately after the real render submission |
| `completedAtMs` | Same clock when that submission's queue-completion receipt resolves |
| `drawCalls`, `primitives` | Actual positive counts encoded for that frame, not constants guessed by the instrumentation |
| `cpuEncodeMs` | Optional measured CPU encoding/submission wall duration |
| `gpuDurationMs` | Optional timestamp-query duration for the explicitly declared render passes |

`renderedFrames` counts GPU-completed render frames.
`lastSubmittedAtMs` and `lastCompletedAtMs` refer to that **last completed
frame**, not newer work still in flight. `nowMs` is fresh at each read.
Keep at least enough records for the poll interval and declared stall budget;
the v1 transport accepts up to 4096 records per read. Lost/overwritten records
invalidate a run, rather than being extrapolated into FPS.

Submission timestamps strictly advance. Completion timestamps are
nondecreasing (clock quantization/batched callback delivery can produce ties).
Completions must be no earlier than their own submissions and no later than
the snapshot. A long batch gap remains a stall even when many callbacks
arrive together.

The harness independently wraps the **actual** main-thread
`GPUAdapter.requestDevice`, canvas configure/current-texture acquisition,
`GPUQueue.submit` and queue-completion receipts. It records the adapter of
the device that configured the selected canvas, catches device loss and
uncaptured GPU errors, and rejects counters exceeding observed completions
or canvas acquisitions. Receipts may straddle the sampling boundary: an
already-acquired texture may complete during the next sample.

This observer adds one asynchronous completion receipt per queue submission,
in addition to the renderer's instrumentation. That overhead is included in
the measurement and must be held constant across compared runs. There is no
per-frame GPU readback/serialization requirement, and no FPS limiter added
by the harness.

The checks catch accidental RAF/submit-only counters and missing work, not
an adversarial renderer that lies about its loaded scene or repaints an image.
The Director's technical review must connect the records to the real draw
path, asset loader and workload. The source-fidelity checkpoint separately
prohibits static source screenshots presented as a game renderer.

## Measurement semantics

* Initial screenshots and input setup occur before warmup; resizes and final
  screenshots occur outside the measured window.
* The measured window uses the harness's sampled window `performance.now()`,
  not an elapsed duration supplied by the application.
* `renderedFps = completedFrameCount * 1000 / measuredWindowMs`. No rounding,
  extrapolation, callback rate or nominal display refresh replaces it.
* `frameIntervalMs` is the distribution of consecutive GPU-completion
  callback intervals, including the previous completed frame at the leading
  boundary. The final no-completion tail is included in gap/stall checks.
* `gpuCompletionLatencyMs` is submission-to-receipt wall time: queue backlog,
  GPU work, IPC and callback scheduling. It is **not isolated GPU frame time**.
* A single post-window queue-drain measurement is separately labeled. It
  does not add frames to the measured window or replace per-frame receipts.
* Optional `gpuDurationMs` requires `timestamp-query` in the **device's enabled
  features**, not just adapter support. Report render-pass query duration in
  milliseconds, its precise pass scope, and sample coverage. Missing timings
  remain `null`, not invented zero-cost GPU work.

WebGPU completion proves completion of submitted work, **not physical
presentation/scanout**. Xvfb is not a physical display benchmark. Browser
compositor/presentation analysis and visual review remain separate evidence.

See [the pre-measurement proposal](../../research/browser-platform/benchmark-proposal.md)
for draft budgets and pending approval inputs. Real acceptance still requires
the actual approved source pack, world workloads, both browser runs and the
Director's separate acceptance record.
