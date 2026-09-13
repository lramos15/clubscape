# Account client API

The generated [protocol](networking.md) is canonical for message shapes.
`hello` permits clients to check protocol and capability support without
authentication. It must distinguish account-service readiness from unavailable
gameplay. A working hello response is not a world connection.

The first headless check uses this sequence against the real server:

1. Negotiate version 1 and assert that gameplay is explicitly unavailable.
2. Register a unique synthetic account; reject duplicate normalized names.
3. Log in and keep the returned opaque bearer token in memory.
4. Read the persisted identity; assert character initialization is not
   fabricated.
5. Log out; verify the revoked token cannot read the account.
6. Log in again and verify the same identity is recovered.

Separate integration evidence must restart the service, check persisted
sessions/accounts, expire a session using the controlled test database, test
concurrent duplicate registration and session-cap enforcement, and exercise
malformed/protocol/authentication errors.

The future real browser signup must use this service, not substitute a seed or
development login. Its approved OSRS-composition UI, character creation,
tutorial and world transition are pending Section 30 requirements.
