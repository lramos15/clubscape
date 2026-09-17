import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { test } from "node:test";
import { CameraSourceMetadata, NativeCameraSession, cameraNanoseconds, cameraWheelRotation } from "../camera.ts";
import type { NativeCameraRuntime } from "../camera.ts";
import { AssetLoader } from "../assets.ts";
import type { ContentManifest } from "../manifest.ts";
import { AppError } from "../errors.ts";
import type { RenderCamera, WorldView } from "../../shared/contracts.ts";
import { SourceSceneStream } from "../../renderer/src/camera.ts";
import type { CameraSourceSample, NativeCameraRenderer } from "../../renderer/src/camera.ts";

const world = () => ({
  revision: "9007199254740993", tick: "9007199254740993",
  player: { id: "actor.consumer-fixture", region: "region.fixture", instance: null, tile: { x: 3215, y: 3218, plane: 0 } },
} as WorldView);
const camera: RenderCamera = { x: 1, y: 2, height: -3, pitch: 2048, yaw: 0, zoom: 662, near: 50, far: 3500, unitsPerTurn: 16384 };
const sample = (world: WorldView, generation = "1"): CameraSourceSample => ({
  context: { actorId: world.player.id, region: world.player.region, instance: world.player.instance,
    revision: world.revision, tick: world.tick, sceneId: "fixture-scene", sceneGeneration: generation },
  renderedActor: { actorId: world.player.id, local: [6080, 6464], plane: 0, sizeTiles: 1 },
  focus: { identity: 42, world_base: { x: 3168, y: 3168 }, logical: [6080, 6464], rendered: [6080, 6464], plane: 0, footprint: 0, kind: "Actor" },
  effects: { shake: [null, null, null, null, null], suppress_jitter: false }, missing: [],
});

// Routing/lifecycle double only. Camera math is tested by the separate actual-WASM suite.
class RuntimeFixture implements NativeCameraRuntime {
  calls: Array<{ method: string; args: unknown[] }> = [];
  bind(...args: Parameters<NativeCameraRuntime["bind"]>): string { this.calls.push({ method: "bind", args }); return "native-delivery"; }
  input(...args: Parameters<NativeCameraRuntime["input"]>): void { this.calls.push({ method: "input", args }); }
  frame(...args: Parameters<NativeCameraRuntime["frame"]>): string { this.calls.push({ method: "frame", args }); return "native-delivery"; }
  resize(...args: Parameters<NativeCameraRuntime["resize"]>): void { this.calls.push({ method: "resize", args }); }
  face_yaw(...args: Parameters<NativeCameraRuntime["face_yaw"]>): void { this.calls.push({ method: "yaw", args }); }
  suspend(): void { this.calls.push({ method: "suspend", args: [] }); }
  free(): void { this.calls.push({ method: "free", args: [] }); }
}

function fixture(definitions: (scene: string) => Promise<string> = async () => "[]") {
  const current = { world: world() as WorldView | null, sample: sample(world()), loaded: true };
  const runtimes: RuntimeFixture[] = [];
  const applied: string[] = [];
  const renderer: NativeCameraRenderer = {
    cameraSceneReady: () => current.loaded,
    cameraSource: () => current.sample,
    cameraScene: () => "actual-renderer-scene-JSON-fixture",
    applyNativeCamera(value) { applied.push(value); return { ...camera }; },
  };
  const size = { width: 1920, height: 1080 };
  const session = new NativeCameraSession(renderer, () => {
    const runtime = new RuntimeFixture(); runtimes.push(runtime); return runtime;
  }, definitions, () => current.world, () => size, () => 123.456);
  return { current, runtimes, applied, size, session, renderer };
}

