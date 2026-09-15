# Browser/WASM build and delivery

This is the bounded `m1-browser-shell` implementation, **not a completed M1
client**. The approved pack stays
`b62e19704e17d3d3e4e819f803ef49ba7cc54034ae407184b423427c65d9674d`,
checked against `web/shared/contracts.ts` and the owner approval record.
Final source fidelity, gameplay, audio,
performance and owner acceptance are unchanged.

## Normal commands

From the repository root:

```sh
pnpm --dir web install --frozen-lockfile
pnpm --dir web wasm
pnpm --dir web renderer
pnpm --dir web render:inputs
pnpm --dir web typecheck
pnpm --dir web test
pnpm --dir web reference:check
pnpm --dir web build
```

Pinned application tooling: pnpm12.4.1, TypeScript7.0.2, Vite8.3.0,
Playwright-core1.63.0, WebGPU types0.1.72. Rust1.98.1 and the installed
wasm-bindgen CLI0.2.128 are required. `wasm.ts` verifies CLI/crate equality and
uses `cargo build --release --lib --target wasm32-unknown-unknown`, followed by
the real `wasm-bindgen --target web` tool. It never hand-writes WASM glue.

Generated protocol JS/WASM are under `web/generated/protocol/`. Generated
declarations are tracked so standalone typechecking works after a clean
checkout; `pnpm build` regenerates the real matching implementation first.
Root `Cargo.lock` is outside this worker's ownership. The integrator must retain
Cargo's `clubscape-wasm` entry and the authorized renderer's wgpu30.0.1/
bytemuck/browser/native dependency graph when merging; renderer dependencies
are not all present in the original base lock. The build command performs normal Cargo resolution,
not a global tool installation or guessed alternate bindgen version.

`pnpm renderer` calls the actual renderer build command, checks
CLI0.2.128, and records the resulting JS/WASM/manifest hashes under
`.local/evidence/renderer-build.json`. `pnpm build` invokes it automatically.
It does not require a JDK/cache or unpack stale raw fixture files to compile.
`pnpm render:inputs` is the separate original-input reproduction step described
below; the delivered browser needs none of that tooling.
The renderer's dev TypeScript project is checked by its own build; the main
app checks `renderer/src`/`pkg` but does not merge the dev fixture's incompatible
global benchmark declaration into the production observer.

### Frozen reference integrity and current authority

The authorized `9bf6969` repair restores the exact pre-approval bytes of
`spec/art-style.md` (`79a15277…`) and `spec/interface-parity.md` (`77cfea36…`).
Their historical status is source context, **not revocation** of the exact
external owner record (`e910cb02…`). Current authority lives in
`spec/browser-implementation.md`, `spec/milestone-01.md` and that approval.
Do not update frozen prose or a hash lock to reflect current progress.

Every delivery runs `reference.ts`: it checks the exact pack/approval bytes,
verifies those frozen context records, then invokes the **unchanged**
`tools/reference-pack/validate.py --require-complete` with full audio decoding.
It never passes `--report`, rebuilds the pack, changes tolerances, or writes the
source evidence tree. The build artifact includes deterministic reference
integrity pins/counts; the dated execution receipt is separate under
`.local/evidence/browser-reference-integrity.json`.

The repaired source passes1,307 hashes,126 cases,71 tutorial states,109 native
images and264 FLACs. The existing85 pack tests remain unchanged and pass.
Neither those checks nor historical validator `owner_approved:false` fields
override the external approval or claim product/candidate acceptance.

With an **owned** account database configured privately in `DATABASE_URL`:

```sh
pnpm --dir web serve
# Or build then serve:
pnpm --dir web dev
```

`serve` runs the existing Rust account service with
`CLUBSCAPE_WEB_ROOT=<repo>/web/dist`; its default bind is `127.0.0.1:4010`.
It does not create a database/account/character/world or proxy requests.
The equivalent server command is:

