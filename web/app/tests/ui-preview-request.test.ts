import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import type { UiPreviewRequest } from "../../ui/index.ts";
import type { RenderAssetManifest } from "../../renderer/src/index.ts";
import { nativeUiPreviewRequest, PlayerModelPreviewProducer } from "../ui-preview-request.ts";
import type { PreviewWorld } from "../ui-preview-request.ts";
import { AppError } from "../errors.ts";

// Metadata adapter fixtures, not a rendered game or an authoritative UI projection.
function fixture() {
  const request: UiPreviewRequest = {
    purpose: "appearance", bounds: { x: 100, y: 200, width: 480, height: 315 },
    modelBounds: { x: 272, y: 291, width: 136, height: 192 },
    sourceWidget: 44499017, modelZoom: 450, modelRotation: [0, 0, 0],
    appearance: { body_type: 0 }, equipment: [],
    base: { asset: "asset.source.osrs.cache2695.npc.2063", sourceNpc: 2063,
      adaptation: "milestones/approvals/m1-reference-pack-v1.3.0.json" },
  };
  const world: PreviewWorld = { revision: "9007199254740993", tick: "9007199254740994",
    player: { id: "actor.preview.fixture", region: "region.osrs.12336", instance: null,
      appearance: { ...request.appearance }, equipment: [] },
    ui: { version: 1, appearance: { base: request.base } } };
  return { request, world };
}
async function manifest(): Promise<RenderAssetManifest> {
  return JSON.parse(await readFile(new URL("../../../assets/compiled/render/manifest.json", import.meta.url), "utf8")) as RenderAssetManifest;
}

test("preview adaptation retains exact source widget parameters and relative native coordinates", async () => {
  const { request, world } = fixture();
  const before = JSON.stringify({ request, world });
  assert.deepEqual(nativeUiPreviewRequest(request, world, await manifest()), {
    width: 480, height: 315, centerX: 240, centerY: 187,
    modelZoom: 450, rotationX: 0, rotationY: 0, rotationZ: 0,
    contentType: 328, rasterizerZoom: 512, offsetX: 0, offsetY: 175,
  });
  assert.equal(JSON.stringify({ request, world }), before);
});

test("unavailable base/loadout, unsupported widget or draft appearance never becomes a dummy world update", async () => {
  const { request, world } = fixture();
  const source = await manifest();
  const actual = nativeUiPreviewRequest(request, world, source);
  assert.deepEqual(nativeUiPreviewRequest({ ...request, base: null, equipment: null }, world, source), actual,
    "Real world metadata supplies missing UI bindings; no fallback loadout or fake actor is created.");
  assert.throws(() => nativeUiPreviewRequest(request, { ...world, ui: { version: 1, appearance: { base: null } } }, source),
    (error: unknown) => error instanceof AppError && error.errorId === "preview.base_metadata_required");
  assert.throws(() => nativeUiPreviewRequest(request, null, source),
    (error: unknown) => error instanceof AppError && error.errorId === "preview.actor_context_required");
  assert.throws(() => nativeUiPreviewRequest({ ...request, sourceWidget: 123 }, world, source), /no published native model/);
  assert.throws(() => nativeUiPreviewRequest({ ...request, appearance: { body_type: 1 } }, world, source), /local approved appearance/);
  assert.deepEqual(world.player.appearance, { body_type: 0 });
});

test("source body/pose and equipment identities are checked without replacing supplied UI data", async () => {
  const { request, world } = fixture();
  const source = await manifest();
  assert.throws(() => nativeUiPreviewRequest({ ...request, base: { ...request.base!, sourceNpc: 1 } }, world, source),
    (error: unknown) => error instanceof AppError && error.errorId === "preview.base_binding_mismatch");
  const missingModel = structuredClone(source);
  delete missingModel.files[source.npc_definitions!.find(npc => npc.npc_id === 2063)!.base_model];
  assert.throws(() => nativeUiPreviewRequest(request, world, missingModel), /body\/reference\/pose/);
  const equipped: PreviewWorld = { ...world, player: { ...world.player, equipment: [{
    slot: "equipment.weapon", item: { id: "item.fixture.unpublished", name: "Fixture", sourceId: 0xffffffff,
      quantity: 1, iconAsset: null, instanceId: "instance.fixture", charges: null, actions: [] },
  }] } };
  assert.throws(() => nativeUiPreviewRequest({ ...request, equipment: null }, equipped, source),
    (error: unknown) => error instanceof AppError && error.errorId === "preview.equipment_model_required");
  assert.equal(equipped.player.equipment[0]!.item!.instanceId, "instance.fixture");
});