test("camera initialization uses real ABI calls only after source loading and never publishes premature readiness", async () => {
  const ready = Promise.withResolvers<string>();
  const f = fixture(() => ready.promise);
  const preparing = f.session.prepare(f.current.world!);
  assert.equal(f.session.ready(), false);
  assert.equal(f.applied.length, 0);
  f.size.width = 2560;
  ready.resolve("[original-definitions-fixture]");
  assert.deepEqual(await preparing, camera);
  assert.equal(f.session.ready(), true);
  const call = f.runtimes[0]!.calls.find((c) => c.method === "bind")!;
  assert.deepEqual(call.args.slice(2), ["[original-definitions-fixture]", "TutorialStartingHouse", 2560, 1080, 123456000n]);
  assert.deepEqual(f.session.frame(140, f.current.world!), camera);
  assert.equal(f.runtimes[0]!.calls.at(-1)?.args[0], 140000000n);
  f.session.resize(1024, 768);
  assert.deepEqual(f.runtimes[0]!.calls.at(-1), { method: "resize", args: [1024, 768] });
  f.session.dispose();
});

test("missing native producer fields block entry before any terrain substitute or runtime bind", async () => {
  const f = fixture(async () => { throw new Error("must not fetch after missing focus"); });
  f.current.sample.focus = null;
  f.current.sample.effects = null;
  f.current.sample.missing = ["actual native logical/rendered focus and source effects are absent"];
  await assert.rejects(f.session.prepare(f.current.world!), (error: unknown) =>
    error instanceof AppError && error.kind === "camera_unavailable" && error.message.includes("No tile-centre"));
  assert.equal(f.session.ready(), false);
  assert.equal(f.runtimes[0]!.calls.some((c) => c.method === "bind"), false);
  assert.deepEqual(f.applied, []);
  f.session.dispose();
});

test("disconnect, actor replacement and teardown cancel old async camera initialization", async () => {
  for (const cancel of ["disconnect", "actor", "dispose"] as const) {
    const loaded = Promise.withResolvers<string>(), f = fixture(() => loaded.promise);
    const preparing = f.session.prepare(f.current.world!);
    if (cancel === "dispose") f.session.dispose();
    else if (cancel === "disconnect") { f.current.world = null; f.session.suspend(); }
    else f.current.world = { ...world(), player: { ...world().player, id: "actor.other" } };
    loaded.resolve("[]");
    await assert.rejects(preparing, (error: unknown) => error instanceof AppError && error.kind === "cancelled");
    assert.deepEqual(f.applied, []);
    assert(!f.runtimes[0]!.calls.some((c) => c.method === "bind"));
    assert(!f.session.ready());
    f.session.dispose();
    assert.equal(f.runtimes[0]!.calls.filter((c) => c.method === "free").length, 1);
  }
});

test("streaming and source-generation changes fence frames and reuse the same persistent native controller", async () => {
  const f = fixture();
  await f.session.prepare(f.current.world!);
  f.current.loaded = false;
  assert.equal(f.session.frame(140, f.current.world!), null);
  assert(!f.session.ready());
  f.current.loaded = true;
  f.current.sample = sample(f.current.world!, "2");
  await f.session.prepare(f.current.world!);
  assert.equal(f.runtimes.length, 1, "rebase/reconnect must not replace native yaw/inertia state");
  assert.equal(f.runtimes[0]!.calls.filter((c) => c.method === "bind").length, 2);
  f.current.sample.context.revision = "7";
  assert.throws(() => f.session.frame(160, f.current.world!), /stale actor/);
  assert(!f.session.ready());
  f.session.reset();
  assert.equal(f.runtimes.length, 2, "explicit session end resets, ordinary scene shifts do not");
  f.session.dispose();
});

test("a source generation changed during metadata loading is cancelled, not initialized or treated as a fatal scene failure", async () => {
  const loaded = Promise.withResolvers<string>(), f = fixture(() => loaded.promise);
  const preparing = f.session.prepare(f.current.world!);
  f.current.sample = sample(f.current.world!, "2");
  loaded.resolve("[]");
  await assert.rejects(preparing, (error: unknown) => error instanceof AppError && error.kind === "cancelled" && error.recoverable);
  assert(!f.session.ready());
  assert(!f.runtimes[0]!.calls.some((c) => c.method === "bind"));
  await f.session.prepare(f.current.world!);
  assert(f.session.ready());
  f.session.dispose();
});

