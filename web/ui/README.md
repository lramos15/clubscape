# Source UI overlay — implementation in progress

This is an actual Canvas2D/DOM interface implementation, **not a complete M1
client or an accepted presentation**. The owner-approved pack is
`b62e19704e17d3d3e4e819f803ef49ba7cc54034ae407184b423427c65d9674d`.
`evidence/` records component-only results and remaining implementation gaps.
The `m1-ui` task remains **blocked**, not done.

The authorized parent integrity repair `9bf6969` is preserved in branch ancestry.
Strict verification passes all 1,307 frozen input hashes and the 85 reference-pack
tests. Historical status text in the hash-locked art/interface documents is not
treated as a revocation of the external approval. Asset preparation now runs the
unchanged strict pack validator before generating assets.

The authorized contract publication `d1532d6` is integrated as `5d56c93`.
`GameplayUiView` / `GameplayUiIntent` are binding, **not proof that the backend
or protobuf bridge implements them**. Actual versioned projection data is
required; absent/unknown/malformed `WorldView.ui` is explicit unsupported/error
feedback rather than a fabricated empty success.

Current component validation: TypeScript and 33 UI unit tests pass; all 20 legacy,
15 versioned and 7 real-audio-observer browser cases pass. The 25 integrated audio
policy/reward tests also pass. All **87/87** native-panel/full-overlay/owner-composition
comparisons pass at the original zero tolerances. This resolves the earlier
shop and guide-family raster differences, but does not complete the missing
controls, live-data projections or real-server acceptance gates below.

The bounded source-mode continuation additionally validates native filter
panels and data-only grid projections, populated recovery/fee/selection and
80-slot scroll projections, and the procedural title effect/current reconnect
banner: **106/106**, zero tolerance. The published-contract continuation adds
**45/45** exact native production, smithing, populated death-preview and reward
frame/projection comparisons. These use controlled source-only inputs and a
deterministic component double, not a real server or final M1 acceptance.
See `evidence/mode-comparison.json` and
`evidence/native-presentations/comparison.json`.
Nine native audio preference states add **18/18** full-overlay source/reprojection
checks at zero tolerance, recorded in `evidence/native-audio-controls/`. Those
browser controls use the real WebAudio adapter, with browser output muted for
shared-host privacy; they do not claim audible world playback.

## Shell integration

