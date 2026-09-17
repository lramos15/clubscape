# Owner-run M-series Mac Chrome + Edge handoff

**Ready-to-run tooling, not a Mac result or M1 acceptance.**
The code was implemented on Sparky/Linux. Native Mac executable discovery,
code-signature checks, WindowServer launch, Metal observations and Seatbelt
collection have **not been executed on a Mac here**. Unit mocks exercise
branching only and are explicitly labeled; they are not hardware evidence.

The owner selected their M-series Mac in
[`milestones/m1-owner-followups.json`](../../milestones/m1-owner-followups.json).
Apple Silicon supplies integrated graphics. The exact model/chip/memory,
macOS build and both installed browser versions must come from the owner's
native run. Available Sparky checks remain useful engineering observations,
not Mac/Edge or final performance evidence.

## 1. Prepare the owner's existing native desktop

Use a logged-in graphical macOS session on the M-series Mac, not a remote
Linux terminal, Rosetta Node process, VM proxy result or XQuartz display.
Use the original installed vendor apps, normally:

```text
/Applications/Google Chrome.app/Contents/MacOS/Google Chrome
/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge
```

User-local `~/Applications` equivalents are also discovered. Open installed
browsers normally once if needed for the OS's ordinary first-launch check;
no Google/Microsoft sign-in or original-game account is needed. This tooling
does **not** install browsers/OS packages, change Gatekeeper/SIP/TCC/AppArmor,
re-sign or rename executables, remove quarantine, disable a sandbox or
change global graphics settings. If a prerequisite is missing or invalid,
record it and use the vendor-supported owner installation/update process
outside the harness; do not bypass the check.

Have native ARM64 Node 24.18+ and pnpm 12.4.1 available. Do not run a global
installer from these instructions. In the supplied repository checkout:

```bash
cd tools/browser-harness
node -p 'process.platform + " " + process.arch + " " + process.version'
pnpm --version
mkdir -p .runtime
TMPDIR="$PWD/.runtime" XDG_CACHE_HOME="$PWD/.runtime/cache" \
  pnpm install --frozen-lockfile --ignore-scripts
pnpm check
pnpm test
```

The first line must report `darwin arm64`, not `linux` or `darwin x64`.
The existing `pnpm test:browser`/`pnpm smoke` fixture configuration is explicitly
Sparky/Linux; use the following Mac discovery/probe commands instead.

## 2. Discover exact local facts, without secrets

```bash
pnpm run handoff discover --out owner-runs/mac-inventory.json
```

This records only model/chip/cores/memory/macOS, selected display/GPU fields,
original browser paths, actual `--version`, binary SHA-256 and signature
identity. It does not collect serial numbers, hardware UUIDs, account IDs,
keychain contents, credentials, browser profiles or broad system dumps.
Do not rename/copy a browser to satisfy discovery. For a legitimate
nonstandard vendor-app location, provide `--chrome "/original/.../Google Chrome"`
or `--edge "/original/.../Microsoft Edge"`.

Native read-only mechanisms:

| Mechanism | What it establishes |
| --- | --- |
| `sw_vers -productVersion/-buildVersion` | Exact macOS release/build |
| `sysctl -n hw.model`, `machdep.cpu.brand_string`, `hw.memsize`, `hw.ncpu`, `hw.optional.arm64` | Model, chip, memory, cores and ARM hardware facts |
| `system_profiler -json SPDisplaysDataType` | Selected GPU model/vendor/Metal-support fields only; serial-bearing nested data is discarded |
| `plutil -extract ... raw -o - Info.plist` | Original vendor bundle ID, executable name and version |
| Read-only Mach-O header parsing | Native ARM64 slice exists; no `lipo` or Xcode installation |
| `codesign --verify --deep --strict -R ...` and `codesign -dv --verbose=4` | Apple-anchored original vendor bundle/signing identity; **not runtime sandbox attestation** |
| Actual executable `--version` and SHA-256 | Exact selected product/version/artifact |

