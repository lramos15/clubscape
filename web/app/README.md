# Browser composition

`main.ts` starts the real WASM bridge and loads the three independent component
entry points:

* `web/renderer/src/index.ts`: actual `createRenderer` + diagnostics extension
* `web/ui/index.ts`: `createUi: CreateUi`
* `web/audio/index.ts`: `createAudio: CreateAudio`

These are Vite module imports, not script-tag loading or a substitute renderer.
All three actual factories are integrated, including the published 61-block
streaming renderer and its model-only player preview. The shell accepts only
published canonical region IDs or an explicitly selected diagnostic fixture.
Source-bound live camera/control bindings are still absent from the current
delivery; normal entry reports that exact gap, not obsolete missing-renderer
or five-scene-only coverage. A recorded-camera streaming diagnostic is separate
and labeled. The small HTML bootstrap diagnostic remains only a fatal startup
fallback, **not** approved title/login presentation.

Build delivery separately checks the exact external reference approval and
the complete unchanged strict pack validator. The restored hash-bound
art/interface context files intentionally keep pre-approval prose; current
implementation authority is in the browser/milestone guidance and external
owner record, not a rewrite of those frozen inputs.

## Published gameplay UI capability

The authorized `d1532d6f` contract publication is present, but its backend/
Protobuf/data implementation is not. The shell treats `game.ui.v1` as a
negotiated capability requiring both an implemented wire decoder and the
**complete** `WorldView.ui.version === 1` projection. Published types alone
cannot enable it. `BrowserApp.gameplayUi()` and the observation-only
`window.__clubscapeClientStateV1.gameplayUi()` distinguish not advertised,
wire unavailable and missing versioned view; legacy world state never receives
a fabricated empty UI object.

The real legacy-server entry displays one recoverable unsupported-capability
notice, not repeated cosmetic failures on every poll. New identity-bound
`GameplayUiIntent` requests still go through WASM, which parses the shared
Rust request type and refuses unsupported wire operations before sending or
allocating a sequence. Old quantity/index/close-interface intents are not
substitutes for new stable entry/menu/reward/confirmation identities.

`gameplay-ui.ts` validates the full published versioned projection, exact
decimal strings and placeholder shapes before state is recursively frozen.
The native shared-DTO projection is prepared and fixture-tested, not wired
to synthetic outcomes. Full M1/benchmark readiness also requires the actual
versioned projection. Renderer/audio APIs, SourceAudioSettings, minimap and
model-preview ownership are unchanged. The authorized `351847f3` correction
is integrated: an active production menu can have `target:null` for genuine
inventory-only production. Null is valid data, not a missing capability or an
absent production menu; no dummy facility is manufactured. A complete projected
menu must still include the target field, and non-null values remain typed
world targets.

Full final-v4 wire integration remains deferred. The relayed tags are
`WorldSnapshot.ui = 20`, `WorldInput.ui = 41`, and the naturally optional
production target field3. Bank request field21 (`expected_bank_revision`)
must take the exact decimal `ui.bank.revision`, never a character/world
revision that changes on passive ticks. The current projection retains that
independent string unchanged; no hand-written wire message, bank precondition
substitution or progression bypass is added ahead of the generated protocol.

`composition.ts` is also dependency-injectable for bounded component tests.
Such injected handles are not an integrated game. The real UI owns its
source-sprite/font composition, selections, drag state, menus and accessible
controls; the shell supplies immutable authoritative state and real services.
`createUi` subscribes itself. Composition does not call `ui.update` a second
time; its separate observer updates only renderer/benchmark state.

## Input and lifecycle

* UI pointer capture/default prevention is checked before world picking.
  A press captured by UI cannot become a world action on release. Right-click
  only requests a context menu; middle drag only changes the camera.
* World picks come only from `RendererHandle.pick()` and are forwarded once to
  the actual `forwardWorldPointer` adapter, including move/primary/context and
  control-key state. The UI owns selected-item/spell/ground/entity action
  choice; the shell never independently sends a duplicate world action.
  The current renderer returns canonical actor/object IDs and source scenery
  footprints. Opaque old hashes, invalid tiles and unknown entity IDs reject;
  there is no nearest-object or ID-suffix guess.