```ts
import {
  createUi, forwardWorldPointer, setUiCamera, onUiCameraRequest,
  getUiPreviewBounds, getUiPreviewRequest, setUiPreview,
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
const request = getUiPreviewRequest(ui);
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
`getUiPreviewBounds()` is available after the character/equipment interface's render.
`setUiPreview()` accepts an actual renderer canvas/bitmap at those exact native
dimensions; no resizing or human-preview substitution is performed. A missing
preview remains a missing renderer integration, not a finished penguin.
`getUiPreviewRequest(ui): Readonly<UiPreviewRequest> | null` returns a detached
descriptor: `purpose` (`appearance`/`equipment`), surface `bounds`, native
`modelBounds`, `sourceWidget`, `modelZoom`, `modelRotation`, approved local
appearance selection, and actual equipment/base metadata. Null equipment/base
means unavailable, not an invented empty loadout. Model parameters are native
widget parameters, not the world-camera ABI. Equipment-preview compositing
occurs at the source model widget's draw position and parent clip.

### Independent display helpers

* `setUiAbilityVisuals(ui, revision, facts)` accepts a read-only map keyed by
  native widget ID. `AbilityVisualTruth` contains optional `level`, `resources`,
  `requirements`, `lowerTier`, and `supersededByMultiSkill` booleans/null.
  It applies only to that exact world revision. Unknown requirements stay
  visible: the UI does not manufacture legal availability. Current skill/rune
  display facts use the immutable snapshot and original information-item
  parameters; execution remains server-authoritative.
* `projectAbilityGrid(widgets, catalogue, kind, mask, facts)` and
  `projectFilterPanel(widgets, kind, mask)` implement the source Classic layout
  and checkbox states. Prayer uses source bits6574–6578; Magic uses
  6605/6609/6606/6607/6608/12137/6548. These are **local display preferences**,
  not game commands or permissions. Escape closes the filter panel; the
  tier-dependent prayer option retains its native disabled state.
* `recoveryTemplate(catalogue, display)`, `projectRecovery(widgets, display)`
  and `recoveryControls(widgets, width, height, display, dispatch)` consume
  explicit `RecoveryDisplay` fields: storage, rows `{id, slot, item, allowed,
  reason}`, selected ID, coffer as decimal text, unit fee, capacity, bank/discard availability,
  and scroll position. Null monetary/capacity fields remain unknown.
  `RecoveryUiCommand` is `select`, `retrieve` (1/5/X/All), `take_all`,
  `bank_all`, `discard_all`, `examine`, or `close`. The adapter must map these
  to the published authoritative APIs; it must not make a partial-quantity
  request into a full-stack reclaim.

Current public `RecoveryView` is rendered at the calibrated native grid
positions. `ui.recovery.cofferBalance` and discard/coffer-offer permissions are
now consumed. Unit fee, capacity and per-row permissions remain unprojected;
`allowed: null` records unknown permission rather than inventing `true`. Existing
whole-item reclaim remains usable; unsupported partial quantities are clearly
identified rather than faked. The separate backend/UI-contract closure owns
the remaining quote, permission and quantity wiring.

### Versioned controller behavior

`gameplay-ui.ts` validates the complete declared projection shape, decimal
strings, stable identities and permissions without implementing gameplay
rules. Frozen immutable projections reuse their validation result. All new
requests go through `AppServices.send()`; no duplicate protocol or
`SourceUi` state is manufactured. U64 money/XP/revisions and signed gram weights
remain strings, with `BigInt` formatting.

Inventory actions retain the declared opaque action ID, canonical item and
instance. Production retains menu/recipe IDs and separate single/make-X
permission. Bank menus and drag/drop retain entry IDs, reject changed
identities, and wait for authoritative tab/quantity/notes updates. Placeholders
remain `value: null`, never a zero-quantity spendable `ItemView`.

Quest rewards project their supplied narrative and structured item/XP/point
fields into the real source text slots9–15; continuation sends the supplied
request without granting anything. Confirmations retain their opaque ID and
exact credit; acceptance/cancellation never clears them optimistically.
Level-up interfaces without a canonical source binding have explicit
`ui.source.reward.layout` feedback and real continuation, **not** a quest-scroll
substitute or completed level-up presentation.

Public chat checks the declared channel/permission and UTF-8 byte limit.
A rejected draft is retained. An acknowledged draft is neither echoed locally
nor cleared until the authoritative own-message update arrives; duplicate
submission while awaiting that update is stopped. Menus, quantity prompts,
drag state, Escape, blur, outside-pointer release and reconnect retain their
identity/error semantics. The world cannot receive clicks through a native
modal's blank interior.

### Source audio controls

The exact native-audio and Cook reward corrections are integrated as `b7cc380`
and `d2082e8` (upstream `888f9384`, then `f74652a5`). Their stable patches match
the authorized originals; the shared `AudioHandle` ABI and frozen source pack
were not changed.

```ts
import { bindUiAudio } from "./ui/index.ts";