The code accepts the original signed Chrome/Edge vendor bundles, not a
renamed Chromium or ad-hoc re-signed copy. Chrome's ordinary UA may contain
`Intel Mac` even in a native ARM build; that text is **not** used to infer
Rosetta. The harness uses ARM64 slices, native Node and UA-CH architecture
where available. The owner can corroborate process `Kind` in Activity Monitor.

Discovery is availability of executables, **not** a GPU launch/performance pass.
An unavailable browser remains unavailable in the JSON; no version is guessed.

## 3. Probe each real browser with a labeled tool fixture

```bash
pnpm owner:probe --inventory owner-runs/mac-inventory.json \
  --browser chrome --run-id mac-chrome-probe
pnpm owner:probe --inventory owner-runs/mac-inventory.json \
  --browser edge --run-id mac-edge-probe
```

Each command starts and health-checks its own loopback-only fixture server,
opens the original pinned browser, requires actual keyboard/mouse input,
draws a one-triangle WebGPU fixture, checks nonblank screenshots and both
resize endpoints, records GPU-completed frames and closes owned resources.
The fixture is not ClubScape or source-fidelity/performance evidence.

On Mac the profile is `mac-metal-default`: **no forced graphics flags**.
It uses the native WindowServer and browser defaults, requires actual
Metal compositor/ANGLE metadata plus an actually configured nonfallback
WebGPU device, and records these separately. CDP compositor metadata is not
an independent statement of the WebGPU backend or physical presentation.
There is no WebGL/SwiftShader fallback, UA spoof, frame limiter or replacement
game art. Linux/Xvfb/Vulkan options cannot be applied to a Mac config.

The private user-data directory and managed temporary files are under
`.runtime/`. macOS uses Chromium's documented `MAC_CHROMIUM_TMPDIR` override;
the original app bundle and home environment are preserved. This isolates
harness-managed files, not all OS-managed caches/logs. No browser is detached.

### Optional bounded native inspection

To give the owner time to inspect the **owned** processes, add
`--inspect-seconds 120` to a probe with a fresh run ID. The hold is capped
at 300 seconds, occurs outside measurement and is unavailable for candidate
benchmarks. It prints `runs/<id>/live-inspection.json` with current owned PIDs,
roles, executable and available sandbox observations.

While that probe is held open:

1. Open Activity Monitor. Select only the listed owned PIDs and inspect their
   process details. In **View > Columns**, record `Kind` and `Sandbox` if that
   macOS release offers them. Do not capture unrelated processes/accounts.
2. Record what was actually shown, together with PID/role, time and macOS
   build. If a field is unavailable, write **unknown**, not yes/pass.
3. Save narrow notes as `runs/<id>/owner-native-notes.json`, or a deliberately
   scoped diagnostic image named `owner-native-processes.png`. Do not copy
   credentials or private unrelated windows into the evidence.
4. Let the hold finish or press Ctrl-C. The wrapper closes its own browser,
   server and any managed display. No `killall`, global policy edit or
   sandbox-disabling relaunch is part of this workflow.

Chromium uses **Seatbelt (`sandbox(7)`)**, not the Mac App Sandbox entitlement.
An absent `com.apple.security.app-sandbox` entitlement is not evidence that
Chrome's renderer sandbox is off. `chrome://sandbox` is not registered for
Mac in the examined Chromium source; Linux namespace/seccomp text is never
used as Mac evidence.
Interpret observations by process role: the main browser/broker is not
expected to have the same sandbox as renderer/GPU processes. Do not require
every entry in the browser's process tree to say "sandboxed."

Supported Node/CDP APIs do not establish every renderer's active Seatbelt
policy. Accordingly the report retains
`nativeAttestation: "unknown-owner-collection-required"` and
`securityCertification: "not-performed"`, including after a good signature
check or successful capture. The optional CDP GPU `sandboxed` boolean is
recorded as a **browser report**, not an independent OS certificate.
Unknown Mac attestation does not become a new Linux-style measurement gate.
An explicit unsandboxed-GPU report is still refused for candidate benchmarks,
and all sandbox-disabling launch switches remain rejected.