* Arrow keys and middle-drag camera/scroll zoom use explicit source camera
  bindings in the current region manifest. There are no fabricated initial
  camera defaults. Keyboard controls are ignored for editable/accessible
  input focus; focus loss clears held keys.
* UI capture/forwarding/resizing use **viewport-local logical pixels**, as the
  real adapter requires. Only renderer picking/resizing is converted to its
  backing pixels; the world stays full resolution. The same camera is sent to
  `renderer.camera` and `setUiCamera`. `onUiCameraRequest` is disposed with input.
* `createCharacter(appearance)` is the actual UI workflow adapter: a new actor
  gets the empty source creation RPC, then joins, then receives a separate
  sequenced `ConfirmAppearance` request with the UI choice. No appearance,
  inventory, XP or stage is seeded in creation. Existing initialized accounts
  join/resume directly at login; repeated appearance confirmation is rejected.
* `services.logout()` requests actual `LeaveWorld` before account logout.
  A source/combat/presence failure does not clear the account or claim logout.
  Literal `send({kind:"request_logout"})` first sends that exact sequenced game
  intent, then finishes the requested account logout. A lost acknowledgement
  never implicitly rejoins an offline body. Uncertain lifecycle retries retain
  the original request UUID/lease; account logout reconciles a lost sequenced
  exit without replaying game effects.
* Mutation requests are serialized, bounded to 32 waiting operations, and
  snapshotted before UI mutation. Lost transport cancels unsent inputs.
  Rejoin/retry uses client-core's operation ID/sequence reconciliation.
* Polling is real `/v1/rpc` every 600ms while connected, without advancing
  authoritative time. Reconnect is bounded/backed off. Passwords are never
  retained for reconnect. Unrecoverable device/protocol/component failures
  stop polling/input and cannot be overwritten by a later snapshot.
* Source presence facts gate input. An observed offline body leaves the visible
  world and requires explicit entry, not automatic rejoin. Connected but
  source-busy/death states remain genuine rendered workloads; they are not
  faked input availability or missing-scene failures.

## Typed source contexts and read-only quotes

`public-state.ts` exposes the additive guarded bank/shop/recovery/permission/
presence data while keeping the shared contract untouched. Recovery panels are
available in `world.recoveryContext.views`; the old singular field is populated
only for one panel. Render decimal-string `fullEntryFee`/`fullSelectionFee`;
the legacy numeric `cost` is deliberately null, never a rounded fee.

`BrowserApp.quote(QuoteRequest)` sends `PollWorld.quote` through WASM and returns
only the server's correlated result. Bank/shop partial transfers and reasons
come from immutable engine planners, not local multiplication or speculative
intents. The result carries its source revision/tick and is not a future
capacity/price guarantee. A changed ItemId/selection rejects the quote while
retaining the actual updated world view and deduplicated events.

For a shop buy, the UI must retain the **displayed row's** `item.id` as
`ShopPurchaseIntent.expected_item`; do not look up a replacement ID from a
possibly changed index, use numeric `sourceId`, or send a calculated price.
The exact identity-bearing wire contract is now integrated:

```ts
await services.send({
  kind: "shop_buy", shop: displayedShop.id, item_index: displayedRow.index,
  quantity, expected_item: displayedRow.item.id,
});
await app.quote({
  kind: "shop_buy", shop: displayedShop.id, itemIndex: displayedRow.index,
  quantity, expected_item: displayedRow.item.id,
});
```

The selection is snapshotted before queuing; uncertain retries keep the
original UUID/sequence/intent/ItemId. A server buy/quote conflict refreshes the
actual view, preserves the original error ID, cancels unsent dependent inputs
and asks the user to choose again. It never resubmits against a replacement
row. This also applies to a rejected uncertain purchase after rejoin. The wire
currently groups source-rule denials under Conflict, so refresh is deliberately
performed for every rejected shop buy/quote rather than guessing a subtype
from human error text. No purchase identity blocker remains.

`inventoryActions` in the display catalog contains only labels extracted from
verified original item `interfaceOptions`, preserving their order. The source
server still validates every item request. PlayedTime/GroundClock persistence
remains private; the browser neither reads those clocks nor sets/advances them.

