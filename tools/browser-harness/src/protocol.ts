/** Read synchronously on the main thread; do not advance the renderer in read(). */
export interface ClubscapeBenchmarkV1 {
  /** Binds audit identity only; must not alter actual build, scene, assets or counters. */
  bindRun?(binding: { contractId: string; contractSha256: string }): void;
  read(afterFrame: number | null): RenderSnapshot;
}

export interface RenderFrame {
  sequence: number;
  submittedAtMs: number;
  /** performance.now() when this render submission's onSubmittedWorkDone resolves. */
  completedAtMs: number;
  drawCalls: number;
  primitives: number;
  cpuEncodeMs?: number;
  /** Timestamp-query duration of the declared render passes, not presentation latency. */
  gpuDurationMs?: number;
}

export interface RenderSnapshot {
  version: 1;
  clock: "performance.now";
  application: "clubscape" | "harness-fixture";
  identity: {
    buildId: string;
    /** Required when the run contract pins the deployed product artifact. */
    buildArtifactSha256?: string;
    sceneId: string;
    routeId: string;
    workloadId: string;
    sourcePackSha256: string | null;
    benchmarkContractSha256: string;
    assetManifestSha256: string;
    settingsSha256: string;
  };
  ready: boolean;
  backend: "webgpu";
  viewport: { width: number; height: number; deviceScaleFactor: number };
  assets: Array<{ id: string; sha256: string; loaded: boolean }>;
  entities: Record<string, number>;
  deviceEpoch: string;
  renderedFrames: number;
  /** The submission timestamp belonging to the last GPU-completed render frame. */
  lastSubmittedAtMs: number;
  lastCompletedAtMs: number;
  nowMs: number;
  /** Every GPU-completed render frame after afterFrame; [] for the initial null read. */
  frames: RenderFrame[];
}

declare global {
  interface Window {
    __clubscapeBenchmarkV1?: ClubscapeBenchmarkV1;
  }
}
