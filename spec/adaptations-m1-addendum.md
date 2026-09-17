# Approved M1 adaptation addendum

The current M1 adaptation register consists of the unchanged
[`adaptations.md`](adaptations.md) and this additive record. The original
register is hash-bound by the approved reference pack's audio contract; do not
edit its bytes or regenerate that pack to record a later bounded approval.

## adaptation.m1.source_based_camera_initialization

**Owner approved.** Record:
[`milestones/approvals/m1-camera-initialization-v1.json`](../milestones/approvals/m1-camera-initialization-v1.json).

Use initial yaw/pitch from the already-approved reference views, actual player
render/focus coordinates and original terrain, measured original constructor
camera preferences, and original preset626 zoom bounds. Record the selected
reference case and exact initial angles/units before production binding.
These are explicitly **ClubScape initialization defaults**, not newly observed
OSRS account defaults. The original source evidence remains unchanged and its
authenticated-session gaps remain honestly labeled.

This approval does not replace the stateful Rust/WASM controller. Preserve the
measured20ms input motor, separate frame follow/orbit, native FOV/zoom program,
terrain/bridge sampling and region rebasing. It does not authorize arbitrary
camera resets, a fixed diagnostic camera, a tile-centre focus substitute,
altered source controls or any other source adaptation. Final visual,
performance, browser and owner-acceptance gates remain mandatory.
