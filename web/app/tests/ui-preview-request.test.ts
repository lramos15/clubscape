import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import type { UiPreviewRequest } from "../../ui/index.ts";
import type { WorldView } from "../../shared/contracts.ts";
import type { RenderAssetManifest } from "../../renderer/src/index.ts";
import { nativeUiPreviewRequest } from "../ui-preview-request.ts";

// Metadata adapter fixtures, not a rendered game or an authoritative UI projection.
function fixture() {
  const request: UiPreviewRequest = {
    purpose: "appearance", bounds: { x: 100, y: 200, width: 480, height: 315 },
    modelBounds: { x: 272, y: 291, width: 136, height: 192 },
    sourceWidget: 44499017, modelZoom: 450, modelRotation: [0, 0, 0],
    appearance: { body_type: 0 }, equipment: [],
    base: { asset: "asset.fixture.base", sourceNpc: 2063, adaptation: "adaptation.fixture" },
  };
  const world = { player: { appearance: request.appearance, equipment: request.equipment },
    ui: { appearance: { base: request.base } } } as unknown as WorldView;
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
  assert.throws(() => nativeUiPreviewRequest({ ...request, base: null }, world, source), /unavailable/);
  assert.throws(() => nativeUiPreviewRequest({ ...request, equipment: null }, world, source), /unavailable/);
  assert.throws(() => nativeUiPreviewRequest(request, null, source), /unavailable/);
  assert.throws(() => nativeUiPreviewRequest({ ...request, sourceWidget: 123 }, world, source), /no published native model/);
  assert.throws(() => nativeUiPreviewRequest({ ...request, appearance: { body_type: 1 } }, world, source), /local approved appearance/);
  assert.deepEqual(world.player.appearance, { body_type: 0 });
});
