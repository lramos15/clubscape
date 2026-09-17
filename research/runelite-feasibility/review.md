# Bounded M1 RuneLite feasibility review

2026-09-14/15, authority/content base
`3fc927788a69fed2aa4d7b77b317ef64acb90be0`.

**Assessment complete; compatibility UNVERIFIED; no support tier.**
This independent Section12 companion neither completes nor blocks the mandatory
browser/headless M1 acceptance gates. No desktop deferral is approved.

## Bounds and architectures

[`experiment.json`](experiment.json) established a maximum of **two architectures
and three live integration invocations before implementation**. One reserved
worker was used, with no nested agents/factories/peer messages. JVM heap was
bounded to3GiB/two active processors, Rust builds to two jobs, PostgreSQL to
two CPUs/512MiB, and each live invocation to600 seconds.

**Architecture A was implemented:** an alternate composition root around the
unchanged official runtime, original startup/canvas/renderer, and real RuneLite
plugin/event components, with generated ClubScape protobuf transport. Original
OSRS game/network loops are kept at the existing startup semaphore; the adapter
owns projection/render scheduling, not authoritative gameplay.

**Architecture B was reserved, not executed:** narrowly intercept the original
RuneLite lifecycle/transport scheduling while retaining its normal application
entry. A's repaired original-runtime preflight made such a fork unnecessary for
the demonstrated launch/render surface. Neither client architecture can fix the
independently reproduced strict server prerequisites. No untested B success or
impossibility claim is made.

The bound is exhausted. The first numbered invocation failed before services
started; it is conservatively counted rather than silently granting another try.
The other failures were followed by a different, diagnostic-driven setup—not
blind repeat launches.

## Exact tested tuple

* Official RuneLite **client1.12.38**, including unmodified
  `net.runelite.client.plugins.xptracker.XpTrackerPlugin`:
  JAR SHA256
  `6dcd0da2fdbf48d35c3d8f666d801d05afa964030c553e806eb173dba97323d5`.
  Plugin-class SHA256
  `52b1ad192ddd43db97864be31b467b0e9eb2512c362d174c3b9a8a2b2e16905f`.
* Original injected-client1.12.38:
  `7fdedf1194261cc5b99faa35e0d2b4e45b6d56665402ccbde7f3aa6207c3f947`.
  Actual original init returned revision **0→240**. Opaque build identifier
  `33653951311.245` is **not** revision245.
* API1.12.38-runtime:
  `6d50aa843c070120a5093c5ead29d72e92742074346241e0359a133dad008852`.
* Frozen cache2695; all original cache file hashes were checked before/after
  native runs. Source/asset/reference files were not changed.
* Decoder cache1.12.39-SNAPSHOT from
  `ac79ed8bd8926bec7bf172aa291574b4d944b0e7`:
  `8056039bee74da629e596896048645085f7753159a064da51c24e9bd99f13e8e`.
  This is distinct from the runtime version.
* Temurin **17.0.20.1+1**, Linux ARM64, executable
  `/home/lramos15/.local/share/jdks/temurin-17.0.20.1+1/bin/java`;
  executable SHA256
  `185fdd3e40d7736c600cb948403f77fd3421e8877e2ff13271673f28a501531f`.
  JDK11 availability was rechecked but is not this adapter's tested tuple.
* Protoc3.21.12 ARM64:
  `aa609ec1d4b08375a140b1a42340684932cd2cd0cd56ffe8d791a72f9f3894bc`.
  The original `protoc --version` failed127, so only this declared local
  dependency was added. No global development environment changes were made.

[`build-inputs.json`](build-inputs.json) records all32 libraries, actual JDK,
generated-schema hashes and commands. The AppImage/launcher2.8.0 was **not used**
in these runs; its prior EULA smoke test is not included as compatibility evidence.

## Executed evidence

| Evidence | Observed result | What it does not prove |
|---|---|---|
| Final developer-only `preflight-a5`, exit0 | Real `ClientUI` and genuine XP Tracker active; unchanged original init/renderer/player; no swallowed native errors | No ClubScape connection, login, XP event, or plugin gain |
| Original native scene | 36,107 tiles;2,646 object tile references; actual source loader/rasterizer | Full live dynamic NPC/object/HUD projection |
| Original penguin player | NPC2063/model21547, 200 vertices/396 faces, original75/128 scale and idle5668;220 player-dependent pixels | Live authoritative avatar/movement, other action animation or equipment fitting |
| Actual RuneLite window | All1,024,000 native canvas pixels matched Robot screen readback;696,282 nonblack pixels; genuine tracker panel visible | A live-state or first-XP demonstration |
| Live invocation1, exit1 | Harness `Path.open(opener=...)` TypeError before database/service startup; fixed and covered by a private-file unit test | Any connection or gameplay |
| Live invocation2, native exit1 | Owned real PostgreSQL/service; RuneLite process sent/decoded real HTTP/protobuf `Hello`; adapter refused gameplay-unavailable readiness | Signup/login, world join, state/events, or any live scene/player |
| Live invocation3, service exit1 | Public `Config::with_game_root` plus unchanged `Service::bind` rejected the actual full-source descriptor with `game_file_size` | Any listening game service or client connection |

Detailed commands and exits are in `preflight-a*.json`, `live-a*.json`,
`build.json`, `build-history.json` and `service-probe-build.json`. Compact exact
logs are preserved under [`evidence/`](evidence/). Binary/image artifacts remain
in the ignored directories identified by the evidence inventory.