test("only an accepted matching actor can supply preview metadata; clear and failed updates cannot leak it", async () => {
  const { request, world } = fixture();
  let frames = 0;
  const producer = new PlayerModelPreviewProducer(await manifest(), async () => { frames++; return null; });
  await assert.rejects(producer.frame(request, world), /No current accepted actor/);
  producer.accept(world, () => {});
  await producer.frame({ ...request, base: null, equipment: null }, world);
  assert.equal(frames, 1);
  const mutable = structuredClone(world);
  producer.accept(mutable, () => {});
  mutable.player.appearance.body_type = 1;
  await assert.rejects(producer.frame(request, mutable), /accepted player snapshot/);
  const changedInstance: PreviewWorld = { ...world, player: { ...world.player, instance: "instance.other" } };
  await assert.rejects(producer.frame(request, changedInstance), /accepted player snapshot/);
  const next: PreviewWorld = { ...world, player: { ...world.player, id: "actor.other.fixture" } };
  assert.throws(() => producer.accept(next, () => { throw new Error("Native update rejected"); }), /Native update rejected/);
  await assert.rejects(producer.frame(request, next),
    (error: unknown) => error instanceof AppError && error.errorId === "preview.actor_context_mismatch");
  assert.equal(frames, 1);
  producer.clear();
  await assert.rejects(producer.frame(request, world), /No current accepted actor/);
  producer.accept(next, () => {});
  await producer.frame(request, next);
  assert.equal(frames, 2);
  world.player.appearance.body_type = 1;
  await assert.rejects(producer.frame(request, world), /accepted player snapshot/);
});

test("pending preview completions are fenced by accepted model changes and same-actor re-entry", async () => {
  const source = await manifest();
  for (const change of ["actor", "appearance", "equipment", "instance", "base", "reentry"] as const) {
    const { request, world } = fixture();
    let complete: (image: ImageData | null) => void = () => {};
    const producer = new PlayerModelPreviewProducer(source, () => new Promise(resolve => { complete = resolve; }));
    producer.accept(world, () => {});
    const result = assert.rejects(producer.frame(request, world),
      (error: unknown) => error instanceof AppError && error.kind === "cancelled"
        && error.errorId === "preview.readback_superseded", change);
    const next = structuredClone(world);
    switch (change) {
      case "actor": next.player.id = "actor.other.fixture"; break;
      case "appearance": next.player.appearance.body_type = 1; break;
      case "equipment": next.player.equipment.push({ slot: "slot.weapon", item: null }); break;
      case "instance": next.player.instance = "instance.other.fixture"; break;
      case "base": next.ui!.appearance.base = null; break;
      case "reentry": producer.clear(); break;
    }
    producer.accept(next, () => {});
    complete(null);
    await result;
  }
});

test("passive ticks and rejected native updates do not invalidate an unchanged pending preview", async () => {
  const { request, world } = fixture();
  let complete: (image: ImageData | null) => void = () => {};
  const producer = new PlayerModelPreviewProducer(await manifest(), () => new Promise(resolve => { complete = resolve; }));
  producer.accept(world, () => {});
  const result = producer.frame(request, world);
  producer.accept({ ...world, revision: "9007199254740994", tick: "9007199254740995" }, () => {});
  const rejected = { ...world, player: { ...world.player, id: "actor.rejected.fixture" } };
  assert.throws(() => producer.accept(rejected, () => { throw new Error("Native update rejected"); }), /Native update rejected/);
  complete(null);
  assert.equal(await result, null);
});

test("a stale native preview failure cannot become an error for the newly accepted actor", async () => {
  const { request, world } = fixture();
  let reject: (error: Error) => void = () => {};
  const producer = new PlayerModelPreviewProducer(await manifest(), () => new Promise((_resolve, failure) => { reject = failure; }));
  producer.accept(world, () => {});
  const result = assert.rejects(producer.frame(request, world),
    (error: unknown) => error instanceof AppError && error.errorId === "preview.readback_superseded");
  producer.accept({ ...world, player: { ...world.player, id: "actor.other.fixture" } }, () => {});
  reject(new Error("Old native readback failure"));
  await result;
});
