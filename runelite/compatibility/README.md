# Experimental original-RuneLite adapter

**Compatibility is unverified. No Tier 1 claim.** The bounded M1 assessment is
complete, not the live demonstration. See
[`review.md`](../../research/runelite-feasibility/review.md) and
[`assessment.json`](../../research/runelite-feasibility/assessment.json).

**Renewed live result:** explicit invocations4/5 are now exhausted. Actual normal
signup/join/native scene/window passed; invocation5 also verified the genuine
tracker baseline and legitimate source appearance/experience events. The guide
interaction was authoritatively rejected asOutOfReach before movement or XP.
The complete tuple did **not** pass; no support tier is claimed. See
[live renewal review](../../research/runelite-feasibility/live-renewal-review.md)
and the separately preserved renewal ledger/evidence. No automatic sixth run.

**Startup repair applied:** 2c5fe68 is integrated as4953c4d. Independent game-root
parsing and the512KiB private descriptor limit resolve both earlier startup
blockers; the unchanged current candidate fits. See
[current closure and unchanged experiment boundary](../../research/runelite-feasibility/startup-2c5fe68.md).
The old startup failures below are historical. The original three-invocation
allocation remains unchanged; the separately authorized4/5 allocation is also
now exhausted.

**Current candidate:** the complete62a003c/e1076a87 actor pair and3a74cbe source
refresh are integrated. Use `game-3a74cbe` / `df3e…b3d` and its matching catalog,
not a200, for any newly admitted candidate. See
[current packaging and preserved-world evidence](../../research/runelite-feasibility/canonical-3a74cbe.md).
Only the source `/revision` changed; older pinned worlds/runs remain untouched.
The original exhausted live bound remains; shared startup blockers are resolved
by the repair above.

**Prior canonical update:** authorized faa8002 is integrated as c841f6a, with the
then-current `a200…d62` source artifact and resolved source contact/clock probes.
At that checkpoint the environment/descriptor blockers remained; they are now
resolved by4953c4d.
The separately named historical pack/catalog and commands remain in
[`canonical-faa8002.md`](../../research/runelite-feasibility/canonical-faa8002.md);
do not overwrite those inputs or treat that old pack as current. No additional
live invocation was admitted by that refresh.

**Shop ABI update:** authorized3310032 thenaf75e17 are integrated, and the
owned buy/quote transport now pins immutable displayed canonical item identities,
refreshes rejected selections, and retains original uncertain operation bytes.
See [shop adaptation and checks](../../research/runelite-feasibility/shop-af75e17.md).
Regenerate the authoritative Java schema using the separately named report
there; old evidence and legacy `None` wire/hash behavior remain unchanged.

This is a Java protocol/state/render/event adapter around the **actual unchanged**
official RuneLite 1.12.38 runtime and injected revision-240 client. It uses the
official `RuneLiteModule`, `ClientUI`, `PluginManager`, `Hooks`, `EventBus`, and
unmodified XP Tracker plugin. It does **not** implement another desktop UI,
terrain renderer, penguin mesh, game authority, or OSRS protocol server.

## What was actually demonstrated

The final **developer-only offline preflight** initialized the original runtime,
loaded the original source scene, assembled the approved penguin through the real
player NPC-transformation path, displayed it in the real RuneLite window, and
opened the genuine XP Tracker panel. Robot screen readback matched all
1,024,000 native canvas pixels. No XP callbacks were generated in this preflight.

The separate live attempt reached a real PostgreSQL-backed ClubScape HTTP
`Hello`, then correctly rejected account-only readiness. The final attempt used
the documented public `Config::with_game_root` constructor and hit the unchanged
strict server's descriptor-size rejection. **No live character, authoritative
movement/XP event, or plugin gain was demonstrated.** The offline scene is not
evidence that it reflected server state.

## Architecture and boundaries

* The composition root is `RuneLiteComposition`, **not `RuneLite.main` or the
  AppImage launcher**. Optional updater/cloud/session/Discord/plugin-hub startup
  is not reproduced or claimed. External RuneLite HTTP is disabled in-process.
* Original `client.init()` runs. The original startup thread stops at the
  existing `runelite.delaystart` semaphore before the OSRS tick/network pump.
  The adapter supplies the client-thread/state/render schedule; no upstream
  class/JAR is patched.
* The original startup canvas is retained. Adding a second canvas hid the
  rendered framebuffer behind the original black canvas; the real-screen
  assertion detected and prevented a false positive.
* `VerifiedCache`/`NativeScene` reuse the verified
  [`tools/source-capture`](../../tools/source-capture/README.md) loading/rendering
  paths without modifying that tool or source assets. The runtime uses
  `rl4.fn`, `ez.dh`, original player/model assembly, and source240's transient
  actor insertion/cleanup (`bj`/`bw`). It does not load reference PNGs.
* Both native local coordinates and path heads must reflect the authoritative
  tile. `WorldPoint` reads path heads, not just the render position. The bridge
  keeps X east/Y north, 128 units/tile, negative-up height, and 16384 scene-angle
  units/turn. The tested Tutorial ground is **-1360**, not zero.