Do not use the sandbox-disabling debugging example in upstream documentation.
Absence of sandbox-denial log messages is not proof of isolation either.
The owner/Director records native observations and any unresolved limitations
separately from capture, performance and final product acceptance.

## 4. Obtain the real product case and forward only loopback

The Director supplies a completed file with the shape of
`config/owner-case.template.json`. The template is intentionally invalid;
do not replace placeholders with invented values or zero hashes. Required
real inputs are:

* approved source-pack ID, approval reference, exact pack-file SHA-256 and
  repository-relative file path (copy the actual approved manifest into this
  checkout; hardware direction is not source-pack approval);
* real deployed product `buildId` and `buildArtifactSha256`, actual
  `assetManifestSha256` and `settingsSha256`;
* source-matched capture case, loaded scene/route/workload, required asset
  IDs/hashes and real entity minima;
* source-approved cameras, draw distance, stock layout/UI scale, resize
  endpoints and capture/input setup; the example resize range is a proposal,
  not an approved product range;
* the fixed pre-measurement contract and budgets. `ownerApprovalRef` for the
  benchmark may remain null; no invented second approval is needed.

Primary sampling remains **1920x1080, DPR 1, 60 rendered FPS** with at least
5 seconds warmup and 60 seconds measurement. Keep frame/stall budgets fixed
before evaluation. Do not reduce resolution/draw distance/content to pass.
Use a desktop arrangement that lets the owner inspect the intended viewport;
the harness does not change macOS display settings. Retina physical pixel
dimensions and physical scanout are not substituted for the requested
1920x1080/DPR1 render viewport.

An already-authorized SSH connection can expose the implementation server:

```bash
# In a separate owner terminal, using the owner's existing SSH host alias:
ssh -N -L 127.0.0.1:4173:127.0.0.1:4173 <existing-ssh-host-alias>
```

Or forward port 4173 through VS Code's Ports UI to a **local loopback** port.
Keep it private. Use `http://127.0.0.1:4173/`, not a public tunnel URL,
`localhost`, a remote hostname or a wildcard. Adjust to the real supplied
server port. Prefer same-origin app/API/WebSocket routing; if another local
forward is necessary, declare its exact origin in `allowedOrigins` before
the run. Forwarding is transparent to the server's required headers.
Do not fix CORS/HTTPS problems with browser security-disabling switches.

The agent has not opened a tunnel, contacted the owner's Mac, scanned devices
or provisioned infrastructure. No original OSRS/other-game login is required.
The real ClubScape fresh-account journey remains a separate parent-owned
acceptance requirement, using only the project's authorized test accounts.

## 5. Generate pinned Chrome and Edge configurations

Assuming the Director's actual case is `config/product-case.json`:

```bash
pnpm run handoff prepare --inventory owner-runs/mac-inventory.json \
  --probe runs/mac-chrome-probe/report.json --browser chrome \
  --case config/product-case.json --out owner-runs/chrome-case.json
pnpm run handoff prepare --inventory owner-runs/mac-inventory.json \
  --probe runs/mac-edge-probe/report.json --browser edge \
  --case config/product-case.json --out owner-runs/edge-case.json
```

Generation preserves the supplied product/source/workload/settings pins and
adds only the actually collected hardware, original executable/version/hash
and observed adapter. It uses the existing owner's M-series selection record
as hardware direction, not invented source or final approval. Wrong-platform,
wrong-browser, changed-binary, software/failed or mismatched probes are not
silently relabeled. Browser paths may contain spaces; they are launched as
literal executable paths without shell reconstruction.

Outputs are exclusive: existing files/run IDs are never overwritten.
If a browser or macOS/GPU configuration changes, collect a new inventory and
probes and freeze a new configuration; do not update pins after seeing a
candidate result merely to reclassify it.

