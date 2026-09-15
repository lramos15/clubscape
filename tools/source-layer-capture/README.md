# Original dynamic M1 source references

These twelve supplementary cases are **controlled offline original-client
rendering**, not authenticated gameplay, a new style/approval policy, or
candidate-renderer acceptance. They use the unchanged injected-client1.12.38,
cache2695/revision240, native maploader/scene/item/animation/minimap/widget
code, and read-only frozen `tools/source-capture` helpers. No source screenshot
is embedded in a candidate or used as a drawing background.

The original pack remains untouched at SHA-256
`b62e19704e17d3d3e4e819f803ef49ba7cc54034ae407184b423427c65d9674d`.
New source images, per-case state/source hashes, original runlogs and pair
metrics are under `assets/reference/osrs240/m1-dynamic/`.

## Cases and qualifications

| Family | Stable case IDs | Original controlled state |
| --- | --- | --- |
| Tutorial door | `tutorial-door-closed`, `tutorial-door-open` | Source object9398, tile3098,3107, plane0, wall type0/model9476; orientations0/1 via native scene replacement. |
| Ground items | `lumbridge-ground-single`, `lumbridge-ground-stack`, `lumbridge-ground-top-three` | Tile3221,3217; native quantity-selected coin models and native value/three-distinct-item pile ordering. |
| Fire | `lumbridge-fire-frame-zero`, `lumbridge-fire-frame-three` | Fire26185/model2260/sequence475, exact frames0/3, source lengths6/6/6/6/6. |
| Roofs | `tutorial-roofs-outside`, `tutorial-roofs-inside`, `tutorial-roofs-hidden` | Actual outside/inside source tile flags0/4; original hide-roofs preference and both native roof-plane paths recorded. |
| Minimap/world state | `lumbridge-minimap-plane-one`, `lumbridge-minimap-mapped-chunk` | Native plane1 rasterization; separately, native source chunk402,402 rotated180 degrees at local chunk6,6. |

The door's source definition still exposes `Open` in both controlled
orientations. The second image proves original rendering of the controlled
open orientation, **not** a captured server open packet or authoritative hinge
choice. Do not infer a different source definition, substitute diagonal
type9/model9477, or relocate the door.

The pile contains native `TileItem` instances. Source `lj.es` selects coins as
the highest-value/top renderable and two additional distinct IDs; it does not
render every queued item or use generic icons. Native draw anchors, support
height, quantities, actual assembled mesh counts and transform/topology hashes
are recorded. The controlled items are not loot/acquisition observations.

Native spawned fire randomizes its starting animation phase. The fixture
reuses its native placement, installs a new **original** `dy` with
`random=false`, advances through `dy.rf/ou.pl`, and pins its last-update clock
after selecting the frame. Frame3 first occurs after19 native advancement
cycles, not a fabricated server tick. All other scenery stays at scene
cycle0. `DynamicObject.getAnimCycle()` returns -1 in this runtime; that API
value is preserved, not misreported as measured phase.

The frozen locked camera draws plane3 with roofs visible for both player
positions, then plane0 when roofs are hidden. The real normal-camera
`cz.ch` selector separately returns3 outside and0 inside. Its result is
**not** falsely claimed to have painted the locked-camera images. This is the
stock distinction the candidate must preserve, not a blanket hide-upper-planes
rule.

The mapped case is a deliberately controlled native `xk`/`rl4.fn` chunk
mapping, not a live instance packet or legitimate travel observation. All
other core104x104 mappings are identity mappings. It performs a second native
maploader pass; actual native HSL tint offsets and loaded geometry counts
are recorded rather than normalized or silently treated as identical state.
No ground pile is present in the plane1/mapped cases.

## Camera, layout and comparison

All frames are1920x1080 with native pixel ratio1/UIscale1, stock
Resizable-Classic root161, brightness0.8, texture resolution128, draw distance25,
and the existing fixture far clip32768. The source scale remains128 units/tile,
negative-up heights and16384 scene angle units/turn. Camera positions/pitch/yaw
are the frozen Tutorial starting-house and Lumbridge castle/plaza inputs.

The original **full-HUD** viewport calculates zoom410. The frozen
viewport-only scene fixtures separately use662. No override forces one
projection onto the other. `--baseline-check` replays the existing frozen
native inventory frame byte-for-byte and verifies this full-HUD projection;
it is an existing-reference replay, not a thirteenth supplementary case.

Use the unchanged approved comparison-policy file and profiles
`native_scene_model` and `native_hud`. Camera translation/frame-index errors
remain0, native HUD/control/glyph errors remain0, and the existing narrow
scene raster-edge limits remain unchanged. Pair deltas are observations
between two SOURCE control states, **not** tolerances or masks for a candidate.
Never subtract those deltas from candidate errors or ignore an entire panel.

`pair-metrics.json` accounts for the complete frame and each changed pixel.
The complete native chat/inventory panels remain identical within each pair.
The sidebar container rectangle also encloses unpainted scene gutters;
the plane-change pair therefore changes some container pixels while its
entire inventory panel remains exact. Those pixels are counted and retained,
not masked. Compare a candidate against the corresponding complete source
case under the already approved profiles.

## Reproduce from a clean checkout

Read `docs/machines/sparky.md` and verify the host first. Supply the already
verified original cache/dependency directory and matching resource JAR:

```bash
python3 tools/source-layer-capture/capture.py \
  --source /home/lramos15/clubscape/.worktrees/m1-consumable-assets/.local/current-source \
  --resources /home/lramos15/clubscape/.worktrees/m1-native-hud/.local/source-capture/tooling
python3 tools/source-layer-capture/capture.py --baseline-check
python3 tools/source-layer-capture/capture.py --verify-only
python3 -m unittest discover -s tools/source-layer-capture -p 'test_*.py' -q
```

The explicit source/resource arguments work on any checkout containing the
same pinned bytes; these local paths are conveniences, not network fetches.
Missing/corrupted inputs fail. All cache files are privately copied and
verified before/after; no write handle opens another worker's cache.
The dependency locks and exact input/JAR/helper hashes are in the case index.
Python/Pillow only inspect final PNGs; Java's unchanged source renderer draws
every frame. No browser, GPU, global installation, personal RuneLite state,
account login or terms action runs.

Use `--case ID` (repeatable) for a selected original state and `--output`
under the owned dynamic-image directory or private
`.local/source-layer-capture/` scratch. Run each case in an isolated JVM/home
with seed0. `OriginalCapture.save` verifies exact native pixels through a PNG
readback; independent checks validate pixels, models, source states, paired
visibility and native errors, including swallowed script/render failures.
JVMs exit and cache handles close on success/failure.

To repeat without touching the published new images:

```bash
python3 tools/source-layer-capture/capture.py \
  --output .local/source-layer-capture/repeat
```

Compare per-case PNG/ARGB hashes and native state/geometry records, not
wall-clock log timestamps or JVM memory addresses. Complete original runlogs
are preserved compressed. Existing source pack validation uses
`python3 tools/reference-pack/validate.py --require-complete` **without**
`--report`, so it does not overwrite frozen evidence.

Native bindings, repaired preparation failures, exact commands, repeatability
and the before/after1307-input integrity records live in `research/source-layers/`.
These references supply source evidence only: no candidate, gameplay,
performance, M1 completion or new presentation approval is asserted.
