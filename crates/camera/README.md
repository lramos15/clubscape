# Source-measured M1 normal camera

`clubscape-camera` is the stateful Rust controller for the unchanged
injected-client1.12.38/build240/cache2695 normal-camera contract. It separates
fixed20ms input cycles from elapsed-nanosecond frame updates and retains both
native integer and injected float paths. It has no browser, renderer, network,
GPU or account dependency.

The source authority is
[`research/camera-source/contract.json`](../../research/camera-source/contract.json),
SHA256 `745dda42fcd53d3225b9f2714e2ebbad7dbf60a76c070da1438a7289e5e0a602`.
The original14 scenarios/354 rows are immutable controlled offline readbacks,
not authenticated camera initialization. Supplemental bytecode interpretations
and exact method/class identities are in [`source-readbacks.json`](source-readbacks.json).
Original artifact notices remain in
[`assets/source/osrs/NOTICE.txt`](../../assets/source/osrs/NOTICE.txt) and
[`tools/cache-import/THIRD_PARTY_NOTICES.txt`](../../tools/cache-import/THIRD_PARTY_NOTICES.txt).
No original JAR, cache, image or approval file is copied over or modified.

## Typed consumer contract

The integrating Rust/WASM client owns one persistent `NormalCamera`. All state,
focus, input, preferences, effects and output records implement Serde. The crate
does not add a second TypeScript controller or its own wasm-bindgen boundary.
The existing WASM owner must expose these operations and handle `CameraError`
explicitly; a failed input is not permission to start with a fixture/zero eye.

| Operation | Required binding |
| --- | --- |
| `initialize(reference, viewport, focus, terrain)` | Returns the camera and exact approved initialization provenance. Supply the actual source actor/render snapshot and loaded source terrain. |
| `fixed_step(input, focus, terrain)` | Exactly one20,000,000ns logical input cycle. Held arrows, source mouse coordinates/button and each routed wheel event are consumed once. |
| `render_frame(elapsed_ns, focus, terrain)` | A frame with explicitly inactive source effects. Elapsed time is independent of the number of input cycles and does not increment the logical cycle. |
| `render_frame_with_effects(elapsed_ns, focus, terrain, effects)` | Use actual active source-effect state and explicit per-channel phase/random inputs. Missing required samples fail; there is no phase/RNG fallback. |
| `resize(viewport)` | Applies native projection/letterboxing. The next frame derives its orbit using the resulting viewport. |
| `rebase(world_base)` | Translates local eye/focal/logical coordinates, unlocks the camera and retains angles, motors, cycle and preferences. Rebase the caller's actual actor/terrain snapshot consistently. |
| `wheel`, `apply_fov_program`, `set_zoom_bounds` | Source listener39/script42 semantics, including widget routing, varbit gates, conversion order, follow-height and persisted logical values. |
| `apply_distance_scale_program` | Source opcode6201 short conversion and256/320 gates, independent of FOV. `set_distance_scale` instead accepts already-valid native shorts. |
| `snapshot`, `restore`, `state`, `output` | Preserve resumable controller state. `restore` validates it. `output` is the last rendered frame, not an eagerly recomputed result after an input/resize/preference change. |

`set_middle_mouse_enabled`, `set_wheel_gates` and `set_target_angles` are explicit
source-state controls, not evidence of a new account preference or reset packet.
`source_state` exposes unbound constructor mechanics with an allocated viewport
and approved preset626 bounds; it cannot render or tick until explicitly bound.
Normal entry must use `initialize`, not restore a zero-world constructor eye.

### Focus, terrain and coordinates

`FocusProvider::camera_focus` must return an actual source focus identity, local
logical coordinates, actual actor render coordinates, plane, source footprint
width and `Actor`/`Point` kind. Logical and render positions are different inputs.
Never substitute the camera eye, infer an actor tile centre, or invent a missing
render position/footprint.

`Terrain` requires the world base, dimensions, original height corners, original
tile settings and **`surface_height`**. Corners include the far edge. Missing,
unloaded or corrupt data must return an error. Camera/focus/terrain base mismatch,
empty terrain and out-of-range footprints fail transactionally.

