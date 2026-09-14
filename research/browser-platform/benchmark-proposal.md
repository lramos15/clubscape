# M1 browser measurement proposal — not approved acceptance

Authored 2026-09-13 EDT, before any real ClubScape candidate measurement.
Only a labeled tool fixture has been measured. No presentation implementation
or source reference pack is supplied by this workstream.

## Unchanged mandatory target

60 **rendered** FPS, 1920×1080 browser viewport, desktop keyboard/mouse,
WebGPU, both actual Google Chrome and actual Microsoft Edge, on an
owner-approved representative modern integrated-graphics environment.
Use the approved stock visuals, resizable-classic layout, UI scale, camera,
draw distance and real tutorial/Lumbridge workloads. Do not substitute a
reduced scene/resolution/draw distance, software renderer, unrelated hardware
run, configured limiter or raw RAF rate.

GB10's integrated classification is established by Vulkan and corroborated
by unified-memory architecture evidence. Its **representativeness is not
established**. Neither "NVIDIA" nor "integrated" by itself settles the latter.

## Proposed numeric protocol

| Input/check | Proposal, to freeze before candidate evaluation |
| --- | --- |
| Primary viewport | 1920×1080 CSS and screenshot pixels; DPR 1 |
| Warmup | At least 5 seconds after fully loaded/readiness-verified state |
| Window | At least 60 seconds per case/run; actual elapsed time recorded |
| Poll interval | 250 ms |
| Missing-progress deadline | 1000 ms since last GPU-completed rendered frame |
| Completed-render throughput | At least 60.0 FPS, **without rounding tolerance** |
| Frame interval p95 | At most 20 ms |
| Frame interval p99 | At most 33⅓ ms |
| Stall threshold | Interval or final tail strictly greater than 50 ms |
| Stall rate | At most 1 per measured minute |
| Largest interval/final tail | At most 100 ms |
| Repetitions | Proposed three independent windows per case per browser; preserve failures rather than averaging them away |
| Resize endpoints | Tool example 1280×720 and 2560×1440; product range still needs source-pack approval |

The harness enforces the per-window numeric checks and both configured resize
endpoints. It does not orchestrate or certify the proposed three-run matrix.
The Director must record the matrix and all runs. The fixture uses a shorter
2.5-second plumbing window and has **no applicable performance-pass decision**.

Frame counters advance after actual rendered WebGPU work completes, with
contiguous per-frame submission/completion records and observed native queue/
canvas activity. See the [instrumentation contract](../../tools/browser-harness/INSTRUMENTATION.md).
CPU encoding, GPU completion wall latency, optional timestamp-query render-pass
time and post-window queue-drain time have distinct labels. Missing GPU-query
coverage remains missing. This does not measure physical scanout.

## Preconditions and unresolved inputs

Before a real candidate run, the Director/owner must pin:

1. Owner-approved source pack: build/date/source snapshot, actual assets and
   source captures, pack file SHA-256, allowed adaptations and approval.
   A latest usable gamepack may be selected; no hard-coded 2695 requirement
   is introduced here.
2. Benchmark contract ID and digest, owner approval reference, actual client
   build/settings/asset-manifest digests, matched capture case and source
   reference identity. Freeze these before measuring.
3. Representative environment: actual GPU/model/device-type and memory
   architecture evidence, CPU, capacity, OS/driver, exact Chrome/Edge builds,
   sandbox/graphics flags, display/viewport/UI settings, and an explicit
   representativeness decision. Current local Edge and GPU-sandbox gaps are
   in the [platform findings](sparky-2026-09-13.md).
4. Source-defined tutorial and Lumbridge routes and loaded-world contents,
   initial cameras, animation/UI state, real asset integrity checks and
   required visible/entity workloads. Include the one-to-five-player cases;
   launch-concurrency workload remains an input, not a guessed target.
5. Per-case visual/audio comparison procedure and numeric fidelity
   tolerances, fixed before candidate evaluation. This package provides
   captures and blank-output rejection, **not source-image tolerances**.

The CLI refuses candidate benchmarks without approved source/contract/hardware
references, the minimum window/warmup, an observed nonfallback renderer device
and a verified GPU-process sandbox. It never grants M1 acceptance.
Reference strings are reviewable attestations, not proof of owner identity.

## Separate acceptance gates

Even a valid, numerically passing window cannot establish:

* full source-faithful scenes, interfaces, sprites, animation and audio;
* legitimate fresh-account sign-up/tutorial/Lumbridge/quest progression;
* persistence, restart/reconnect/death recovery and server correctness;
* both-browser coverage from one browser result;
* representative hardware from a vendor name or a tool-only fixture;
* explicit owner visual/audio presentation acceptance.

Keep candidate renders, unapproved source observations and approved external
reference-pack fixtures distinct. Never promote the first client output into
its own source baseline. Parent-owned M1 acceptance stays blocked until its
actual inputs and evidence exist.
