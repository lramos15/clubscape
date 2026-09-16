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
traceable visual/audio pack under Section 30.2. This gate is now satisfied by the
exact external [v1.3.0 owner record](../milestones/approvals/m1-reference-pack-v1.3.0.json).
It authorizes implementation against the frozen source, not final candidate
presentation acceptance. Do not rewrite the approved manifest's pre-approval
fields or substitute generated placeholders. Source-supported inferences and
the narrowly approved loot/audio adaptations retain their exact classifications.
The pack also hash-locks the original `art-style.md` and `interface-parity.md`
context documents. Their pre-approval status text is historical; the external
approval and [current browser boundary](browser-implementation.md) are the
current authority. Preserve those two frozen files byte-for-byte.

Before presentation/performance implementation, Section 36 requires the
benchmark contract and pinned representative hardware/browser configuration.
The pre-implementation budgets and available Sparky engineering profile are
fixed in `spec/m1-benchmark-contract.json`. The owner will run real Chrome/Edge
acceptance on an M-series Mac after implementation; exact native inventory
and measurements remain pending. Host/tool availability cannot satisfy the
gate, and Sparky engineering evidence does not stand in for unrun Mac/Edge
measurements.

Every acceptance record must link actual evidence and its tested revision.
Pending, blocked, deferred, or unexecuted checks are not passes. Final owner
acceptance must identify the exact reviewed build and visual/audio evidence.
Any later milestone needs its own separately approved execution.

## Execution limits and recovery

The shared project ceiling is 25 active AI agents including the Director.
Only explicitly reserved workers may run; current assignments are in the
durable task ledger. Nested delegation is forbidden. No factory is
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

The [recovery/audio checkpoint](../milestones/evidence/m1-dying-audio-repairs.json)
preserves the distinction between native repairs and the actual normal-account
journey. That same source-5e account has completed all 70 tutorial transitions,
Learning the Ropes' 1QP reward, legitimate Lumbridge arrival, copper mining,
bank/shop operations, goblin combat and loot: 321 checks across three
invocations, including two verified private database restores. It is now
preserved at a dying-state `PollWorld409` failure. The
[immutable actual-state diagnosis](../tools/journey-tests/evidence/dying-ui-style-7fa370e14c9b421e.json)
now confirms the repaired stale-combat-style cause: no equipment remains, while
the selected sword style refers to the sword in the actual grave. Native and
PostgreSQL prerequisites pass. The
[current bounded authorization](../milestones/evidence/m1-dying-observation-authorization.json)
admits a tested observation-only adapter and one real restore/read-only public
observation, followed by a new protected checkpoint. It does not yet authorize
player gameplay continuation. Do not repin, heal, skip a death phase, replay
retention, or recreate missing trace events.

The newer [authority candidate](../milestones/evidence/m1-authority-controls.json)
uses source 5b3; its controls, audio history and animation contracts are verified
separately, without repinning existing worlds. The exact composed audio/storage
fixture passes after the native cue-timing repair. That fixture is not the
complete production browser client, a fresh browser journey, or owner audio
acceptance.

Three [source facts](../research/interface-contracts/remaining-source-questions.json)
remain unverified: ordinary-grave Bank-All permission or deliberate disablement,
inventory dough motion or deliberate absence, and Empty motion or deliberate
absence for all seven bound variants. The source worker is parked after its
bounded evidence pass. These are acceptance blockers, not permission to invent
an animation, claim a source-disabled control, or waive a required interaction.
The implemented contracts and source pins are unchanged; other M1 work continues.

The [terrain/clip checkpoint](../milestones/evidence/m1-terrain-clips.json)
integrates the original assembly-time terrain pass, exact outer-band minimaps
and twelve additional actor clips. Missing or malformed terrain now fails
without replacing the previous scene. All 15 native minimaps and the actual
Chrome comparisons of five source scenes and 58 models are pixel-exact.
These component results do not establish a complete composed interface or
fresh browser journey. The explicit equipment-fitting gate still fails
931 of 2,034 legal item-frames; permitted gear fitting, turned scenery and
frozen-window performance remain unfinished.

The [native UI comparisons](../milestones/evidence/m1-ui-controls.json),
[renderer/audio results](../milestones/evidence/m1-render-audio-integration.json)
and [reviewed travel repair](../milestones/evidence/m1-ui-travel-interruption.json)
remain qualified separately. Remaining controls, the full journey, runtime
presentation, performance and owner review still need their own evidence.
