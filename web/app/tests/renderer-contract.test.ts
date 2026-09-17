import assert from "node:assert/strict";
import { test } from "node:test";
import { CanvasGpuClock } from "../gpu-clock.ts";
import { canonicalPick } from "../picking.ts";
import { sameOriginFetch } from "../source-fetch.ts";
import type { WorldView } from "../../shared/contracts.ts";
import type { Fetch } from "../transport.ts";
import type { RenderAssetManifest, RendererDiagnostics } from "../../renderer/src/index.ts";
import { residentRendererAssets, sourceScenePlacement } from "../render-state.ts";
import { presentationOptions } from "../presentation.ts";

test("real canvas/queue observation uses completion receipts, not raw Date timestamps or read counters", async () => {
  let complete: () => void = () => {};
  const receipt = new Promise<void>((resolve) => { complete = resolve; });
  const queue = { submit() {}, onSubmittedWorkDone: () => receipt } as unknown as GPUQueue;
  const context = { configure() {}, getCurrentTexture: () => ({}) } as unknown as GPUCanvasContext;
  const canvas = { getContext: () => context } as unknown as HTMLCanvasElement;
  const clock = new CanvasGpuClock(canvas);
  context.configure({ device: { queue } as GPUDevice, format: "bgra8unorm" });
  const raw = { sequence: 1, submittedAtMs: Date.now(), completedAtMs: Date.now(), drawCalls: 2, primitives: 12 };
  await assert.rejects(clock.completed(clock.mark(), raw), /no observed canvas acquisition/);
  const before = clock.mark();
  context.getCurrentTexture();
  queue.submit([]);
  let finished = false;
  const frame = clock.completed(before, raw).then((value) => { finished = true; return value; });
  await Promise.resolve();
  assert.equal(finished, false);
  complete();
  const value = await frame;
  assert(value.submittedAtMs >= 0 && value.completedAtMs <= performance.now() && value.completedAtMs >= value.submittedAtMs);
  assert(value.submittedAtMs < 1_000_000_000_000);
  assert.equal(value.sequence, raw.sequence);
  assert.equal(value.primitives, raw.primitives);
  clock.dispose();
});

test("two inflight world frames retain their own receipts despite late native results and interleaved offscreen previews", async () => {
  const receipts: Array<{ resolve(): void; reject(error: Error): void }> = [];
  const queue = { submit() {}, onSubmittedWorkDone: () => new Promise<void>((resolve, reject) => {
    receipts.push({ resolve, reject });
  }) } as unknown as GPUQueue;
  const context = { configure() {}, getCurrentTexture: () => ({}) } as unknown as GPUCanvasContext;
  const canvas = { getContext: () => context } as unknown as HTMLCanvasElement;
  const clock = new CanvasGpuClock(canvas);
  context.configure({ device: { queue } as GPUDevice, format: "bgra8unorm" });
  const first = clock.mark();
  queue.submit([]); // offscreen world fill precedes its canvas blit
  context.getCurrentTexture();
  queue.submit([]);
  const second = clock.mark();
  queue.submit([]); // a model-only preview has no game-canvas acquisition
  context.getCurrentTexture();
  queue.submit([]);
  queue.submit([]); // preview work after both world submissions
  assert.equal(receipts.length, 2);
  receipts[0]!.resolve();
  await Promise.resolve();
  receipts[1]!.resolve();
  const raw = { submittedAtMs: Date.now(), completedAtMs: Date.now(), drawCalls: 2, primitives: 12, gpuDurationMs: 0.75 };
  const secondResult = await clock.completed(second, { ...raw, sequence: 2 });
  const firstResult = await clock.completed(first, { ...raw, sequence: 1 });
  assert(firstResult.submittedAtMs <= secondResult.submittedAtMs);
  assert(firstResult.completedAtMs <= secondResult.completedAtMs);
  assert.equal(firstResult.gpuDurationMs, 0.75);
  await assert.rejects(clock.completed(first, { ...raw, sequence: 1 }), /no observed canvas acquisition/);
  await assert.rejects(clock.completed(clock.mark(), { ...raw, sequence: 3 }), /no observed canvas acquisition/);
  const third = clock.mark();
  context.getCurrentTexture();
  queue.submit([]);
  receipts[2]!.reject(new Error("Actual device loss"));
  await assert.rejects(clock.completed(third, { ...raw, sequence: 3 }), /Actual device loss/);
  clock.dispose();
});

