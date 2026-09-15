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

## Implemented UI4 wire and current component boundary

The authorized FINAL4 implementation through `78fcec4` is present. The shell treats `game.ui.v1` as a
negotiated capability requiring both an implemented wire decoder and the
**complete** `WorldView.ui.version === 1` projection. Published types alone
cannot enable it. `BrowserApp.gameplayUi()` and the observation-only
`window.__clubscapeClientStateV1.gameplayUi()` distinguish not advertised,
wire unavailable and missing versioned view; legacy world state never receives
a fabricated empty UI object.

All20 current `GameplayUiIntent` variants go through the shared Rust request
enum, generated Protobuf and protocol validators. Document pages, placeholders,
stable entry/menu/reward/confirmation identities and exact bank preconditions
are not approximated with legacy quantity/index operations.

`gameplay-ui.ts` validates the full published versioned projection, exact
decimal strings and placeholder shapes before state is recursively frozen.
The native shared-DTO projection is wired to actual server responses, never
synthetic outcomes. Full M1/benchmark readiness also requires the actual
versioned projection. Renderer/audio APIs, SourceAudioSettings, minimap and
model-preview ownership are unchanged. The authorized `351847f3` correction
is integrated: an active production menu can have `target:null` for genuine
inventory-only production. Null is valid data, not a missing capability or an
absent production menu; no dummy facility is manufactured. A complete projected
menu must still include the target field, and non-null values remain typed
world targets.

The implemented tags are
`WorldSnapshot.ui = 20`, `WorldInput.ui = 41`, and the naturally optional
production target field3. Bank request field21 (`expected_bank_revision`)
must take the exact decimal `ui.bank.revision`, never a character/world
revision that changes on passive ticks. The current projection retains that
independent string before request queueing. Explicit stale retry preconditions
remain unchanged; the backend performs mutable bank checks after durable
deduplication. No character revision, hand-written wire or progression bypass
substitutes for this contract.

The current UI-owner module predates the added active-tab/document and
placeholder/document-page cases. Its exhaustive TypeScript schema/dispatch
tables and component fixture do not yet compile against FINAL4. Its audio
fixture also lacks the native snapshot's required `preferences` field; these
are five current UI-owner compile errors, and those files
remain outside this worker's edit scope. Normal `pnpm build` is therefore
blocked on the UI-owner follow-up, not on the implemented WASM/API mapping.
Diagnostic Vite bundling is explicitly separate from a passing production
typecheck. Real UI4 entry/experience/source-lock/reconnect checks can run, but
do not prove complete UI consumption or final acceptance.

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

`clubscape.preferences.v2` retains device/title-only **source normalized slider
positions**, explicitly marked `native-source-slider-v1`, and the named visual
profile. Values are quantized to source integer percentages. Old v1 provisional
linear-gain values are not silently reinterpreted: the old record is retained,
native defaults apply, and a recoverable migration notice asks for explicit
source controls. Tokens/account names/passwords are never preferences.
This older device record is never migrated into a character's saved playlists,
mute memory or native flags. In-world volume requests use the player binding
below, not the global device record.

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
`8281dd16f01e996569661ddd711022af803ed62412a075698ca6c247ce164399`.
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
counts and dynamic minimap. The backend now supplies `game.observer.v1` movement/action fields, including
final running steps during idle/exhaustion and stable action timing. They are
forwarded with exact absence/null/string semantics. The shell never chooses a
pose from a run checkbox or nearby object. Explicit renderer consumption is
still an owner-side dependency; the older activity/adjacency fallback remains
unaccepted, as do unverified gear-fit claims.

The authorized `d5320e1` minimap API is integrated with regenerated matching
bindings. Its merge retained the existing motion/gear code rather than importing
unrelayed predecessors. `MinimapRelay` validates the actual 512x512 RGBA/mask,
scale4/margin48, base/plane/revision and forwards once per renderer revision
when a real UI sink is provided. The present UI has no live-surface/icon setter,
so the actual raster/notes/stats/map-element IDs are observed but not replaced
by a static PNG or claimed delivered. Edge-five-tile/instance/icon limitations
remain explicit and `complete` is not full-surface fidelity.

`5ab678d` could not be consumed independently: it requires unrelayed
`547353b`, `710d29e`, `83636e7`, `61e986b` (and earlier motion/gear work).
Its attempted follow-up was aborted; no missing renderer logic or passing
comparison evidence was reconstructed.

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
values, the actual applied native preference record and UI-applied music
state, not just stored slider values.
Automatic track changes and waveform/fade counters are not preferences.
Identical settings do not repeatedly invalidate the hash while frames run.

The authorized `64bd3257` native preference closure is integrated. The shell
uses the exact audio-owned parser/serializer, defaults and control APIs.
There is no longer a missing preference-helper dependency. `PlayerAudioPreferenceStore`
stores only native `SourceAudioPreferences` v1 under
`clubscape.player-audio.v1.<actual-actor-id>`. The actor is a separate key:
account IDs/names, unlocks, playheads, permission and privacy mute are not in
the record. Three exact100-slot arrays retain null holes and numbered identity;
current and remembered percentages remain separate. Native source defaults,
mute fallbacks, flag policy, group encodings and next-track choice stay with audio.

