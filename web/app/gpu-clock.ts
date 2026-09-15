import type { RenderFrame } from "../shared/contracts.ts";
import { AppError } from "./errors.ts";

interface Submission { ordinal: number; submittedAtMs: number; done: Promise<number> }
export interface FrameMark { ordinal: number; acquisitions: number }

/** Observe the real canvas device/queue. This never submits, drives frames, or counts RAF. */
export class CanvasGpuClock {
  #ordinal = 0;
  #acquisitions = 0;
  #last: Submission | null = null;
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
      if (queues.has(queue)) return;
      queues.add(queue);
      const submit = queue.submit;
      const done = queue.onSubmittedWorkDone;
      const wrappedSubmit: GPUQueue["submit"] = function (commands) {
        submit.call(queue, commands);
        const submittedAtMs = performance.now();
        const receipt = done.call(queue).then(() => performance.now());
        // The renderer's promise owns error propagation; suppress a detached rejection.
        void receipt.catch(() => {});
        owner.#last = { ordinal: ++owner.#ordinal, submittedAtMs, done: receipt };
      };
      queue.submit = wrappedSubmit;
      owner.#restore.push(() => { if (queue.submit === wrappedSubmit) queue.submit = submit; });
    };
    const wrappedTexture: GPUCanvasContext["getCurrentTexture"] = function () {
      const value = texture.call(context);
      owner.#acquisitions++;
      return value;
    };
    context.configure = wrappedConfigure;
    context.getCurrentTexture = wrappedTexture;
    this.#restore.push(() => {
      if (context.configure === wrappedConfigure) context.configure = configure;
      if (context.getCurrentTexture === wrappedTexture) context.getCurrentTexture = texture;
    });
  }

  mark(): FrameMark { return { ordinal: this.#ordinal, acquisitions: this.#acquisitions }; }

  async completed(mark: FrameMark, frame: RenderFrame): Promise<RenderFrame> {
    const submission = this.#last;
    if (!submission || submission.ordinal <= mark.ordinal || this.#acquisitions <= mark.acquisitions) {
      throw new AppError("Renderer frame has no observed canvas acquisition and real GPU submission.", { kind: "benchmark", recoverable: false });
    }
    return { ...frame, submittedAtMs: submission.submittedAtMs, completedAtMs: await submission.done };
  }

  dispose(): void { for (const restore of this.#restore.reverse()) restore(); }
}