```sh
CLUBSCAPE_WEB_ROOT="$PWD/web/dist" cargo run -p clubscape-server
```

Provide `CLUBSCAPE_GAME_ROOT` independently for real source gameplay. The
account-only server remains explicitly unavailable for characters/worlds.
Use standard HTTPS/SSH forwarding, not URL credentials, external RPC targets,
wildcard CORS, plaintext public binds or sandbox-disabling switches.
The server loads immutable verified files at startup; rebuilds require a
server restart. There is intentionally no unsafe cross-origin Vite dev proxy.

## Source content and assets

`web/app/manifest.ts` defines the public `ContentManifest` projection:

```text
schemaVersion: 1
sourcePackSha256: exact approved source pack digest
contentRevision: the generated WorldJoined content revision
artifactSha256: actual compiler-validated world.csc digest
catalog: display names/source IDs, equipment slots, quest completion IDs,
         generic asset references and explicit source icon bindings
assets[]: { id, url, sha256, bytes, contentType }
bootstrap[]: required startup asset IDs
aliases: explicit original source path IDs -> declared asset IDs
rendererManifest: declared asset ID, or null when unavailable
regions[authoritativeRegionId]:
  { sceneId, sceneAsset, requiredAssets[], routeId, workloadId, camera, controls }
```

Every URL is a canonical same-origin `/content/` or `/assets/` route. No
queries, fragments, percent-encoding, hidden paths, traversal, or external
redirects are accepted. Assets are explicit public images/audio/fonts/
JSON/GLB/binary files, not arbitrary repository files. Missing required
metadata/assets fail explicitly. Null cameras/controls are unavailable,
not permission to invent source cameras. `SourceCameraControls` records
the renderer/source input bindings; exact source behavior/comparison still
needs validation with the integrated adapter.

The public catalog is generated from the existing compiled content, never
from independently maintained item/world/price/guard definitions:

```sh
pnpm --dir web content \
  --artifact content/m1/game-content.csc.gz \
  --bindings assets/compiled/browser/presentation.json \
  --asset-root .local/game \
  --output .local/game/content/manifest.json
```

The committed artifact3 input now exists. The example presentation bindings
and asset-root are **not supplied product presentation assets**; the renderer/
UI/audio preparation owners must supply those real compiled outputs.
The bindings JSON supplies `sourcePackSha256`, explicit `assets`,
`bootstrap`, `rendererManifest`, `renderer`, `regions`, and optional `icons`
mapping known item/skill IDs to declared original raster assets. Original
`inventoryActions` labels can be provided explicitly; an existing
ContentManifest's `catalog.icons`/`catalog.inventoryActions` are also retained.
Renderer coverage/asset mappings and the same gzip carrier format survive
this entrypoint as well as `source:bundle`. It cannot replace catalog
names, completion stages, source IDs, initial state or authority rules.
`project-content` revalidates the real artifact in `Runtime` mode first;
invalid/TestFixture artifacts fail. Gzip is bounded and passed to the native
compiler over stdin. Region scene-asset IDs must match that
compiled content. Files reside under `asset-root` at their public URL paths
without the leading slash. No entire cache is copied.

Current canonical artifact3 is `m1.source-backed.v3.0e506f3dab24bbe0`, raw SHA-256
`df3e2a452c100ecd94d2abc68e5cb1556f58090700474f547e7fd36de6682b3d`.
The checked projection contains118 item definitions,29 runtime regions,
one shop and5,010 compiled referenced asset IDs. Its exact six remaining
unresolved paths are preserved in `contentValidation`, not treated as active
failures or erased. `crates/server/src/game_service/readiness.rs` owns the
executable F2P inactivity proofs and revalidates restored/mutated state.
The shell neither duplicates those proofs nor bypasses them. The authorized
`faa8002` native fix passes source-solid near-face contact, Single1/Make-X-one3,
owner-online ground clocks and the120000 played-tick boundary. The former
two-native-failure interlock is obsolete. Compiler/display projection alone
still does not establish a deployed server's readiness; that remains the
actual backend's job.
Current projection/run evidence:
`.local/evidence/browser-shell-df3e2a45/source-projection.json`,
`source-refresh.json` and `source-run-pin.json` in that same directory.
Earlier `.local/evidence/m1-artifact3-projection.json` remains historical.