`clubscape.preferences.v2` stores schema-checked **source normalized slider
positions**, explicitly marked `native-source-slider-v1`, and the named visual
profile. Values are quantized to source integer percentages. Old v1 provisional
linear-gain values are not silently reinterpreted: the old record is retained,
native defaults apply, and a recoverable migration notice asks for explicit
source controls. Tokens/account names/passwords are never preferences.
Browser storage denial/quota failure leaves memory-only settings.

## Actual loading and benchmark observations

`AssetLoader` validates same-origin paths, response MIME, length and SHA-256.
`image()` waits for real native image decoding; `json()` validates UTF-8/JSON.
`bytes()` means fetched, **not decoded**. `decode(id, decoder)` lets real
audio/renderer adapters register successful decoding of verified bytes.
`retain(ids)` releases previous-region caches, retaining requested startup/
region assets rather than downloading the whole source cache.

The delivery tool supplies real canonical definitions, original audio, and
the actual compiled UI catalogue/primitives. `ui/manifest.json` and `ui/*`
images resolve through explicit aliases and provenance SHA/byte checks; no
whole panel/reference image is published as UI. Startup accounting includes
the catalogue, title and source sprite dependencies, not merely three audio
metadata files. Lazy item/portrait/minimap images remain on demand.

The authorized512-KiB descriptor and independent GAME_ROOT parsing repair is
integrated. The actual canonical world now starts standalone and alongside
the code bundle, using game-owned nonoverlapping asset routes. The real UI
test exercises signup, wrong-password feedback, login, empty source creation,
appearance confirmation, logout/relogin and a real server restart while the
browser retains its memory token and the exact world/artifact pin.
`window.__clubscapeClientStateV1.read()` is an immutable observation-only view
for this boundary; it exposes no credential, mutation or outcome setter.
The source title and actual streamed starting region are visibly rendered;
logical/backing resize boundaries are tested. Earlier native-size penguin
preview receipts predate the richer UI request contract and are historical.
The current legacy server lacks its authoritative preview base metadata;
that absence is explicit rather than an invented base/loadout.
The current real run receives ten dynamic object records through WASM.
Their `object_id` must be a valid canonical ObjectId with source metadata in
the validated content catalog. `sourceId` is never parsed from an ID suffix or
substituted from `definition_id`. IDs, tiles, instances, state, optional door
state, quarter turns and exact string expiry remain intact. Unknown canonical
metadata or out-of-range rotation fails explicitly.
This is not a complete Tutorial Island journey.

## Actual streamed renderer and explicit presentation diagnostics

The initial six renderer commits and authorized continuations `e96e3b8`,
`1937201`, `c098133`, `0c541d9` are integrated through the actual
`web/renderer/src/index.ts` factory and `web/renderer/build.sh`.
The shell does not implement a rasterizer, animation policy or gear fit.
Build output uses wasm-bindgen0.2.128; actual build hashes are recorded in
`.local/evidence/renderer-build.json`. The current original renderer manifest is
`a6a1b3dcd307aa1b8e1f8c3e850c087f78c0e818c644d47e12c58595ad3b6464`.
All 122 compressed buffers for its 61 blocks were reproduced byte-exactly from
the pinned original inputs and are included in browser/server delivery.

`renderer.ts` validates that exact manifest, passes the compiled WASM URL and
actual asset base, forwards source `WorldView` including `dynamicObjects`, and
maps actual diagnostics. `regionSceneId(square)` selects real block assembly;
the native adapter owns its 104x104 rebuild/recenter and resource eviction.
Unchanged immutable views are not reapplied on UI-only notices.
`source-fetch.ts` enforces same-origin, redirect-error, no-cookie/no-referrer
policy even for component factories that internally call global `fetch`.

Normal region IDs are never rewritten to fixture names. Explicit
`/?presentation_scene=tutorial-starting-house` selects that named fixture for
**early presentation only**, with a visible warning and workload identity
`early-presentation-not-journey`. Its exact camera comes from the approved
source capture, not a claimed live-player initial camera. Real UI requests,
source account/character state and server restart remain real, but the fixture
scene/layout must not be counted as legitimate journey/streaming proof.
Other exported fixture names are equally explicit; unsupported names fail.

