# Renewed live assessment: invocations4/5

**Bounded assessment complete: the complete compatibility tuple failed.**
Two additional invocations were executed and both failed. Total usage is5/5
conservatively counted invocations, one materially distinct architecture.
No sixth attempt, support tier, desktop deferral or M1 acceptance is authorized.

Authority: eec354c00d67d88686794085de2c8ecb31ef91df, integrated as1f1a703.
The exact authority record is `milestones/m1-runelite-live-renewal.json`.
The new [`live-renewal.json`](live-renewal.json) references the original
`experiment.json` and `assessment.json` by SHA256; neither original file changed.

The authority cherry-pick conflicted with unrelated, absent fleet task entries.
Resolution kept this branch's existing fleet bookkeeping and applied only the
RuneLite4/5 acceptance change plus the exact new authority file. It did not
import an unrelated gameplay-UI completion claim or expand worker ownership.

## Pinned tuple and real service

The unchanged official RuneLite1.12.38 client/plugin JAR, injected-client1.12.38
revision240, cache2695, API runtime, decoder and JDK are the original hash tuple
in `build-inputs.json`. No upstream JAR, signature, source asset or model was
modified. The adapter uses the real RuneLite UI, original scene/canvas/player
renderer, genuine Hooks/EventBus and XP Tracker—not a replacement native app.

The actual current **normal** `clubscape-server` binary was rebuilt before
admission; binary SHA256:

```text
0333abbb117f91e8f2b600bac18c4d452c3b1e569aaedee80539efdda378d3e9
```

Both invocations used the preserved `game-3a74cbe` candidate:

```text
artifact: df3e2a452c100ecd94d2abc68e5cb1556f58090700474f547e7fd36de6682b3d
revision: m1.source-backed.v3.0e506f3dab24bbe0
```

Before admission, all5,011 declared asset files were checked for allowed storage
extensions, confinement and exact hashes. Storage uses `.bin`/`.json`; extensionless
URLs are not confused with storage filenames. No guard or source asset was
removed. The root/artifact/catalog/world identity was not repacked or repinned.

The real isolated PostgreSQL-backed service reported `game.v1` readiness with the
strict `ordinary_normal_f2p` profile and its six source-proved inactive paths.
The old environment/descriptor and engine-review defects were **not** interlocks.

## Invocation4: actual join/render, signed-package failure

Code checkpoint: eb001170feefc5d2fa7066221b72536f6e39fe1c.
Elapsed time including cleanup:13.072 seconds; native exit1, server shutdown0.

Observed through real HTTP/protobuf:

* fresh public Register/Login/CreateCharacter with empty options and Join;
*24 source skills, empty inventory, HP10, prayer1, energy10000;
* source position3094,3106, `stage.tutorial.appearance`;
* actual player projected into original native coordinates and penguin
  NPC2063/model21547, original75/128 scale, 200 vertices/396 faces;
* original source scene displayed in the actual RuneLite window;
  all1,024,000 native canvas pixels matched Robot screen readback;
* real source ticks4→7→8, not an accelerated client clock.

Before any gameplay input or XP, baseline inspection failed:

```text
SecurityException: net.runelite.client.plugins.xptracker.XpTrackerReadback
signer information does not match other classes in the same package
```

The adapter helper had been placed in the signed upstream package. Compilation
and earlier panel-only preflights did not load this path. **No signature was
removed or JAR repacked.** The helper was moved to the adapter's own package and
uses narrowly pinned read-only reflection. A focused check verifies the official
plugin remains signed, the helper is outside that package, and an uninitialized
plugin cannot count as a real baseline. This is a correction within architectureA,
not a new architecture or synthetic-XP fallback.

## Invocation5: baseline and legitimate transitions, real OutOfReach

Code checkpoint:389c3adbfbe84e723500f7ed25e0840b7041c8c5.
Elapsed time including private backup/cleanup:17.46 seconds; native exit1,
server shutdown0.

The signed-package correction worked. Real normal creation/join and the native
window again passed. The actual XP Tracker baseline was verified at source
tick8/revision10:

* source fishing XP tenths0;
* native absolute integer XP0;
* genuine tracker initialized, remaining initialization ticks0, gain0;
* **zero fake callbacks**.

The account then performed real source intents:

| Operation | Receipt/event evidence |
|---|---|
| ConfirmAppearance, sequence1 | request157dd1f7-88aa-417a-ac56-65455b28828a; tick10/revision13; `appearance_confirmed`, source counter and `tutorial_advanced` events; stage became `stage.tutorial.experience` |
| SelectExperience, sequence2 | requeste01e6e71-2aac-48ca-aef9-d777fc1f98a0; tick13/revision17; `experience_selected`, source counter and `tutorial_advanced` events; stage became `stage.tutorial.guide_greeting` |
| Talk-to `spawn.gielinor_guide`, sequence3 | requestdf2ba9db-c879-4ec3-acb6-baca20897da3; authoritative HTTP409; error ID245ca9ba-29ef-4886-937a-12d18b1c90d3; server `game_error_code=OutOfReach` |

The driver assumes the guide can be talked to immediately after source selection.
It checks advertised actions but neither navigates to the current target nor
rechecks current target reach after pacing. The source guide is mobile, has a
one-tick step cadence, wander radius3, and Talk-to reach1. The actual failed-tick
guide tile was **not recorded**, so this evidence does not prove a particular
wander step, sight obstruction, or an engine defect. It does prove that the
driver's unverified adjacency assumption is insufficient.

The original source reach/collision guard rejected the request; it was not
weakened, retried blindly or replaced with a grant. No walking/gathering input
was reached. No server XP event, `StatChanged` XP callback, plugin gain, final
movement screenshot or complete-tuple record exists.

## Coverage and precision

**Observed in live invocation5:** real account/world connection, normal source
state, actual native local-player coordinates/model/scene/window, source game
ticks, genuine tracker baseline, and committed appearance/experience events
stored in the adapter's typed projection.

**Not demonstrated:** authoritative movement shown natively, first XP action,
positive server XP→native StatChanged→genuine plugin gain, native shop/HUD/menu
interaction, other-player/NPC/ground-item registries, dynamic object presentation,
audio, reconnect/restart, or broad generic plugins.

The implementation retains source XP in tenths and defines native display as
`floor(current_tenths/10) - floor(initial_tenths/10)`. Source deltas must match
deduplicated committed event IDs, and the real tracker baseline must precede XP.
That positive/fractional live mapping remains **unverified**: both invocations
produced no XP. An initialized zero-gain tracker is not success.

The earlier 109 controlled source images and offline preflight were not credited
as new live proof. The joined screenshots above are produced after actual
authenticated source snapshots in these invocations.

## Preservation, resources and stopping

Both owned services/containers exited and were verified absent. JVM/Xvfb cleanup
was owned and bounded; cache/runtime library hashes stayed unchanged. Credentials
were kept in memory/ignored private files, not committed or logged.

Invocation4 used a disposable database and retained public protocol/state logs;
no private database archive was made. It had no gameplay input beyond creation.
Invocation5 additionally retains a consistent read-only `pg_dump` of its owned
synthetic world/account database before removal. It includes private auth/RNG
material, is mode0600 under ignored artifacts, and **must not be published or
read as a substitute for public gameplay evidence**. Only its path/hash/size are
recorded. It preserves the acknowledged appearance/experience progress; no direct
world/XP/quest SQL or reseeding was used.

The database snapshots belonged to distinct isolated invocation databases.
Invocation5 was a fresh normal account, not a claimed restoration of invocation4.
No persistence/restart acceptance is asserted.

All executed invocations stayed within the renewed600-second,3GiB JVM,
two-processor/two-Rust-job,2CPU/512MiB PostgreSQL limits. No nested agents,
peer messages, external source account/login, graphics/sandbox change, new
desktop app or further milestone was used. The allocation is exhausted:
**stop here; no automatic sixth invocation**.

## Recommendation and exact next scope

Continue architectureA only after a separately authorized bounded follow-up:

1. Replace the driver's static interaction assumptions with actual typed
   target positions, evaluated options and legitimate server walking; re-poll
   after pacing and record target/actor tiles and the relevant permission.
2. Add moving-target/source-contact driver regressions without changing
   authoritative collision/reach or inventing a successful dialogue.
3. Resume the public normal route, verifying the complete positive first-XP
   correlation and genuine panel/overlay rendering together.

This is likely1–3 engineer-days of focused driver/interop work, not a commitment
or impossibility claim; additional live gaps may appear. There is no evidence
that a new native client or upstream signature modification is needed.
Any desktop deferral still requires owner approval and has not been granted.