The complete actor fix pair `62a003c` + `e1076a8`, then source refresh
`3a74cbe`, are integrated. Source JSON comparison changes **only `/revision`**;
gameplay rules, geometry, assets and source policies are otherwise identical.
Queued/current attack deadlines and effective presence remain engine-owned.
There is no remaining shop/actor review-finding blocker.

Existing `.local/source-definition-a200ca08` and `.local/source-audio-a200ca08`
bundles retain raw artifact
`a200ca08c80f6a3fc812fe02ca95f52325e183084f83b12e8d470a1f37cc6d62`
for historical diagnostics. They are never rewritten to the new canonical
revision. A new candidate uses a new output directory and a **different
isolated world UUID**; acknowledged progress is not migrated or reseeded.

`run-pins.ts` captures an existing bundle's world UUID, actual artifact,
public-content revision/hash and descriptor/asset-manifest hashes without
consulting the latest canonical source. `assertSourceRunPin` rejects changes
even when replacement files are internally self-consistent. New bundles
include private `source-run-pin.json`; the isolated runner also compares the
captured pin before/after its checks. Keep that exact pin across reconnect/
restart within a run. Current canonical refreshes authorize a new candidate,
not a silent artifact swap inside a retained world.

### Real original-definition delivery

```sh
pnpm --dir web source:bundle .local/source-definition-df3e2a45 <new-isolated-world-uuid>
```

This new-directory-only tool strictly projects the current canonical artifact,
uses the existing verified source-publication loader, and emits only the5,010
required original item/NPC/object/region definitions. Region gzip is decoded
losslessly and checked against its original JSON digest; collection members
are canonically serialized without changing fields. Original inventory action
labels come from `interfaceOptions`. Source hashes/selectors/transformations
are recorded in the private `source-provenance.json`; no whole cache or private
game state is exposed. Original geometry and asset IDs are retained.

The real audio, UI and streaming WebGPU factories/assets are integrated.
All 61 published source blocks are reproduced and delivered, alongside actual
skeletal/gear/live-layer inputs and the model-only preview. The renderer owns
104x104 assembly/recenter. Live camera/control source bindings are still
absent; normal entry reports that specific gap rather than selecting a
fixture or inventing a spawn camera. Missing animation/running observer fields,
dynamic minimap and real rendered entity counts remain explicit dependencies.

The authorized `2c5fe68b` repair resolves the former descriptor blocker:
512KiB admits the actual406,574-byte map, with the exact limit/limit+1 covered
by backend tests. GAME_ROOT also works independently of WEB_ROOT. Actual
canonical standalone startup and game+web startup now pass. Public asset/
request budgets are unchanged, no asset IDs are trimmed, and old
`game_file_size` evidence remains historical rather than a current interlock.

The authorized audio factory `7ea817f5` is now integrated. New source-bundle
outputs additionally include its exported `AUDIO_INPUTS` documents and exact
264 unchanged FLACs plus two original reference WAVs, with no alias
invented for the source silence. Original source path IDs resolve through
explicit same-origin aliases. The private game membership map still contains
only the5,010 compiler-required IDs; presentation delivery does not hide or inflate
that descriptor's source validation. No waveform, gain, delay, loop or playlist
is changed by the build layer.

