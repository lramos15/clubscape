# Protocol and transport

The canonical schema is `crates/protocol/proto/account.proto`. Generate Rust
bindings with Prost and preserve numbered fields. Native and future WASM clients
share the generated protocol crate. The initial supported version is exactly
1; incompatible versions fail explicitly without performing an operation.

Send `ClientMessage` as `application/x-protobuf` to `POST /v1/rpc`. Responses
use the same media type and a `ServerMessage` with exactly one result. Each
request supplies a valid UUID correlation ID; errors include a separate server
error UUID. Enforce a 16 KiB request limit, including malformed bodies. Do not
log the message body, authorization header or plaintext credentials.

Commands are `hello`, `register`, `login`, `current_account` and `logout`.
Protected commands use an `Authorization: Bearer <token>` header. Raw session
tokens are returned only on successful login; registration does not log in
implicitly. `hello` advertises only implemented account/session capabilities
and explicitly reports that gameplay is not yet available.

Use meaningful HTTP statuses alongside the structured protocol error:
invalid input 400, authentication failure 401, conflict 409, unsupported
version 426, resource exhaustion 429, internal failure 500, unavailable
dependency 503. Unsupported media types use 415 and oversized requests 413.
Never encode errors as a successful empty result.

No CORS wildcard or browser local-storage token persistence is part of the
contract. Future browser sessions keep credentials out of URLs and use the
same-origin service. The development server binds only to loopback; reject
non-loopback bind configuration. Plaintext local development is not production
transport-security evidence. Deployment requires a separately configured
standard TLS boundary before any public access.

Schema changes must reserve removed tags and keep old messages decodable.
Add new capabilities explicitly; do not infer support from version alone.
World-update deltas and authoritative gameplay sequencing remain future work
inside M1, not implemented features of this account protocol.