**Director instrumentation contract:** the real renderer must implement
[`window.__clubscapeBenchmarkV1.read(afterFrame)`](../../tools/browser-harness/INSTRUMENTATION.md),
with actual loaded identities/assets/workloads and contiguous GPU-completed
render-frame records. It must include `identity.buildArtifactSha256` for these
generated cases. Since per-browser/hardware contract hashes are determined
on the owner's Mac, implement optional synchronous `bindRun({contractId,
contractSha256})` for audit identity only, or supply matching preconfigured
cases. The harness does not inject build/source/scene readiness or metrics.
An empty scene, raw RAF count, submit-only counter or nominal limiter is rejected.

## 6. Run, preserve both successes and failures, then clean up

Ensure no stale `BROWSER_EXECUTABLE` override points to a different browser;
generated configs already pin the original path.

```bash
pnpm capture --config owner-runs/chrome-case.json --run-id mac-chrome-case-1
pnpm capture --config owner-runs/edge-case.json --run-id mac-edge-case-1
```

Run separately, not concurrently. Keep the measured tab visible and do not
inspect DevTools/Activity Monitor, move/resize it, sleep the Mac or interact
outside the declared controls during a candidate window. Keep capture settings
and power state consistent and record relevant conditions with the run.
Repeat the agreed source-defined case matrix/repetitions using fresh IDs;
preserve failing runs instead of averaging or renaming them into passes.

The report contains exact observed host/browser/graphics data, immutable
configuration/pins, screenshots and pixel checks, real frame distributions/
stalls, optional query coverage and accurately labeled GPU-completion wall
latency. It does not prove physical presentation, source fidelity, audio,
legitimate progression, persistence, a complete both-browser matrix or
owner presentation acceptance.

```bash
pnpm run handoff bundle \
  --runs mac-chrome-probe,mac-edge-probe,mac-chrome-case-1,mac-edge-case-1 \
  --out owner-runs/mac-review-bundle
pnpm run handoff cleanup --manifest owner-runs/mac-review-bundle/manifest.json
```

The bundle includes ordinary JSON/PNG artifacts only, hashes every file,
preserves failed results and native platform labels, and omits profiles,
cookies and browser storage. It is bounded to 30 runs, 50 MiB per artifact
and 512 MiB total; split a larger matrix into multiple bundles. Review console
text, URLs and images before sharing the directory with the Director.
Cleanup verifies archive/raw bytes and completed browser cleanup before
removing only the named raw run directories. It never deletes the retained
bundle, source pack, vendor app or another user's process. Stop the separately
owned SSH tunnel with Ctrl-C when done.

The bounded handoff task is complete when these tools/instructions are ready.
**Actual owner Mac measurements, native observations and final M1 acceptance
remain unrun until the owner executes them against the supplied real product.**

## Authoritative basis and limits

* [Microsoft Edge supported OS/Apple Silicon support](https://learn.microsoft.com/en-us/deployedge/microsoft-edge-supported-operating-systems):
  native Apple Silicon support since Edge 88; the exact installed build and
  its supported macOS version must be recorded on the owner's machine.
* [Chromium Mac sandbox design](https://chromium.googlesource.com/chromium/src/+/main/sandbox/mac/README.md):
  Seatbelt is distinct from App Sandbox.
* [Chromium WebUI registration at 153.0.8010.12](https://chromium.googlesource.com/chromium/src/+/153.0.8010.12/chrome/browser/ui/webui/chrome_web_ui_configs.cc):
  `SandboxInternalsUIConfig` is not registered for Mac.
* [Chromium Mac temporary-directory implementation](https://chromium.googlesource.com/chromium/src/+/153.0.8010.12/base/files/file_util_apple.mm):
  `MAC_CHROMIUM_TMPDIR` is the hermetic override before `NSTemporaryDirectory`.
* [CDP SystemInfo schema](https://github.com/ChromeDevTools/devtools-protocol/blob/master/json/browser_protocol.json):
  device/process facts and optional GPU dictionaries, not a universal
  renderer sandbox-attestation API.
* [Apple code-signing technical note](https://developer.apple.com/library/archive/technotes/tn2206/_index.html):
  original bundle signature checking; only read-only verification is used.
* [Apple Activity Monitor process/column guide](https://support.apple.com/guide/activity-monitor/view-information-about-processes-actmntr1001/mac):
  inspect current owned process information and available columns.