The authorized native policy/Cook delta commits `888f9384` and `f74652a5`
are also integrated. Runtime volume arguments are source normalized slider
positions, with native defaults255/127/127 and nonlinear lookup inside audio.
The shell uses a v2 preference-semantic marker instead of reinterpreting old
linear values, delegates typed source-scene/music-state inputs, preserves
coherent before/after committed reward batches, and exposes actual native
control observations. No source gain/distance/varp/next-song math is duplicated.
The current bridge still lacks actual128-unit listener, all placed emitters,
bound original varps and actual manual/unlock-state preferences; no dummy values
are supplied. Native source-bound next-track selection needs no caller callback.

The authorized `f9e466d3` closure is integrated. All four required `AUDIO_INPUTS`
metadata routes and all nine original native255 assets are delivered.
`assets/manifests/osrs/audio-m1-supplement.json` is pinned at
`840aef91bac9a1fd042bdb1c3662ff92a378e279f48335108730e168af550d91`
and served through `/content/audio/supplement.json`. Exact IDs have prefix
`asset.source.osrs.cache2695.audio-supplement.` with suffixes
`music.{64,327,163,145}.native255` and `jingle.{40,54,58,64,65}.native255`;
payload URLs retain their original
`/assets/source/osrs/audio-supplement/<kind>/<id>-native255.flac` paths.
The nine files add45,199,584 bytes and never replace an old ID or waveform.
The existing266 payloads and three base metadata hashes remain exact.

`SourceAudioSession.setMusicState` delegates the actual source mode, unlocks,
selection, playlist, loop and Modern/Classic state to the audio owner.
There is no required caller-guessed next-song callback and no missing-four-
tracks/five-jingles blocker. The owner chooses a native255 representation only
where calibrated. Control observation distinguishes configured channel levels,
reference128/255 gains, and each actual voice's rendered/applied levels.
No blind128 amplification or limiter is added by the shell.

Native-control shell evidence is separate from earlier provisional-volume runs:
`.local/evidence/native-audio-title-v2/result.json` records actual default
255/127/127 levels and a50% music position producing native mixer44 (asset
calibration44/128), not a linear0.5 gain. The real canonical source entry under
`.local/evidence/early-native-audio-v2/result.json` records those actual defaults
while explicitly reporting the then-missing inputs. They are historical:
the next-selector and nine-publication blockers are now closed, while actual
scene/manual-state/event provenance remains separate.

The new immutable candidate is `.local/source-audio-native255-df3e2a45`,
world UUID `61d4c53c-1b63-4d50-b515-7587f3cd6cf6`, with the same canonical
`df3e2a45...` game artifact. Previous world/asset bundles are not rewritten.
Its audio input/control fixture checks all nine actual served decodes against
published float-channel hashes and the four actual music representations
through the native state API. The fixture's selections are not account
unlocks and it fabricates no world or committed gameplay event.
Receipts are `.local/evidence/audio-closure-controls/result.json` (including
`titleAudio.supplement`) and `.local/evidence/audio-closure-source-entry/result.json`.
The latter uses only the actual canonical account/world path; absent source
scene/manual-state facts remain explicitly unavailable.

```sh
GAME=.local/source-audio-native255-df3e2a45
CLUBSCAPE_CLIENT_MANIFEST="$GAME/content/manifest.json" \
CLUBSCAPE_CLIENT_ASSET_ROOT="$GAME" \
CLUBSCAPE_CONTENT_OWNER=game pnpm --dir web build
# With an owned DATABASE_URL supplied privately:
CLUBSCAPE_GAME_ROOT="$PWD/$GAME" pnpm --dir web serve
```

The authorized UI commits `069b5028`, `f34663c3`, `6a1b11fa` add the actual
self-subscribing UI. `ui-assets.ts` verifies `assets/compiled/ui/provenance.json`
and selects catalogue-referenced title/sprite/item/portrait/minimap PNGs plus
the published compass/seven dot primitives used directly by the UI's runtime
minimap adapter. All are hash-verified through the same provenance; no
panel/evidence PNG is copied. There are1,187 image primitives and7,181 total
source/audio/UI/render assets in the current combined candidate. The
24,658,127-byte UI catalogue remains an independent bounded asset; it is not
inserted into actor snapshots.

