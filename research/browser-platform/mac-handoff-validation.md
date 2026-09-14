# Mac handoff tooling validation - 2026-09-14

Scope: the bounded `m1-mac-handoff` implementation on Sparky, starting from
`bb62a83`. **No native Mac or Mac Edge execution occurred.** The owner-run
workflow is [owner-mac-handoff.md](owner-mac-handoff.md).

## Verified locally

* Locked package-local installation and `pnpm check` passed.
* `pnpm test`: **79 tests passed**. The original 60 cases are retained, with
  one additional deployed-artifact identity check and 18 owner/platform
  cases. Mac command responses, signatures and hardware in unit tests are
  explicitly marked **UNIT MOCK ONLY**, not real observations.
* `pnpm test:browser`: **15 actual Linux Chrome cases passed**. The original
  14 cases still work; the additional case verifies optional runtime
  `bindRun` receives only audit contract identity, not scene/build/assets
  or invented metrics.
* The existing `pnpm smoke` still launches original Chrome for Testing
  153.0.8010.12 with sandbox enabled, draws/captures the labeled tiny fixture,
  records GPU-completed frames and cleans its owned processes/runtime.
* `pnpm run handoff discover` found that original cached Chrome, recorded
  real Linux/ARM64 facts and recorded Edge unavailable. It did not produce
  a Mac availability or acceptance claim.
* `pnpm owner:probe` with the existing Sparky Vulkan profile and a one-second
  inspection hold exercised real launch, live PID collection, capture and
  automatic cleanup.
* CLI `bundle` preserved both a valid tool observation and a deliberate
  failed graphics trial. CLI `cleanup` verified the archived/raw bytes before
  deleting only those two raw run directories.

The commands were run from `tools/browser-harness`:

```bash
pnpm check
pnpm test
BROWSER_EXECUTABLE="$HOME/.cache/ms-playwright/chromium-1243/chrome-linux-arm64/chrome" pnpm test:browser
BROWSER_EXECUTABLE="$HOME/.cache/ms-playwright/chromium-1243/chrome-linux-arm64/chrome" \
  pnpm smoke --run-id linux-mac-handoff-smoke
pnpm run handoff discover --out owner-runs/sparky-inventory.json
pnpm owner:probe --inventory owner-runs/sparky-inventory.json --browser chrome \
  --profile sparky-vulkan-x11 --inspect-seconds 1 --run-id sparky-owner-handoff-probe
```

## Bounded alternative graphics trial

One headful/Xvfb trial used `linux-webgpu-default-x11`, which retains the
existing per-launch WebGPU/blocklist options but does not force Vulkan or
ANGLE selection:

```bash
pnpm owner:probe --inventory owner-runs/sparky-inventory.json --browser chrome \
  --profile linux-webgpu-default-x11 --run-id sparky-default-graphics-trial
```

The actual compositor renderer was:

```text
ANGLE (Mesa, llvmpipe (LLVM 20.1.2 128 bits), OpenGL 4.5 (Core Profile) Mesa 25.2.8-0ubuntu0.24.04.2)
```

The browser reported WebGPU enabled, but GPU sandbox false and the owned
GPU process still showed `NoNewPrivs: 1`, `Seccomp: 0`, `Seccomp_filters: 0`.
The harness **rejected** the software backend before treating it as a
rendered-workload pass. The profile was not adopted as a replacement.

The projected, genuinely local receipts are retained in
[mac-handoff-local-observations.json](mac-handoff-local-observations.json).
They preserve each run's timestamp, Linux platform, exact executable/hash,
renderer, GPU-process observations, failures and confirmed cleanup; no Mac
result is synthesized. The diagnostic profile removes the Linux Vulkan
feature override described by
[Chromium's GPU feature definition](https://chromium.googlesource.com/chromium/src/+/153.0.8010.12/gpu/config/gpu_finch_features.cc),
rather than weakening a sandbox.

The established `sparky-vulkan-x11` profile continued to produce actual
NVIDIA GB10/Vulkan nonblank fixture captures, with the same recorded GPU
seccomp limitation. No pure-headless screenshot loop, global graphics/
security change, driver/browser replacement, sandbox-disable switch,
original-game account interaction or external-machine access was used.

## Deliberate policy boundaries

The Linux candidate GPU-sandbox guard remains. Mac uses Seatbelt-aware
reporting rather than `/proc` or fabricated Linux sandbox-page text.
Missing Mac runtime attestation remains **unknown**, not a security pass;
an explicitly unsandboxed GPU still fails candidate benchmark validation.
All security-disabling launch switches remain forbidden.

The extra benchmark-owner approval check was removed because a separately
approved benchmark is not a literal product checkpoint. Pre-measurement
contract identity/digest, fixed budgets, actual hardware/browser pins and
source-pack approval remain required. The existing owner's M-series
selection is used as hardware direction; it does not approve a source pack,
invent exact machine results or grant final visual/audio acceptance.

Native Mac availability, original vendor signature results, actual Metal/
WebGPU operation, per-role Seatbelt observations, the real source/workload
matrix and product measurements remain the owner's execution responsibilities.
The Director must supply the actual approved source pack/product case and
wire dynamic build-artifact identity plus optional audit-only `bindRun`
(or matching preconfigured contract hashes) in the real renderer.
Neither local tests nor unit mocks certify those gates.
