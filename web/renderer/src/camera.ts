import type { RenderCamera } from "../../shared/contracts.ts";

export interface CameraContext {
  actorId: string;
  region: string;
  instance: string | null;
  revision: string;
  tick: string;
  sceneId: string;
  sceneGeneration: string;
}

export interface CameraSourceSample {
  context: CameraContext;
  renderedActor: { actorId: string; local: [number, number]; plane: number; sizeTiles: number } | null;
  focus: {
    identity: number;
    world_base: { x: number; y: number };
    logical: [number, number];
    rendered: [number, number];
    plane: number;
    footprint: number;
    kind: "Actor" | "Point";
  } | null;
  effects: {
    shake: Array<{ random_radius: number; sine_amplitude: number; sine_frequency: number; phase: number; random_sample: number | null } | null>;
    suppress_jitter: boolean;
  } | null;
  missing: string[];
}

export interface NativeCameraRenderer {
  cameraSceneReady(): boolean;
  cameraSource(): CameraSourceSample;
  cameraScene(): string;
  applyNativeCamera(delivery: string): RenderCamera;
}

/** Coalesces source loads; only the latest requested actor/layout/base may publish a scene. */
export class SourceSceneStream<T, P> {
  #wanted: { key: string; target: T } | null = null;
  #running: Promise<void> | null = null;
  #failure: unknown = null;
  #prepare: (target: T) => Promise<P>;
  #commit: (target: T, prepared: P) => void;
  #key: (target: T) => string;

  constructor(key: (target: T) => string, prepare: (target: T) => Promise<P>, commit: (target: T, prepared: P) => void) {
    this.#key = key;
    this.#prepare = prepare;
    this.#commit = commit;
  }

  get busy(): boolean { return this.#running !== null; }
  requireReady(): void {
    if (this.#failure !== null) throw this.#failure;
    if (this.busy) throw new Error("The current source scene is still streaming.");
  }
  cancel(): void { this.#wanted = null; this.#failure = null; }

  request(target: T): Promise<void> {
    const key = this.#key(target);
    if (this.#wanted?.key !== key) this.#wanted = { key, target };
    this.#failure = null;
    this.#running ??= this.#run();
    return this.#running;
  }

  async #run(): Promise<void> {
    try {
      while (this.#wanted !== null) {
        const request = this.#wanted;
        const prepared = await Promise.resolve().then(() => this.#prepare(request.target));
        if (request !== this.#wanted) continue;
        this.#commit(request.target, prepared);
        if (request === this.#wanted) this.#wanted = null;
      }
    } catch (error) {
      this.#failure = error;
      this.#wanted = null;
      throw error;
    } finally { this.#running = null; }
  }
}