Use a **new** directory/world UUID when adding UI to an earlier pinned bundle:

```sh
pnpm --dir web render:inputs
pnpm --dir web source:bundle .local/source-stream-candidate <new-isolated-world-uuid>
CLUBSCAPE_CLIENT_MANIFEST=.local/source-stream-candidate/content/manifest.json \
CLUBSCAPE_CLIENT_ASSET_ROOT=.local/source-stream-candidate \
CLUBSCAPE_CONTENT_OWNER=game \
pnpm --dir web build
```

The actual game root owns7,182 public asset/manifest routes; the web root serves
only the compiled HTML/JS/CSS/WASM and build identity, with no overlapping routes.
The independent account/audio Chrome check instantiates
the actual audio factory through the shell, verifies recoverable gesture
feedback, original Scape Main0 on genuine input, actual decoded-buffer
observations and source-clock advancement, then checks disconnect versus real
title reset. It does not use a mock world/cue loop, replace the audio-owner
fixture, or establish gameplay/presentation or source-policy calibration.
Evidence appears as `titleAudio` in the normal browser result JSON.

For actual UI/WASM/canonical-world integration:

```sh
CLUBSCAPE_CLIENT_MANIFEST=.local/source-stream-candidate/content/manifest.json \
CLUBSCAPE_CLIENT_ASSET_ROOT=.local/source-stream-candidate \
CLUBSCAPE_CONTENT_OWNER=game \
pnpm --dir web build

CLUBSCAPE_RECORDED_CAMERA=tutorial-starting-house \
CLUBSCAPE_BROWSER_EVIDENCE=.local/evidence/source-stream-candidate \
CLUBSCAPE_BROWSER_EXECUTABLE=/path/to/verified/native/chrome \
pnpm --dir web test:isolated --game-root .local/source-stream-candidate
```

The source-mode runner uses its bounded owned PostgreSQL database. It first
starts the real world **without** WEB_ROOT, stops it gracefully, then starts the
same pinned world alongside the built browser code. Real source UI controls
perform registration, auth-error/login, empty creation, sequenced appearance,
logout/relogin. A real server restart at the same origin retains the memory
token and acknowledged character state; no replacement artifact, seed or
network mock is used. Credentials stay in browser memory and are never
reported. Evidence includes actual title pixels, startup/run pins and the
onboarding result. The recorded-camera mode loads the actual canonical
region's blocks, not a fixture scene; it remains a camera diagnostic rather
than a live-camera/game-journey proof. Without this explicit mode, the current
null source camera/control binding fails closed. The earlier UI-only and
five-fixture onboarding proofs remain historical. Complete journey, world
fidelity, real audio-scene/selection wiring and M1 acceptance
remain separate. The legacy descriptor-probe environment name now selects
real startup, not an expected failure.

For a game-owned asset bundle:

```sh
CLUBSCAPE_CLIENT_MANIFEST=.local/game/content/manifest.json \
CLUBSCAPE_CLIENT_ASSET_ROOT=.local/game \
CLUBSCAPE_CONTENT_OWNER=game \
pnpm --dir web build
```

The default content route is `/content/manifest.json`; change it only with
`CLUBSCAPE_CONTENT_PATH` to match the server's actual WorldJoined path.
`game` is the default content owner: files are verified/pinned by the web
build but **not duplicated** into its routes. The game root's existing
`clubscape-game-assets.json` must serve those same explicit paths/hashes;
`clubscape-game.json` must map the existing asset IDs and content manifest.
This tool does not alter either private server/game configuration.

Use `CLUBSCAPE_CONTENT_OWNER=web` only when those asset routes belong to
`CLUBSCAPE_WEB_ROOT`, e.g. account-only title/login presentation. It copies
only listed public files and the manifest. The server correctly rejects
overlapping web/game routes; do not ship the same routes through both roots.