`/?presentation_camera=tutorial-starting-house` instead uses only that
explicit recorded camera over the **actual authoritative region's streamed
blocks**. It does not fetch a fixture scene. The recorded camera must belong
to that source square. `sceneId` reports the actual assembled `blocks@x,y`;
`routeId` remains the server region and workload is
`recorded-camera-not-journey`. Neither diagnostic is a verified live spawn/
arrival camera. Unknown/duplicate URL parameters and URL secrets reject.

The camera uses16384 units/turn, near50 and
the renderer's **viewport-only** `sourceZoomForViewportHeight(backingHeight)`
helper (662 at1080). Live resize updates
the same camera/zoom in renderer and UI. An early fixture with no live control
calibration keeps its recorded camera: no guessed mouse sensitivity or scroll
curve. Source live camera/control bindings must be supplied before normal
entry. A logical privacy `instance` is not guessed to be the original client's
instanced-map flag; stock roof behavior stays renderer-owned.

The independent byte-identical frozen inventory/full-HUD replay in
`53fbd543828cda12f28f2ec30b8f78c9d914ccef` reports native full-HUD zoom410,
not viewport-only662, at1080. Current composed captures therefore do **not**
establish matched full-HUD projection, even when their GPU frames complete.
The presentation observer and new source-run receipts label this explicitly.
The renderer owns resolution against matched original source cases; the shell
does not override zoom to410, rescale HUD/minimap coordinates, or alter the
native-size model preview to approximate a match.

That commit's12 dynamic cases and
`assets/reference/osrs240/m1-dynamic/case-index.json` (SHA-256
`dde300e30ff909e539c283fa7053c673abc9428ce357894f0727be08da8ad557`)
are offline original-render references only. They are not client assets,
authoritative gameplay, or candidate acceptance. No supplemental capture
import was needed for this shell-side classification, and frozen inputs remain
untouched.

The current native frame timestamps still use wall-clock `Date.now()`.
`CanvasGpuClock` independently observes **actual game-canvas texture acquisition,
queue submission and `onSubmittedWorkDone` receipt**. A ledger associates each
native sequence with its own canvas blit, even when two native promises settle
out of order. Offscreen preview/fill submissions never replace that receipt
or increment world frame counts. The actual adapter's two-frame pipeline is
enabled without a shell serial limiter. Submission/completion clocks use
`performance.now()`; native measured GPU pass durations are retained.
`read()` never renders, drains records or advances counters. The additional
receipt observation remains part of measurement overhead.

Loaded squares come from native diagnostics. Append-only fetch history is not
claimed as residency after eviction. The current WASM
`scenePlacement().blocks` incorrectly tests `scene_id().is_none()`, even for
successfully assembled `blocks@` scenes. The owned adapter normalizes only
that flag when actual base/size/scene/loaded-square diagnostics agree, reports
the compatibility issue, and preserves `nativeScenePlacement` alongside it.
Neither placement observation is a dynamic minimap.

`ModelPreview` now consumes the complete `getUiPreviewRequest()` every frame:
purpose, surface/model bounds, source widget, model zoom/rotation, local approved
appearance, actual equipment and base. The detached request is frozen while
pending; changes to any of those inputs invalidate stale readbacks even at
the same canvas size.

The owned `frameUiPreview` adapter maps a supported source widget's model
centre into the exact native-sized surface and forwards its source model
parameters to `framePlayerPreview`. It does not change the world-camera zoom
or rescale UI geometry. Authoritative base/equipment must be available and
match the renderer's actual actor; unsupported widgets or local appearance
edits that the current renderer cannot apply independently fail explicitly.
No world snapshot is rewritten to render a tentative UI selection. Null
equipment/base is unavailable, never a dummy empty loadout or guessed base.
The current legacy-v3 initial request is 480x315/widget679:73 with unavailable
base/equipment, so it is reported unavailable and publishes no substitute.

Successful RGBA readbacks still use a same-sized canvas/`putImageData` followed
by `setUiPreview`, with one pending preview independent of two world frames.
There is no source PNG, human preview, UI scaling or world-FPS credit. Full
base/local-appearance/equipment preview alignment and actor reset remain
renderer/final-v4 dependencies.

