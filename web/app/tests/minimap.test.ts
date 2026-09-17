import assert from "node:assert/strict";
import { test } from "node:test";
import type { MapIconSprite, MinimapSurface } from "../../renderer/src/index.ts";
import { MinimapRelay } from "../minimap.ts";

function surface(): MinimapSurface {
  return {
    width: 512, height: 512, scale: 4, marginX: 48, marginY: 48, baseX: 3056, baseY: 3056,
    plane: 0, revision: 1, complete: false, notes: ["source sidecar is incomplete"],
    stats: { terrainTiles: 10, wallMarks: 2, diagonalMarks: 1, mapScenes: 3, unresolved: 1 },
    icons: [{ x: 12, y: 13, plane: 0, element: 456 }],
    sourceIconMismatches: null,
    pixels: { width: 512, height: 512, data: new Uint8ClampedArray(512 * 512 * 4) } as ImageData,
    mask: new Uint8Array(512 * 512),
  };
}

test("a real minimap surface is delivered once per renderer revision without rescaling or replacing icon IDs", () => {
  const sent: MinimapSurface[] = [];
  const relay = new MinimapRelay((value) => sent.push(value), () => {});
  const value = surface();
  relay.update(value, "device-a");
  relay.update(value, "device-a");
  assert.equal(sent.length, 1);
  assert.equal(sent[0], value);
  assert.equal(relay.observe()?.icons[0]?.element, 456);
  assert.equal(relay.observe()?.complete, false);
  assert.equal(relay.observe()?.fullSurfaceFidelityAccepted, false);
  relay.update({ ...value, revision: 2 }, "device-a");
  relay.update({ ...value, revision: 2 }, "device-b");
  assert.equal(sent.length, 3);
  assert(Object.isFrozen(relay.observe()?.stats));
});

test("original icon pixels and offsets are relayed independently without guessing projection units", () => {
  const delivered: ReadonlyMap<number, MapIconSprite>[] = [], rasters: MinimapSurface[] = [];
  const pixels = { width: 2, height: 3, data: new Uint8ClampedArray(24) } as ImageData;
  const sprites = new Map<number, MapIconSprite>([[456, {
    element: 456, width: 2, height: 3, offsetX: 1, offsetY: 2,
    maxWidth: 4, maxHeight: 6, category: 7, pixels,
  }]]);
  const relay = new MinimapRelay((value) => rasters.push(value), (error) => { throw error; },
    (value) => delivered.push(value));
  relay.update(surface(), "one");
  relay.update(surface(), "one", sprites);
  relay.update(surface(), "one", sprites);
  assert.equal(rasters.length, 1, "Loading icon sprites does not redraw an unchanged native map.");
  assert.equal(delivered.length, 1);
  assert.notEqual(delivered[0], sprites, "UI map mutations cannot evict renderer-owned sprite entries.");
  assert.equal(delivered[0]!.get(456)!.pixels, pixels);
  assert.equal(delivered[0]!.get(456)!.offsetX, 1);
  assert.equal(relay.observe()?.sourceIconMismatches, null);
  assert.deepEqual(relay.observe()?.iconSprites, { available: true, count: 1, bytes: 24, delivered: true });
  assert.equal(relay.observe()?.iconProjection, "native-helper-available-ui-unbound");
  assert.equal(relay.observe()?.fullSurfaceFidelityAccepted, false);
});

test("decoded icon sprites with no real UI sink are explicit, not counted as drawn icons", () => {
  const errors: string[] = [];
  const relay = new MinimapRelay(() => {}, (error) => errors.push(error.message));
  const sprites = new Map<number, MapIconSprite>([[456, {
    element: 456, width: 0, height: 0, offsetX: 0, offsetY: 0, maxWidth: 0, maxHeight: 0, category: 0,
    pixels: { width: 1, height: 1, data: new Uint8ClampedArray(4) } as ImageData,
  }]]);
  relay.update(surface(), "one", sprites);
  assert.equal(relay.observe()?.iconSprites.delivered, false);
  assert.equal(relay.observe()?.iconSprites.available, true);
  assert.equal(errors.length, 1);
  assert.match(errors[0]!, /No world128 radius or clipping rule was guessed/);
  assert.throws(() => relay.update({ ...surface(), sourceIconMismatches: -1 }, "one", sprites), /invalid native minimap/);
});

test("a missing UI sink is explicit and never promotes static minimap images or geometry completeness", () => {
  const errors: string[] = [];
  const relay = new MinimapRelay(null, (error) => errors.push(error.message));
  relay.update(surface(), "device");
  relay.update({ ...surface(), revision: 2 }, "device");
  assert.equal(errors.length, 1);
  assert.match(errors[0]!, /no relayed live-surface\/icon setter/);
  assert.equal(relay.observe()?.delivered, false);
  assert.throws(() => relay.update({ ...surface(), scale: 2 }, "device"), /invalid native minimap/);
});