## Static deployment identity

### Published renderer reproduction, delivery and real composition

The initial six renderer commits and ordered continuations `e96e3b8`,
`1937201`, `c098133`, `0c541d9` are integrated. `render-assets.ts` pins manifest
`a6a1b3dcd307aa1b8e1f8c3e850c087f78c0e818c644d47e12c58595ad3b6464`.
The runtime graph includes the 61 blocks, source textures, sequences, NPC
definitions, penguin/human-retarget inputs, equipment, dynamic objects,
ground-item models, preview metadata and explicit diagnostic scenes.

`pnpm render:inputs` uses the existing original exporter in a separate owned
`.local/render-inputs-a6a1b3dc` directory. All122 compressed block buffers
match the published SHA/length pins exactly:55,720,421 bytes. The Java
exporter's disk inventory omits65 absent validation/raw-twin records; its
metadata and every retained file record are structurally identical to the
published manifest. The script checks that exact distinction, retains the
exporter's receipt, and packages the original **byte-exact** manifest, never a
new source identity. Missing runtime records or any changed value fail.
Evidence is `.local/evidence/render-block-reproduction.json`.

The first reproduction needs the guide-verified original JDK/cache tooling.
Subsequent packaging can use those verified outputs, or an explicit
`CLUBSCAPE_RENDER_INPUTS` directory containing the same pinned inputs.
Neither owner browsers nor the serving machine need Java/cache data.
Only runtime dependencies are public: validation bakes/tables and raw
scene/block duplicates are not shipped. Gzip URLs keep their exact names;
physical `.gz.bin` carriers satisfy the server allowlist without changing
bytes or content encoding.

The current immutable native255 source-stream candidate has7,181 declared assets
(221,090,292 public bytes including its content manifest), separate from all
retained earlier bundles. Create a **new** directory and world UUID:

```sh
pnpm --dir web renderer
pnpm --dir web render:inputs
pnpm --dir web source:bundle .local/source-stream-candidate <new-isolated-world-uuid>
CLUBSCAPE_CLIENT_MANIFEST=.local/source-stream-candidate/content/manifest.json \
CLUBSCAPE_CLIENT_ASSET_ROOT=.local/source-stream-candidate \
CLUBSCAPE_CONTENT_OWNER=game \
pnpm --dir web build

CLUBSCAPE_RECORDED_CAMERA=tutorial-starting-house \
CLUBSCAPE_BROWSER_EVIDENCE=.local/evidence/source-stream-candidate \
CLUBSCAPE_BROWSER_EXECUTABLE=/path/to/verified/native/chrome \
pnpm --dir web test:isolated --game-root .local/source-stream-candidate
```

Serve that result with a privately configured owned `DATABASE_URL`:

```sh
CLUBSCAPE_GAME_ROOT="$PWD/.local/source-stream-candidate" \
CLUBSCAPE_WEB_ROOT="$PWD/web/dist" \
cargo run -p clubscape-server
# Open /?presentation_camera=tutorial-starting-house for recorded-camera diagnostics.
```

This mode keeps actual scene `blocks@3056,3056`, route `region.osrs.12336`,
and workload `recorded-camera-not-journey`. The first real entry loads nine
published squares and forwards ten canonical dynamic objects. Its native
480x315 model-only preview does not advance world frame counters. Full source
signup/creation/appearance/logout/relogin and same-artifact server restart are
real. The separate game-device-loss fault check destroys only that browser's
actual configured device and verifies explicit UI failure/no fallback.

The earlier `CLUBSCAPE_EARLY_SCENE` / `presentation_scene` mode remains an
explicit five-fixture diagnostic. Neither mode silently maps a canonical
region to a fixture. Without a recorded diagnostic or real configured live
camera, normal entry reports the camera binding gap.

