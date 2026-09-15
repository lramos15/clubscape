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

`pnpm renderer` calls the actual owned renderer unpack/build commands, checks
CLI0.2.128, and records the resulting JS/WASM/manifest hashes under
`.local/evidence/renderer-build.json`. `pnpm build` invokes it automatically.
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
`bootstrap`, `rendererManifest`, `regions`, and optional `icons` mapping known
item/skill IDs to declared original raster assets. It cannot replace catalog
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

The real audio, UI and initial WebGPU factories/assets are now integrated.
The renderer's five named source fixture scenes are not a full authoritative
region map; normal uncovered-region entry is explicit unavailability, never
an arbitrary fixture or blank/software3D fallback. Live region/camera/actions/
equipment/minimap/model-preview coverage remains renderer-owner continuation.

The authorized `2c5fe68b` repair resolves the former descriptor blocker:
512KiB admits the actual406,574-byte map, with the exact limit/limit+1 covered
by backend tests. GAME_ROOT also works independently of WEB_ROOT. Actual
canonical standalone startup and game+web startup now pass. Public asset/
request budgets are unchanged, no asset IDs are trimmed, and old
`game_file_size` evidence remains historical rather than a current interlock.

The authorized audio factory `7ea817f5` is now integrated. New source-bundle
outputs additionally include its exported `AUDIO_INPUTS` documents and exact
264 FLACs plus two original reference WAVs, with no alias
invented for the source silence. Original source path IDs resolve through
explicit same-origin aliases. The private game membership map still contains
only the5,010 compiler-required IDs; presentation delivery does not hide or inflate
that descriptor's source validation. No waveform, gain, delay, loop or playlist
is changed by the build layer.

The authorized native policy/Cook delta commits `888f9384` and `f74652a5`
are also integrated. Runtime volume arguments are source normalized slider
positions, with native defaults255/127/127 and nonlinear lookup inside audio.
The shell uses a v2 preference-semantic marker instead of reinterpreting old
linear values, delegates typed source-scene/next-music inputs, preserves
coherent before/after committed reward batches, and exposes actual native
control observations. No source gain/distance/varp/next-song math is duplicated.
The current bridge still lacks actual128-unit listener, all placed emitters,
bound original varps and next-selection provenance; no dummy values are supplied.

The remaining music64/327/163/145 and native255 jingle40/54/58/64/65
publications belong to the audio owner. Current `AUDIO_INPUTS`/266 frozen
payloads stay exact; new supplemental IDs/routes must be relayed before they
can be delivered. No missing native representation is replaced by scaling
the frozen128 input. Native policy calibration is no longer a generic blocker;
the exact pending data/publications and real-source wiring are.

Native-control shell evidence is separate from earlier provisional-volume runs:
`.local/evidence/native-audio-title-v2/result.json` records actual default
255/127/127 levels and a50% music position producing native mixer44 (asset
calibration44/128), not a linear0.5 gain. The real canonical source entry under
`.local/evidence/early-native-audio-v2/result.json` records those actual defaults
while explicitly reporting no supplied source scene or next-music selector.
These checks are not completion of the missing authoritative audio inputs.

The authorized UI commits `069b5028`, `f34663c3`, `6a1b11fa` add the actual
self-subscribing UI. `ui-assets.ts` verifies `assets/compiled/ui/provenance.json`
and selects only catalogue-referenced title/sprite/item/portrait/minimap PNGs,
plus the real catalogue/provenance JSON. It does not copy panel/evidence PNGs.
There are1,179 image primitives and6,460 total source/audio/UI assets. The
24,658,127-byte UI catalogue remains an independent bounded asset; it is not
inserted into actor snapshots.

Use a **new** directory/world UUID when adding UI to an earlier pinned bundle:

```sh
pnpm --dir web source:bundle .local/source-ui-df3e2a45-v2 <new-isolated-world-uuid>
CLUBSCAPE_CLIENT_MANIFEST=.local/source-ui-df3e2a45-v2/content/manifest.json \
CLUBSCAPE_CLIENT_ASSET_ROOT=.local/source-ui-df3e2a45-v2 \
CLUBSCAPE_CONTENT_OWNER=game \
pnpm --dir web build
```

