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
| `submit_selected(uuid, intentJson, itemId)` | Compatibility adapter that writes the supplied canonical ItemId into exact `expected_item`; conflicting identity fields reject without replacement. New callers can use `submit` with `expected_item` directly. |
| `request_is_shop_buy(requestBytes)` | Inspect a retained request with generated Protobuf so rejected uncertain purchases refresh views without a JS wire parser. |
| `set_catalog(json)` | Install the content-revision-matched, display-only catalog. |
| `state()` | JSON public state; no bearer token, world lease, other-player containers, or closed bank contents. |
| `authorization()` | Transport-only memory accessor. Never serialize, log, persist, put in a URL, or expose through `AppServices`. |
| `free()` | Dispose this protocol state. |

Registration/login input is `{ "loginName": "...", "password": "..." }`.
The shell does not save or automatically retry those fields. Creation takes
exactly `{}`; source appearance/experience confirmation are separate
sequenced game intents. Registration is not login.

The TypeScript `AppServices.createCharacter(appearance)` method now composes
that empty creation RPC, real join and separate `ConfirmAppearance` for the
actual source UI. Its argument is never forwarded as creation options.
Already initialized accounts resume rather than creating/reconfirming again.

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
  selections carry the UI-displayed **canonical `row.item.id`** as
  `expected_item`. The authorized `3310032`/`af75e17` repairs are integrated:
  Protobuf `ShopBuy.expected_item` is optional string tag4, while WorldInput
  shop-buy tag25 and quote shop-buy tag3 are unchanged. New browser requests
  require identity; no numeric source ID or client price substitutes for it.
  The compatibility adapter never silently overwrites a conflicting ID.
  Read-only quote selections likewise carry `expected_item`; the older local
  quote spelling `itemId` remains an alias but always emits the new wire field.
* Client-core retains original operation UUID, sequence and expected ItemId
  across uncertain retries. A stale rejection does not advance the sequence.
  Legacy None JSON/wire/canonical-v1 hashes—including historical committed
  extra-row operations—remain governed by the unchanged shared compatibility
  path; retries are never rewritten from a current row. New index-only browser
  submissions are refused, although the server still supports legacy immutable
  fixed-source rows. Shared/engine tests cover extra capacity reclamation,
  tombstones, source restock phase, two actors and restart.
* Original inventory **action-label** bindings can be carried in the catalog
  from the verified source collection's `interfaceOptions`. They are labels,
  not an item-ownership or permission verdict. Missing labels stay explicit.
* Additive public properties are typed in `web/app/public-state.ts`;
  `web/shared/contracts.ts` remains unchanged.
* Animation strings and source audio asset IDs are passed through. An absent
  source animation/cycle/portrait remains absent; there is no pose clock or
  inferred level-up/quest-completion reward.
  Audio payloads mark only the client-core-validated committed stream.
  The real audio adapter rejects missing group/delay/correlation/widget data
  explicitly; this wire does not yet carry those fields. No 600ms tick is
  relabeled as an audio source cycle and no incompatible generic XP/text/damage
  keys are sent to the factory.
  An explicit `level_up.skill` is preserved as `payload.skillId`; the bridge
  never invents before/after levels, quest attribution, a completion ID or
  group33. Native audio now validates actual committed Cook deltas and owns
  deferral/no-level-gain behavior. Its source-scene/varp/owner inputs are a
  separate pending projection, not guessed from this server's tile/quest data.

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
The authorized `faa8002` fix now supplies correct source-solid contact,
owner-online ground clocks and private authoritative playtime. Its source
probes pass; the former two-probe interlock is obsolete. No playtime/ground-clock
field or setter is exposed through this bridge.
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

## Published `game.ui.v1` boundary (contract-only)

The authorized `d1532d6f` contract is integrated. `GameplayUiRequest` and
`GameplayUiView` are reused directly from `clubscape-game-types`, and the
browser uses the exact `GameplayUiIntent`/`GameplayUiView` shared types.
No engine, Protobuf message, capability advertisement or canonical data
implementation was included in that publication.

`BridgeState.capabilities` records the actual validated ServerHello values.
`gameplayUiWireSupported` is explicitly **false** until the generated wire
adapter is implemented. Valid new UI requests are parsed through the exact
shared Rust request enum, then rejected as `unsupported_capability` or
`unsupported_protocol` **before sequence allocation or transport**. They are
never downgraded to old production/bank/item actions that discard menu, entry,
presentation, confirmation or expected item/instance identities. A future
server advertising `game.ui.v1` cannot make this old decoder create/join a
world while silently dropping its required UI view.

`gameplay_ui::project` is a pure Rust-DTO to shared-TypeScript-DTO projection,
ready for the eventual generated snapshot adapter. It is **not exposed as a
WASM state setter or accepted client outcome**. It preserves every stable
entry/menu/presentation/confirmation/item/instance ID, declared appearance/
ability/permission state, public chat text and source continuation. Balances,
fees, XP, revisions and weight remain exact decimal strings. A bank placeholder
has `value:null`, never a spendable zero-quantity stack. No guards, prices,
grants, rewards, volumes, model previews or minimap data are computed here.

The runtime does not install that projection from fixtures or source defaults.
Legacy `WorldView.ui` stays **absent**, not `{version:1,...empty values}`.
The pending `production.target` nullable correction has not been relayed:
this implementation follows the currently published target type and never
adds a dummy spawn for inventory-only production. Integrate that exact shared
type change and actual engine/protocol projection before enabling wire support.