The two-frame shell clock uses a per-native-sequence canvas submission/
completion ledger; offscreen previews cannot steal receipts. Real loaded
square diagnostics govern residency, not append-only fetch history.
The native `scenePlacement.blocks` flag has a demonstrated ABI bug (false
for `blocks@` scenes). The owned adapter normalizes that one flag only after
actual assembly/base/size/square agreement, preserves the raw observation
and reports the mismatch. No minimap, entity-count or motion fidelity is
inferred from it.

The published native CPU/GPU suite now runs all mandatory scene/NPC cases
without runtime skipping. Ten obsolete, ignored raw twins left by the prior
owned unpack were hash-identified and removed; tests then used the current
published gzip bytes. No renderer tests, tolerances or source inputs changed.
Locally reproducible validation-only suites remain explicitly ignored.

Sparky measurements remain engineering-only. The first repaired streamed run
(`.local/evidence/source-stream-a6a1-v2/result.json`) measured605 genuine
completed frames in10,120.905ms:59.7773fps, maximum completion gap60.655ms,
nine gaps over33.4ms. This **fails** the60fps/gap gates; missing rendered
entity counts and `game.ui.v1` keep readiness false. No frame limiter,
rounded-up cadence, physical-display or owner-Mac/Edge acceptance is claimed.

`web/dist/clubscape-web.json` implements the existing server contract:

```json
{
  "schema_version": 1,
  "files": [
    {
      "url": "/",
      "path": "index.html",
      "sha256": "<actual file SHA-256>",
      "content_type": "text/html; charset=utf-8"
    }
  ]
}
```

The real manifest enumerates all built JS/CSS/WASM and selected public source
files. It enforces the server's bounded extensions/file counts/sizes and rejects
symlinks. No directory listing, repo root, `.env`, Rust source, source map,
private `world.csc`, or private world descriptor is published.

`/client/build-artifact.json` pins code files and game-owned asset records.
Its SHA-256 is the observer's `buildArtifactSha256`. The artifact explicitly
excludes itself, `/client/build.json` and the outer delivery manifest to avoid
a self-referential hash. `/client/build.json` contains that verified pointer,
actual build ID, approved source/benchmark file digests, content pin and
component-export presence. Server startup pins both files in its outer
manifest. Owner `bindRun` overrides only the separate canonical audit-contract
digest, not these artifact/source/settings/asset identities.

## Bounded proof and remaining integration

```sh
cargo test -p clubscape-wasm -p clubscape-client-core
cargo clippy -p clubscape-wasm --all-targets -- -D warnings
pnpm --dir web test
pnpm --dir web build

# Verify this executable/profile against docs/machines/sparky.md first:
CLUBSCAPE_BROWSER_EXECUTABLE=/home/lramos15/.cache/ms-playwright/chromium-1243/chrome-linux-arm64/chrome \
pnpm --dir web test:isolated
```

The isolated runner owns a CPU2/RAM512MiB PostgreSQL container on a random
loopback port, a real account server serving this exact built bundle, and
headful Chrome under Xvfb. It verifies real WASM + HTTP signup/login/error/
logout/revocation/relogin/account reconnect, no invented character, immutable
public-state/privacy behavior and zero fake benchmark frames. Credentials
are generated only inside memory and never reported. Its UI-independent
helper is **not a UI signup or game-journey test**.

Sparky evidence is in `.local/evidence/browser-shell/result.json` and the
explicit bootstrap integration diagnostic screenshot. The observed native
Chrome is153.0.8010.12; namespace/seccomp checks pass. Its Vulkan GPU-process
sandbox observation is **false**, the known host limitation, not a security
pass or reason to disable anything. The local run uses the documented graphics
flags plus `--enable-automation` for command-line introspection, removes
Playwright's unsafe SwiftShader opt-in, and leaves namespace/seccomp enabled.
Xvfb uses an explicit in-project authority file; browser profiles/scratch
files stay in the project and owned processes/resources are cleaned.

