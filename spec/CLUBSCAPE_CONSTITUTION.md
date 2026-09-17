# ClubScape constitution

The owner's [master contract](../prompt.md) is authoritative. Its kickoff
SHA-256 is `964ba0936df1673036ba5a3c86fd36b79a53a091345f174f16e7bce1b6224849`.
Derived specifications must not reduce its full-game target or approval gates.

- OSRS is the sole RuneScape gameplay reference. The reference baseline must be
  identifiable and frozen, with unknown inventory coverage reported honestly.
- Preserve baseline mechanics, world layouts, interfaces and source assets
  except for explicitly approved adaptations. The first slice has the narrower
  presentation permissions in Section 30.
- Rust owns authoritative gameplay; clients request actions, never outcomes.
  PostgreSQL owns durable account and player state.
- One shared simulation serves graphical and headless clients. Toolchain,
  API-only, mock, and renderer-only checks cannot establish gameplay acceptance.
- Only the current approved milestone may execute. Full/release scope,
  reference-pack approval, and owner presentation acceptance are distinct.
- Keep durable source records, task dependencies, evidence and revision-bound
  approvals. Never replace a failed gate with a weaker target.
- At most 25 active AI agents project-wide, including the Director, with no
  owner-imposed AI spending cap and no bypass of provider/runtime limits.

The active execution contract is [M1](milestone-01.md).
