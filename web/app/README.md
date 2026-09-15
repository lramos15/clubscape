# Browser composition

`main.ts` starts the real WASM bridge and loads the three independent component
entry points:

* `web/renderer/index.ts`: `createRenderer: CreateRenderer`
* `web/ui/index.ts`: `createUi: CreateUi`
* `web/audio/index.ts`: `createAudio: CreateAudio`

These are Vite module imports, not script-tag loading or a substitute renderer.
Missing components fail visibly with an integration error ID. The small HTML
bootstrap diagnostic is **not approved title/login presentation**.

`composition.ts` is also dependency-injectable for bounded component tests.
Such injected handles are not an integrated game. The real UI owns its
source-sprite/font composition, selections, drag state, menus and accessible
controls; the shell supplies immutable authoritative state and real services.

## Input and lifecycle

* UI pointer capture/default prevention is checked before world picking.
  A press captured by UI cannot become a world action on release. Right-click
  only requests a context menu; middle drag only changes the camera.
* World left-clicks use `RendererHandle.pick()`: exact tiles become `Walk`;
  entity IDs and server-evaluated action names become typed `InteractWith`.
  There is no guessed target or local pathfinding/authority mutation.
* Arrow keys and middle-drag camera/scroll zoom use explicit source camera
  bindings in the current region manifest. There are no fabricated initial
  camera defaults. Keyboard controls are ignored for editable/accessible
  input focus; focus loss clears held keys.
* `UiHandle.capturesPointer`, optional `worldContext(pick,x,y)`, renderer picking
  and component resizing use **canvas backing-pixel coordinates**. Canvas CSS
  size remains the whole viewport. DPR is honored without downscaling.
* `services.logout()` requests actual `LeaveWorld` before account logout.
  A source/combat/presence failure does not clear the account or claim logout.
  Literal `send({kind:"request_logout"})` remains that exact typed game intent;
  it is not substituted for the account-lifecycle API.
* Mutation requests are serialized, bounded to 32 waiting operations, and
  snapshotted before UI mutation. Lost transport cancels unsent inputs.
  Rejoin/retry uses client-core's operation ID/sequence reconciliation.
* Polling is real `/v1/rpc` every 600ms while connected, without advancing
  authoritative time. Reconnect is bounded/backed off. Passwords are never
  retained for reconnect. Unrecoverable device/protocol/component failures
  stop polling/input and cannot be overwritten by a later snapshot.

Only `clubscape.preferences.v1` stores local, schema-checked normalized channel
volumes and the named visual profile. Tokens/account names/passwords are not
preferences. Browser storage denial/quota failure leaves memory-only settings.

## Actual loading and benchmark observations

`AssetLoader` validates same-origin paths, response MIME, length and SHA-256.
`image()` waits for real native image decoding; `json()` validates UTF-8/JSON.
`bytes()` means fetched, **not decoded**. `decode(id, decoder)` lets real
audio/renderer adapters register successful decoding of verified bytes.
`retain(ids)` releases previous-region caches, retaining requested startup/
region assets rather than downloading the whole source cache.

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
  "preferences": <bounded Settings.read() value>
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