All distances use128 source units/tile. Height is negative-up. Array order for
eye/focal output is `[horizontal_x, height, horizontal_y]`; it is not the
RuneLite integer getters' X/Y/Z naming. World horizontal coordinates are
`base_tile*128 + local_source_units`. Native angles use16384 units/turn; legacy
angles use2048. Keep these units until the renderer's explicit conversion.

There are two distinct native height algorithms:

* Frame following uses float bilinear height, original fractional footprint
  sampling, bridge flags, actor adjustment -8 and the follow-height offset.
* Fixed-tick ground/terrain-pitch queries use original rendered tile/model
  triangles, source query order, float barycentric interpolation and final
  integer truncation, then original ground-decoration `raise`.

`SurfaceTriangle::height_at` implements that barycentric calculation. The
production provider must use the real source tile paint/model topology and
decoration. `integer_bilinear_height` is only for the source's *known absent
surface* branch; it is not a replacement for missing rendered geometry.
The provider applies the native surface query's bridge semantics itself.

For example, source object941 at3215,3218 has `raise=7`: surface -464 becomes
native -471. At3223,3218, native triangle height -240 differs from bilinear -238.
The conformance-only paint adapter is not a general production surface decoder.
An existing bilinear-only renderer helper does not fulfill this trait.

### Scheduling, controls and output

The caller supplies source20ms logical steps separately from frame elapsed
nanoseconds. Retain the scheduler's remainder and deliver held-key releases and
wheel events correctly; do not multiply fixed-cycle acceleration by RAF delta
or repeat a wheel event for every catch-up cycle. Supply coherent actor/world
snapshots at each operation. Actor movement interpolation is outside this crate.
Transport `u64` nanoseconds through WASM without JavaScript integer precision loss.

Mouse coordinates are absolute source-client logical pixels, not browser
`movementX/Y`; native button4 maps to `MouseButton::Middle`. Wheel `rotation`
is the native integer wheel count, not raw browser `deltaY`. The browser adapter
must normalize events and supply the actual widget route without changing rates.

Native/legacy velocities, signed truncation, opposed-key precedence, residual
release rotation, middle-button preference gating and mouse-history smoothing
remain separate state. Pitch bounds are1024..3064. Following snaps only when
either displacement is **greater than500**, otherwise retaining the original
double-intermediate/float-storage smoothing. The9x9 terrain pitch accumulator
uses the original24/80 rise/fall divisors.

`CameraOutput` contains native/table eye and angles, the original float eye and
radian angles, both focal representations, both radial distances and the fitted
`Projection`. Keep the native and float rendering lanes distinct; never combine
one lane's eye with guessed or differently scaled angles. Orbit changes the eye,
not just yaw/pitch fields. Source integer multiplication/wrapping is retained,
including at large native distance scales.

Distance factors, logical FOV, encoded FOV shorts and pixel projection zoom are
different quantities. Wheel input changes FOV and follow height, not distance
factors. Native conversion makes repeated512-origin wheel steps487 then461.
Script42 clamps logical values, encodes FOV, recalculates the viewport, sets
follow height and persists logical values in that order. Ordered integer zoom
bounds are supported; source short gates and opcode5530's nonnegative
follow-height gate are preserved rather than replaced by an invented zoom range.

`FrameEffects` covers the original five stock shake channels and their
shake-dependent pitch floor. Phase/frequency/amplitude and random draws are
explicit frame inputs; this camera does not invent event activation, phase
advancement, server packets or a new RNG seed. Suppression skips jitter, not the
pitch-floor constraint. These branches have bytecode-derived boundary tests;
the354 original readback rows do not contain active-effect observations.

The region probe changes the logical base without replacing its scene buffers.
Its immediate ground query consequently changes before returning to the old
base. This is retained by the fixture adapter, not reinterpreted as a completed
region load. The real client must supply the actual loaded/rebased scene and
readiness state rather than reset the camera or reuse mislabeled terrain.
Server-locked rendering errors explicitly; free-camera, plugin speed/relaxer
controls and invented server reset behavior are not implemented.

## Approved initialization, not source-session facts