The actual game root owns6,461 public asset/manifest routes; the web root serves
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
CLUBSCAPE_CLIENT_MANIFEST=.local/source-ui-df3e2a45-v2/content/manifest.json \
CLUBSCAPE_CLIENT_ASSET_ROOT=.local/source-ui-df3e2a45-v2 \
CLUBSCAPE_CONTENT_OWNER=game \
pnpm --dir web build

CLUBSCAPE_BROWSER_EVIDENCE=.local/evidence/source-ui-df3e2a45-v2 \
CLUBSCAPE_BROWSER_EXECUTABLE=/path/to/verified/native/chrome \
pnpm --dir web test:isolated --game-root .local/source-ui-df3e2a45-v2
```

The source-mode runner uses its bounded owned PostgreSQL database. It first
starts the real world **without** WEB_ROOT, stops it gracefully, then starts the
same pinned world alongside the built browser code. Real source UI controls
perform registration, auth-error/login, empty creation, sequenced appearance,
logout/relogin. A real server restart at the same origin retains the memory
token and acknowledged character state; no replacement artifact, seed or
network mock is used. Credentials stay in browser memory and are never
reported. Evidence includes actual title pixels, startup/run pins and the
onboarding result. With the initial renderer, normal region coverage is not
yet mapped and fails explicitly. The earlier UI-only onboarding proof remains
historical; explicit early-presentation mode below is not a replacement
journey proof. Complete journey, world fidelity, real audio-scene/selection wiring and M1 acceptance
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

### Actual renderer asset delivery and early composition

`render-assets.ts` pins original manifest
`3fd1ec1953183de5537a2e7d239389c8dceed50c2e5d658dcc82c49113468845`
and publishes59 checked buffers after lossless unpack of the10 scene/model
gzip twins. Compressed URLs retain the exact names expected by the actual
adapter; physical `.gz.bin` carriers satisfy the existing server extension
allowlist without changing bytes or using content-encoding tricks. Native
validation-only optional tables/bakes are not fabricated if absent.

The combined source/UI/audio/render bundle has6,520 declared assets and is
separate from older immutable bundles. Use a new directory and world UUID:

```sh
pnpm --dir web renderer
pnpm --dir web source:bundle .local/source-render-df3e2a45 <new-isolated-world-uuid>
CLUBSCAPE_CLIENT_MANIFEST=.local/source-render-df3e2a45/content/manifest.json \
CLUBSCAPE_CLIENT_ASSET_ROOT=.local/source-render-df3e2a45 \
CLUBSCAPE_CONTENT_OWNER=game \
pnpm --dir web build

CLUBSCAPE_EARLY_SCENE=tutorial-starting-house \
CLUBSCAPE_BROWSER_EVIDENCE=.local/evidence/early-render-df3e2a45 \
CLUBSCAPE_BROWSER_EXECUTABLE=/path/to/verified/native/chrome \
pnpm --dir web test:isolated --game-root .local/source-render-df3e2a45
```

The early runner adds only a named `presentation_scene` query—never a token,
account identity or game outcome—and uses real UI/signup/source creation/
appearance/logout/relogin and same-artifact server restart. Its screenshot
contains the actual WebGPU scene with real UI, not a source PNG in a viewport.
Sparky checks find1,336,507 nonblack pixels and11,068 colours in the initial
composed1920×1080 starting-house capture. Genuine GPU-completed frames advance;
the current missing rendered-entity-count diagnostics keep benchmark readiness
false. This is **early presentation**, not a full region/journey/performance
pass. The unchanged native renderer GPU tests compare all five scenes against
CPU/source at zero differences; those fixture checks remain separate.

Without `CLUBSCAPE_EARLY_SCENE`, the current live-region test records an explicit
coverage blocker rather than borrowing a fixture. No alias/mapping from
`region.osrs.12336` to `tutorial-starting-house` is invented.

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

1. The actual renderer entrypoint/build are integrated. Consume its continuation
   for authoritative region streaming, action/equipment packs, exact canonical
   object picks, dynamic minimap and model-only preview; do not promote named
   fixture coverage into those obligations.
2. Supply the real compiled source asset/region/camera bindings and a matching
   server public ContentManifest/asset deployment. Do not substitute the test
   fixtures for those resources.
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
   emitters, committed Cook attribution and the additive source publications
   remain concrete inputs, not permission to invent gains or next songs. The actual UI consumes
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
