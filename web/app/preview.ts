import { AppError, deepFreeze, invariant } from "./errors.ts";

export interface PreviewBounds { x: number; y: number; width: number; height: number }
export interface PreviewObservation {
  state: "closed" | "pending" | "ready" | "unavailable" | "failed";
  nativeSize: { width: number; height: number } | null;
  completedReadbacks: number;
  publishedImages: number;
  inFlight: boolean;
}
export interface PreviewHooks {
  bounds(): Readonly<PreviewBounds> | null;
  frame(size: { width: number; height: number }): Promise<ImageData | null>;
  publish(surface: HTMLCanvasElement | null): void;
  report(error: AppError): void;
  surface?(): HTMLCanvasElement;
}

export class ModelPreview {
  #hooks: PreviewHooks;
  #key: string | null = null;
  #generation = 0;
  #inFlight = false;
  #disposed = false;
  #surface: HTMLCanvasElement | null = null;
  #state: PreviewObservation["state"] = "closed";
  #size: PreviewObservation["nativeSize"] = null;
  #completed = 0;
  #published = 0;

  constructor(hooks: PreviewHooks) { this.#hooks = hooks; }

  update(owner: string | null): void {
    if (this.#disposed) return;
    const bounds = this.#hooks.bounds();
    const size = bounds && owner !== null ? { width: bounds.width, height: bounds.height } : null;
    const key = size ? `${owner}/${size.width}x${size.height}` : null;
    if (key !== this.#key) {
      this.#key = key;
      this.#generation++;
      this.#size = size;
      this.#state = size ? "pending" : "closed";
      if (this.#surface !== null) this.#hooks.publish(null);
      this.#surface = null;
    }
    if (!size || this.#inFlight || this.#state === "failed" || this.#state === "unavailable") return;
    invariant(Number.isSafeInteger(size.width) && Number.isSafeInteger(size.height)
      && size.width > 0 && size.height > 0 && size.width <= 2560 && size.height <= 1440,
    "The source model preview has invalid native dimensions.", "renderer_preview");
    this.#inFlight = true;
    const generation = this.#generation;
    void this.#hooks.frame(size).then((image) => {
      if (image !== null) this.#completed++;
      if (this.#disposed || generation !== this.#generation) return;
      if (image === null) {
        this.#state = "unavailable";
        this.#hooks.publish(null);
        this.#hooks.report(new AppError("The actual renderer has no loaded player body for this native-size preview.", { kind: "renderer_preview" }));
        return;
      }
      invariant(image.width === size.width && image.height === size.height,
        "The renderer preview readback does not match the exact native UI size.", "renderer_preview");
      const surface = this.#surface ?? this.#hooks.surface?.() ?? document.createElement("canvas");
      if (surface.width !== size.width) surface.width = size.width;
      if (surface.height !== size.height) surface.height = size.height;
      const context = surface.getContext("2d");
      invariant(context, "The native-size model preview surface is unavailable.", "renderer_preview");
      context.putImageData(image, 0, 0);
      this.#surface = surface;
      this.#hooks.publish(surface);
      this.#published++;
      this.#state = "ready";
    }).catch((error: unknown) => {
      if (this.#disposed) return;
      if (generation === this.#generation) {
        this.#state = "failed";
        this.#hooks.publish(null);
      }
      this.#hooks.report(error instanceof AppError ? error
        : new AppError("The actual renderer failed its model-only preview GPU readback.", { kind: "renderer_preview" }));
    }).finally(() => { this.#inFlight = false; });
  }

  observe(): Readonly<PreviewObservation> {
    return deepFreeze({ state: this.#state, nativeSize: this.#size ? { ...this.#size } : null,
      completedReadbacks: this.#completed, publishedImages: this.#published, inFlight: this.#inFlight });
  }

  dispose(): void {
    this.#disposed = true;
    this.#generation++;
    this.#surface = null;
    this.#key = null;
    this.#size = null;
    this.#state = "closed";
    this.#hooks.publish(null);
  }
}
