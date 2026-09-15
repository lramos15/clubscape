import { AppError } from "./errors.ts";

export async function checkCapability(environment: {
  secure: boolean; gpu: Pick<GPU, "requestAdapter"> | undefined; wasm: typeof WebAssembly | undefined;
} = { secure: globalThis.isSecureContext, gpu: navigator.gpu, wasm: globalThis.WebAssembly }): Promise<void> {
  if (!environment.secure) throw new AppError("ClubScape requires HTTPS (or a loopback development origin).", { kind: "capability", recoverable: false });
  if (!environment.wasm || !environment.wasm.validate(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]))) {
    throw new AppError("This browser does not support the required WebAssembly runtime.", { kind: "capability", recoverable: false });
  }
  if (!environment.gpu) throw new AppError("WebGPU is unavailable. Use a supported Chrome or Edge browser with WebGPU enabled. There is no WebGL or software fallback.", { kind: "capability", recoverable: false });
  let adapter: GPUAdapter | null;
  let timeout: ReturnType<typeof setTimeout> | undefined;
  try {
    adapter = await Promise.race([
      environment.gpu.requestAdapter({ forceFallbackAdapter: false }),
      new Promise<null>((resolve) => { timeout = setTimeout(() => resolve(null), 10_000); }),
    ]);
  }
  catch { adapter = null; }
  finally { if (timeout !== undefined) clearTimeout(timeout); }
  if (!adapter || adapter.info.isFallbackAdapter) {
    throw new AppError("No supported hardware WebGPU adapter is available. No fallback renderer was started.", { kind: "capability", recoverable: false });
  }
}

export function watchCanvasDevice(
  canvas: HTMLCanvasElement,
  changed: (epoch: string, ready: boolean, timestamps: boolean) => void,
  failed: (error: AppError) => void,
): () => void {
  const context = canvas.getContext("webgpu");
  if (!context) throw new AppError("The WebGPU game canvas could not be created.", { kind: "capability", recoverable: false });
  const original = context.configure;
  const devices = new WeakMap<GPUDevice, string>();
  let disposed = false;
  let active: GPUDevice | null = null;
  const wrapper: GPUCanvasContext["configure"] = function (configuration) {
    original.call(context, configuration);
    const previous = active;
    active = configuration.device;
    let epoch = devices.get(active);
    if (epoch === undefined) {
      const device = active;
      epoch = crypto.randomUUID();
      const deviceEpoch = epoch;
      devices.set(device, epoch);
      device.addEventListener("uncapturederror", () => {
        if (!disposed && active === device) failed(new AppError("The game renderer reported a WebGPU device error.", { kind: "device", recoverable: false }));
      });
      void device.lost.then(() => {
        if (!disposed && active === device) {
          changed(deviceEpoch, false, device.features.has("timestamp-query"));
          failed(new AppError("The WebGPU rendering device was lost. Reload to create a new hardware device; no fallback was started.", { kind: "device", recoverable: false }));
        }
      });
    }
    if (previous !== active) changed(epoch, true, active.features.has("timestamp-query"));
  };
  context.configure = wrapper;
  return () => {
    disposed = true;
    if (context.configure === wrapper) context.configure = original;
  };
}
