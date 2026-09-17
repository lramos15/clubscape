import type { RenderCamera, WorldView } from "../shared/contracts.ts";
import type { CameraContext, CameraSourceSample, NativeCameraRenderer } from "../renderer/src/camera.ts";
import type { AssetLoader } from "./assets.ts";
import type { NativeCamera } from "../generated/protocol/clubscape_wasm.js";
import { AppError, appError, invariant } from "./errors.ts";

export interface NativeCameraInput {
  arrows: { left: boolean; right: boolean; up: boolean; down: boolean };
  mouse: [number, number];
  button: "Released" | "Primary" | "Secondary" | "Middle";
  wheel: { rotation: number; route: "Camera" | "OtherWidget" } | null;
}

export type NativeCameraRuntime = Pick<NativeCamera, "bind" | "input" | "frame" | "resize" | "face_yaw" | "suspend" | "free">;

export interface NativeCameraInputSink {
  ready(): boolean;
  mouse?(): [number, number] | null;
  input(at: bigint, input: NativeCameraInput): void;
  faceYaw(yaw: number): void;
}

export function cameraNanoseconds(milliseconds: number): bigint {
  invariant(Number.isFinite(milliseconds) && milliseconds >= 0 && milliseconds <= Number.MAX_SAFE_INTEGER / 1_000_000,
    "The browser camera timestamp is invalid or loses nanosecond integer precision.", "camera_unavailable");
  const nanoseconds = Math.round(milliseconds * 1_000_000);
  invariant(Number.isSafeInteger(nanoseconds),
    "The browser camera timestamp loses nanosecond integer precision.", "camera_unavailable");
  return BigInt(nanoseconds);
}

/** One signed DOM wheel event is one transport step; native 20ms accumulation and FOV own its effect. */
export function cameraWheelRotation(deltaY: number, deltaMode: number): number {
  invariant(Number.isFinite(deltaY) && [0, 1, 2].includes(deltaMode),
    "The browser wheel event has an invalid delta/mode.", "camera_unavailable");
  return deltaY === 0 ? 0 : Math.sign(deltaY);
}

export function requireCameraSource(sample: CameraSourceSample, world: WorldView): void {
  const c = sample.context;
  invariant(c.actorId === world.player.id && c.region === world.player.region && c.instance === world.player.instance
    && c.revision === world.revision && c.tick === world.tick,
  "The camera source belongs to a stale actor/world revision.", "camera_unavailable");
  if (sample.missing.length || sample.focus === null || sample.effects === null || sample.renderedActor === null) {
    throw new AppError(`Native camera source input is unavailable: ${sample.missing.join("; ")
      || "actual logical/rendered focus, footprint, plane and explicit effects are required"}. No tile-centre, zero or recorded-camera fallback was used.`,
    { kind: "camera_unavailable" });
  }
}

function bindingKey(context: CameraContext): string {
  return JSON.stringify([context.actorId, context.region, context.instance, context.sceneId, context.sceneGeneration]);
}

export class CameraSourceMetadata {
  #assets: AssetLoader;
  #required = new Set<string>();
  #requiredAsset: (id: string) => void;

  constructor(assets: AssetLoader, requiredAsset: (id: string) => void = () => {}) {
    this.#assets = assets;
    this.#requiredAsset = requiredAsset;
  }

