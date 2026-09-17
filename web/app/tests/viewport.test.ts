import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { fullHudViewport, sourceZoomForViewportHeight } from "../../renderer/src/index.ts";
import { resizeFullHud } from "../viewport.ts";

test("composed HUD projection matches every original viewport sample, without reusing the viewport-only curve", async () => {
  const table = JSON.parse(await readFile(new URL("../../../assets/compiled/render/hud/zoom-table.json", import.meta.url), "utf8")) as {
    samples: Array<{ canvas: [number, number]; viewport: [number, number]; zoom: number }>;
  };
  assert.equal(table.samples.length, 16);
  for (const sample of table.samples) {
    const [width, height] = sample.canvas;
    assert.deepEqual(fullHudViewport(width, height),
      { x: 0, y: 0, width: sample.viewport[0], height: sample.viewport[1], zoom: sample.zoom });
  }
  assert.equal(fullHudViewport(1920, 1080).zoom, 410);
  assert.equal(fullHudViewport(1024, 768).zoom, 292);
  assert.equal(fullHudViewport(2560, 1440).zoom, 547);
  assert.equal(sourceZoomForViewportHeight(1080), 662, "The diagnostic helper is not the normal controller, even when its constructor FOV has the same numerical result.");
});

test("normal resize sends actual backing dimensions to Rust without running the controlled full-HUD FOV helper", () => {
  const calls: Array<[string, ...number[]]> = [];
  const apply = {
    nativeCamera: (width: number, height: number) => { calls.push(["native", width, height]); },
    world: (width: number, height: number) => { calls.push(["world", width, height]); },
    camera: (_zoom: number) => { throw new Error("fixture zoom must never reach normal entry"); },
    ui: (width: number, height: number) => { calls.push(["ui", width, height]); },
    observe: (width: number, height: number, scale: number) => { calls.push(["observe", width, height, scale]); },
  };
  const first = resizeFullHud(null, 1920, 1080, 2, apply, "native-controller");
  assert.deepEqual(calls, [["native", 3840, 2160], ["world", 3840, 2160], ["ui", 1920, 1080], ["observe", 1920, 1080, 2]]);
  assert.equal(first.zoom, null, "only an actual native frame supplies its projection");
  assert.equal(resizeFullHud(first, 1920, 1080, 2, apply, "native-controller"), first);
  assert.equal(calls.length, 4);
  assert.throws(() => resizeFullHud(first, 32768, 1080, 1, apply, "native-controller"), /unsupported/);
});

test("duplicate ResizeObserver/window notifications send each actual resize and camera update once", () => {
  const calls: Array<[string, ...number[]]> = [];
  const apply = {
    world: (width: number, height: number) => { calls.push(["world", width, height]); },
    camera: (zoom: number) => { calls.push(["camera", zoom]); },
    ui: (width: number, height: number) => { calls.push(["ui", width, height]); },
    observe: (width: number, height: number, scale: number) => { calls.push(["observe", width, height, scale]); },
  };
  const first = resizeFullHud(null, 1920, 1080, 1, apply);
  assert.deepEqual(calls, [["world", 1920, 1080], ["camera", 410], ["ui", 1920, 1080], ["observe", 1920, 1080, 1]]);
  assert.equal(resizeFullHud(first, 1920, 1080, 1, apply), first);
  assert.equal(calls.length, 4);
  const wider = resizeFullHud(first, 2560, 1080, 1, apply);
  assert.deepEqual(calls.slice(4), [["world", 2560, 1080], ["camera", 410], ["ui", 2560, 1080], ["observe", 2560, 1080, 1]]);
  const highDpi = resizeFullHud(wider, 2560, 1080, 2, apply);
  assert.equal(highDpi.width, 5120);
  assert.equal(highDpi.zoom, fullHudViewport(5120, 2160).zoom);
  assert.deepEqual(calls.slice(8).map((call) => call[0]), ["world", "camera", "observe"]);
  assert(Object.isFrozen(highDpi));
  assert.throws(() => resizeFullHud(highDpi, 0, 1080, 1, apply), /invalid/);
  assert.throws(() => resizeFullHud(highDpi, 1920, 1080, NaN, apply), /invalid/);
  assert.throws(() => resizeFullHud(highDpi, 1, 1, 1, apply), /unsupported native/);
});