* Source penguin NPC2063/model21547 is assembled at its original 75/128 scale,
  with source idle5668/walk5666. Only the offline idle rendering was observed.
  Equipment fitting and other action animations are not verified.
* `account.proto` and `game.proto` remain authoritative and unmodified.
  Checksum-pinned protoc3.21.12 generates Java-lite bindings against the official
  runtime's existing protobuf-javalite3.21.12. No handwritten protobuf parser
  or altered account-v1 tag is used.
* `WorldProjection` keeps full/delta entity baselines and deduplicates committed
  event IDs. It rejects history gaps and XP changes without matching server
  events. Only actual `xp_gained` events can produce `StatChanged`.
  `XpTrackerReadback` only observes the real plugin and configures its normal
  overlay; it never supplies or changes XP.
* `FirstXpJourney` is an unverified, bounded real-input driver for fresh signup
  through first fishing XP. It cannot seed XP/items/stages or advance ticks.
  It is not the independent full headless/browser M1 acceptance journey.
* `ShopSelection` binds a displayed row's canonical `item.id` to both buy and
  quote. `PendingWorldInput` retains exact operation bytes for explicit retries;
  a refresh cannot silently retarget a purchase. This does not implement native
  shop widgets or establish cross-process recovery.

## Original-assessment build and small checks

These default output names reproduce the original400ec9e assessment checkout.
On the updated canonical checkout, use the separately named outputs in the
[current reproduction](../../research/runelite-feasibility/canonical-3a74cbe.md)
instead; the packer intentionally rejects overwriting historical source inputs.

Read `AGENTS.md`, `prompt.md` Sections12/30/44, and
`docs/machines/sparky.md` first. The following JDK path was checked on the current
ARM64 host; it is not a universal host requirement.

```sh
JDK17=/home/lramos15/.local/share/jdks/temurin-17.0.20.1+1
python3 tools/runelite-compatibility/prepare.py --java-home "$JDK17"
python3 tools/runelite-compatibility/pack.py
python3 tools/runelite-compatibility/build.py --java-home "$JDK17"
python3 -m unittest discover -s tools/runelite-compatibility -p 'test_*.py' -q
env -u _JAVA_OPTIONS -u JAVA_TOOL_OPTIONS -u JDK_JAVA_OPTIONS \
  "$JDK17/bin/java" -ea -Xmx256m -XX:ActiveProcessorCount=2 \
  -Duser.home="$PWD/runelite/compatibility/artifacts/build-home" \
  -Djava.io.tmpdir="$PWD/runelite/compatibility/artifacts/build-home" \
  -cp "runelite/compatibility/artifacts/classes:$(cat runelite/compatibility/artifacts/classpath.txt)" \
  ProjectionChecks
python3 tools/runelite-compatibility/build-service.py
runelite/compatibility/artifacts/rust-target/debug/clubscape-runelite-service-probe \
  --inspect-artifact runelite/compatibility/artifacts/game/world.csc
python3 tools/runelite-compatibility/validate.py --java-home "$JDK17"
```

Preparation reuses only the pinned public source/library paths, or downloads
checksum-locked declared dependencies. It never scans personal RuneLite
profiles. All cache/JAR copies, generated sources, binaries, screenshots and
JVM state stay under ignored `runelite/compatibility/artifacts/`.

The **offline** preflight command used a unique evidence directory:

```sh
python3 tools/runelite-compatibility/run.py preflight \
  --name preflight-a5 --java-home "$JDK17"
```

Existing evidence directories are deliberately not overwritten. A separately
requested reproduction needs a new directory name. This command is not a live
integration attempt or permission to claim compatibility.

## Integration runner and stopping rule

The runner creates an owned, bounded, loopback PostgreSQL container, uses the
real shared service, checks readiness, runs the original runtime, and tears
down only its own processes/container. Passwords stay in memory/ignored
mode0600 files and are removed. Tokens are never logged. Java `user.home` and
scratch paths are explicitly isolated; inherited Jagex credentials and Java
option overrides are removed. Xvfb uses an owned abstract local display,
`-nolisten tcp -nolisten unix -nolock`, and an actual responsiveness check.
No system graphics/sandbox settings are changed.

The three numbered invocation reports retain actual commands/exits. Attempt1
failed in the harness before any service; it is conservatively charged to the
bound. Attempt2 exercised the documented environment launch. Attempt3 used
the small Rust `service-probe`: it calls the **existing public configuration
API and unchanged `Service`**, not another authority implementation.

After shared fixes and renewed explicit bounds, the runner's `--game-root` and
`--catalog` arguments select the current pack. It verifies current canonical
artifact identity and matching catalog/content revision before starting services.
Character-created/world-joined revisions must match that selected catalog too.

**Both allocations are exhausted: original1/2/3 and renewed4/5. Do not reset
either ledger, launch a sixth invocation, alter content/guards or start another
adapter architecture to evade this stop.** The two service prerequisites are
resolved. Future live work needs separately explicit bounds. No desktop deferral
or replacement native-client strategy has been approved.