  get requiredAssets(): string[] { return [...this.#required]; }

  async definitions(scene: string): Promise<string> {
    const value: unknown = JSON.parse(scene);
    invariant(value !== null && typeof value === "object" && "surfaces" in value && Array.isArray(value.surfaces),
      "The native source scene omitted its rendered surface records.", "camera_unavailable");
    const objects = new Set<number>();
    for (const surface of value.surfaces as unknown[]) {
      if (surface === null) continue;
      invariant(typeof surface === "object" && "decoration" in surface,
        "The native source surface omitted its decoration identity.", "camera_unavailable");
      if (surface.decoration === null) continue;
      invariant(typeof surface.decoration === "number" && Number.isSafeInteger(surface.decoration) && surface.decoration >= 0,
        "The native source surface has an invalid object identity.", "camera_unavailable");
      objects.add(surface.decoration);
    }
    const definitions: Array<{ id: number; raise: number }> = [];
    const pending = [...objects];
    let next = 0;
    await Promise.all(Array.from({ length: Math.min(4, pending.length) }, async () => {
      for (;;) {
        const object = pending[next++];
        if (object === undefined) return;
        const id = `asset.source.osrs.cache2695.object.${object}`;
        const asset = this.#assets.manifest.assets.find((a) => a.id === id);
        invariant(asset?.contentType.startsWith("application/json"),
          `Ground-decoration object ${object} needs its existing original id/raise metadata in the public asset manifest. No model-height substitute was used.`,
          "camera_unavailable");
        this.#required.add(id);
        this.#requiredAsset(id);
        const raw = await this.#assets.json(id);
        invariant(raw !== null && typeof raw === "object" && "id" in raw && raw.id === object
          && "raise" in raw && typeof raw.raise === "number" && Number.isSafeInteger(raw.raise)
          && raw.raise >= -2147483648 && raw.raise <= 2147483647,
        `Original ground-decoration object ${object} omitted its exact decoded raise.`, "camera_unavailable");
        definitions.push({ id: object, raise: raw.raise });
      }
    }));
    definitions.sort((a, b) => a.id - b.id);
    return JSON.stringify(definitions);
  }
}

export class NativeCameraSession implements NativeCameraInputSink {
  #renderer: NativeCameraRenderer;
  #create: () => NativeCameraRuntime;
  #runtime: NativeCameraRuntime;
  #definitions: (scene: string) => Promise<string>;
  #currentWorld: () => WorldView | null;
  #size: () => { width: number; height: number };
  #now: () => number;
  #generation = 0;
  #binding: string | null = null;
  #ready = false;
  #disposed = false;
  #mouse: [number, number] | null = null;

  constructor(renderer: NativeCameraRenderer, create: () => NativeCameraRuntime,
    definitions: (scene: string) => Promise<string>, currentWorld: () => WorldView | null,
    size: () => { width: number; height: number }, now: () => number = () => performance.now()) {
    this.#renderer = renderer;
    this.#create = create;
    this.#runtime = create();
    this.#definitions = definitions;
    this.#currentWorld = currentWorld;
    this.#size = size;
    this.#now = now;
  }

  ready(): boolean { return this.#ready && !this.#disposed; }
  identity(): string | null { return this.#binding; }
  mouse(): [number, number] | null { return this.#mouse === null ? null : [...this.#mouse]; }

  async prepare(world: WorldView): Promise<RenderCamera> {
    invariant(!this.#disposed, "Native camera has been disposed.", "camera_unavailable");
    this.suspend();
    const generation = this.#generation;
    invariant(this.#renderer.cameraSceneReady(), "Actual camera scene is still streaming.", "camera_unavailable");
    const before = this.#renderer.cameraSource();
    requireCameraSource(before, world);
    const key = bindingKey(before.context);
    const scene = this.#renderer.cameraScene();
    const definitions = await this.#definitions(scene);
    if (this.#disposed || generation !== this.#generation || this.#currentWorld() !== world) {
      throw new AppError("Native camera preparation was cancelled after its actor/world changed.", { kind: "cancelled" });
    }
    if (!this.#renderer.cameraSceneReady()) {
      throw new AppError("The source camera scene changed during initialization.", { kind: "cancelled" });
    }
    const sample = this.#renderer.cameraSource();
    requireCameraSource(sample, world);
    if (bindingKey(sample.context) !== key) {
      throw new AppError("The source scene changed during camera initialization.", { kind: "cancelled" });
    }
    const { width, height } = this.#size();
    try {
      // The chosen approved view supplies angles only. The Rust constructor supplies FOV, never the full-HUD fixture helper.
      const delivery = this.#runtime.bind(scene, JSON.stringify(sample), definitions, "TutorialStartingHouse",
        width, height, cameraNanoseconds(this.#now()));
      const camera = this.#renderer.applyNativeCamera(delivery);
      if (this.#disposed || generation !== this.#generation || this.#currentWorld() !== world) {
        throw new AppError("The native camera owner changed before initialization completed.", { kind: "cancelled" });
      }
      this.#binding = key;
      this.#ready = true;
      return camera;
    } catch (error) {
      this.suspend();
      throw appError(error, "The real native camera could not initialize.");
    }
  }

  frame(now: number, world: WorldView): RenderCamera | null {
    if (!this.ready()) return null;
    try {
      if (!this.#renderer.cameraSceneReady()) { this.suspend(); return null; }
      const sample = this.#renderer.cameraSource();
      requireCameraSource(sample, world);
      if (bindingKey(sample.context) !== this.#binding) { this.suspend(); return null; }
      return this.#renderer.applyNativeCamera(this.#runtime.frame(cameraNanoseconds(now), JSON.stringify(sample)));
    } catch (error) {
      this.suspend();
      throw appError(error, "The native camera rejected its current source frame.");
    }
  }

  input(at: bigint, input: NativeCameraInput): void {
    invariant(this.ready(), "Native camera input is suspended.", "camera_unavailable");
    try { this.#runtime.input(at, JSON.stringify(input)); this.#mouse = [...input.mouse]; }
    catch (error) { this.suspend(); throw appError(error, "The native camera rejected a physical input."); }
  }

  faceYaw(yaw: number): void {
    invariant(this.ready(), "Native camera compass input is suspended.", "camera_unavailable");
    try { this.#runtime.face_yaw(yaw); }
    catch (error) { throw appError(error, "The native camera rejected a compass request."); }
  }

  resize(width: number, height: number): void {
    if (this.#binding === null || this.#disposed) return;
    try { this.#runtime.resize(width, height); }
    catch (error) { this.suspend(); throw appError(error, "The native camera rejected its resized viewport."); }
  }

  suspend(): void {
    this.#generation++;
    this.#ready = false;
    if (!this.#disposed) this.#runtime.suspend();
  }

  reset(): void {
    if (this.#disposed) return;
    this.suspend();
    this.#runtime.free();
    this.#runtime = this.#create();
    this.#binding = null;
    this.#mouse = null;
  }

  dispose(): void {
    if (this.#disposed) return;
    this.suspend();
    this.#disposed = true;
    this.#runtime.free();
  }
}
