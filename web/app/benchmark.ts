import type { ClubscapeBenchmarkV1, RenderSnapshot } from "../../tools/browser-harness/src/protocol.ts";
import type { RenderFrame } from "../shared/contracts.ts";
import type { BuildConfig } from "./build.ts";
import type { AssetObservation } from "./assets.ts";
import { deepFreeze, invariant } from "./errors.ts";
import { isHash } from "./identity.ts";
import type { AudioSnapshot } from "../audio/index.ts";
import type { PreviewObservation } from "./preview.ts";

export interface RendererObservation {
  ready: boolean;
  sceneId: string;
  assets: Array<{ id: string; sha256: string; loaded: boolean }>;
  entities: Record<string, number>;
  gpuTimestampPassScope: string | null;
  /** Applied render-profile settings, excluding changing actor/camera poses. */
  settings: Readonly<Record<string, unknown>> | null;
  scenePlacement?: { baseX: number; baseY: number; sizeTiles: number; blocks: boolean } | null;
  nativeScenePlacement?: { baseX: number; baseY: number; sizeTiles: number; blocks: boolean } | null;
  loadedSquares?: number[];
  playerAnimationAvailable?: boolean;
}

export class Benchmark implements ClubscapeBenchmarkV1 {
  #build: BuildConfig;
  #auditSha: string;
  #binding: { contractId: string; contractSha256: string } | null = null;
  #frames: RenderFrame[] = [];
  #waiting = new Map<number, RenderFrame>();
  #rendered = 0;
  #last: RenderFrame | null = null;
  #settingsSha = "";
  #viewport = { width: 0, height: 0, deviceScaleFactor: 1 };
  #epoch = "device-not-created";
  #scene: { id: string; route: string; workload: string; manifestSha: string; assets: Map<string, string> } | null = null;
  #assets: AssetObservation[] = [];
  #renderer: RendererObservation | null = null;
  #deviceReady = false;
  #worldReady = false;
  #timestampFeature = false;
  #startupMs: number | null = null;
  #audio: unknown = null;
  #preview: Readonly<PreviewObservation> | null = null;
  #now: () => number;

  constructor(build: BuildConfig, now: () => number = () => performance.now()) {
    this.#build = build;
    this.#auditSha = build.benchmarkContractSha256;
    this.#now = now;
  }

