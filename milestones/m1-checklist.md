# M1 delivery checklist

Last status review: **2026-09-17**. Status checkpoint: `84fec2d`; integrated
application code checkpoint: `3eff49c2`.

**M1 is in progress, not accepted. Normal browser gameplay entry is still
blocked, and the owner-testing deployment has not happened.** The server-side
journey and individual components are further along than the playable client.

This is the day-to-day remaining-work tracker for the complete
[Section 30 starter journey](../prompt.md#30-starting-vertical-slice).
The [acceptance gates](m1-starter-journey.json) and
[task ledger](m1-tasks.json) retain authoritative gate state, dependencies,
ownership and execution history. Update this checklist alongside them when
work changes; checkbox counts are not a completion percentage.

## Completed checkpoints, not full milestone acceptance

- [x] Freeze OSRS build 240/cache 2695 and obtain the
  [owner-approved reference pack](approvals/m1-reference-pack-v1.3.0.json).
  This is implementation permission, not acceptance of ClubScape's output.
- [x] Implement and qualify the
  [native account/game persistence and source-content infrastructure](evidence/m1-engine-review-repairs.json).
- [x] Demonstrate the legitimate saved-account journey across staged runs:
  all 70 Tutorial Island transitions, Lumbridge arrival and activities,
  death/grave recovery, and Cook's Assistant with its correct reward.
  The [completed remainder](evidence/m1-saved-water-execution-02-result.json)
  adds water filling, dough, one natural range burn and postquest recovery.
  This historical chain is **not** a fresh full journey on one current
  headless/browser candidate; `full_journey_passed` remains false.
- [x] Integrate and qualify the reviewed
  [native/WASM camera consumers and terrain-height presence repair](evidence/m1-camera-consumers-root-qualification.json).
  Live player-focus/effect producers are not included in that completion.
- [x] Record [conditional owner-testing deployment approval](approvals/m1-owner-mac-testing-deployment-v1.json):
  a playable candidate first, then Mac access through localhost over SSH.
  The current engineering preview is not approved for deployment.

## 1. Make normal browser play work

The next ready task is **`M1-CAMERA-PRODUCER-CONTRACTS`**. Its current scope is
read-only assessment of existing source evidence, not another original-client
invocation or permission to remove the entry guard.

- [ ] Establish source-backed live actor identity, fine logical position,
  render interpolation, camera footprint and effect producer contracts.
  Record supported mappings or exact missing evidence in
  `evidence/m1-camera-live-producer-contracts.json`.
  See the [current producer gap](evidence/m1-camera-live-producer-gap.json).
- [ ] Implement and connect those producers to the qualified camera consumers;
  finish required normal-scene metadata and enable normal entry only with
  valid actor/world/scene inputs. Do not substitute tile centers, zero
  footprints or assumed inactive effects. Task: `M1-CAMERA-INTEGRATION`.
- [ ] Implement source-correct click-to-approach-and-act for required world
  interactions, preserving the selected action and authoritative permission
  checks. Manual walk-then-click is not the completed behavior.
  Task: `M1-WORLD-ACTION-ROUTING`.
- [ ] Finish actual runtime UI/native bindings: equipment and skill models,
  minimap/markers, notifications, preferences and player previews.
  Task: `M1-UI-INTEGRATION`.
- [ ] Deliver real instance/template and actor metadata to the relevant
  renderer/UI consumers; do not infer templates from opaque instance IDs.
- [ ] Complete and demonstrate the real sign-up/login-to-world path, loading,
  authentication/reconnect feedback, required controls and supported resizing.
  Preserve tutorial unlocks and explicit capability/out-of-scope feedback;
  missing required functionality must not become an unavailable feature.

## 2. Finish source-faithful presentation

- [ ] Independently assess the
  [parked renderer fitting/certificate repair](evidence/m1-renderer-handoff-0665f94.json),
  repair remaining geometry/style failures, and integrate only qualified work.
  The root's last verified fitting result is **931 failing poses out of 2,034**.
  The worker's historical 3/2,034 result predates its certificate repair and
  cannot replace that baseline. Both authorized full-table runs are consumed;
  a corrected full-table measurement needs its own admission.
- [ ] Finish turned/instanced scenery and full-scene rendering, player/gear
  deformation and required animation timing, including remaining Death's
  Office start-phase coverage. Isolated exact model/scene captures do not
  establish fidelity of the composed game.
- [ ] Resolve remaining source UI/preview discrepancies, including the
  archive 84 `contentType 328` versus template 0 mismatch.
- [ ] Resolve [remaining source behavior questions](../research/interface-contracts/remaining-source-questions.json):
  ordinary-grave Bank-All permission, dough motion and all seven Empty
  variants. Close the water-animation evidence gap as well. Missing evidence
  is not proof that a control is disabled or an animation is absent.
- [ ] Reconcile the approved public source/content/asset inventory and validate
  the exact candidate's manifests and stable IDs. Do not silently promote
  unreviewed generated assets or change frozen reference inputs.
- [ ] Complete the real startup-to-world presentation checkpoint: source-matched
  entry screens, Tutorial Island/Lumbridge, tree, animated goblin, penguin,
  resizable classic HUD and actual music/sound playback.
- [ ] Produce reproducible composed-browser captures for every required entry,
  scene, interface and animation case; compare against the approved reference
  pack and unchanged tolerances, with approved adaptations reviewed separately.
- [ ] Complete and demonstrate whole-journey runtime audio: genuine source
  triggers, spatial/scene inputs, timing, playback/preference controls and
  audible output. Imported recordings and isolated cue fixtures are not enough.

## 3. Prove the assembled gameplay candidate

- [ ] Complete the entire fresh-account journey headlessly on the selected
  current candidate: normal starting state, full tutorial, legitimate
  Lumbridge activities and complete Cook's Assistant. Preserve historical
  evidence rather than relabeling it as a current-build run.
- [ ] Complete that same journey through the real browser and authoritative
  server, without diagnostic camera inputs, progression skips, seeded rewards
  or fixture-only controls.
- [ ] Demonstrate onboarding and postquest logout/login, reconnect and server
  restart recovery, plus source-defined death/recovery, without losing
  acknowledged progress or duplicating items, XP or rewards.
- [ ] Close the candidate's migration/extension and security/recovery gates
  with the relevant real behavior evidence, not infrastructure smoke results.
- [ ] Record the exact public runtime inventory, candidate revision, relevant
  review and normal-entry/required-interaction readiness in
  `evidence/m1-owner-playable-candidate.json`.
  Task: `M1-OWNER-TEST-CANDIDATE-READINESS`.

## 4. Deploy for owner testing, then close acceptance

Deployment is already conditionally approved. **Final Mac measurements and
owner presentation acceptance follow deployment; they must not become
prerequisites for the deployment that enables them.**

- [ ] Package the playable candidate and approved public assets, excluding
  credentials, private checkpoints, development/session material and unrelated
  files from the build context and image.
- [ ] Follow the [scoped Compose deployment plan](evidence/m1-owner-compose-deployment.json):
  add only ClubScape app/database services to `/home/lramos15/compose.yaml`,
  use separate persistent owner-testing data, publish the app on loopback only
  and keep the database private. Preserve all existing services and data.
- [ ] Verify the actual application, RPC and asset readiness; record deployed
  revision/image identities and provide the exact Mac SSH-forward command and
  localhost URL. An account-only or camera-blocked page is not a playable
  deployment. Task: `M1-OWNER-COMPOSE-DEPLOYMENT`.
- [ ] Record exact Mac hardware, macOS, Chrome and Edge versions, and complete
  both browsers' fresh-account and presentation/recovery acceptance paths.
- [ ] Meet the unchanged [benchmark contract](../spec/m1-benchmark-contract.json):
  1920x1080, at least 60 completed FPS, p95 frame time at most 20 ms, p99 at most
  33.333333 ms and maximum stall 100 ms, using its frozen workloads, 30-second
  warmup and 180-second measurement window. Include the specified server
  workloads; local Sparky results do not certify the owner's Mac.
- [ ] Complete independent style and technical review of the final candidate
  and its evidence; repair failures rather than deferring them as polish.
- [ ] Obtain explicit owner visual **and** audio acceptance of the exact
  reviewed build and evidence.
- [ ] Close every machine-readable M1 gate, record the accepted revision and
  durable evidence, park unfinished workers and stop. Do not begin M2, push,
  or claim a public/production release.

## Keeping this tracker trustworthy

Only check an item when its described outcome is implemented, demonstrated
and durably linked to the relevant revision/evidence. Update affected gate/task
records in the same checkpoint; preserve historical evidence and frozen
approvals rather than rewriting them. Several older task/spec notes describe
superseded failures: the saved cooking/recovery remainder has passed, and the
old camera-height and five-TypeScript-error blockers are not current failures.

This checklist does not grant execution authority. Both saved continuations
are consumed; protected checkpoints are not general resume tokens. Existing
source/browser/GPU/operator/RuneLite and full-renderer-run limits remain in
force. Conditional deployment approval does not renew them, and routine M1
engineering does not require another blanket milestone approval.

RuneLite feasibility remains a separate, parked Section 12 assessment, not a
prerequisite for browser M1 acceptance. No later milestone is authorized.
