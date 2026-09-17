import assert from "node:assert/strict";
import { test } from "node:test";
import { ModelPreview } from "../preview.ts";
import type { PreviewBounds } from "../preview.ts";
import { AppError } from "../errors.ts";
import type { UiPreviewRequest } from "../../ui/index.ts";

const settle = () => new Promise<void>((resolve) => setImmediate(resolve));
const pixels = (width: number, height: number) => ({ width, height, data: new Uint8ClampedArray(width * height * 4) }) as ImageData;
const descriptor = (bounds: PreviewBounds): UiPreviewRequest => ({
  purpose: "appearance", bounds, modelBounds: { ...bounds, width: 136, height: 192 },
  sourceWidget: 44499017, modelZoom: 450, modelRotation: [0, 0, 0], appearance: { body_type: 0 },
  equipment: null, base: null,
});

test("model-only previews preserve exact native size, bound readbacks, and discard closed/stale results", async () => {
  let bounds: PreviewBounds | null = { x: 10, y: 20, width: 480, height: 315 };
  const requests: Array<{ request: Readonly<UiPreviewRequest>; complete(image: ImageData | null): void }> = [];
  const published: Array<HTMLCanvasElement | null> = [], painted: ImageData[] = [], errors: AppError[] = [];
  const surface = { width: 0, height: 0, getContext: () => ({ putImageData(image: ImageData, x: number, y: number) {
    assert.deepEqual([x, y], [0, 0]); painted.push(image);
  } }) } as unknown as HTMLCanvasElement;
  const preview = new ModelPreview({
    request: () => bounds ? descriptor(bounds) : null,
    frame: (request) => new Promise((complete) => requests.push({ request, complete })),
    publish: (image) => published.push(image), surface: () => surface, report: (error) => errors.push(error),
  });
  preview.update("source-character");
  preview.update("source-character");
  assert.equal(requests.length, 1, "readbacks are bounded independently from world frames");
  assert.deepEqual(requests[0]!.request, descriptor(bounds));
  assert(Object.isFrozen(requests[0]!.request.appearance));
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
  assert.deepEqual(requests[2]!.request.bounds, bounds);
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
      request: () => descriptor({ x: 0, y: 0, width: 480, height: 315 }),
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

test("local appearance and native widget changes invalidate stale preview readbacks at the same size", async () => {
  let request = descriptor({ x: 0, y: 0, width: 480, height: 315 });
  const pending: Array<(image: ImageData) => void> = [];
  const received: Readonly<UiPreviewRequest>[] = [];
  const preview = new ModelPreview({
    request: () => request,
    frame: (value) => { received.push(value); return new Promise((resolve) => pending.push(resolve)); },
    publish: (image) => assert.equal(image, null), report: () => {},
  });
  preview.update("actor.fixture");
  request = { ...request, appearance: { body_type: 1 }, modelZoom: 550 };
  preview.update("actor.fixture");
  pending[0]!(pixels(480, 315));
  await settle();
  assert.equal(preview.observe().publishedImages, 0);
  preview.update("actor.fixture");
  assert.deepEqual(received[1], request);
  preview.dispose();
  pending[1]!(pixels(480, 315));
  await settle();
  assert.equal(preview.observe().publishedImages, 0);
});

test("closed or superseded previews cannot report old failures into a new actor entry", async () => {
  let open = true;
  let reject: (error: Error) => void = () => {};
  const errors: AppError[] = [];
  const preview = new ModelPreview({
    request: () => open ? descriptor({ x: 0, y: 0, width: 480, height: 315 }) : null,
    frame: () => new Promise((_resolve, failure) => { reject = failure; }),
    publish: image => assert.equal(image, null),
    report: error => errors.push(error),
  });
  preview.update("actor.first");
  open = false;
  preview.update(null);
  reject(new AppError("Old actor metadata failed.", { kind: "renderer_preview_unavailable" }));
  await settle();
  assert.equal(preview.observe().state, "closed");
  assert.equal(errors.length, 0);
  open = true;
  preview.update("actor.second");
  preview.dispose();
  reject(new Error("Old GPU failure after dispose"));
  await settle();
  assert.equal(errors.length, 0);
});

test("new real base metadata retries an unavailable request without a per-frame failure loop", async () => {
  let ready = false, calls = 0;
  const errors: AppError[] = [];
  const preview = new ModelPreview({
    request: () => descriptor({ x: 0, y: 0, width: 480, height: 315 }),
    async frame() {
      calls++;
      if (!ready) throw new AppError("Base not available.", { kind: "renderer_preview_unavailable" });
      return null;
    },
    publish: image => assert.equal(image, null), report: error => errors.push(error),
  });
  preview.update("actor.fixture:base-unavailable");
  await settle();
  preview.update("actor.fixture:base-unavailable");
  assert.equal(calls, 1);
  ready = true;
  preview.update("actor.fixture:actual-new-base");
  await settle();
  assert.equal(calls, 2);
  preview.dispose();
});

test("a UI close before the next animation frame prevents publication of a completed readback", async () => {
  let open = true;
  let complete: (image: ImageData | null) => void = () => {};
  const errors: AppError[] = [];
  const preview = new ModelPreview({
    request: () => open ? descriptor({ x: 0, y: 0, width: 480, height: 315 }) : null,
    frame: () => new Promise(resolve => { complete = resolve; }),
    publish: image => assert.equal(image, null),
    report: error => errors.push(error),
  });
  preview.update("actor.fixture");
  open = false;
  complete(pixels(480, 315));
  await settle();
  assert.equal(preview.observe().state, "closed");
  assert.equal(preview.observe().completedReadbacks, 1);
  assert.equal(preview.observe().publishedImages, 0);
  assert.equal(errors.length, 0);
  preview.dispose();
});

test("producer invalidation is retriable, not a missing-metadata error for the current preview", async () => {
  let reject: (error: Error) => void = () => {};
  let calls = 0;
  const errors: AppError[] = [];
  const preview = new ModelPreview({
    request: () => descriptor({ x: 0, y: 0, width: 480, height: 315 }),
    frame: () => {
      calls++;
      return new Promise((_resolve, failure) => { reject = failure; });
    },
    publish: image => assert.equal(image, null),
    report: error => errors.push(error),
  });
  preview.update("actor.fixture");
  reject(new AppError("Preview metadata changed during the readback.",
    { kind: "cancelled", errorId: "preview.readback_superseded" }));
  await settle();
  assert.equal(preview.observe().state, "pending");
  assert.equal(preview.observe().problem, null);
  assert.equal(errors.length, 0);
  preview.update("actor.fixture");
  assert.equal(calls, 2);
  reject(new AppError("Current source metadata is unavailable.", { kind: "renderer_preview_unavailable" }));
  await settle();
  assert.equal(preview.observe().state, "unavailable");
  assert.equal(errors.length, 1, "Genuine current metadata failures still surface.");
  preview.dispose();
});
