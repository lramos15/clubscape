# Source UI overlay — implementation in progress

This is an actual Canvas2D/DOM interface implementation, **not a complete M1
client or an accepted presentation**. The owner-approved pack is
`b62e19704e17d3d3e4e819f803ef49ba7cc54034ae407184b423427c65d9674d`.
`evidence/` records component-only results and remaining source differences.
The `m1-ui` task remains **blocked**, not done.

## Shell integration

```ts
import {
  createUi, forwardWorldPointer, setUiCamera, onUiCameraRequest,
  getUiPreviewBounds, setUiPreview,
} from "./ui/index.ts";

const ui = await createUi(overlayCanvas, services, assets);
ui.resize(innerWidth, innerHeight);

// The same camera supplied to the actual renderer:
setUiCamera(ui, camera);
const stopCameraCommands = onUiCameraRequest(ui, yaw => {
  camera = { ...camera, yaw };
  renderer.camera(camera);
  setUiCamera(ui, camera);
});

// On the shell's world-input path; do not dispatch the same action twice.
if (!ui.capturesPointer(x, y)) {
  forwardWorldPointer(ui, {
    kind: rightClick ? "context" : "primary",
    x, y, pick: renderer.pick(x, y), control: controlKey,
  });
}

// Optional actual model-only renderer preview, never a reference panel/capture.
const bounds = getUiPreviewBounds(ui);
// Render an RGBA model-only surface at bounds.width × bounds.height, then:
setUiPreview(ui, modelPreviewSurface);
```

`createUi(canvas, AppServices, ClientAssets): Promise<UiHandle>` implements the
unchanged shared contract. The canvas must already have a parent. Keep world and
overlay canvases separate; do not clear the world canvas from UI code. The UI
subscribes to `services.subscribe()` itself. Explicit `ui.update()` is supported,
but is not required in addition to that subscription. `dispose()` unsubscribes,
removes its DOM controls/listeners, clears credential references and releases UI
resources. Call `stopCameraCommands()` when removing the shell adapter.

Coordinates are viewport-local logical pixels, source UI scale 1. Tested sizes:
1024×768, 1920×1080 and 2560×1440, DPR 1. Do not pass device-scaled dimensions as
logical dimensions or stretch the canvas independently of its input layer.
The input layer tracks the canvas's screen rectangle. The renderer must do its
own picking; the overlay never fabricates a picked entity or world coordinate.

The preview hooks are presentation adapters, not new gameplay contracts.
`getUiPreviewBounds()` is available after the character interface's render.
`setUiPreview()` accepts an actual renderer canvas/bitmap at those exact native
dimensions; no resizing or human-preview substitution is performed. A missing
preview remains a missing renderer integration, not a finished penguin.

## Asset contract

`ClientAssets.json("ui/manifest.json")` must resolve to
`assets/compiled/ui/manifest.json`. Image IDs in that catalogue start with `ui/`
and resolve under `assets/compiled/`. `ClientAssets.image()` must return decoded
same-origin images and the shell asset loader should verify the pinned hashes in
`ui/provenance.json`. The UI rejects the wrong pack/cache identity or missing
native font metrics. Do not map these asset IDs to full reference screenshots.

The catalogue contains original:

* sprite frames, offsets, canvas dimensions and palettes;
* CP1252 fonts 494/495/496/497, 256 masks/advances each and native ascent;
* native widget readbacks and static definitions;
* quantity-dependent item icons painted by `Client.createItemSprite`, without
  baked quantities; runtime quantities come from `ItemView`;
* model-only NPC portraits with parent-clip offsets (a model can exceed its
  nominal 32×32 widget);
* native scene minimap rasters and original map-dot/compass sprites.

Inventory, bank, shop and worn-item ownership/quantities are projected from the
current immutable `WorldView`. The native template inventories and synthetic
fixture dialogue are **not** used as live player data. Live dialogue and journal
text come from the public views; the two unrecorded source transcript entries
remain explicitly unknown. `xpTenths` is formatted using `BigInt`.

`runEnergy` is consumed in the server/content centipercent representation
(normal full energy is 10000), displayed divided by 100. The shell must not
normalize it a second time. `ItemView.iconAsset`, when supplied, must be a
native-size item icon, not a panel. Item actions, authoritative quantities,
server rejection messages and error IDs must survive the shell's adaptation.

Static minimap terrain is not evidence of dynamic-door/instance fidelity.
An authoritative/rendered dynamic minimap surface and exact tutorial minimap
visibility are still integration gaps.

## Implemented control paths

Real accessible DOM username/password/confirmation and quantity/search inputs
sit over original bitmap lettering. Other transparent DOM controls expose
labels, focus and disabled states without replacing the source artwork.
Credentials are memory-only; UI code does not use local storage or log them.

