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
| `prepare(uuid, operation, json)` | Binary account/session request. Operations: `hello`, `register`, `login`, `account`, `logout`, `create_character`, `join`, `poll`, `quote`, `leave`. |
| `submit(uuid, intentJson)` | Parse the shared typed intent and let client-core assign the joined session, sequence, and observed character revision. |
| `request_id(requestBytes)` | Extract correlation identity with generated Protobuf, without a JS wire implementation. |
| `receive_for(uuid, responseBytes)` | Require this exact HTTP request's response, then validate/reconcile through client-core. An old duplicate cannot acknowledge a different pending request. |
| `receive(responseBytes)` | Packet-oriented reconciliation/deduplication API; the HTTP shell uses `receive_for` instead. |
| `transport_lost()` | Preserve uncertain input, never passwords. |
| `retry_uncertain_input()` | After a real rejoin, return either the same operation/sequence with its renewed lease, or no request when commitment is proven. |
| `retry_lifecycle()` | Retry a transport-uncertain/unavailable leave or logout with its original UUID and lease, without rejoining. |
| `submit_selected(uuid, intentJson, itemId)` | Preserve an explicitly selected shop ItemId at the WASM boundary. Purchases remain unsent until the exact identity-bound wire ABI is relayed. |
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
decimal string, including recovery full-entry/full-selection fees. Quantities remain bounded source U32 values. No local XP,
items, quest stages, prices, quotes, action permissions, or poses are invented.

Errors are thrown as structured JSON strings with `kind`, `message`,
`errorId`, `code`, `retryAfterSeconds`, and `recoverable`. Server correlation
IDs survive unchanged; client failures receive separate local IDs in TS.
Malformed acknowledgements do not consume or discard an uncertain operation.

## Integrated lifecycle/context APIs and remaining ABI work

The explicitly authorized parent commits `1ea2e653` and `e8dfa2df` are
cherry-picked here. The former engine-view/presence interlocks are obsolete.
The bridge consumes their real generated protocol and keeps source engine
errors intact. It does not implement another guard/price/readiness engine.

* `ProduceSelected` preserves Single versus Make-X-of-one, and `OpenGrave`/
  `OpenDeathOffice` use their exact typed variants. No surrogate actions.
* Authorized bank slots, deposit/withdraw permissions, shop ItemIds/stock/unit
  prices, guarded dialogue, presence, interaction denials, pickup permissions,
  temporary-object identity and source assets/dimensions are projected directly.
  `bank.allowNotes` denotes wire request support, not per-item eligibility:
  the actual bank/quote planners still decide the transfer.
* `recoveryContext.views[]` preserves **all** authorized panels, layouts,
  storage, active ticks and decimal-string `fullEntryFee`. The legacy singular
  `recovery` is populated only for exactly one panel. Its numeric `cost` stays
  null; UI adapters must use `fullEntryFee`, never coerce/round U64 fees.
* `quote` builds a typed `PollWorld.quote`; it never allocates an input sequence.
  Correlated responses return an immutable `QuoteView` plus source revision/tick.
  Partial reasons/totals are server values. Wrong/stale item or recovery
  selections return `quoteError` while still reconciling the valid world/events.
  Quotes are response-only, not sticky prices returned by `state()`.
* Shop rows retain ItemId both as `row.itemId` and `row.item.id`; purchase
  selections carry the UI-chosen ID into `submit_selected`, never infer it
  from an index. The separate expected-item purchase-wire safety fix is still
  pending exact relay. Both raw/index-only and selected purchases are refused
  before sequence allocation until that ABI is integrated.
* Inventory **action-label** bindings are still absent, not an engine/context
  API blocker. Source-labelled UI adapters must supply these separately.
* Additive public properties are typed in `web/app/public-state.ts`;
  `web/shared/contracts.ts` remains unchanged.
* Animation strings and source audio asset IDs are passed through. An absent
  source animation/cycle/portrait remains absent; there is no pose clock or
  inferred level-up/quest-completion reward.

## Validated source projection

```sh
cargo run -p clubscape-wasm --bin project-content -- <validated-world.csc>
```

This native-only helper reloads the real compiled artifact in `Runtime` mode,
rejects invalid/TestFixture content, and projects names, source
IDs, asset IDs, equipment slots, quest completion-stage identifiers and
region-to-scene asset relationships. It never exports flags, RNG, initial
inventory/stats, guards, rewards, shop prices or private runtime state.
Icons require explicit presentation bindings; model assets are not relabeled
as inventory sprites. It reports every remaining source binding without
blanket-rejecting the six proved inactive alternatives and without classifying
them itself. Actual readiness belongs to the server's executable
`readiness.rs` profile checks; `runtimeReadinessEstablished` remains false.
The pending solid-target/manual-drop clock engine probes are not bypassed.
The owned build tool also accepts canonical `.csc.gz` through bounded
decompression and stdin. See `tools/web-build/README.md`.

```sh
cargo test -p clubscape-wasm -p clubscape-client-core
cargo clippy -p clubscape-wasm --all-targets -- -D warnings
cargo check -p clubscape-wasm --lib --target wasm32-unknown-unknown
cd web && pnpm wasm
```

Rust fixtures test wire mapping, malformed/stale/duplicate replies, uncertain
outcomes, auth revocation, precision and privacy. They do not establish a real
character/world journey or presentation fidelity.