The adapter's diagnostics currently expose actual loaded assets/scene/device/
timestamps, but discard the raw WASM `entities_drawn` field and do not expose
per-kind workload counts. `observe().entities` therefore stays `{}` and
benchmark readiness stays false; no manifest/static/server-snapshot count is
passed off as rendered entities. Renderer continuation must supply the real
counts and dynamic minimap. The backend still emits an empty player animation
and no actual running/active-animation observer fields. Those are forwarded
unchanged with explicit unsupported-motion feedback; the shell never chooses
a pose from a run checkbox or adjacent object. The imported renderer's
activity/adjacency fallback is an unaccepted interop gap, not source motion
evidence. Hat/shield gear-fit violations also remain unaccepted.

The real streamed Chrome/Xvfb check is engineering-only. It covers actual UI
signup, wrong-password feedback, empty creation/appearance, source block
fetches, ten canonical dynamic objects, exact preview-request/unavailability
semantics, pinned server restart,
logout/relogin, and an actual game-device-loss fault check. It keeps
`game.ui.v1` and rendered-count readiness false. Frame distributions and
failing 60-FPS/gap budgets remain exact evidence, not a Mac/Edge/M1 acceptance.

## Actual audio adapter

The ordered UI handoff `6988951`, `b7d6f8a`, `669e1f4`, `b76babf`,
`b151d01`, `53ed974`, `8d99bf1`, then nullable corrections `0ef2344` and
`a85ff40` is integrated after its shared/audio prerequisites. No broad backend
or final-v4 source-generation commit was pulled in with those owned patches.
The UI still owns exactly one AppServices subscription; no extra `ui.update`
or UI-side `audio.update(world, events)` feed is installed.

Composition binds `bindUiAudio` to the **real native AudioHandle**, not the
SourceAudioSession wrapper. The UI observes source percentages, raw mixers and
actual128/255 voice representations. Its title gesture reaches the same
`unlockAudio` path and mute leaves channel percentages intact. UI disposal
detaches its observer without taking ownership of the audio engine.
Applied settings hashes also include the observed native master/channel/mute
values and actual UI-applied music preferences, not just stored slider values.
Automatic track changes and waveform/fade counters are not preferences.
Identical settings do not repeatedly invalidate the hash while frames run.

For supplied source music state, the single committed `audio.update(world,
events)` precedes `setUiMusicState(ui, actualPlayerId, state)`. The latter
applies the state to both native audio and UI; `getUiMusicState` is used for
applied-state observation without replaying that setter. The player ID is
checked again before handling `onUiMusicStateChange` persistence feedback.
The versioned preference helper has not been relayed: applied in-session
preferences get explicit persistence-unavailable feedback rather than being
silently saved under another player, storing unlocks, or inventing restored
values. Native Skip Track, exactly three saved playlists and remembered mute
are likewise left to the audio owner's forthcoming public helpers, not mode
flips or a guessed storage schema.

`audio.ts` wraps the imported factory and the authorized native policy/Cook
delta updates `888f9384` + `f74652a5`; it is not a second audio engine. `SourceAudioSession` forwards
validated world/events and delegates mute, volume, unlock, disconnect and
disposal. The trusted UI handler reaches the factory's real `resume()` call
before any unrelated await. `AUDIO_GESTURE_REQUIRED` remains recoverable, and
`soundEnabled` comes from actual running/unlocked/output-enabled/connected/
unmuted state—not factory resolution or a completed button promise.

Transport loss or an observed offline body invokes only `disconnected()`.
It does **not** call `update(null,[])`: that reset is reserved for actual
title selection and acknowledged logout. Committed source/correlation fields
are forwarded unchanged, including widget153/quest/completion IDs when they
are actually supplied.

