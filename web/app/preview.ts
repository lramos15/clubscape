import { AppError, deepFreeze, invariant } from "./errors.ts";
import type { UiPreviewRequest } from "../ui/index.ts";
import { canonicalJson } from "./identity.ts";

export interface PreviewBounds { x: number; y: number; width: number; height: number }
export interface PreviewObservation {
  state: "closed" | "pending" | "ready" | "unavailable" | "failed";
  nativeSize: { width: number; height: number } | null;
  completedReadbacks: number;
  publishedImages: number;
  inFlight: boolean;
  purpose: UiPreviewRequest["purpose"] | null;
  sourceWidget: number | null;
  problem: string | null;
}
export interface PreviewHooks {
  request(): Readonly<UiPreviewRequest> | null;
  frame(request: Readonly<UiPreviewRequest>): Promise<ImageData | null>;
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
  #request: Readonly<UiPreviewRequest> | null = null;
  #problem: string | null = null;

  constructor(hooks: PreviewHooks) { this.#hooks = hooks; }

  update(owner: string | null): void {
    if (this.#disposed) return;
    const request = owner === null ? null : this.#hooks.request();
    const size = request ? { width: request.bounds.width, height: request.bounds.height } : null;
    const key = request ? canonicalJson({ owner, request }) : null;
    if (key !== this.#key) {
      this.#key = key;
      this.#generation++;
      this.#size = size;
      this.#request = request;
      this.#problem = null;
      this.#state = size ? "pending" : "closed";
      if (this.#surface !== null) this.#hooks.publish(null);
      this.#surface = null;
    }
    if (!request || !size || this.#inFlight || this.#state === "failed" || this.#state === "unavailable") return;
    invariant(Number.isSafeInteger(size.width) && Number.isSafeInteger(size.height)
      && size.width > 0 && size.height > 0 && size.width <= 2560 && size.height <= 1440,
    "The source model preview has invalid native dimensions.", "renderer_preview");
    this.#inFlight = true;
    const generation = this.#generation;
    void this.#hooks.frame(deepFreeze(structuredClone(request))).then((image) => {
      if (image !== null) this.#completed++;
      if (this.#disposed || generation !== this.#generation) return;
      if (image === null) {
        this.#state = "unavailable";
        this.#problem = "The actual renderer has no loaded player body for this native-size preview.";
        this.#hooks.publish(null);
        this.#hooks.report(new AppError(this.#problem, { kind: "renderer_preview" }));
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
        this.#state = error instanceof AppError && error.kind === "renderer_preview_unavailable" ? "unavailable" : "failed";
        this.#problem = error instanceof AppError ? error.message : "The actual renderer failed its model-only preview GPU readback.";
        this.#hooks.publish(null);
      }
      this.#hooks.report(error instanceof AppError ? error
        : new AppError("The actual renderer failed its model-only preview GPU readback.", { kind: "renderer_preview" }));
    }).finally(() => { this.#inFlight = false; });
  }

  observe(): Readonly<PreviewObservation> {
    return deepFreeze({ state: this.#state, nativeSize: this.#size ? { ...this.#size } : null,
      completedReadbacks: this.#completed, publishedImages: this.#published, inFlight: this.#inFlight,
      purpose: this.#request?.purpose ?? null, sourceWidget: this.#request?.sourceWidget ?? null, problem: this.#problem });
  }

  dispose(): void {
    this.#disposed = true;
    this.#generation++;
    this.#surface = null;
    this.#key = null;
    this.#size = null;
    this.#request = null;
    this.#problem = null;
    this.#state = "closed";
    this.#hooks.publish(null);
  }
}
