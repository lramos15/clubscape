export interface ObservedDevice {
  id: number;
  adapter: { vendor: string; architecture: string; device: string; description: string };
  fallbackAdapter: boolean | null;
  adapterFeatures: string[];
  features: string[];
  submissions: number;
  completedSubmissions: number;
  lastCompletedAtMs: number | null;
  canvasAcquisitions: number;
  canvasConfigurations: number;
  lost: boolean;
}

export interface GpuObservation {
  secureContext: boolean;
  webgpuAvailable: boolean;
  canvas: { width: number; height: number; cssWidth: number; cssHeight: number } | null;
  device: ObservedDevice | null;
  errors: string[];
}

// Playwright serializes this function. Keep all runtime dependencies inside it.
export function installGpuObserver(): void {
  const host = globalThis as any;
  const errors: string[] = [];
  const devices = new WeakMap<object, any>();
  const queues = new WeakMap<object, any>();
  const canvases = new WeakMap<object, any>();
  let nextId = 0;
  const appendError = (message: string) => {
    if (errors.length < 100) errors.push(message);
  };
  const available = !!host.navigator.gpu && !!host.GPUAdapter && !!host.GPUQueue && !!host.GPUCanvasContext;
  if (available) {
    const requestDevice = host.GPUAdapter.prototype.requestDevice;
    host.GPUAdapter.prototype.requestDevice = async function (...args: any[]) {
      const device: any = await Reflect.apply(requestDevice, this, args);
      const info = this.info ?? {};
      const state = {
        id: ++nextId,
        adapter: Object.fromEntries(["vendor", "architecture", "device", "description"].map((k) => [k, String(info[k] ?? "")])),
        fallbackAdapter: typeof info.isFallbackAdapter === "boolean" ? info.isFallbackAdapter
          : typeof this.isFallbackAdapter === "boolean" ? this.isFallbackAdapter : null,
        features: [...device.features].sort(),
        adapterFeatures: [...this.features].sort(),
        submissions: 0,
        completedSubmissions: 0,
        lastCompletedAtMs: null,
        canvasAcquisitions: 0,
        canvasConfigurations: 0,
        lost: false,
        queue: device.queue,
      };
      devices.set(device, state);
      queues.set(device.queue, state);
      device.lost.then((loss: any) => {
        state.lost = true;
        appendError(`device-lost: ${loss.reason}: ${loss.message}`);
      });
      device.addEventListener("uncapturederror", (event: any) => appendError(`uncaptured-webgpu-error: ${event.error.message}`));
      return device;
    };
    const configure = host.GPUCanvasContext.prototype.configure;
    host.GPUCanvasContext.prototype.configure = function (descriptor: any) {
      const result = Reflect.apply(configure, this, [descriptor]);
      const state = devices.get(descriptor.device);
      if (!state) appendError("Canvas configured with a device not observed on the main thread");
      else {
        state.canvasConfigurations++;
        canvases.set(this.canvas, state);
      }
      return result;
    };
    const getTexture = host.GPUCanvasContext.prototype.getCurrentTexture;
    host.GPUCanvasContext.prototype.getCurrentTexture = function () {
      const result = Reflect.apply(getTexture, this, []);
      const state = canvases.get(this.canvas);
      if (state) state.canvasAcquisitions++;
      return result;
    };
    const submit = host.GPUQueue.prototype.submit;
    host.GPUQueue.prototype.submit = function (...args: any[]) {
      const result = Reflect.apply(submit, this, args);
      const state = queues.get(this);
      if (state) {
        const sequence = ++state.submissions;
        state.queue.onSubmittedWorkDone().then(() => {
          state.completedSubmissions = Math.max(state.completedSubmissions, sequence);
          state.lastCompletedAtMs = performance.now();
        }).catch((error: Error) => appendError(`queue-completion-error: ${error.message}`));
      }
      return result;
    };
  }
  Object.defineProperty(host, "__clubscapeHarnessObserverV1", {
    configurable: false,
    writable: false,
    value: Object.freeze({
      read(selector: string) {
        const canvas = document.querySelector(selector);
        const state = canvas ? canvases.get(canvas) : null;
        const bounds = canvas?.getBoundingClientRect();
        const { queue: _, ...device } = state ?? {};
        return {
          secureContext: host.isSecureContext,
          webgpuAvailable: available,
          canvas: canvas instanceof HTMLCanvasElement && bounds ? {
            width: canvas.width, height: canvas.height, cssWidth: bounds.width, cssHeight: bounds.height,
          } : null,
          device: state ? device : null,
          errors: [...errors],
        };
      },
      async drain(selector: string) {
        const canvas = document.querySelector(selector);
        const state = canvas ? canvases.get(canvas) : null;
        if (!state) return { available: false };
        const startedAtMs = performance.now();
        const throughSubmission = state.submissions;
        await state.queue.onSubmittedWorkDone();
        return {
          available: true,
          kind: "queue-completion-wall-time-after-window",
          elapsedMs: performance.now() - startedAtMs,
          throughSubmission,
          meaning: "Completion of all previously submitted queue work, including backlog; NOT per-frame GPU execution or presentation time",
        };
      },
    }),
  });
}
