# TOOL FIXTURE ONLY — not ClubScape, a source pack or M1 acceptance

These are copied, hash-verified outputs of:

```bash
cd tools/browser-harness
BROWSER_EXECUTABLE="$HOME/.cache/ms-playwright/chromium-1243/chrome-linux-arm64/chrome" \
  pnpm smoke --run-id tool-fixture-receipt-2026-09-14
```

The run started at `2026-09-14T00:29:46.881Z`. The report records the exact
browser/configuration, executable digest, real renderer-device observation,
viewport and canvas backing sizes, desktop input and cleanup.

`manifest.json` lists SHA-256 and byte size for every copied generated artifact:

* `report.json`, `events.json`, `rendered-frames.json`;
* primary before/after viewport and surface PNGs;
* minimum/maximum resize viewport and surface PNGs.

All images deliberately show only a colored split rendered by one WebGPU
triangle. Viewport images include a prominent **TOOL FIXTURE ONLY** header.
The separate surface captures exclude that header so it cannot conceal a
blank canvas. There is no source game imagery or generic replacement game UI.

The report's `valid-measurement` status means tool plumbing was valid.
`budgetDecision` is **`not-applicable-tool-fixture`**,
`m1Acceptance` is **`not-evaluated`**, and `baselineApproved` is **false**.
The source pack is null and the hardware contract unapproved.
These images must never be promoted to source references or product baselines.

GPU timing here is completion-receipt wall latency, not timestamp-query GPU
execution time or physical presentation. Renderer namespace/seccomp passes,
but the GPU process reports no seccomp filter; the harness refuses real
candidate benchmarks under that condition. Real Edge was unavailable.

Historical paths, port numbers and PIDs in the receipt identify that completed
run, not live services. Its raw run folder may be removed after archiving;
the files in this directory are the persistent receipt. The source/contract
digests and image hashes were verified during archiving.