The decoder, original WAV half-gain, queues, silence, sample offsets, fades,
positional mixes and playlist policy remain exclusively in `web/audio`.
`AudioHandle.volume` now receives a **source normalized slider position**,
never a provisional linear gain. The shell forwards it exactly once; native
integer percentage lookup and nonlinear mixing remain in the audio engine.
Defaults come from `sourceAudioDefaults()` (effective255/127/127), not old
linear unity assumptions. `SourceAudioSession.controls()`, `BrowserApp.audioControls()`
and the read-only client observer expose actual positions/mixer/master state,
with `sourceSliderToMixer` and `sourceMixerToAssetGain` used only for control
observation—not applied again to playback. Channel `referenceGains` explicitly
distinguish native128/native255 calibration; they are not a claim about an
active voice. `voices` reports the actual asset ID, rendered native level,
applied native level and gain from `observeAudioState`. Thus a pending
low-to-high jingle replacement cannot be mislabeled as already at full gain.
The native music mode IDs remain
area0/shuffle1/single2. Old `native_midi_end` directives are not emitted.

`mountApplication` accepts optional `sourceAudio.scene(world)` and
`sourceAudio.music(world)` producers returning the imported `SourceAudioScene`
and `SourceMusicState`. The latter preserves actual mode, Modern/Classic,
unlocked groups, selected group, playlist and loop preference. The wrapper
delegates `setSourceAudioScene`/`setSourceMusicState` without computing attenuation,
retention, owner visibility, fades, next songs or varps. Scene coordinates stay
in128-unit space; orientation, instance/owner and varp values are unchanged.
An unavailable new scene stops stale ambience with explicit feedback, not an
empty-scene success. The first joined snapshot establishes the native actor
without replaying old events, then binds its scene/music state after the
audio-owned reset. Later scene inputs precede the coherent event batch; music
state binds after the current world/area. No intermediate empty world update
destroys Cook reward deltas. Identical music states do not restart playback.

The native audio component now owns source-bound continuation. A next-song
callback is **not required**. `musicContinuation:"native-bound"` identifies
that policy, while `playback` reports the actual background plan.
`sourceSceneSupplied` and `sourceMusicStateSupplied` describe supplied inputs,
not proof that every emitter/asset is ready. `providedMusicState` is an immutable
last accepted input, not an unlock grant or an echo of guessed source defaults.
An omitted music update retains that actor's last supplied preference; title/
actor changes clear the shell record and different actors never inherit it.
Current renderer/protocol exports still lack the real listener/scenery/varp
projection and manual/unlock-state input. The known M1 area's internal
continuation is not blocked on either an invented unlock list or caller selector.

Native policies are now calibrated, not the earlier generic gain/distance/fade
placeholders. The authorized `f9e466d3` publication closes the nine native255
input gaps. Actual source-scene/event/manual-state wiring remains separate.
Do not convert tiles to guessed
listener centres, use only interactable entities as all audible scenery, or
derive varps from quest-stage names.

`AssetLoader` supports an explicit `aliases` table for the original
`AUDIO_INPUTS` metadata path IDs and payload paths. Aliases resolve only to
declared hash-pinned same-origin assets, never arbitrary repository paths.
The source bundle includes all four required metadata documents,264 unchanged
FLACs, two original cue WAVs and nine additive native255 FLACs. The fourth
`AUDIO_INPUTS.supplement` route has SHA-256
`840aef91bac9a1fd042bdb1c3662ff92a378e279f48335108730e168af550d91`
and resolves to `/content/audio/supplement.json`. Its payload URLs retain
`/assets/source/osrs/audio-supplement/<kind>/<id>-native255.flac` exactly.
Old IDs, paths, hashes and the frozen pack remain unchanged.
Only metadata loads before a gesture; payloads are
decoded by the actual factory on demand. Observations use its successful
`decoded`/`evicted` notices; a fetch attempt is not counted as a load, and
float PCM cache bytes are not mistaken for transfer bytes.

**Current event-wire dependency:** generated `game.Event` still has only
kind/asset/actor/event IDs plus generic outcome fields. It does not carry
numeric audio/sequence groups, received delay/repeat, actual source cycles,
action/cue correlations, or Cook widget/completion/reward linkage. The WASM
bridge marks only its already-validated committed event stream and removes
incompatible generic payload keys. Missing timing/group/correlation fields
remain missing and produce explicit audio feedback, not a guessed zero delay,
server-tick conversion, duplicate animation cue or synthetic completion.
`SourceAudioSession.update` is ready to pass the exact enriched events once
the backend/renderer/UI boundary is relayed. Unit forwarding checks are not
a substitute for that legitimate journey integration.

