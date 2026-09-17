# Protocol client state

`clubscape-client-core` is a native/WASM-compatible protocol state machine, not
a renderer, browser UI or proof of the M1 player journey. Transport, rendering,
source content and the authoritative world service are separate layers.

`prepare(request_id, Command)` builds a validated account/session request.
`submit_action(request_id, Action)` assigns the joined world session and the
next server-issued sequence. Only one game mutation may be outstanding; the
input/UI layer may queue/coalesce unsent requests, but must not invent a second
sequence. Account credentials are not retained for retries.

`handle_response` / `decode_response` require matching protocol/request IDs
and result types. Authentication tokens remain in memory and are excluded from
`Debug`; the transport-only accessor must never be logged or persisted by the
browser shell. Gameplay requires both its capability and availability flag.

Snapshots contain the complete local player and visible ground-item list.
`full_snapshot=false` means entity updates merge by ID, with removals supplied
explicitly. Source content/assets are streamed independently. Revisions never
roll back the displayed state. Server events require stable `event_id` values
and are deduplicated with bounded history; a malformed batch cannot poison
that history or replace the last valid snapshot.

`transport_lost` retains only an uncertain game intent, not passwords.
After rejoining, the server's next sequence either proves that operation
committed or allows `retry_uncertain_input` with the original operation ID and
intent under the renewed session. Unknown write outcomes are not converted into
success or a new operation. The server's journal must hash the intent rather
than ephemeral auth/session IDs. Invalid acknowledgements do not advance the
client sequence.

The current tests execute protocol reconciliation with synthetic messages;
they are not real browser/server gameplay evidence. Use the later live journey
suite for that gate.

```sh
cargo test -p clubscape-client-core
cargo clippy -p clubscape-client-core --all-targets -- -D warnings
cargo check -p clubscape-client-core --target wasm32-unknown-unknown
```
