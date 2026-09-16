# Native recovery context contract

This additive contract is reserved by `M1-NATIVE-RECOVERY-CONTRACT`. It extends
`game.ui.recovery.v1` through optional `ui.recovery.management.context`, whose
own version is1. Absence means unsupported; an open empty Office has a present
context with empty slots. Content4, state/runtime/UI1 and the current source
artifact are unchanged. The protocol extension uses `UiRecoveryManagement`
field5 and `GameplayUiRequest` oneof field26.

## Consumer boundary

`RecoveryContextView.identity` comes from the actual contextual interface
session. A grave names its real death; an Office names its real interface and
opaque live instance, not an arbitrary first death. `slots` is the complete
owned visible container in authoritative contiguous slot order. Each row keeps
its death, recovery entry, original item, current physical storage and existing
per-entry permissions/quotes. Render the selected outline on that slot.
Do not add per-death tabs or use singular `WorldView.recovery` as the context.

`counts.entries` counts visible occupied slots and is the native title
numerator: the retained scroll fixture says80/120, while the short16-slot
fixture says16/120. `nativeItemTypes` counts distinct original item IDs across
those slots for type grouping, **not** the title numerator; null explicitly
means source identity is incomplete. `capacity` and `capacityUnit` come from the canonical death policy:
grave entries versus Office `recovery_slot_key` item types/instances.
`stored` counts actual storage, while `offered` includes still-grave rows
visible from the Office. Offered count may exceed capacity; a read-only view
does not move or delete those rows. These are not per-entry executable
inventory/bank capacities. The canonical policy currently declares120 for both
storages; the controlled native capture did not establish that policy.

For a selected source item in native Office669, `selectedTypeCaption` supplies the original
`INV_TOTAL` visible-type quantity and multiplication by the selected entry's
current one-unit fee input. This is display data, not an executable combined
quote. Original669 script3492 uses selected slot7 for the outline, but five
seven-arrow rows display35 arrows and, at42 each,1,470. The selected entry still
contains7 and its whole-entry quote is294. Different captured valuations or
paid credits can make the source display multiplication differ from the
combined transaction fee. Neither the UI nor this contract interprets
`INV_TOTAL` as original-server selected-item All dispatch across death records.
Legacy `recovery_take` retains its exact single-record selected-entry semantics.
Other recovery interfaces retain an explicit unavailable selected-type caption;
the Office script is not silently applied to grave602 or another source group.

`takeAll.selection` is an exact observation precondition. Echo it unchanged as
one `{kind:"recovery_take_all", selection}` request. It includes the actual
context and ordered nonempty owned records, with every entry's ID, remaining
quantity and physical storage. No desired items, fees or destination are
accepted. Ownership/context validation and exact stale comparison precede one
shared `plan_recovery` over all batches, installed atomically. The planner
recomputes funds, fee credits/caps, overflow, auto-equip and capacity using the
current trusted state. A valid partial transfer has its actual per-entry
quantities/fees and `partial:true`; a wholly refused request changes nothing.
`takeAll.plan` is that combined executable preview, not a sum of independent
entry capacities or whole-stack quotes.

The new inventory request has no `expected_bank_revision`, matching legacy
inventory recovery. Existing Bank-All and bank operations continue echoing the
exact `management.bankRevision` / `ui.bank.revision`, checked after durable
duplicate lookup. A committed operation retry returns its original receipt
before any changed selection is revalidated. Existing request and response
byte bounds remain in force.

## Forwarding and ownership

Rust definitions are in `crates/game-types/src/recovery_context.rs`; exact
TypeScript definitions are in `web/shared/contracts.ts`. The implementation publishes the public converter
`clubscape_protocol::recovery_context_to_wire`.

The existing production response encoder is outside this worker's permitted
paths. Director must add only this initializer field in
`crates/server/src/game_service/ui_wire.rs::recovery_management`:

```rust
context: value.context.map(clubscape_protocol::recovery_context_to_wire),
```

No server-source edit, new pricing helper, client request loop, GPU capture,
source regeneration or saved-world restart is authorized by this contract.
Any UI exhaustive-switch adjustment is Director-owned; do not hide it with
typecheck exclusions or casts. The implementation is in `93aefa6`, following
API-first `155ac6f`. `verification.json` records owned native/controller
coverage and the exact outstanding server/UI integration errors. The durable
multi-record regression is added under `crates/server/tests/game_storage/`;
it reuses the existing isolated PostgreSQL guard/harness and cannot run until
the Director-owned server initializer is wired.

## Source evidence and retained qualifications

The existing `../control-bindings.json` retains original669 scripts3490
(1/5/X/All and whole-context Take-All) and3492 (selected slot plus
`INV_TOTAL` caption). The unchanged `tools/ui-assets/UiModeCapture.java`
scroll fixture explicitly supplies80 rows,120 allocated slots, selected slot7,
varp261=12345 and varp263=42. Its80/120 title and35/1,470 caption are retained
native component facts; the16 distinct original types are a separate grouping
of the explicit inputs. These are not a player journey or new source-server
capture.

Ordering across death records retains the existing engine's stable map/vector
order; it is not a newly claimed original-server sorting algorithm. The
canonical Office overflow action above120 remains unresolved and must surface
its existing source refusal, not an invented deletion order. Original-server
selected-item All across multiple records is not established by the native
caption. Normal-grave Bank-All, dough and the seven Empty motion/absence facts
remain independently unverified as recorded in
`../remaining-source-questions.json`.

The unchanged artifact pin is raw
`5b3ba5f108ed3fec8f8b5f7f49b429c059e21b6f192ec99a0569616e08330059`,
gzip `8da1e85a1e2ebb0355e29e7a15a0ca7cdab2f2751bc1341dd5efd5322b925ca6`.
This work does not accept UI, presentation, performance or M1.
