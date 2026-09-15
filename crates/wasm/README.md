# Real browser protocol/state bridge

This crate composes `clubscape-client-core`, generated
`clubscape-protocol` Protobuf, and the existing typed `GameIntent`. It is
not another game engine, renderer, seeded world, or M1 acceptance record.
The exact `wasm-bindgen` crate **and CLI** version is **0.2.128**.

## Browser ABI

`web/app/bridge.ts` is bundled as `/client/bridge.js`. Its
`createProtocolClient()` initializes real WASM and returns `BrowserClient`.
The library also exports `BrowserApp`, `RpcTransport`, and `checkCapability`.
The raw matching bindgen JS/WASM pair is available under `/client/wasm/`.

| Rust/WASM method | Meaning |
| --- | --- |
| `prepare(uuid, operation, json)` | Binary account/session request. Operations: `hello`, `register`, `login`, `account`, `logout`, `create_character`, `join`, `poll`, `leave`. |
| `submit(uuid, intentJson)` | Parse the shared typed intent and let client-core assign the joined session, sequence, and observed character revision. |
| `request_id(requestBytes)` | Extract correlation identity with generated Protobuf, without a JS wire implementation. |
| `receive_for(uuid, responseBytes)` | Require this exact HTTP request's response, then validate/reconcile through client-core. An old duplicate cannot acknowledge a different pending request. |
| `receive(responseBytes)` | Packet-oriented reconciliation/deduplication API; the HTTP shell uses `receive_for` instead. |
| `transport_lost()` | Preserve uncertain input, never passwords. |
| `retry_uncertain_input()` | After a real rejoin, return either the same operation/sequence with its renewed lease, or no request when commitment is proven. |
| `set_catalog(json)` | Install the content-revision-matched, display-only catalog. |
| `state()` | JSON public state; no bearer token, world lease, other-player containers, or closed bank contents. |
| `authorization()` | Transport-only memory accessor. Never serialize, log, persist, put in a URL, or expose through `AppServices`. |
| `free()` | Dispose this protocol state. |

Registration/login input is `{ "loginName": "...", "password": "..." }`.
The shell does not save or automatically retry those fields. Creation takes
exactly `{}`; source appearance/experience confirmation are separate
sequenced game intents. Registration is not login.

`state()`/`receive_for()` return the `BridgeState` documented in
`web/app/client.ts`. The browser parses and recursively freezes it before
passing `AppState`/`WorldView` to components. Every U64 revision, sequence,
tick, XP amount, history floor, and dynamic-object expiry crosses JS as a
decimal string. Quantities remain bounded source U32 values. No local XP,
items, quest stages, prices, quotes, action permissions, or poses are invented.

Errors are thrown as structured JSON strings with `kind`, `message`,
`errorId`, `code`, `retryAfterSeconds`, and `recoverable`. Server correlation
IDs survive unchanged; client failures receive separate local IDs in TS.
Malformed acknowledgements do not consume or discard an uncertain operation.

## Current generated-API gaps

Base `f64f52b` has no `ProtoGame` or shared `ContentManifest` implementation.
This bridge uses the actual generated messages and `ClientCore`, rather than
an imagined API. A public, display-only manifest projection is provided by
the owned build tooling.

* `ProduceSelected`, `OpenGrave`, and `OpenDeathOffice` exist in the shared
  intent type but have no corresponding generated wire variants. They fail
  explicitly as `unsupported`, without downgrade to another action.
* The current wire lacks guarded bank/shop/recovery panels and inventory
  action menus. Those panels remain `null`; inventory menus are explicitly
  unavailable. The backend owner's new typed fields must be mapped here after
  integration, not reconstructed from prices/guards on the client.
* Entity action names are enabled only when the server marks permissions
  evaluated. Ground `canTake` requires its evaluated permission.
* Dynamic objects, history gaps, appearance confirmation, experience, combat
  style, active death and unavailable views are preserved as **additive public
  properties** in the JSON view. Component adapters must consume the needed
  fields; `web/shared/contracts.ts` is unchanged.
* Animation strings and source audio asset IDs are passed through. An absent
  source animation/cycle/portrait remains absent; there is no pose clock or
  inferred level-up/quest-completion reward.

## Validated source projection

```sh
cargo run -p clubscape-wasm --bin project-content -- <validated-world.csc>
```

This native-only helper reloads the real compiled artifact in `Runtime` mode,
rejects unresolved bindings/TestFixture content, and projects names, source
IDs, asset IDs, equipment slots, quest completion-stage identifiers and
region-to-scene asset relationships. It never exports flags, RNG, initial
inventory/stats, guards, rewards, shop prices or private runtime state.
Icons require explicit presentation bindings; model assets are not relabeled
as inventory sprites. See `tools/web-build/README.md`.

```sh
cargo test -p clubscape-wasm -p clubscape-client-core
cargo clippy -p clubscape-wasm --all-targets -- -D warnings
cargo check -p clubscape-wasm --lib --target wasm32-unknown-unknown
cd web && pnpm wasm
```

Rust fixtures test wire mapping, malformed/stale/duplicate replies, uncertain
outcomes, auth revocation, precision and privacy. They do not establish a real
character/world journey or presentation fidelity.
