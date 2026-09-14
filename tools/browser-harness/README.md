# Desktop browser capture and measurement harness

**Infrastructure, not M1 acceptance.** Nothing here implements ClubScape
presentation. The only included scene is an explicitly labeled one-triangle
tool fixture. It is not game art, a source reference, a representative workload,
or evidence of 60 FPS in the game.

Read [the host findings](../../research/browser-platform/sparky-2026-09-13.md)
and [the instrumentation contract](INSTRUMENTATION.md) before using results.
The master requirements remain `prompt.md` §§24, 30, 36 and
`spec/performance.md`; this package does not replace them.

For the owner's selected M-series Mac, use the
[owner-run Chrome/Edge handoff](../../research/browser-platform/owner-mac-handoff.md).
It includes native discovery, original app verification, probes, per-browser
configuration generation, forwarded-loopback operation, bundling and cleanup.
Mac paths are implemented and unit-tested but **not natively executed on
Sparky**. `pnpm test:browser` and the checked-in fixture config are Linux
validation, not a substitute for the Mac workflow.

## Install and verify

Run from this package directory, not the repository root. Node 24.18+ is
required. Dependencies and the native TypeScript compiler are pinned by the
local manifest and lockfile; there are no root-workspace changes.

```bash
cd tools/browser-harness
mkdir -p .runtime
TMPDIR=.runtime XDG_CACHE_HOME="$PWD/.runtime/cache" \
  pnpm install --frozen-lockfile --ignore-scripts
pnpm check
pnpm test
```

On the verified Sparky installation:

```bash
export BROWSER_EXECUTABLE="$HOME/.cache/ms-playwright/chromium-1243/chrome-linux-arm64/chrome"
pnpm test:browser
pnpm smoke
```

`smoke` starts a loopback-only, ephemeral-port fixture server, health-checks it,
opens real Chrome, captures and samples, then closes its browser, server and
display. The fixture requires the configured real mouse click and keyboard
event before reporting ready. Browser tests also use only their own servers;
the allowlist-negative test verifies that a second owned server receives
**zero** browser requests.

No process is detached. No browser download, OS package installation, display
configuration, driver change, AppArmor change or executable renaming is
performed. The package-local dependency store is `.runtime/pnpm-store`.
The two-column image from `smoke` must never be treated as a game screenshot.

## Headful display and sandbox

On Linux, `src/desktop.ts` uses an existing `DISPLAY`, or starts installed Xvfb on an
automatically selected display for the entire child process. The reproducible
screen is 2720×1600×24, large enough for the maximum 2560×1440 viewport plus
browser chrome. It uses MIT-MAGIC-COOKIE-1 authorization, no TCP listener,
`-nolisten unix -nolock`, and Linux's abstract Unix transport. Authority files
are private package-local files. There is no filesystem X11 socket or lock
file, no `mktemp`, and no external scratch directory.

Chromium needs a short Unix singleton-socket path: the child is run from this
package with a short **relative** `TMPDIR` under `.runtime/`. Profiles, home,
configuration and caches are isolated there and removed after the run.
The Linux executable itself remains at its original AppArmor-covered path.
On macOS the wrapper uses the native WindowServer desktop, not Xvfb/XQuartz.
It preserves `HOME` and the installed vendor app, isolates the browser profile,
and supplies Chromium's documented `MAC_CHROMIUM_TMPDIR` override for owned
temporary files. It does not apply Linux XDG/sandbox assumptions to macOS.

The browser is always `headless: false`, `chromiumSandbox: true`. Arbitrary
launch arguments are not accepted. `--enable-automation` enables command-line
attestation. Playwright's default `--enable-unsafe-swiftshader` is removed.
The exact effective command line is recorded, and sandbox-disabling switches
are rejected. No user-agent override exists.

Launch profiles are:

* `desktop-default`: installed desktop browser defaults, plus automation.
* `sparky-vulkan-x11`: the machine-record arguments
  `--enable-unsafe-webgpu --enable-features=Vulkan --use-angle=vulkan
  --enable-gpu --ignore-gpu-blocklist --ozone-platform=x11`.
* `mac-metal-default`: **no graphics overrides**, only the normal automation
  argument; macOS/ARM64 must be pinned and actual Metal compositor metadata
  observed. The renderer's WebGPU device is separately recorded.
* `linux-webgpu-default-x11`: a bounded diagnostic alternative without forced
  Vulkan/ANGLE selection. On Sparky it produced software llvmpipe and was
  rejected; it is not a verified replacement profile.

Linux namespace/PID/network/seccomp sandbox diagnostics are required.
**These do not imply the GPU process is sandboxed.** The working Sparky
Vulkan profile reports `gpuProcessSandboxed: false`; this is preserved in
reports. A real candidate benchmark refuses that profile. Fixture/capture
observations are not an all-process-isolation certificate. The attempted
early-GPU-sandbox option disabled hardware WebGPU and was not adopted.
The wrapper does not alter host security to address this.

macOS uses Chromium's Seatbelt sandbox, not Linux seccomp or the App Sandbox
entitlement. `chrome://sandbox` is not queried on Mac. Original app signatures
and ARM64 slices can be checked with native APIs, but they do not attest
runtime Seatbelt policies. Mac renderer attestation stays explicitly unknown;
the owner can collect native observations during a bounded probe hold.
Unknown Mac attestation does not masquerade as a pass or prevent measurement
solely because Linux diagnostics are absent. An explicit unsandboxed-GPU
report still rejects a candidate benchmark. The existing Linux GPU sandbox
guard is unchanged. All reports say `securityCertification: not-performed`.

## Real-client invocation

