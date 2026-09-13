# M1: complete starter journey

Status: **in progress, not accepted**.

## Fleet continuation

The owner's 2026-09-13T23:30:04.367Z request reopens M1 execution in fleet mode
and permits the latest usable verified gamepacks/cache instead of requiring
cache 2695. Record the replacement identity and any material source-contract
differences; retain previous evidence as historical rather than silently
rewriting it. This changes the reference-selection constraint, not the full
journey, fidelity, approval, browser or performance acceptance requirements.
See `milestones/m1-fleet-resumption.json`.

## Authority and boundary

The owner's 2026-09-13 execution request approves only the first milestone.
`prompt.md` Sections 30 and 44 define that milestone in full. This record does
not replace or narrow them. The starting revision is `973c872`; it contains the
project prompt and machine documentation, but no game implementation.

Required deliverables are real UI account creation, the complete normal-account
Tutorial Island journey, legitimate arrival in reference-mapped Lumbridge,
copper mining, inventory/equipment/XP, banking, shops, goblin combat and
death/recovery, and the complete Cook's Assistant quest. Browser and headless
clients must use the same authoritative server. Acknowledged progress must
survive logout, reconnect, and server restart without loss or duplication.

The whole entry sequence, visible world, animations, interfaces, and audio must
meet the owner-approved source reference pack. Acceptance also requires Chrome
and Edge, 60 FPS at 1920x1080 on pinned modern integrated graphics, independent
style/technical review, and explicit owner presentation acceptance of the
reviewed build. Toolchain smoke tests are not evidence for these gates.

## Non-goals

No Mining 1-99, Club Penguin minigame/cosmetic loop, bulk content production,
native desktop replacement, WebGL/mobile support, production deployment, or
complete WaddleWorks platform is authorized by this execution. Drafting the
full/release contracts does not approve them. RuneLite feasibility is separate
from browser acceptance and must not be reported as compatibility on the basis
of upstream compilation or launch.

## Gates and prerequisites

The machine-readable acceptance record is
[`milestones/m1-starter-journey.json`](../milestones/m1-starter-journey.json).
Task dependencies and path ownership are recorded in
[`milestones/m1-tasks.json`](../milestones/m1-tasks.json).

Before presentation implementation, the owner must approve a concrete,
traceable visual/audio pack under Section 30.2. No such pack or approval exists
at kickoff. Do not substitute generated placeholders or approve it on the
owner's behalf. Independently testable account, protocol, persistence, content
validation, and checkpoint infrastructure may proceed while the pack is
blocked. Source-dependent gameplay must retain visible reference gaps.

Before presentation/performance implementation, Section 36 requires the
benchmark contract and pinned representative hardware/browser configuration.
Sparky has an NVIDIA GB10; no representative integrated-graphics benchmark
contract has been pinned for this execution. Host/tool availability alone
cannot satisfy that gate, and the GPU model name is not proof of equivalence.

Every acceptance record must link actual evidence and its tested revision.
Pending, blocked, deferred, or unexecuted checks are not passes. Final owner
acceptance must identify the exact reviewed build and visual/audio evidence.
Any later milestone needs its own separately approved execution.

## Execution limits and recovery

The shared project ceiling is 25 active AI agents including the Director.
At kickoff, no other agents are visible. Only explicitly reserved workers may
run; nested delegation is forbidden for these initial tasks. No factory is
authorized or required. Worker completion, failure, cancellation, or parking
must be recorded before releasing its reservation.

There is no owner-imposed AI spending cap. Provider quotas and runtime limits
still apply; their exact remaining capacities are not exposed. Do not invent
usage totals, bypass throttling, or purchase infrastructure. Local load tests
must use bounded synthetic accounts and isolated project services.

Work occurs on `milestone/01-starter-journey`, not on `main`. Do not touch
unrelated running services or machine configuration. Commit durable
checkpoints with completed work, commands/results, blockers, and executable
next tasks. No push to `main` or unreviewed integration is authorized.

Stop when all acceptance gates pass. If required inputs, owner approval,
hardware, or external execution limits prevent completion, continue independent
unblocked milestone work where possible, then report the milestone as
incomplete with a durable checkpoint. Never relabel that checkpoint as an
accepted slice.