test("fixture publication cannot set ready after an owner change inside the renderer boundary", async () => {
  const f = fixture();
  const apply = f.renderer.applyNativeCamera;
  f.renderer.applyNativeCamera = (delivery) => {
    const result = apply(delivery);
    f.current.world = null;
    f.session.suspend();
    return result;
  };
  await assert.rejects(f.session.prepare(f.current.world!), (error: unknown) => error instanceof AppError && error.kind === "cancelled");
  assert(!f.session.ready());
  f.session.dispose();
});

test("source assembly coalesces newer layouts without publishing stale scenes or premature ready flags", async () => {
  const loads = new Map<number, ReturnType<typeof Promise.withResolvers<string>>>();
  const firstStarted = Promise.withResolvers<void>(), secondStarted = Promise.withResolvers<void>();
  const committed: Array<[number, string]> = [];
  const stream = new SourceSceneStream((target: number) => String(target), async (target) => {
    const load = Promise.withResolvers<string>();
    loads.set(target, load);
    (target === 1 ? firstStarted : secondStarted).resolve();
    return load.promise;
  }, (target, result) => { committed.push([target, result]); });
  const first = stream.request(1);
  await firstStarted.promise;
  const second = stream.request(2);
  assert.equal(first, second);
  assert(stream.busy);
  loads.get(1)!.resolve("old");
  await secondStarted.promise;
  assert.deepEqual(committed, []);
  loads.get(2)!.resolve("new");
  await second;
  assert.deepEqual(committed, [[2, "new"]]);
  assert(!stream.busy);
  stream.requireReady();
});

test("cancelled and failed scene assemblies cannot masquerade as a completed source scene", async () => {
  const load = Promise.withResolvers<number>();
  let commits = 0;
  const stream = new SourceSceneStream(String, () => load.promise, () => { commits++; });
  const pending = stream.request(1);
  stream.cancel();
  load.resolve(1);
  await pending;
  assert.equal(commits, 0);
  const failed = new SourceSceneStream(String, () => { throw new Error("original block missing"); }, () => { commits++; });
  await assert.rejects(failed.request(2), /original block missing/);
  assert(!failed.busy);
  assert.throws(() => failed.requireReady(), /original block missing/);
  assert.equal(commits, 0);
});

test("fixture metadata transport enforces id/hash/raise without proving production asset availability", async () => {
  const bytes = new TextEncoder().encode('{"id":941,"raise":7}');
  const id = "asset.source.osrs.cache2695.object.941";
  const assets = new AssetLoader({ assets: [{ id, url: "/assets/object-941.json", bytes: bytes.length,
    sha256: createHash("sha256").update(bytes).digest("hex"), contentType: "application/json" }] } as ContentManifest, "fixture", {
    fetch: async () => new Response(bytes, { headers: { "content-type": "application/json" } }),
  });
  const metadata = new CameraSourceMetadata(assets);
  assert.equal(await metadata.definitions('{"surfaces":[{"decoration":941},{"decoration":null},null]}'), '[{"id":941,"raise":7}]');
  assert.deepEqual(metadata.requiredAssets, [id]);
  await assert.rejects(metadata.definitions('{"surfaces":[{"decoration":942}]}'), /object 942.*original id\/raise/);
  assets.dispose();
});

test("camera timestamps and signed DOM event normalization are transport, not FOV or rate mechanics", () => {
  assert.equal(cameraNanoseconds(123.456), 123456000n);
  for (const invalid of [-1, NaN, Infinity, Number.MAX_SAFE_INTEGER]) assert.throws(() => cameraNanoseconds(invalid));
  assert.throws(() => cameraNanoseconds(Number.MAX_SAFE_INTEGER / 1_000_000));
  for (const mode of [0, 1, 2]) {
    assert.equal(cameraWheelRotation(123, mode), 1);
    assert.equal(cameraWheelRotation(-0.25, mode), -1);
    assert.equal(cameraWheelRotation(0, mode), 0);
  }
  assert.throws(() => cameraWheelRotation(Infinity, 0));
  assert.throws(() => cameraWheelRotation(1, 7));
});