// AppServices routes each normalized SOURCE position once:
// audioVolume(channel, position) { audio.volume(channel, position); }
// unlockAudio() { return audio.unlock(); }
const stopAudioUi = await bindUiAudio(ui, audio);
// Optional on teardown; ui.dispose() also detaches the observer.
stopAudioUi();
```

`bindUiAudio(UiHandle, AudioHandle): Promise<() => void>` consumes the audio
owner's `observeAudioState`. It observes actual source integer percentages and
raw mixers, and draws current slider/playback/permission state; it never installs guessed
defaults into the graph. `sourceAudioDefaults`, `sourceSliderToMixer` and
`sourceMixerToAssetGain` preserve the effective **255/127/127** defaults and
native nonlinear semantics. A music slider at50% is sent as **0.5**, not44/255,
44/128 or a provisional half-amplitude gain. Master50 is applied before the
lookup: with channel100, native levels are44/22/22. Diagnostic gain values are
not substituted for percentages in the visible source controls.

The native Audio settings subpage uses original sprites, geometry, mute
overlays and source tooltip strings. Track clicks/drag use the source integer
position formula; arrows/Home/End/PageUp/PageDown provide keyboard access.
Fast keyboard input reads the latest actual observation, not a stale painted
thumb value. Escape stops the drag without pretending to roll back changes
already applied to the graph. Source master-zero grey thumbs retain source
behavior rather than a guessed game-permission rule.

Channel Mute/Unmute remembers the actual in-session percentage. If a persisted
zero setting's remembered value was never supplied, explicit
`ui.audio.remembered_mute` feedback requests that value; no saved preference is
invented. Native source first-use fallback values are distinct from effective
constructor defaults. The title toggle uses real activation/global `mute()`
and **does not reset the three channel positions**. UI disposal detaches only
its observer; audio lifetime remains owned by the shell. Actual audio error
messages/codes are surfaced. Without a binding, thumbs remain unprojected and
controls explicitly unavailable rather than falsely showing100%.

Music modes retain native **area0 / shuffle1 / single2** control identities.
Manual selection still requires the actual unlocked-track/preferences/request
bridge. The UI does not fabricate `unlocked:true`, a playlist, a committed
music event or the old rejected `native_midi_end` directive.

The shell/renderer, not this UI, must call `setSourceAudioScene` with the actual
128-unit listener, plane/instance/owner, placed scenery emitters/orientations
and original varps, and connect `setSourceMusicSelector` to actual selection.
Native helpers own distance, retention, visibility and fades; do not reintroduce
caller-computed gain/distance. Do not turn a reconnect into `audio.update(null,
[])`. Likewise, clicking reward Continue sends only the gameplay request:
Cook level audio requires coherent committed before/after skill batches,
`skillId` and completion attribution, followed by the **actual** matching
Widget153 close. No base-level gain means no level cue. The UI never generates
that delta or close event from an optimistic click.

Original Modern Lumbridge groups64/327/163/145 and native255 jingle
representations40/54/58/64/65 remain pending the audio owner's additive
publication/AUDIO_INPUTS relay. No frozen file is changed or missing variant
scaled/invented here. The broader All Settings window and persisted mute/music
preference projection remain explicit required integrations.

## Asset contract

`ClientAssets.json("ui/manifest.json")` must resolve to
`assets/compiled/ui/manifest.json`. Image IDs in that catalogue start with `ui/`
and resolve under `assets/compiled/`. `ClientAssets.image()` must return decoded
same-origin images and the shell asset loader should verify the pinned hashes in
`ui/provenance.json`. The UI rejects the wrong pack/cache identity or missing
native font metrics. Do not map these asset IDs to full reference screenshots.
`native-widget-pool-v1` losslessly interns repeated source widgets in the wire
catalogue. `UiAssets.load()` decodes it automatically; direct catalogue consumers
should call `decodeUiCatalogue(raw)`. This keeps the many source-mode cases from
duplicating the complete HUD in a large startup download.

The catalogue contains original:

* sprite frames, offsets, canvas dimensions and palettes;
* CP1252 fonts 494/495/496/497, 256 masks/advances each and native ascent;
* native widget readbacks and static definitions;
* quantity-dependent item icons painted by `Client.createItemSprite`, without
  baked quantities; runtime quantities come from `ItemView`;
* model-only NPC portraits with parent-clip offsets (a model can exceed its
  nominal 32×32 widget);
* native scene minimap rasters and original map-dot/compass sprites.
* source title palettes/rune masks and current information-item parameters for
  the normal prayer/spellbook UI, without substituting unused alternate books;
* original placeholder definitions/icons, not alpha-tinted normal items;
* isolated original static production/reward model artwork, including the
  actual parchment models, never a finished panel screenshot.

The raster follows the native 16.16 trimmed-sprite draw extents, including a
possible final row beyond the nominal scaled rectangle; parent clipping still
applies. Overflowing centered labels use Java's truncating integer division.
Decorative item shadow values are read from the actual pinned `gp.ae → lj.ab`
argument (`lw.dm * 880555563`); unshadowed icons are generated by the original
item painter, not repaired by editing candidate pixels.

The original multi-skill script2046 is initialized with explicit source-only
choice/amount inputs for one through eighteen choices; source430 supplies the
bronze smithing table, and source972 the populated death preview. Item models
are isolated by the native renderer, not cut out of a finished panel. Their
keys retain source widget context as well as model/item/zoom/rotation:
the native first-column clip is not interchangeable with later columns.
Identical model-only images are content-addressed and reused. Additional
original item definitions and their raw hashes are recorded in provenance.

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
Lazy image failures do not cause an automatic retry loop. The in-client error
Retry action retries failed UI assets explicitly before asking the shell to
continue; disposal clears loaded-image, pending-request and raster caches.

The title effect is a port of the original procedural `cs.fl/je/as/bu` path:
source rune masks, palettes, noise/blur, integer compositing, wave offsets and
20ms source cycles. Native seeded effect captures are compared separately from
startup composition images. No finished title capture or prerecorded flame
movie is used in the client. Loading bars continue to use only actual app
counters. The current-source reconnect banner follows `lu.bz` and font494.

Startup and error presentation remains inside the seven approved source-framed
compositions. Long actual errors/IDs paginate at native glyph boundaries without
overlap, and inputs retain their values on rejection. No ClubScape legal terms,
business policies, Jagex account creation, Jagex Launcher flow or external
authentication have been invented. Historical Jagex terms/auth captures remain
reference-only, not ClubScape policy.

Only the approved penguin base and `body_type` A/B are selectable. The original
human kit/colour/pronoun control locations remain, with explicit necessary
unavailability; they are not requests to introduce new penguin cosmetics.

### Shop identity and stale views

The exact authorized shop repairs are integrated in order as `44e6de1`
(`3310032`) and `480d434` (`af75e17`). Their existing server quote/lifecycle
prerequisites were absent from the initial `f64f52b` UI base. The UI commits were
therefore replayed on the repairs' recorded parent `b3e8e12`; this preserves
`faa8002` (private ground items/owner-online clocks/source contact) and `9bf6969`
without reconstructing or editing unrelated backend changes. Both shop patch
IDs match their authorized originals exactly.

Every new UI `shop_buy`, including fixed-source rows and Buy-1/5/10/50/X,
sends `expected_item` equal to the **displayed row's canonical `item.id`**.
Numeric `sourceId`, a later occupant of the same index, and client prices are
never substituted. Held context menus and amount prompts retain that identity.
When a newer view has a different item at the index, the action is stopped and
the user must select the current item again.

This UI does not issue a separate quote RPC: Value uses the authoritative
`ShopView` price only if the displayed identity still matches. Shell/simulator
quote adapters must populate the same `expected_item` field on the existing
protobuf `ShopBuy`; no quote tag, default, legacy bytes or v1 intent-hash
behavior is changed here.

Server `StaleCommand` and uncertain transport failures retain their actual
message/error ID. The UI never automatically reconstructs or retries a buy
against a replacement row. The shell publishes the refreshed authoritative
view through the existing subscription and must retain the **original full
intent** for any uncertain transport replay. Selecting a newly displayed item
is a new user action, not a retargeted retry. Component tests cover row reuse
while a menu/amount/Value action is held, all quantity modes, and both rejection
classes; these tests do not duplicate backend trading rules.

## Outstanding scope — not hidden or accepted

See [`contract-gaps.json`](contract-gaps.json) for the published/wired subset and
exact residual requests. It is **not a fork of `web/shared/contracts.ts`**.

Additional UI implementation/fidelity work remains:

* complete validated source projection of all enabled/disabled/highlight
  signatures (the 71 states, 29 families and 11 signatures are retained);
* native source comparison of live data projections, not only source-widget
  replay, including all modal/choice/scroll/selected/disabled variants;
* real backend/protobuf/canonical-data implementation of `game.ui.v1`,
  including the separately promised nullable production-target correction;
* canonical level-up chat/popup source associations and complete native layouts;
* published production Make-All and bank default-All quantity semantics;
* authoritative recovery fee-unit/capacity/per-row/bank-all data and partial
  retrieval/bank-all requests; coffer/discard/coffer-offer wiring is complete;
* the actual renderer preview and dynamic minimap surface/state;
* real source-scene/varp/music/event wiring and the forthcoming additive audio
  publications; native audio sidebar sliders/current observation are complete;
* persisted remembered mute/music preferences and the broader All Settings
  window projection.

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
node --test ui/tests/unit.test.ts ui/tests/gameplay-ui.test.ts ui/tests/audio-controls.test.ts
node --test audio/audio.test.ts audio/native-policy.test.ts audio/reward-levels.test.ts
cd ..
python3 tools/ui-assets/glyph_proof.py

mkdir -p web/ui/.cache/xvfb
TMPDIR="$PWD/web/ui/.cache/xvfb" xvfb-run --auto-servernum \
  --server-args='-screen 0 2560x1440x24 -nolisten tcp' \
  node web/ui/tests/component-browser.mjs
TMPDIR="$PWD/web/ui/.cache/xvfb" xvfb-run --auto-servernum \
  --server-args='-screen 0 2560x1440x24 -nolisten tcp' \
  node web/ui/tests/gameplay-ui-browser.mjs
TMPDIR="$PWD/web/ui/.cache/xvfb" xvfb-run --auto-servernum \
  --server-args='-screen 0 2560x1440x24 -nolisten tcp' \
  node web/ui/tests/source-browser.mjs
python3 tools/ui-assets/compare.py
TMPDIR="$PWD/web/ui/.cache/xvfb" xvfb-run --auto-servernum \
  --server-args='-screen 0 2560x1440x24 -nolisten tcp' \
  node web/ui/tests/modes-browser.mjs
python3 tools/ui-assets/compare_modes.py
TMPDIR="$PWD/web/ui/.cache/xvfb" xvfb-run --auto-servernum \
  --server-args='-screen 0 2560x1440x24 -nolisten tcp' \
  node web/ui/tests/presentations-browser.mjs
python3 tools/ui-assets/compare_modes.py --presentations
TMPDIR="$PWD/web/ui/.cache/xvfb" xvfb-run --auto-servernum \
  --server-args='-screen 0 2560x1440x24 -nolisten tcp' \
  node web/ui/tests/audio-browser.mjs
python3 tools/ui-assets/compare_modes.py --audio-ui
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

Additional mode preparation is owned by `UiModeCapture`: it invokes the actual
native filter click scripts, supplies explicitly recorded UI preference bits,
and initializes source containers525/636 plus context-specific variables261–263
for the recovery fixtures. The silent original SFX queue is initialized so
native click scripts do not abort; any native `Client error` rejects preparation.
These test balances/fees are never copied into production state.

`UiScriptDump` is a bounded read-only source-contract inspection helper.
The optional MIT CFR artifact in `tools/ui-assets/dependencies.json` was used
locally to inspect only the original title-effect arithmetic. It is not a
runtime dependency, does not upload source code, and changes no global tool or
security configuration. Native preparation itself uses the existing pinned
Java/cache dependencies.

`UiPresentationCapture` initializes original scripts2046/430/972 and the native
reward widget's explicit text fields. Source-only production/recovery quantities
are recorded in `evidence/native-presentations/source-inputs.json`. The
read-only `inspect_scripts.py` helper accepts a script ID, `enum:ID`, or a
bounded `widget:GROUP` search against that same verified local cache.

After recording `test-results/typecheck.log`, unit TAP, component captures and
the three passing comparison reports, `python3 tools/ui-assets/archive_evidence.py`
archives the evidence and source/candidate/diff images. It refuses failed or
incomplete reports, preserves the existing zero tolerances, and keeps M1
acceptance false. Archived native references allow comparison after owned
scratch outputs have been cleaned.
