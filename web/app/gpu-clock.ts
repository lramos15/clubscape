import type { RenderFrame } from "../shared/contracts.ts";
import { AppError } from "./errors.ts";

interface Submission { submittedAtMs: number; done: Promise<number> }
export interface FrameMark { ordinal: number }

/** Observe the real canvas device/queue. This never submits, drives frames, or counts RAF. */
export class CanvasGpuClock {
  #ordinal = 0;
  #armed = false;
  #pendingCanvas = false;
  #queue: GPUQueue | null = null;
  #submissions = new Map<number, Submission>();
  #restore: Array<() => void> = [];

  constructor(canvas: HTMLCanvasElement) {
    const context = canvas.getContext("webgpu");
    if (!context) throw new AppError("The renderer canvas has no WebGPU context.", { kind: "device", recoverable: false });
    const configure = context.configure;
    const texture = context.getCurrentTexture;
    const queues = new WeakSet<GPUQueue>();
    const owner = this;
    const wrappedConfigure: GPUCanvasContext["configure"] = function (configuration) {
      configure.call(context, configuration);
      const queue = configuration.device.queue;
      owner.#queue = queue;
      if (queues.has(queue)) return;
      queues.add(queue);
      const submit = queue.submit;
      const done = queue.onSubmittedWorkDone;
      const wrappedSubmit: GPUQueue["submit"] = function (commands) {
        submit.call(queue, commands);
        if (queue !== owner.#queue || !owner.#pendingCanvas) return;
        owner.#pendingCanvas = false;
        const submittedAtMs = performance.now();
        const receipt = done.call(queue).then(() => performance.now());
        // completed() propagates this rejection with its corresponding native frame.
        void receipt.catch(() => {});
        if (owner.#submissions.size >= 128) throw new AppError("Canvas GPU receipt history overflowed.", { kind: "benchmark", recoverable: false });
        owner.#submissions.set(++owner.#ordinal, { submittedAtMs, done: receipt });
      };
      queue.submit = wrappedSubmit;
      owner.#restore.push(() => { if (queue.submit === wrappedSubmit) queue.submit = submit; });
    };
    const wrappedTexture: GPUCanvasContext["getCurrentTexture"] = function () {
      const value = texture.call(context);
      if (owner.#armed) {
        if (owner.#pendingCanvas) throw new AppError("A canvas texture was acquired without submitting its preceding frame.", { kind: "benchmark", recoverable: false });
        owner.#pendingCanvas = true;
      }
      return value;
    };
    context.configure = wrappedConfigure;
    context.getCurrentTexture = wrappedTexture;
    this.#restore.push(() => {
      if (context.configure === wrappedConfigure) context.configure = configure;
      if (context.getCurrentTexture === wrappedTexture) context.getCurrentTexture = texture;
    });
  }

  mark(): FrameMark {
    this.#armed = true;
    return { ordinal: this.#ordinal };
  }

  async completed(mark: FrameMark, frame: RenderFrame): Promise<RenderFrame> {
    // The native sequence advances at the canvas blit, not when its promise settles.
    // Offscreen previews never acquire this canvas and never enter this ledger.
    const submission = this.#submissions.get(frame.sequence);
    if (!Number.isSafeInteger(frame.sequence) || !submission || frame.sequence <= mark.ordinal) {
      throw new AppError("Renderer frame has no observed canvas acquisition and real GPU submission.", { kind: "benchmark", recoverable: false });
    }
    this.#submissions.delete(frame.sequence);
    return { ...frame, submittedAtMs: submission.submittedAtMs, completedAtMs: await submission.done };
  }

  dispose(): void {
    for (const restore of this.#restore.reverse()) restore();
    this.#restore = [];
    this.#submissions.clear();
    this.#pendingCanvas = false;
  }
}
