# Independent original scenery phase observations

This is additive evidence for the existing `tutorial-door-closed`,
`tutorial-door-open`, and `tutorial-roofs-hidden` references. No original
image, case index, helper, camera, RNG sequence or animation target changed.
No candidate image or candidate frame-search result was read.

The unchanged source case index has SHA-256
`dde300e30ff909e539c283fa7053c673abc9428ce357894f0727be08da8ad557`.
The new machine-readable index is
`assets/reference/osrs240/m1-dynamic/phases/phase-index.json`.

| Placement, plane0 | Native shape/orientation | Sequence | Active frame before/after draw |
| --- | --- | ---: | ---: |
| Fireplace24969 at3095,3102 | 10 / 2 | 477 | 4 / 4 |
| Torch196 at3096,3105 | 4 / 3 | 481 | 3 / 3 |
| Torch196 at3096,3110 | 4 / 1 | 481 | 2 / 2 |

These values were independently observed in **all three** exact original
replays. `dy.ac` is the active rendering controller; `dy.aa` is separately
recorded auxiliary previous-controller state and must not be substituted.

The native scene cycle is observed as0 throughout. Each object's decoded
`dy.ao` last-update cycle is -1 before drawing and0 afterward. The active
`qr.as` within-frame cycle is observed as0 before drawing and1 afterward.
No zero phase was assigned by the probe. The original public
`DynamicObject.getAnimCycle()` returns -1; sidecars mark that API unavailable
and separately report verified native clock fields.

Sequence477 lengths are `[7,6,6,6,1]`. In the original native stepper, an
accumulator equal to the frame duration remains on that frame; only a value
greater than the duration advances. Thus this replay's frame4 at offset1 is
not frame0. Sequence481 lengths are `[7,6,6,6,6]`. The observed frames, not
candidate fitting, are the independent inputs for these fixed source cases.

## Replay and verify

```bash
python3 tools/source-layer-capture/probe_phases.py
python3 tools/source-layer-capture/probe_phases.py --verify-only
python3 -m unittest discover -s tools/source-layer-capture -p 'test_phases.py' -q
python3 tools/source-layer-capture/probe_phases.py \
  --output .local/source-layer-phases/repeat
```

The new Java host invokes the unchanged original `LayerCapture` setup and draw
order. It inserts read-only getters/field reads after map loading, immediately
before drawing, and immediately after drawing. It never fetches a model,
steps or resets an animation, selects a target frame, draws an additional RNG
value, or changes the original scene cycle. Replay PNGs stay private/ignored
and must match the old PNG and original ARGB hashes exactly; no thirteenth
reference case or new composition is published.

The Python host checks the old helper/index hashes, copies/reuses the same
pinned private-cache inputs and compiles only the new probe plus the unchanged
original helpers. `--source` and `--resources` accept the existing verified
input directories; no new gamepack, account or freshness lookup is involved.
Old helper files, original images and the frozen pack remain untouched.

Sidecars retain source object/tile/plane/type/orientation, active and previous
native controller fields, API readings, signed multipliers, source clocks,
source-only replay arguments, original input hashes and probe hashes.
`native-fields.json` and `native-bytecode.txt` identify the checked native
readers/writers and boundary condition. `validation.json` records tests,
repeated exact replays and unchanged source/frozen-pack integrity.

Consume these frames and source within-frame offsets for the corresponding
original source case. They are not a global live-world clock or authenticated
gameplay observation. Do not mask the flame/torches or treat a candidate fit
as source evidence. No gameplay, presentation, performance or M1 acceptance
is claimed, and no existing comparison tolerance is changed.
