import assert from "node:assert/strict";
import { test } from "node:test";
import type { MinimapSurface } from "../../renderer/src/index.ts";
import { MinimapRelay } from "../minimap.ts";

function surface(): MinimapSurface {
  return {
    width: 512, height: 512, scale: 4, marginX: 48, marginY: 48, baseX: 3056, baseY: 3056,
    plane: 0, revision: 1, complete: false, notes: ["source sidecar is incomplete"],
    stats: { terrainTiles: 10, wallMarks: 2, diagonalMarks: 1, mapScenes: 3, unresolved: 1 },
    icons: [{ x: 12, y: 13, plane: 0, element: 456 }],
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