`config/candidate.template.json` is **deliberately incomplete and rejected**.
Before implementation/measurement, the Director must replace every required
input and freeze the pre-measurement contract. Its numeric defaults are proposals,
not owner approval. In particular provide:

* approved source-pack ID, repository-relative file path, SHA-256 of that
  file's exact bytes, and owner approval reference;
* real build/artifact, asset-manifest/settings digests, capture case, loaded scene,
  representative route/workload, required asset IDs/hashes and entity counts;
* a reviewed representative integrated-graphics hardware/browser contract,
  model/CPU/memory/OS/driver evidence and exact observed adapter fields;
* approved viewport/layout/scale, resizing range, settings and sampling
  budgets; the example range 1280×720–2560×1440 is only a tool proposal;
* actual browser product, exact version and original executable location.

The source-pack approval is mandatory. A separate benchmark-owner approval
reference is optional metadata, not another product checkpoint; do not
fabricate one. The Mac generator uses the existing owner's hardware-direction
record for the selected M-series family and actual discovery/probe data for
the exact machine/browser. It never treats that record as source-pack or
final presentation approval.

The source-pack object has this shape (values must be real, not placeholders):

```json
{
  "id": "owner-approved-pack-id",
  "path": "research/approved-pack/manifest.json",
  "sha256": "<64 lowercase hexadecimal digits>",
  "ownerApprovalRef": "the recorded owner approval"
}
```

There is no required gamepack number. A newer usable source is acceptable
only when it belongs to the owner's approved pinned pack.

Start the actual client/server separately; this tool does not seed accounts,
implement routes, or certify gameplay. With its test URL explicitly allowed:

```bash
export BROWSER_EXECUTABLE=/absolute/path/to/the/real/browser
node src/cli.ts --config config/your-approved-case.json --print-contract-hash
pnpm capture --config config/your-approved-case.json --run-id chrome-case-run-1
```

Use a separately version-pinned config with `"product": "edge"` and the
**actual Microsoft Edge executable** for Edge. Chrome pretending to be Edge,
Chromium presented as Google Chrome, mismatched executable versions, and
software backends are rejected. Microsoft does not currently publish the
needed native Linux ARM64 Edge package; see the findings.
That Linux availability limitation does not apply to the owner's supported
native Mac Edge path.

## Capture classifications and output

Each new `runs/<run-id>/` is created exclusively; a reused ID fails rather
than overwriting evidence. The CLI exits nonzero on failures, missing inputs,
invalid samples or candidate budget failures, and prints a concise result.
Full diagnostics are persistent in:

* `report.json`: UTC timestamps, exact configuration/contract digest,
  executable version/hash, browser CDP/version/brand data, backend/adapter,
  viewport/DPR, desktop input settings, images/hashes/pixel checks,
  frame distributions/stalls and cleanup status;
* `events.json`: page console, HTTP responses/failures, denied requests,
  uncaught errors, crashes and executed desktop inputs;
* `rendered-frames.json`: completed-frame records and polling observations
  for a completed benchmark window;
* separately named viewport and surface PNGs, before/after measurement and
  at both resize endpoints. Screenshot work is outside the measured window.

Prefixes and report purposes distinguish `tool-fixture`, `candidate` and
`source-observation`. All reports set `baselineApproved: false` and
`m1Acceptance: "not-evaluated"`. There is no baseline-promotion command.
The fixture marker cannot be used as a candidate or source reference.

Generated inventories/configs and bundle directories live under ignored
`owner-runs/`. `pnpm run handoff bundle` copies only ordinary JSON/PNG artifacts,
preserves failed results, records platform labels and SHA-256 hashes, and
excludes browser profiles/storage. `pnpm run handoff cleanup` verifies the
bundle and raw-run bytes before removing only the named raw run directories.
See the owner handoff for exact commands and privacy review.

`"mode": "capture"` does not assert rendered-frame performance. A
`source-observation` is capture-only and additionally requires
`sourceObservation: { referenceBuild, sourceSnapshot, provenance }`; it is an
**unapproved source observation**, not a fidelity baseline. Source observation
can capture a non-WebGPU source surface; candidate/fixture captures require
an actually configured hardware WebGPU canvas. Both still record the browser
backend and check screenshots.

Black, white, transparent and almost-uniform screenshots are rejected.
Both the entire viewport and the selected render surface are checked, so a
label outside an otherwise blank canvas cannot hide the failure. Native
canvas backing size is checked against CSS size at DPR 1. Nonblank pixels
alone do not prove correct gameplay, content, audio or visual fidelity.

Only explicit `http://127.0.0.1:<port>` origins are allowed; navigations,
redirects, fetches and WebSockets are routed accordingly. No wildcard,
remote host, credentials, alternate loopback spelling, file URL or service
worker is accepted. This is application-request containment, not a host
firewall or proof that all browser-internal networking is disabled.

Use synthetic test accounts. Reports may contain console text and test URLs;
do not commit secrets or private authenticated captures. Run artifacts are
ignored by Git. The deliberately labeled evidence under
`research/browser-platform/` is tool evidence only.

## Limits of a successful command

A valid candidate measurement only establishes this harness's preconditions
and the declared per-case numeric checks. Source-pack/owner reference strings
are attestations to be reviewed, not cryptographic proof of an approval.
Client workload/asset/entity declarations require review against the real
loader and renderer; the harness cannot prove that a dishonest scene manifest
describes the pixels.

Source comparisons with pre-agreed visual tolerances, animation/audio,
fresh-account gameplay, persistence, both-browser coverage, representative
hardware and explicit owner presentation acceptance remain separate gates.
Neither this package nor the tiny fixture certifies M1.
