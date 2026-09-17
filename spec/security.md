# Initial security contract

The server is authoritative. The complete client-hostility requirements remain
in `prompt.md` Sections 7, 24 and 37; authentication infrastructure alone cannot
satisfy the full milestone security gate.

For the independent account increment:

- Hash passwords with Argon2id, unique random salts, at least 19 MiB memory,
  two iterations and one lane. Run hashing off the async executor and admit at
  most four concurrent password operations. Unknown-account login verifies
  against a dummy hash to avoid a cheap account-existence timing distinction.
- Generate bearer tokens from the operating-system CSPRNG. Persist only their
  digests; validate token encoding and exact decoded size before database work.
- Bound requests, password lengths, password concurrency and login/register
  attempts. Use the actual socket peer, not spoofable forwarding headers, for
  initial development rate limits. Bound limiter memory and return a retry
  delay when rejecting admission.
- Use parametrized SQL, a unique normalized account identifier, foreign keys,
  schema constraints, transaction boundaries and database-time session expiry.
- Return a generic credential error for unknown users and incorrect passwords.
  Internal failures require sanitized structured logs and an error ID, never
  silent defaults or exception text containing database URLs.
- Fail startup when configuration, migration or database access is invalid.
  Refuse non-loopback plaintext serving. Do not modify host security settings
  or expose PostgreSQL beyond loopback.
- Generate development/test credentials locally into ignored, permission-0600
  files. Keep CI credentials ephemeral; do not commit secrets.

Account recovery, production TLS/proxy trust configuration, browser token
handling, game-command replay/order protection and economy/quest authorization
require further implementation and review. Do not call them complete.
