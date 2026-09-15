# Approved browser implementation boundary

The exact pack approved by the owner is
`milestones/approvals/m1-reference-pack-v1.3.0.json`. Its SHA-256 and
`spec/m1-benchmark-contract.json` are embedded in `web/shared/contracts.ts`.
No output from the new client becomes a source reference. Final visual/audio
and Mac Chrome/Edge acceptance remain unexecuted.

The pack hash-locks `spec/art-style.md` and `spec/interface-parity.md` as
pre-approval context documents. They deliberately retain their original bytes,
including historical status text; they do not revoke the later external owner
approval. Current implementation authority and status live in this document,
`spec/milestone-01.md` and the milestone/approval records. Do not rewrite those
frozen context files or relax the pack verifier to update a status label.

## Independent surfaces

- `crates/renderer`, `tools/render-assets`, `assets/compiled/render`: Rust/WASM
  WebGPU source scene/model/texture/animation rendering, picking, modular
  penguin/equipment appearance, original-data preparation and faithful
  reference comparison. Export a JS adapter implementing `CreateRenderer`.
  The renderer owns source geometry and genuine completed-render counters.
- `web/ui`: TypeScript source-sprite/font Canvas2D overlay and real accessible
  input/control handling for approved startup and in-game interfaces. Export
  `createUi` implementing `CreateUi`. It consumes immutable authoritative
  `AppState` and sends requests through `AppServices`; no gameplay outcomes.
- `web/audio`: actual browser WebAudio playback over original lossless assets,
  source queues/triggers/region transitions and the two approved adaptations.
  Export `createAudio` implementing `CreateAudio`.
- `web/app`, `crates/wasm`, `tools/web-build`: browser/WASM protocol bridge,
  same-origin transport, asset streaming, capability feedback, request/reconnect
  orchestration, build entrypoint and renderer/UI/audio composition.

The shared browser contract is type-only and source-neutral; an absent optional
view is `null`, never a fake empty success. IDs/quantities/vitals/quest state come
from the authoritative server and validated content. U64 revisions/ticks/XP
cross the JS boundary as decimal strings to avoid precision loss.

The composition owner may adapt exact generated Rust/JS ABI names behind its
`RendererHandle` adapter, but cannot replace the actual renderer with screenshots
or duplicate authoritative rules. The world and overlay use separate canvases:
the 3D world remains full viewport, while the source UI overlays its exact
stock frame regions. Rendering and UI code must never paint whole reference
captures as the product; individual original source sprites/glyphs/icons and
technical model conversions are legitimate reuse.

Render and audio inputs stream by current content/region need. Credentials and
opaque account tokens remain memory-only, are never logged or put in URLs,
and are not serialized into public UI state. Browser transport is same-origin;
standard TLS/SSH forwarding is used when accessing remote development hosts.
Do not add a CORS wildcard, public plaintext bind or sandbox-disable shortcut.

The UI owns selection/hover/drag/menu state, never item or XP ownership.
Failed authoritative actions show their actual reason/error ID and reconcile
the resulting state; they do not optimistically grant items or advance stages.
Genuine source-locked tutorial controls stay locked. Out-of-scope controls
retain their approved placement and explicit feedback, not fake success.

Use original source font masks/advances/ascent, sprite offsets/alpha, native
HUD anchors, material transforms, HSL palette/lighting and animation timing
from the approved pack. Generic CSS game skins, modern panels, PBR substitutes,
invented snowy terrain, and resampled source artwork are not equivalent.

## Measurement and errors

The composition layer exposes `window.__clubscapeBenchmarkV1` using the exact
contract in `tools/browser-harness/src/protocol.ts`. Only actual GPU-completed
renderer frames increment the counter; RAF calls, static screenshots and a
configured limiter do not. `read()` is observation-only. Include actual loaded
scene/route/assets/entity counts, device epoch, source/build/settings hashes
and known GPU timestamp availability.

The primary viewport is 1920x1080/DPR1 at UI scale1; supported resizing is
1024x768 through2560x1440. Do not lower draw distance/resolution to pass.
Visible world, overlays and original-source audio must be exercised in real
browser/server scenarios and independently compared with the frozen inputs.

Capability, device-loss, asset, audio, transport and authorization failures must
surface explicitly. There is no WebGL/software renderer fallback in M1.
Temporary synthetic fixtures are labeled tool/test-only and cannot pass
gameplay, source fidelity or performance acceptance.

## Authoritative M1 interface completion

`web/ui/contract-gaps.json` identifies missing implementation contracts, not
permission to drop required controls. Task `M1-GAMEPLAY-UI-CONTRACTS` owns the
source engine/data/protocol closure. Publish its exact shared types before
dependent UI/shell wiring; keep existing renderer/audio interfaces unchanged.

Use an additive, versioned `WorldView.ui` projection for active interfaces,
production selections, current reward presentation, selected combat style,
source visibility/locks/highlights, ability availability, effective equipment
statistics/weight, bank management and death/recovery information. Absence
means an older unsupported capability, not a fabricated empty successful
state. The M1 client must negotiate and receive the complete current projection.
Nullable members inside it mean genuinely closed/not-applicable source state.
U64 amounts, coffer balances and clocks remain decimal strings in JavaScript.

Queries must reuse source guards and transaction planners without speculative
intent execution, RNG, duplicate pricing rules or invented zero statistics.
Production menus retain their real target and legal recipes; recipe selection
distinguishes Single and Make-X-of-one. Reward/continuation state comes from the
actual committed operation and source presentation metadata, not unrelated
inventory/XP deltas. Dismissal never grants a reward again.

Expose real inventory actions for the available M1 items, including source
drinking, burial and applicable reading/emptying. Reuse existing mechanics and
source bindings; do not map an action to fabricated self-use or a dummy spawn.
Preserve the selected item's identity, slot, consumption/replacement behavior
and its own delays rather than treating every consumable as food.

Complete the required bank tab/order/placeholder/insert/deposit-equipment
controls against the same persistent bank. Placeholders are non-spendable
metadata, never zero-quantity or duplicate owned items. Source controls for
containers/items absent from M1 retain their genuine disabled/unavailable
state; this is not authorization to add new acquisition families.
Death preview, coffer display and relevant recovery/discard controls must use
actual source valuation/ownership and confirmation semantics.

Public chat must use an authenticated, bounded, source-conformant route with
correct sender/audience, input validation and replay behavior; a typed line
must not simply disappear. This does not authorize clans, private social
systems, moderation/admin privileges or the later CP social milestone.
Do not interrupt gameplay or change source action cadence just to carry chat.

Appearance exposes the currently declared source choices and approved penguin
base; do not create new human kits, colours or penguin cosmetics. Actual preview
and dynamic minimap surfaces remain renderer-owned. Audio controls expose the
actual audio runtime's values/policy through the shell, not guessed server
volume defaults. Native local filters/layouts remain UI-owned. Preserve the
full 71-state journey, source rules, all earlier fixes and approval boundaries.