Requests go through `AppServices`, including registration/login/logout,
appearance confirmation, interface opening/closing, walking, world interaction,
equipment changes, eating/dropping, item-on-item/world, inventory swaps, bank
deposit/withdraw, notes and quantities, shop prices/buy/sell, dialogue choices,
combat styles/auto-retaliate, run, prayers, spells, experience selection and
reclaim requests. There are no client grants, inventory mutations or quest/XP
advancements.

Context menus preserve cancellation and disabled feedback. Pointer drag uses the
source 5-cycle/5-pixel threshold. Slot identities are checked before dispatch.
Shift actions use the original item's shift-action metadata. Failed requests
show their actual message/error ID and retain authoritative state; the amount
entry is retained on rejection. A fulfilled service promise does not fabricate a
new account, world or inventory update.

## Outstanding scope — not hidden or accepted

See [`contract-gaps.json`](contract-gaps.json) for exact requested public-field
extensions. That file is **a request, not a fork of `web/shared/contracts.ts`**.

Additional UI implementation/fidelity work remains:

* prayer/spell filter modes; their buttons report a required UI implementation
  gap, not an invented missing server field;
* complete validated source projection of all enabled/disabled/highlight
  signatures (the 71 states, 29 families and 11 signatures are retained);
* native source comparison of live data projections, not only source-widget
  replay, including all modal/choice/scroll/selected/disabled variants;
* equipment-stat values/kept-on-death presentation, production selection,
  quest/level reward payloads and their dismissal sequencing;
* fully calibrated populated grave/recovery grids and all native discard/fee
  controls; current per-item reclaim actions are not recovery fidelity proof;
* appearance kit/colour/pronoun choices beyond the public content's body type,
  and the actual renderer preview;
* in-game source audio sliders/music controls and current values;
* native authentication-failure/terms/animated-title mode coverage beyond the
  implemented source-framed startup/error compositions;
* source sprite item/decorative effects in the two remaining native replay
  differences recorded in `evidence/source-comparison.json`.

Controls whose service capability is absent do not silently succeed or invent
values. Out-of-scope controls retain their source placement and explicit
feedback. Required-but-incomplete controls are identified as required gaps,
not relabeled out of scope. This implementation is not ready to satisfy the
task's entire control/fidelity matrix.

## Validation

Read the machine guide before browser/native work. No global packages,
permissions, sandbox, drivers or security configuration were changed.
The existing frozen web dependencies are used.

```bash
cd web
pnpm exec tsc --noEmit
node --test ui/tests/unit.test.ts
cd ..
python3 tools/ui-assets/glyph_proof.py

mkdir -p web/ui/.cache/xvfb
TMPDIR="$PWD/web/ui/.cache/xvfb" xvfb-run --auto-servernum \
  --server-args='-screen 0 2560x1440x24 -nolisten tcp' \
  node web/ui/tests/component-browser.mjs
TMPDIR="$PWD/web/ui/.cache/xvfb" xvfb-run --auto-servernum \
  --server-args='-screen 0 2560x1440x24 -nolisten tcp' \
  node web/ui/tests/source-browser.mjs
python3 tools/ui-assets/compare.py
```

The browser tests use sandboxed **headful** Chrome under Xvfb. Set
`CLUBSCAPE_CHROME` for a different approved executable. Browser work files use
the short owned `web/ui/.s/` path because Chromium Unix socket names have a
length limit. Test servers bind random loopback ports and close on completion.

The component double is deliberately not a server simulator: requests are
recorded, rejected or acknowledged; it never applies game rewards or rules.
Component screenshots use a transparent/black world surface and cannot prove
gameplay, world fidelity or performance.

The source comparison uses the **same original runtime and declared source
fixture values**, with only the world-content handler detached for a transparent
component surface. The original 16 full-frame source PNG hashes are verified
unchanged by instrumentation. Every UI panel pixel and the entire UI-only
canvas are compared at zero tolerance. Nested panel diagnostics must not be
summed as disjoint partitions. No whole UI panels are masked. Model-only
portraits, sprites, fonts and geometry are painted individually.

The seven composition comparisons replay the exact approved composition text
and first-title-paint state (the source has not yet drawn the world switcher).
They validate shared raster primitives and anchors, **not** all production
startup data projections. Source-widget replay, interaction tests, actual-server
journeys, final visual/audio approval, Mac/Edge performance and RuneLite
compatibility remain separate gates.

## Rebuilding original assets

```bash
python3 tools/ui-assets/prepare.py --native
```

This reuses the pinned cache and tooling from the existing source workers,
verified before reuse and the cache verified afterward. Override `--source`
and `--tooling` for other approved locations. It compiles unchanged source-host
classes plus owned instrumentation in `tools/ui-assets/.cache/`; the original
JAR and source paths are not modified. A later `prepare.py` without `--native`
repackages those readbacks. It never downloads tools or contacts an account.

The additional player-preview interfaces deliberately suppress the original
**human** preview; their source frame/control readbacks do not constitute an
accepted penguin preview. This boundary is recorded in the compiled provenance.
Original RuneScape artwork/logos remain source material; no replacement logo or
new stylistic approval is claimed.