For Cook reward audio, the shell forwards **one coherent** committed world/
event batch after the previous immutable world, preserving `skillId`,
`previousLevel`, `level`, `completionId` and the source-selected group when
actually supplied. The audio owner captures/validates the before/after
base-level/XP delta and defers it until matching widget153 closure. The wrapper
does not manufacture group33/level4, derive a jingle from XP, split the batch
with an intermediate empty update, or generate a cue for no level gain.
Existing wire `level_up.skill`, if supplied, is preserved as `skillId`; absent
attribution/group/level/widget fields remain absent. A spatial-input error
does not discard the committed world/event batch.

Four Modern Lumbridge groups64/327/163/145 and source-native255 jingle inputs
40/54/58/64/65 are now published and delivered. Their IDs have prefix
`asset.source.osrs.cache2695.audio-supplement.` and suffix
`<kind>.<id>.native255`; music64 and jingle64 remain distinct. Representation
choice, offsets and calibration stay entirely with audio. No blind128
amplification, limiter, alias substitute or base-pack rewrite is added.

The real Chrome shell check now exercises source title track0 through this
factory, a genuine trusted input, actual22050-Hz decoding/clock advancement,
recoverable startup permission feedback, retained disconnect selection, and
explicit title reset. It uses no fake world or game-cue loop and does not
certify complete source-scene/music-selection wiring, gameplay, host-speaker
perception or presentation acceptance. Checks now also observe native defaults
and a real50% source-slider lookup without double normalization. The additive
input/control fixture decodes all nine served FLACs against independently
published float-channel hashes and exercises all four real music inputs through
`setMusicState`, including idempotence and locked-selection rejection. Its
explicit music-state fixtures are not account unlocks; it creates no synthetic
world or committed gameplay/jingle event. Full-duration native continuation
and jingle calibration remain the separate audio-owner component proofs.

Renderer frame promises must resolve **after that actual render submission's
`GPUQueue.onSubmittedWorkDone()` receipt**. RAF requests stay pipelined; no
frame is counted from RAF, a timer, an empty submission, or a screenshot.
Frame sequences are contiguous; malformed receipts/history overruns invalidate
the observer instead of estimating frames.

For complete `window.__clubscapeBenchmarkV1` readiness, the actual renderer
also needs the optional, observation-only extension described by
`RendererObservation` in `benchmark.ts`:

```ts
observe(): {
  ready: boolean;
  sceneId: string;
  assets: Array<{ id: string; sha256: string; loaded: boolean }>;
  entities: Record<string, number>;
  gpuTimestampPassScope: string | null;
  settings: Readonly<Record<string, unknown>> | null;
}
```

Return real decoder residency, loaded entity counts and **applied** stable
render-profile settings, not a copy of expected workload/configuration values.
Dynamic actor/camera poses are not stable profile settings. Missing observation
data leaves readiness false and counts unknown. No entity count comes from a
manifest declaration or an unrendered server snapshot.

Settings digest is SHA-256 of canonical compact JSON:

```text
{
  "visual": {
    "declaredProfile": <build's frozen visual profile>,
    "renderer": <actual applied settings from observe()>
  },
  "preferences": <bounded Settings.read() value with only explicit audioOverrides()>
}
```

It remains unavailable until real applied renderer settings exist. Device
epochs come from the device that actually configured the game canvas, not the
capability probe. Device loss is terminal feedback, never a WebGL/software
fallback. `read()` observes only; `bindRun` may bind the owner audit contract
digest once and cannot change assets/settings/build/scene/counters.

Missing GPU durations are omitted, not zero. Diagnostics distinguish enabled
timestamp-query features, declared pass scope/sample coverage, actual WASM
startup time and loader byte/fetch/decode observations. GPU completion is not
physical scanout; the independent browser harness and owner Mac runs remain
mandatory.

Commands, delivery formats and exact remaining integration requirements are
in [`tools/web-build/README.md`](../../tools/web-build/README.md).