The offline probes were deliberately separate. Diagnostics repaired:

1. Original initialization starts a real thread; give it nonzero dimensions and
   verify its existing delaystart semaphore before supplying adapter state.
2. Native `WorldPoint` reads actor path heads, not only local render coordinates.
3. Source240 transient actor insertion uses the `rl17` lists and native `bw()`
   cleanup. The older `gc()` path did not clear those entries.
4. An extra canvas hid real rasterization behind the original black canvas.
   The first framebuffer-only check did **not** prove presentation. The added
   actual-window assertion failed in `preflight-a4`; retaining the original
   startup canvas (`td`) fixed it in `preflight-a5`.

An early preflight visibility diagnostic used the misleading phrase “same live”
inside its method description; the run was explicitly offline and never live
evidence. The final preflight removes that wording and adds explicit
developer-only classification plus the real-screen assertion. Earlier failures
are not relabeled successes.

## Shared prerequisites reproduced, not bypassed

### Independent game-root configuration

At this base, `Config::from_env` reads `CLUBSCAPE_GAME_ROOT` only **inside**
the `CLUBSCAPE_WEB_ROOT` branch. A documented game-only launch consequently
starts account-only. Invocation2 reached the real service and rejected that
state; it did not continue with a synthetic/offline game.

Director action: make game-root parsing independent, keep blank/invalid settings
fail-closed, and test absent/game-only/invalid/combined environments in isolated
subprocesses. The existing public constructor was a legitimate diagnostic route,
not a weakened server or a change to game authority.

### Descriptor cannot contain the actual source asset membership

The **actual strict compiler API** loads the frozen artifact and reports5010
referenced assets. Even mapping all of them to the theoretical shortest allowed
`/assets/` URL requires267,049 bytes for the assets object alone. This exceeds
the server's262,144-byte descriptor limit before other fields. The real per-asset
source pack produces a283,075-byte descriptor.

[`strict-compiler-capacity.json`](strict-compiler-capacity.json) is executable
compiler evidence, not an ad-hoc schema inference. Invocation3 confirms the
real loader failure. The packed artifact hash is
`0181ec69456101defccbc8dd99b579ed849caae6301d801387732bea70fe98f5`.
No assets, source scenery, progression, rules, readiness proofs or permissions
were removed to make it fit.

Director action: increase the bounded private descriptor limit sufficiently
for M1 (for example1MiB), retaining all hash/path/symlink/asset-membership/
readiness/loopback guards, and add a real full-source capacity/startup test.
No protocol tag or authoritative gameplay-rule change is required.

## Remaining compatibility surface and cost

Live signup/login/join, authoritative movement/XP events and genuine XP Tracker
gain are **all unverified**. The real-input first-XP driver and event-to-plugin
mapping compiled and have small codec/projection tests, but never reached their
live paths. Invented XP callbacks, seeded progress/items or a PNG viewport were
not used to fill this gap.

Other players/NPCs, dynamic doors/objects, ground items, equipment, native chat,
full source HUD/widget state, menu-to-input dispatch, recovery/reconnect and
non-idle/walk avatar action fitting remain incomplete or unverified. Public
typed entity data is retained but not broadly materialized in native registries.
No tile/ground overlay or other plugin is claimed. The original stock runtime
entry and its complete core-service lifecycle are not verified by the alternate
composition root.

**Recommendation: continue narrowly after the two shared prerequisites are
fixed, pending owner review/renewed experiment bounds.** Rough planning ranges,
not commitments or proof of success:

* Shared configuration/capacity fixes and real-source regressions:0.5–1
  engineer-day.
* Renewed end-to-end first-XP integration with this unchanged-runtime approach:
  2–5 engineer-days, with remaining native/event/thread invariants verified
  against actual server receipts. This may reveal additional prerequisites.
* If normal upstream entry/lifecycle preservation is required instead, the
  reserved focused lifecycle interception is a viable **untested** alternative:
  approximately5–10 engineer-days initially, plus revalidation on every pinned
  runtime update. It cannot solve the two shared service defects.
* Broader generic-plugin/HUD coverage is a separate, likely multi-week effort,
  not earned by the proposed first-XP demonstration.

Maintenance risks are explicit native obfuscation bindings, original client
thread/lifecycle assumptions, fractional-XP display semantics and incomplete
OSRS-specific state. No upstream class patch currently exists, but an alternate
composition root and roughly a few dozen native bindings still require
maintenance; “unchanged JAR” does not mean zero integration cost.

This is not an impossibility finding or a deadline/tool-absence argument.
Browser/headless work continues independently. A desktop deferral would require
separate owner approval, which has **not** been granted. There is no permission
to substitute another native client or start a later milestone.

## Validation and cleanup

Final Java compile passed;9 generated-codec/projection checks and3 Python
harness checks passed. The owned public-service probe passed locked/offline
build, Clippy with warnings denied and rustfmt checks. These are not game or
plugin-update acceptance. The final offline run preserved unchanged cache/JAR
hashes and had no native rendering exceptions.

Every owned JVM/Xvfb exited. Both created PostgreSQL containers were explicitly
stopped and verified absent; the regular server exited cleanly0 and the strict
probe exited1 on its diagnosed startup rejection. Credential files were removed.
No private RuneLite profile/account was read, no external account was used, and
no graphics/sandbox/public-bind/infrastructure changes were made.

Only this reserved worker was used. Billing totals are not exposed to this
worker; no nested-agent usage is attributable to this task. The assessment,
code and evidence are preserved; all further live admissions are stopped.
