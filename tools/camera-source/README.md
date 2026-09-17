# Original normal-camera source probes

This utility calls unchanged injected-client1.12.38 camera, input, source
script, terrain, viewport and region-rebase methods against cache2695/build240.
It does not run an original account, login handler or network pump. No
production camera fields, frozen helpers, assets or approval inputs are edited.

The qualified state contract is
[`research/camera-source/contract.json`](../../research/camera-source/contract.json).
It intentionally does **not** fit the existing TypeScript scalar-rate scaffold.

## Reproduce

Read `docs/machines/sparky.md` and verify the current host/JDK first.
Use the existing verified source/cache/tooling directory:

```bash
python3 tools/camera-source/probe_camera.py --all \
  --source /home/lramos15/clubscape/.worktrees/m1-runtime-inputs/.local/current-source
python3 tools/camera-source/probe_camera.py --verify-only
python3 -m unittest discover -s tools/camera-source -p 'test_*.py' -q
python3 tools/camera-source/probe_camera.py --all \
  --output .local/camera-source/repeat
```

`--scenario NAME` is repeatable for targeted probes. Inputs are privately
copied and checked against the existing locks; absent/corrupted dependencies
fail rather than downloading a new runtime. Java uses isolated worktree home
and scratch directories. Original `tools/source-capture`/`LayerCapture` helpers
are compiled read-only to provide native cache/world/widget bindings.
Their chosen camera is discarded before the normal-camera tests; it is never
relabeled as an entry default.

Every scenario runs in a fresh JVM. Native camera state is measured before
and after the fixed-cycle and render-frame paths, not calculated by a
replacement camera. Source scene painting is used only to obtain original
orbit/projection readbacks; there is no renderer benchmark or new reference
image. Actor logical and render coordinates are declared controlled inputs;
actor movement interpolation itself is not claimed tested.

## Important source results

* Original `tq.run` schedules20ms cycles. The injected source separately uses
  elapsed nanoseconds for frame-angle/follow updates. Do not apply a constant
  radians-per-RAF-second approximation or merge these two update phases.
* Arrow motors accelerate and damp with signed integer truncation. Mouse
  delta/history smoothing and release behavior are stateful. Pitch clamps,
  yaw wrapping and float/native paths remain distinct.
* Follow uses the source focus entity, not camera-eye XY. A displacement
  greater than500 snaps; equality still smooths. Terrain height uses original
  footprint/bilinear/bridge logic and the source follow-height offset.
* Original wheel listener39 invokes source script42. It changes logical FOV,
  encoded projection and follow height, not the independent orbit-distance
  scale. Source rounding makes it different from adding to pixel zoom.
* Source preset626 contains bounds128..896; scripts603/604/605 also accept
  externally supplied bounds. The probe invokes the preset explicitly.
  Its actual authenticated startup invocation/parameters remain unknown.
* Native region transitions preserve absolute world position while rebasing
  local camera/focal/player coordinates and unlocking the camera.

Constructor state, helper-only UI state and controlled initialized states are
labeled separately. Constructor yaw0/pitch1024/eye0, the source preset, and the
old five fixed fixture cameras do not establish Tutorial/Lumbridge account
arrival cameras. The constructor follow-height offset is50; the partially
initialized helper UI produces25, not a second asserted account default.
The exact remaining initialization questions are in the
contract; no request for credentials or a source account is made.

## Evidence and integration

`research/camera-source/traces/manifest.json` binds native traces, original
input/helper/probe hashes, JVM arguments and source-only replay commands.
`native-evidence.json` and `native-methods.json.gz` identify the methods and
source script payloads. `validation.json` records repeatability, native
quality checks and unchanged frozen-input checks.

The original frame updater depends on source actor-render coordinates. An
early incomplete probe omitted that input and therefore followed zero; it is
not retained as a normative follow trace. Wheel handling also required real
camera bound initialization, rather than the helper's uninitialized Varcs.
These diagnosed prerequisites are not silently converted into account defaults.

The source-scope script scan inspects camera-bound writers/callers only. It
records the two-byte original script0 payload separately because it is below
a script header size; no invalid initializer is fabricated. All other scanned
source scripts are decoded against the index revision, not game build240.

The downstream Rust/WASM implementation needs persistent motor, target-angle,
render-frame focal, terrain, FOV/configuration and rebase state. Use the native
trace sequences as checks. Leave source-session initialization unknown where
it is genuinely unobserved; this source handoff does not authorize a guessed
normal-entry camera, production implementation, new reference policy, M1
acceptance or performance claim.