Only a confirmed storage `null` uses `sourceAudioPreferenceDefaults()`.
Denied/failed reads, invalid return values, corrupt JSON, unknown versions and
invalid native records produce explicit errors, retain the stored bytes, and
do not apply defaults or rewrite the record. Per-character writes are serial;
pending values coalesce with explicit stored/superseded receipts. A read waits
for that actor's outstanding writes. A failed save rejects active/pending
receipts and cannot silently restore stale storage; explicit persistence can
retry without an observer-driven retry loop.

`PlayerAudioComposition` connects the awaited `ClientHooks.prepareAudio` to
the single synchronous boundary:

```text
load genuine actor record -> validate separately supplied source unlocks
-> bind actual UI observer/native-control adapter
-> publish authoritative world
-> audio.update(world, committedEvents)
-> applySourceAudioPreferences(audio, actorId, record, actualUnlockedGroups)
-> setUiMusicState(ui, binding.playerId, binding.musicState)
```

No storage/network await or intermediate empty world update splits the last
three calls. Only the returned native record is saved. Unchanged snapshots
do not reproject/rewrite preferences, and the native record remains immutable.
Pending storage loads are cancellable without preventing requested logout.
Disconnect retains genuine loaded state while cancelling controls; acknowledged
title/logout clears the native binding and reloads even the same actor on
re-entry. Old UI control objects, async Skip and save completions are fenced
by entry generation as well as actor ID. A reconnect during a pending write
obtains a current-entry receipt rather than remaining permanently pending.
Disposal detaches observers and flushes real writes, surfacing failures.

The optional owned injection port
`Components.bindUiAudioPreferences(ui, controls): () => void` requires the
**actual UI owner's native-control routing**. It receives an entry-scoped
`PlayerAudioControls` object with `read`, `observe`, `setMusic`,
`selectPlaylist`, `setSavedPlaylist`, `editSavedPlaylist`, `toggleMute`,
`setPercent`, `skip` and `persistCurrent`. Mutations delegate the corresponding
native API and project its returned canonical state; Skip never flips modes.
UI handlers must still start `services.unlockAudio()` in the trusted handler
before awaiting anything (native Skip starts its own required resume).
The detach function must remove those entry's handlers.

`loadComponents()` currently has no relayed implementation for that port.
The existing UI's legacy mute map and different legacy `setSourceMusicState`
cannot represent rich bound preferences, so world controls remain detached
rather than silently using them. `onUiMusicStateChange` persistence is installed
only with the real new adapter and requires the current entry's actual native
binding. The default host also lacks authoritative music unlock inputs.
It reports `audio.preferences.ui_adapter_required` and
`audio.preferences.source_unlocks_required`, keeping world audio disconnected
without blocking real server/renderer/UI progress. It never invents `[]` or
grants a region's tracks, buffers old events for replay, or claims audio ready.

`window.__clubscapeClientStateV1.audioPreferenceStatus()` is observation-only:
load origin, entry/phase/save/error state, actual UI binding/source-input
availability and last applied actor/revision/tick. It exposes no credential or
mutation API. `audioControls().preferences` is the actual native binding or
null, not a guessed projection of a pending record.

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
`sourceAudio.unlockedGroups(world)` producers returning real `SourceAudioScene`
and authoritative published source groups. The older `sourceAudio.music(world)`
producer is accepted only for its actual `unlockedGroups` when no explicit
unlock producer exists; its legacy selection/playlist/loop fields are never
interpreted as saved slots or native flags. Genuine client preferences come
only from the native record. The wrapper delegates scene/preference APIs without computing attenuation,
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
The player coordinator retains only the same actor's genuine loaded record;
title/actor changes clear it and different actors never inherit it.
Current renderer/protocol exports still lack the real listener/scenery/varp
projection and authoritative unlock input. Internal native selection is
implemented, but does not establish those supplied facts or authorize playback
with unbound constructor preferences. No caller selector is required.

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

The additional native preference/storage fixture uses the real AudioHandle,
verified original files and browser localStorage. It exercises actual load/
apply/save ordering, source remembered/fallback percentages and nonlinear
mixers, three-slot holes, typed Skip status, same-ID re-entry and delayed
coalesced writes. Its explicit world/unlock/UI-port fixtures are **not** the
actual game UI adapter or a legitimate journey. It runs after game GPU teardown;
it does not prove simultaneous rendering/audio performance.

Sparky runs exposed `AUDIO_LOADING_LATE`/`AUDIO_TIMING_LATE` for native click2266.
The original failed runs are retained, and the narrower diagnostic compares
coordinated controls with direct native API calls without the preference
coordinator. Both paths exceeded the unchanged native
20.045351473922903ms dispatch tolerance (26.4399092970522ms observed in the
profiling run); API-call durations were below0.5ms. That is evidence of a real
native/host timing gap, not proof of its root cause or permission for a timing
override. The runner records exact dispatches/failures and exits unsuccessfully
when that gate fails, even when all storage/entry contracts passed.
Browser-local test output is muted for shared-host privacy; speakers, Mac/Edge
and M1 acceptance remain unexecuted.

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
    "renderer": <actual applied settings from observe()>,
    "audio": <observed master/volumes/nativeMixer/privacy mute>,
    "playerAudioPreferences": <actual applied native record or null>,
    "music": <actual UI-applied mode/area/selection/playlist/loop or null>
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