Before integrated-client completion:

The `d1532d6f` versioned gameplay UI publication is contract-only. Current
`game.ui.v1` requests are recognized through the exact shared Rust enum but
rejected before wire/sequence allocation; `WorldView.ui` remains absent on
the actual legacy server. The pure DTO projector and browser version/decimal/
identity validation are prepared, not enabled as a backend implementation.
Actual ServerHello capability, generated wire support and a complete version-1
view are all required. The pending production-target nullability correction
is not approximated with a fake target. Evidence for this current unsupported
boundary is recorded separately under
`.local/evidence/early-render-ui-contract-v1/result.json`.

1. Streamed source assets, action/equipment packs, canonical picks and the
   native-size preview are composed. The current backend's empty player
   animation and absent running/active-animation observers still need exact
   protocol/view/renderer alignment. The imported renderer's activity/adjacency
   fallback is not accepted source motion. Dynamic minimap, rendered entity
   counts and remaining gear-fit violations stay renderer-owner work.
2. Supply actual source-bound live camera/control bindings for normal entry.
   Published fixture cameras are deliberately not promoted to live defaults.
   The current private/public game deployment is real and hash-verified;
   there is no missing-block or old descriptor-size interlock. A native actor
   reset API is also needed to show a fresh uncreated-character preview after
   another world session without reusing prior gear.
3. Existing backend/content commits are integrated: guarded contexts,
   quotes, presence, all three additive intents and lifecycle-journal retries
   are mapped. Align UI consumption of multi-panel recovery and exact U64 fees,
   plus source inventory action labels. Shop expected-item/capacity fixes
   `3310032` and `af75e17` are integrated: new buys and quotes send the displayed
   canonical `item.id` as exact `expected_item` (ShopBuy tag4). Legacy None
   bytes/hashes and original uncertain intents are preserved. Rejected
   selections refresh without retargeting or silently retrying another item.
   No shop identity/backend capacity or game-root descriptor blocker remains.
   Source contact/offline-clock and512KiB descriptor/environment repairs are
   integrated without trimmed content or seeded state. The complete actor/collision/style
   repairs are integrated without a new browser outcome API.
   The newly published `GameplayUiView`/`GameplayUiIntent` still requires its
   actual engine, generated Protobuf and canonical data implementation before
   versioned production/reward/confirmation/ability/bank/death/chat/appearance
   controls can be admitted. Preserve the stable IDs and string numeric fields
   when that implementation is relayed; no old-view empty-success fallback.
4. The shell observes real canvas queue completions and adapts actual renderer
   diagnostics/source zoom. The renderer still needs to expose rendered entity
   counts (not discarded raw stats) and the
   actual source audio event provenance described in `web/app/README.md`.
   The audio factory and real decoder observations are integrated; source
   cycle/group/delay/action/cue and Cook-widget linkage are not present in
   the current event wire and must not be guessed. Native gain/spatial/fade/
   duration/default policies are integrated; actual source listener/varps/
   emitters, committed Cook attribution and manual/unlock-state preferences
   remain concrete inputs. All nine native255 publications and internal
   next-track selection are integrated, not remaining blockers. The actual UI consumes
   `forwardWorldPointer`, `setUiCamera` and `onUiCameraRequest` in logical pixels;
   renderer picking remains backing-pixel based. Source entity/pose/morph/dynamic-object
   state must come from the real view, not fixtures or fabricated animations.
5. The actual source UI registration/character/appearance/logout/restart path
   now passes against the canonical server. Complete the remaining full journey,
   asset/
   device failures and nonblank world/UI/audio. Run the independent frozen
   source comparisons and genuine completed-frame harness, then the owner's
   M-series Mac Chrome **and** Edge acceptance runs. The historical infrastructure
   helper and the current explicit early fixture composition do not pass any
   of those gates.
