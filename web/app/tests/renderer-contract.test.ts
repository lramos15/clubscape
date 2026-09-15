import assert from "node:assert/strict";
import { test } from "node:test";
import { CanvasGpuClock } from "../gpu-clock.ts";
import { canonicalPick } from "../picking.ts";
import { sameOriginFetch } from "../source-fetch.ts";
import type { WorldView } from "../../shared/contracts.ts";
import type { Fetch } from "../transport.ts";

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

test("picks preserve actual canonical targets and never convert unmapped scenery hashes to actions", () => {
  const world = { player: { id: "actor.fixture" }, entities: [{ id: "spawn.fixture", kind: "npc" }] } as WorldView;
  const tile = { x: 3094, y: 3106, plane: 0 };
  let hash = 0x200000000000n;
  for (const byte of new TextEncoder().encode("spawn.fixture")) hash = BigInt.asIntN(64, hash * 31n + BigInt(byte));
  assert.deepEqual(canonicalPick({ kind: "entity", id: hash.toString(), tile }, world), { kind: "entity", id: "spawn.fixture", tile });
  assert.equal(canonicalPick({ kind: "entity", id: "938398489", tile }, world), null);
  assert.deepEqual(canonicalPick({ kind: "tile", tile }, world), { kind: "tile", tile });
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
