import assert from "node:assert/strict";
import { test } from "node:test";
import { ModelPreview } from "../preview.ts";
import type { PreviewBounds } from "../preview.ts";
import type { AppError } from "../errors.ts";

const settle = () => new Promise<void>((resolve) => setImmediate(resolve));
const pixels = (width: number, height: number) => ({ width, height, data: new Uint8ClampedArray(width * height * 4) }) as ImageData;

test("model-only previews preserve exact native size, bound readbacks, and discard closed/stale results", async () => {
  let bounds: PreviewBounds | null = { x: 10, y: 20, width: 480, height: 315 };
  const requests: Array<{ size: { width: number; height: number }; complete(image: ImageData | null): void }> = [];
  const published: Array<HTMLCanvasElement | null> = [], painted: ImageData[] = [], errors: AppError[] = [];
  const surface = { width: 0, height: 0, getContext: () => ({ putImageData(image: ImageData, x: number, y: number) {
    assert.deepEqual([x, y], [0, 0]); painted.push(image);
  } }) } as unknown as HTMLCanvasElement;
  const preview = new ModelPreview({
    bounds: () => bounds, frame: (size) => new Promise((complete) => requests.push({ size, complete })),
    publish: (image) => published.push(image), surface: () => surface, report: (error) => errors.push(error),
  });
  preview.update("source-character");
  preview.update("source-character");
  assert.equal(requests.length, 1, "readbacks are bounded independently from world frames");
  assert.deepEqual(requests[0]!.size, { width: 480, height: 315 });
  requests[0]!.complete(pixels(480, 315));
  await settle();
  assert.deepEqual([surface.width, surface.height], [480, 315]);
  assert.equal(painted.length, 1);
  assert.equal(preview.observe().publishedImages, 1);
  preview.update("source-character");
  bounds = null;
  preview.update(null);
  requests[1]!.complete(pixels(480, 315));
  await settle();
  assert.equal(preview.observe().state, "closed");
  assert.equal(preview.observe().completedReadbacks, 2);
  assert.equal(preview.observe().publishedImages, 1, "late GPU readback cannot reopen the interface");
  assert.equal(published.at(-1), null);
  bounds = { x: 40, y: 20, width: 400, height: 250 };
  preview.update("next-authoritative-owner");
  assert.deepEqual(requests[2]!.size, { width: 400, height: 250 });
  requests[2]!.complete(pixels(400, 250));
  await settle();
  assert.deepEqual([surface.width, surface.height], [400, 250]);
  assert.equal(preview.observe().publishedImages, 2, "a new native interface reopens without a disposed controller");
  assert.equal(errors.length, 0);
  preview.dispose();
});

test("wrong-size, absent and rejected preview output is explicit, never resized or replaced", async () => {
  for (const result of ["wrong-size", "unavailable", "gpu-error"] as const) {
    const errors: AppError[] = [];
    let requests = 0;
    const preview = new ModelPreview({
      bounds: () => ({ x: 0, y: 0, width: 480, height: 315 }),
      async frame() {
        requests++;
        if (result === "gpu-error") throw new Error("fixture GPU rejection");
        return result === "unavailable" ? null : pixels(240, 158);
      },
      publish: (image) => assert.equal(image, null),
      report: (error) => errors.push(error),
    });
    preview.update("owner");
    await settle();
    preview.update("owner");
    assert.equal(requests, 1, "a failure cannot create a per-frame retry loop");
    assert.equal(errors.length, 1);
    assert.equal(errors[0]!.kind, "renderer_preview");
    assert.equal(preview.observe().publishedImages, 0);
    preview.dispose();
  }
});