[`initialization.json`](initialization.json) records the selected reference
paths/hashes and exact native/legacy angle conversions before consumer binding.
Both selected approved views contribute yaw0/pitch2048 (legacy0/256,45 degrees).
Only their **angles** are reused; their fixture eye and projection zoom are not
copied into normal entry.

Owner approval
[`m1-camera-initialization-v1.json`](../../milestones/approvals/m1-camera-initialization-v1.json),
SHA256 `f7f1ce92826fb1bafc1e17ecbbaace34e0851c4e5044cbcfafa86817ac3539e1`,
permits explicit ClubScape defaults: actual player/terrain-derived focus and
orbit, constructor middle-camera false, follow-height50, distance256/320,
encoded FOV256/205 and source preset626 bounds128..896.

The approved constructor FOV produces calculated projection zoom662 at1920x1080.
The controlled full-HUD conformance helper instead has FOV127/127 and zoom410.
Neither number substitutes for the source projection calculation or establishes
an authenticated account setting.

Initialization does not run script42, advance a synthetic server tick, infer
saved FOV values or apply fixture eye coordinates. Ordinary follow/rebase never
reinitializes to a chosen reference. Authenticated Tutorial/Lumbridge camera
packets, session/persisted camera settings and reset behavior remain unobserved.
The base adaptation register and its approved addendum remain read-only.

## Reproduce and inspect conformance

Read `docs/machines/sparky.md` before builds. With the existing locked toolchain
and source publications in a clean checkout:

```bash
cargo test --locked -p clubscape-camera
cargo clippy --locked -p clubscape-camera --all-targets -- -D warnings
cargo check --locked -p clubscape-camera --target wasm32-unknown-unknown
cargo fmt -p clubscape-camera -- --check
PYTHONDONTWRITEBYTECODE=1 python3 tools/camera-source/probe_camera.py --verify-only
PYTHONDONTWRITEBYTECODE=1 python3 tools/reference-pack/validate.py --require-complete
```

No native client login, GPU run, dependency refresh or new source download is
needed. The conformance test pins the source contract, native method bundle,
trace manifest, every trace and the actual original terrain/definition inputs.
Initialization tests separately pin the owner response and source angle records.

[`conformance.json`](conformance.json) is a generated, implementation/input-bound
report:14 scenarios,354 original rows plus336 intervening fixed-tick states,
39,438 integer scalar comparisons and6,210 float-bit comparisons. Every original
integer matches; float bits match with maximum absolute error0. Each scenario
lists counts by integer/float field, and failures retain the scenario, row,
phase, expected/actual values and float bits. No epsilon, fitted golden or
pruned source row is used. Repeated native raw/decoded aliases count separately.

The historical source trace's `nr.decoded` uses an incorrect diagnostic
multiplier. The test reproduces that diagnostic and the original raw integer
without treating it as a terrain height. Production `logical_focus_ground`
holds the genuine source surface height; see `source-readbacks.json`.

Tests always write `target/camera-conformance.json`. To republish the report
after an intentional controller change:

```bash
CAMERA_CONFORMANCE_REPORT=crates/camera/conformance.json \
  cargo test --locked -p clubscape-camera --test conformance
cmp target/camera-conformance.json crates/camera/conformance.json
```

The additional tests cover500/501, fractional footprints/bridges, terrain
pitch24/80, signed rounding/motors, missing or wrong source inputs, serialization,
FOV/native-short gates, resizing/letterboxing, rebase invariants and explicit
effect inputs. Bytecode-derived edge tests are not new original trace captures.
WASM compilation is not browser execution or cross-platform visual acceptance.

## Remaining integration boundary

Director's separate **M1-CAMERA-INTEGRATION** owns the crate dependency and WASM
exports, browser event/clock transport, real focus/terrain/surface/effect
providers, native viewport binding, region readiness and live renderer output.
The old scalar-rate shell cannot represent this state. Normal bundle metadata
must bind the approved initialization provenance and this persistent controller,
not fill `camera:null`/`controls:null` with fixture constants.

Production consumers are deliberately not changed here. Normal entry,
authenticated player-journey behavior, matched renderer/browser captures,
performance and owner/M1 presentation acceptance remain separate obligations.