  bindRun(binding: { contractId: string; contractSha256: string }): void {
    invariant(typeof binding.contractId === "string" && binding.contractId.length > 0
      && binding.contractId.length <= 192 && isHash(binding.contractSha256), "Invalid benchmark audit binding.", "benchmark");
    invariant(this.#binding === null, "The benchmark audit identity cannot be rebound.", "benchmark");
    this.#binding = { ...binding };
    this.#auditSha = binding.contractSha256;
  }

  settings(hash: string): void {
    invariant(hash === "" || isHash(hash), "Invalid settings digest.", "benchmark");
    this.#settingsSha = hash;
  }
  startup(milliseconds: number): void { this.#startupMs = milliseconds; }
  preview(state: Readonly<PreviewObservation>): void { this.#preview = structuredClone(state); }
  audio(state: AudioSnapshot): void {
    this.#audio = {
      contextState: state.contextState, sampleRate: state.sampleRate, pendingGesture: state.pendingGesture,
      unlocked: state.unlocked, outputEnabled: state.outputEnabled, muted: state.muted,
      connected: state.connected, disposed: state.disposed, currentTime: state.currentTime,
      volumes: { ...state.volumes }, cache: { ...state.cache }, background: structuredClone(state.background),
      volumeSemantics: "native-source-slider-v1", nativeMixer: { ...state.nativeMixer }, masterPercent: state.masterPercent,
      playingVoices: state.voices.length, policyLimits: [...state.policyLimits],
    };
  }
  viewport(width: number, height: number, deviceScaleFactor: number): void {
    this.#viewport = { width, height, deviceScaleFactor };
  }
  device(epoch: string, ready: boolean, timestamps: boolean): void {
    this.#epoch = epoch;
    this.#deviceReady = ready;
    this.#timestampFeature = timestamps;
  }
  worldReady(value: boolean): void { this.#worldReady = value; }
  scene(id: string, route: string, workload: string, manifestSha: string, assets: Map<string, string>): void {
    this.#scene = { id, route, workload, manifestSha, assets: new Map(assets) };
    this.#renderer = null;
  }
  assets(observations: AssetObservation[]): void { this.#assets = observations.map((value) => ({ ...value })); }
  renderer(observation: RendererObservation | null): void {
    if (observation !== null) {
      invariant(Object.entries(observation.entities).length <= 64
        && Object.values(observation.entities).every((count) => Number.isSafeInteger(count) && count >= 0),
      "Invalid actual renderer entity counts.", "benchmark");
      invariant(observation.assets.length <= 20_000
        && new Set(observation.assets.map((asset) => asset.id)).size === observation.assets.length,
      "Invalid actual renderer asset observations.", "benchmark");
      invariant(observation.settings === null || (typeof observation.settings === "object"
        && !Array.isArray(observation.settings) && JSON.stringify(observation.settings).length <= 16 * 1024),
      "Invalid applied renderer settings observation.", "benchmark");
    }
    this.#renderer = observation === null ? null : structuredClone(observation);
  }

  completed(frame: RenderFrame): void {
    invariant(Number.isSafeInteger(frame.sequence) && frame.sequence > this.#rendered
      && !this.#waiting.has(frame.sequence) && frame.sequence <= this.#rendered + 128
      && Number.isSafeInteger(frame.drawCalls) && frame.drawCalls > 0
      && Number.isSafeInteger(frame.primitives) && frame.primitives > 0
      && Number.isFinite(frame.submittedAtMs) && Number.isFinite(frame.completedAtMs)
      && frame.submittedAtMs >= 0 && frame.completedAtMs >= frame.submittedAtMs
      && frame.completedAtMs <= this.#now()
      && (frame.cpuEncodeMs === undefined || (Number.isFinite(frame.cpuEncodeMs) && frame.cpuEncodeMs >= 0))
      && (frame.gpuDurationMs === undefined || (this.#timestampFeature && Number.isFinite(frame.gpuDurationMs) && frame.gpuDurationMs >= 0)),
    "Renderer did not supply a valid GPU-completed frame receipt.", "benchmark");
    this.#waiting.set(frame.sequence, { ...frame });
    for (;;) {
      const next = this.#waiting.get(this.#rendered + 1);
      if (!next) break;
      invariant(this.#last === null || (next.submittedAtMs > this.#last.submittedAtMs
        && next.completedAtMs >= this.#last.completedAtMs), "Renderer frame timestamps regressed.", "benchmark");
      this.#waiting.delete(next.sequence);
      this.#rendered = next.sequence;
      this.#last = next;
      this.#frames.push(next);
      if (this.#frames.length > 4096) this.#frames.shift();
    }
  }

  read(afterFrame: number | null): RenderSnapshot {
    invariant(afterFrame === null || (Number.isSafeInteger(afterFrame) && afterFrame >= 0 && afterFrame <= this.#rendered),
      "Invalid benchmark frame cursor.", "benchmark");
    invariant(afterFrame === null || this.#frames.length === 0 || afterFrame >= this.#frames[0]!.sequence - 1,
      "GPU completion history was overrun; this measurement window is invalid.", "benchmark");
    const scene = this.#scene;
    const renderer = this.#renderer;
    const assets = Array.from(scene?.assets ?? [], ([id, hash]) => {
      const loaded = this.#assets.some((asset) => asset.id === id && asset.sha256 === hash && asset.decoded)
        || renderer?.assets.some((asset) => asset.id === id && asset.sha256 === hash && asset.loaded) === true;
      return { id, sha256: hash, loaded };
    });
    return deepFreeze({
      version: 1, clock: "performance.now", application: "clubscape",
      identity: {
        buildId: this.#build.buildId, buildArtifactSha256: this.#build.buildArtifactSha256,
        sceneId: scene?.id ?? "unloaded", routeId: scene?.route ?? "startup", workloadId: scene?.workload ?? "none",
        sourcePackSha256: this.#build.sourcePackSha256, benchmarkContractSha256: this.#auditSha,
        assetManifestSha256: scene?.manifestSha ?? this.#build.content?.sha256 ?? "",
        settingsSha256: this.#settingsSha,
      },
      ready: this.#deviceReady && this.#worldReady && isHash(this.#settingsSha) && this.#last !== null
        && renderer?.ready === true && renderer.settings !== null && scene !== null && renderer.sceneId === scene.id
        && assets.length > 0 && assets.every((asset) => asset.loaded) && Object.keys(renderer.entities).length > 0,
      backend: "webgpu", viewport: { ...this.#viewport }, assets,
      entities: { ...renderer?.entities }, deviceEpoch: this.#epoch,
      renderedFrames: this.#rendered, lastSubmittedAtMs: this.#last?.submittedAtMs ?? 0,
      lastCompletedAtMs: this.#last?.completedAtMs ?? 0, nowMs: this.#now(),
      frames: afterFrame === null ? [] : this.#frames.filter((frame) => frame.sequence > afterFrame).map((frame) => ({ ...frame })),
      diagnostics: {
        rendererObservationAvailable: renderer !== null,
        scenePlacement: renderer?.scenePlacement ?? null,
        nativeScenePlacement: renderer?.nativeScenePlacement ?? null,
        loadedSquares: renderer?.loadedSquares ?? null,
        playerAnimationAvailable: renderer?.playerAnimationAvailable ?? null,
        wasmStartupMs: this.#startupMs,
        gpuTiming: {
          timestampQueryEnabled: this.#timestampFeature,
          passScope: renderer?.gpuTimestampPassScope ?? null,
          samples: this.#frames.filter((frame) => frame.gpuDurationMs !== undefined).length,
        },
        assetFetches: this.#assets.map((value) => ({ ...value })),
        audio: this.#audio,
        modelPreview: this.#preview,
      },
    } satisfies RenderSnapshot & { diagnostics: unknown });
  }
}