test("picks preserve actual canonical targets and scenery footprints without guessing hashes or nearby actors", () => {
  const world = { player: { id: "actor.fixture" }, entities: [{ id: "spawn.fixture", kind: "npc" }] } as WorldView;
  const tile = { x: 3094, y: 3106, plane: 0 };
  assert.deepEqual(canonicalPick({ kind: "entity", id: "spawn.fixture", tile }, world), { kind: "entity", id: "spawn.fixture", tile });
  assert.equal(canonicalPick({ kind: "entity", id: "938398489", tile }, world), null);
  const scenery = { kind: "tile" as const, tile, scenery: { objectId: 9398, type: 0, spanX: 1, spanY: 2 } };
  assert.deepEqual(canonicalPick(scenery, world), scenery);
  assert.equal(canonicalPick({ kind: "tile", tile: { ...tile, plane: 4 } }, world), null);
});

test("stream residency comes from actual loaded squares, not append-only fetch history", () => {
  const manifest = {
    scenes: [{ name: "fixture", file: "scenes/a.bin", file_gz: "scenes/a.bin.gz" }],
    blocks: [
      { square: 12336, file: "blocks/12336.bin", file_gz: "blocks/12336.bin.gz" },
      { square: 12592, file: "blocks/12592.bin", file_gz: "blocks/12592.bin.gz" },
    ],
    minimap_blocks: [{ square: 12336, file: "minimap/blocks/12336.bin" }, { square: 12592, file: "minimap/blocks/12592.bin" }],
  } as RenderAssetManifest;
  const state = {
    sceneId: "blocks@3048,3056", loadedSquares: [12592],
    assets: ["palette.bin", "scenes/a.bin.gz", "blocks/12336.bin.gz", "blocks/12592.bin.gz", "blocks/12592.bin.gz",
      "minimap/blocks/12336.bin", "minimap/blocks/12592.bin"]
      .map((id) => ({ id, sha256: "a".repeat(64), loaded: true })),
  } as RendererDiagnostics;
  const result = residentRendererAssets(manifest, state);
  assert.equal(result.length, 6);
  assert.deepEqual(result.map((asset) => asset.loaded), [true, false, false, true, false, true]);
  assert.equal(state.assets[2]!.loaded, true, "native history was not mutated");
});

test("the legacy placement flag is adapted only when actual completed block-assembly diagnostics agree", () => {
  const state = { sceneBase: { x: 3048, y: 3056 }, sceneId: "blocks@3048,3056", loadedSquares: [12336] } as RendererDiagnostics;
  const raw = { baseX: 3048, baseY: 3056, sizeTiles: 104, blocks: false };
  assert.deepEqual(sourceScenePlacement(state, raw), { ...raw, blocks: true });
  assert.equal(raw.blocks, false, "the native observation remains intact");
  assert.throws(() => sourceScenePlacement({ ...state, loadedSquares: [] }, raw), /disagrees/);
  assert.throws(() => sourceScenePlacement(state, { ...raw, baseX: 3056 }), /disagrees/);
  assert.deepEqual(sourceScenePlacement({ ...state, sceneId: `${state.sceneId}#instance_template.death.office` }, raw,
    "instance_template.death.office"), { ...raw, blocks: true });
  assert.throws(() => sourceScenePlacement({ ...state, sceneId: `${state.sceneId}#instance_template.other` }, raw,
    "instance_template.death.office"), /disagrees/);
  assert.equal(sourceScenePlacement({ ...state, sceneBase: null, sceneId: "fixture" }, raw), raw);
});

test("all actual component fetches retain same-origin/redirect/credential policy", async () => {
  const policy = sameOriginFetch((async (_input, init) => {
    assert.equal(init?.redirect, "error");
    assert.equal(init?.mode, "same-origin");
    assert.equal(init?.credentials, "omit");
    return new Response("ok");
  }) as Fetch, "http://127.0.0.1:4010");
  await policy("/assets/compiled/render/palette.bin", { redirect: "follow", credentials: "include" });
  await assert.rejects(policy("https://outside.example/asset.bin"), /same-origin/);
  await assert.rejects(policy("/assets/palette.bin?token=private"), /secrets/);
});

test("recorded cameras over streamed source regions are explicit diagnostics, not URL secrets or fixture fallbacks", () => {
  assert.deepEqual(presentationOptions(""), { earlyScene: null, recordedCamera: null });
  assert.deepEqual(presentationOptions("?presentation_camera=tutorial-starting-house"),
    { earlyScene: null, recordedCamera: "tutorial-starting-house" });
  assert.throws(() => presentationOptions("?presentation_scene=one&presentation_camera=two"), /not both/);
  assert.throws(() => presentationOptions("?token=private"), /never credentials or tokens/);
  assert.throws(() => presentationOptions("?presentation_scene=one&presentation_scene=two"), /explicit named source/);
  assert.throws(() => presentationOptions("?presentation_camera=https://outside.example/"), /explicit named source/);
});
