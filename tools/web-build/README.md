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
pnpm --dir web typecheck
pnpm --dir web test
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
Only the root Cargo lock's new workspace-member entry is needed—dependency
versions already existed in the base lock. Root `Cargo.lock` is outside this
worker's ownership; the integrator must retain Cargo's new `clubscape-wasm`
member entry when merging. The build command performs normal Cargo resolution,
not a global tool installation or guessed alternate bindgen version.

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
  --artifact .local/game/world.csc \
  --bindings assets/compiled/browser/presentation.json \
  --asset-root .local/game \
  --output .local/game/content/manifest.json
```

Those example input paths are **not supplied product assets**. The renderer/
UI/audio preparation owners must supply real compiled presentation bindings.
The bindings JSON supplies `sourcePackSha256`, explicit `assets`,
`bootstrap`, `rendererManifest`, `regions`, and optional `icons` mapping known
item/skill IDs to declared original raster assets. It cannot replace catalog
names, completion stages, source IDs, initial state or authority rules.
`project-content` revalidates the real artifact in `Runtime` mode first;
unresolved/TestFixture artifacts fail. Region scene-asset IDs must match that
compiled content. Files reside under `asset-root` at their public URL paths
without the leading slash. No entire cache is copied.

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

1. Merge the actual `web/{renderer,ui,audio}/index.ts` adapters and run the
   renderer owner's actual WASM/assets build step. Add that agreed build ABI
   to orchestration once its real output paths are known.
2. Supply the real compiled source asset/region/camera bindings and a matching
   server public ContentManifest/asset deployment. Do not substitute the test
   fixtures for those resources.
3. Align the backend's new generated guarded bank/shop/recovery/menu/lifecycle
   fields in `crates/wasm/src/view.rs`, and add the three missing exact intent
   variants when the protocol owner exposes them. Preserve source errors,
   public-state privacy and client-core lease/sequence reconciliation.
4. Align renderer `observe()`/applied settings/GPU completion ABI and actual
   audio decode observations described in `web/app/README.md`. UI context menus
   may expose `worldContext(pick,x,y)`. Source entity/pose/morph/dynamic-object
   state must come from the real view, not fixtures or fabricated animations.
5. Exercise actual source UI registration and the real character/world journey
   against the source server, including gameplay reconnect/restart, asset/
   device failures and nonblank world/UI/audio. Run the independent frozen
   source comparisons and genuine completed-frame harness, then the owner's
   M-series Mac Chrome **and** Edge acceptance runs. The infrastructure helper,
   its black diagnostic canvas, and the missing-adapter build do not pass any
   of those gates.
