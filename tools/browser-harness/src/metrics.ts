import { PNG } from "pngjs";
import { z } from "zod";
import { contractHash, digest, requireCondition, viewportSchema } from "./config.ts";
import type { HarnessConfig, Viewport } from "./config.ts";
import type { RenderFrame, RenderSnapshot } from "./protocol.ts";
import type { GpuObservation } from "./observer.ts";

const nonnegative = z.number().nonnegative().finite();
const count = z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER);
const frameSchema = z.strictObject({
  sequence: count,
  submittedAtMs: nonnegative,
  completedAtMs: nonnegative,
  drawCalls: count.positive(),
  primitives: count.positive(),
  cpuEncodeMs: nonnegative.optional(),
  gpuDurationMs: nonnegative.optional(),
});
const snapshotSchema = z.strictObject({
  version: z.literal(1),
  clock: z.literal("performance.now"),
  application: z.enum(["clubscape", "harness-fixture"]),
  identity: z.strictObject({
    buildId: z.string(),
    buildArtifactSha256: digest.optional(),
    sceneId: z.string(),
    routeId: z.string(),
    workloadId: z.string(),
    sourcePackSha256: digest.nullable(),
    benchmarkContractSha256: digest,
    assetManifestSha256: digest,
    settingsSha256: digest,
  }),
  ready: z.literal(true),
  backend: z.literal("webgpu"),
  viewport: viewportSchema,
  assets: z.array(z.strictObject({ id: z.string(), sha256: digest, loaded: z.boolean() })).max(10_000),
  entities: z.record(z.string(), count),
  deviceEpoch: z.string().min(1),
  renderedFrames: count.positive(),
  lastSubmittedAtMs: nonnegative,
  lastCompletedAtMs: nonnegative,
  nowMs: nonnegative,
  frames: z.array(frameSchema).max(4096),
});

export interface Sample {
  observedAtMs: number;
  viewport: Viewport;
  snapshot: RenderSnapshot;
  gpu: GpuObservation;
}

export function expectedIdentity(config: HarnessConfig): RenderSnapshot["identity"] {
  const c = config.contract;
  return {
    buildId: c.buildId,
    ...(c.buildArtifactSha256 ? { buildArtifactSha256: c.buildArtifactSha256 } : {}),
    sceneId: c.sceneId,
    routeId: c.routeId,
    workloadId: c.workloadId,
    sourcePackSha256: c.sourcePack?.sha256 ?? null,
    benchmarkContractSha256: contractHash(c),
    settingsSha256: c.settingsSha256,
    assetManifestSha256: c.assetManifestSha256,
  };
}

export function validateViewport(actual: Viewport, expected: Viewport): void {
  requireCondition(actual.width === expected.width && actual.height === expected.height
    && actual.deviceScaleFactor === expected.deviceScaleFactor, "Wrong viewport or device pixel ratio");
}

export function validateGpu(gpu: GpuObservation, config: HarnessConfig): void {
  requireCondition(gpu.secureContext && gpu.webgpuAvailable, "WebGPU secure context unavailable");
  requireCondition(gpu.canvas && gpu.device && gpu.device.canvasConfigurations > 0, "No observed WebGPU device configured on capture canvas");
  requireCondition(gpu.errors.length === 0 && !gpu.device.lost, `WebGPU error/device loss: ${gpu.errors.join("; ")}`);
  const device = gpu.device;
  requireCondition(device.completedSubmissions <= device.submissions, "GPU completions exceed actual submissions");
  requireCondition(device.fallbackAdapter === false, "Fallback/unknown WebGPU adapter cannot establish hardware rendering");
  requireCondition(!/swiftshader|llvmpipe|lavapipe|software|basic render/i.test(JSON.stringify(device.adapter)),
    "Software WebGPU adapter rejected");
  const expected = config.contract.hardware.expectedAdapter;
  if (expected) {
    for (const field of ["vendor", "architecture", "device", "description"] as const) {
      requireCondition(device.adapter[field] === expected[field], `Wrong renderer adapter ${field}`);
    }
  }
  requireCondition(gpu.canvas.width === Math.round(gpu.canvas.cssWidth)
    && gpu.canvas.height === Math.round(gpu.canvas.cssHeight), "Canvas backing resolution differs from native CSS pixels");
}

export function validateSample(
  raw: Sample,
  config: HarnessConfig,
  previous?: Sample,
  viewport = config.contract.viewport,
): Sample {
  const snapshot = snapshotSchema.parse(raw.snapshot);
  const sample = { ...raw, snapshot };
  const c = config.contract;
  requireCondition(Number.isFinite(sample.observedAtMs), "Missing harness monotonic clock");
  validateViewport(sample.viewport, viewport);
  validateViewport(snapshot.viewport, viewport);
  validateGpu(sample.gpu, config);
  requireCondition(snapshot.application === (config.purpose === "tool-fixture" ? "harness-fixture" : "clubscape"),
    "Wrong application identity");
  const identity = expectedIdentity(config);
  for (const key of Object.keys(identity) as Array<keyof typeof identity>) {
    requireCondition(snapshot.identity[key] === identity[key], `Wrong pinned identity: ${key}`);
  }
  requireCondition(Math.abs(snapshot.nowMs - sample.observedAtMs) <= 100, "Renderer clock is not current performance.now");
  requireCondition(snapshot.lastSubmittedAtMs <= snapshot.lastCompletedAtMs && snapshot.lastCompletedAtMs <= snapshot.nowMs
    && snapshot.nowMs - snapshot.lastCompletedAtMs <= c.measurement.staleAfterMs, "Stale or future rendered-frame counter");
  const assets = new Map(snapshot.assets.map((a) => [a.id, a]));
  requireCondition(assets.size === snapshot.assets.length, "Duplicate loaded asset identity");
  for (const required of c.requiredAssets) {
    const actual = assets.get(required.id);
    requireCondition(actual?.loaded && actual.sha256 === required.sha256, `Required loaded asset missing/mismatched: ${required.id}`);
  }
  for (const [entity, minimum] of Object.entries(c.minimumEntities)) {
    requireCondition((snapshot.entities[entity] ?? -1) >= minimum, `Required workload missing: ${entity}`);
  }
  const device = sample.gpu.device!;
  requireCondition(snapshot.renderedFrames <= device.completedSubmissions && snapshot.renderedFrames <= device.canvasAcquisitions,
    "Rendered-frame counter exceeds observed GPU completions/canvas acquisitions (RAF or submit-only is not rendering)");
  if (!previous) {
    requireCondition(snapshot.frames.length === 0, "Initial null read must return no historical frame records");
    return sample;
  }
  requireCondition(sample.observedAtMs > previous.observedAtMs && snapshot.nowMs > previous.snapshot.nowMs,
    "Monotonic sampling clock did not advance");
  requireCondition(snapshot.deviceEpoch === previous.snapshot.deviceEpoch && device.id === previous.gpu.device?.id,
    "Renderer/device epoch changed during measurement");
  const difference = snapshot.renderedFrames - previous.snapshot.renderedFrames;
  requireCondition(difference >= 0, "Rendered-frame counter decreased");
  requireCondition(snapshot.frames.length === difference, "Missing/overflowed/duplicated per-rendered-frame records");
  requireCondition(difference <= device.completedSubmissions - previous.gpu.device!.completedSubmissions,
    "Counter advanced without matching real WebGPU completions");
  let sequence = previous.snapshot.renderedFrames;
  let lastTime = previous.snapshot.lastSubmittedAtMs;
  let lastCompleted = previous.snapshot.lastCompletedAtMs;
  for (const frame of snapshot.frames) {
    requireCondition(frame.sequence === ++sequence, "Non-contiguous rendered-frame sequence");
    requireCondition(frame.submittedAtMs > lastTime && frame.submittedAtMs <= snapshot.nowMs,
      "Non-monotonic/future rendered-frame timestamp");
    requireCondition(frame.completedAtMs >= frame.submittedAtMs && frame.completedAtMs >= lastCompleted
      && frame.completedAtMs <= snapshot.nowMs, "Invalid/non-monotonic GPU completion timestamp");
    requireCondition(frame.gpuDurationMs === undefined || device.features.includes("timestamp-query"),
      "GPU duration reported without enabled timestamp-query feature");
    lastTime = frame.submittedAtMs;
    lastCompleted = frame.completedAtMs;
  }
  requireCondition(snapshot.lastSubmittedAtMs === lastTime, "Last-submission timestamp does not match frame records");
  requireCondition(snapshot.lastCompletedAtMs === lastCompleted, "Last-completion timestamp does not match frame records");
  return sample;
}

export function distribution(values: number[]) {
  requireCondition(values.length > 0 && values.every((v) => Number.isFinite(v) && v >= 0), "Invalid/empty metric distribution");
  const sorted = [...values].sort((a, b) => a - b);
  const percentile = (p: number) => sorted[Math.max(0, Math.ceil(p * sorted.length) - 1)];
  return {
    samples: sorted.length,
    min: sorted[0],
    mean: sorted.reduce((a, b) => a + b, 0) / sorted.length,
    p50: percentile(0.5),
    p95: percentile(0.95),
    p99: percentile(0.99),
    max: sorted[sorted.length - 1],
  };
}

export function summarizeFrames(start: Sample, end: Sample, frames: RenderFrame[], config: HarnessConfig) {
  const elapsedMs = end.observedAtMs - start.observedAtMs;
  requireCondition(elapsedMs >= config.contract.measurement.durationMs, "Measurement window is shorter than declared");
  requireCondition(frames.length > 0 && frames.length === end.snapshot.renderedFrames - start.snapshot.renderedFrames,
    "No rendered frames or incomplete frame coverage");
  let lastTime = start.snapshot.lastCompletedAtMs;
  const intervals = frames.map((frame) => {
    const interval = frame.completedAtMs - lastTime;
    lastTime = frame.completedAtMs;
    return interval;
  });
  const tailMs = end.observedAtMs - lastTime;
  requireCondition(tailMs >= 0 && intervals.every((v) => v >= 0), "Invalid rendered-frame timing");
  const gaps = [...intervals, tailMs];
  const budget = config.contract.measurement;
  const stalls = gaps.filter((v) => v > budget.stallThresholdMs);
  const timing = distribution(intervals);
  const renderedFps = frames.length * 1000 / elapsedMs;
  const stallsPerMinute = stalls.length * 60_000 / elapsedMs;
  const maxGapMs = Math.max(...gaps);
  const failures: string[] = [];
  if (renderedFps < budget.minimumFps) failures.push("rendered FPS below 60 (no rounding tolerance)");
  if (timing.p95 > budget.p95FrameMs) failures.push("p95 frame interval over budget");
  if (timing.p99 > budget.p99FrameMs) failures.push("p99 frame interval over budget");
  if (stallsPerMinute > budget.maxStallsPerMinute) failures.push("stall rate over budget");
  if (maxGapMs > budget.maxGapMs) failures.push("maximum frame/tail gap over budget");
  const gpu = frames.flatMap((f) => f.gpuDurationMs === undefined ? [] : [f.gpuDurationMs]);
  const cpu = frames.flatMap((f) => f.cpuEncodeMs === undefined ? [] : [f.cpuEncodeMs]);
  return {
    elapsedMs,
    renderedFrames: frames.length,
    renderedFps,
    counterMeaning: "GPU-completed rendered frames; NOT RAF callbacks, submit-only throughput, or physical display presents",
    frameIntervalMs: timing,
    trailingGapMs: tailMs,
    maxGapMs,
    stalls: { thresholdMs: budget.stallThresholdMs, count: stalls.length, perMinute: stallsPerMinute, wholeGapTotalMs: stalls.reduce((a, b) => a + b, 0) },
    cpuEncodeMs: cpu.length ? distribution(cpu) : null,
    gpuRenderPassDurationMs: gpu.length ? distribution(gpu) : null,
    gpuTimestampCoverage: gpu.length / frames.length,
    gpuCompletionLatencyMs: distribution(frames.map((frame) => frame.completedAtMs - frame.submittedAtMs)),
    gpuCompletionMeaning: "Submission-to-onSubmittedWorkDone wall time, including queue backlog, IPC, and callback scheduling; NOT isolated GPU execution time",
    gpuTimingMeaning: "Optional timestamp-query render-pass duration; not display/presentation latency",
    budgetFailures: failures,
    budgetDecision: config.purpose === "tool-fixture" ? "not-applicable-tool-fixture" : failures.length ? "fail" : "pass",
  };
}

export function inspectImage(bytes: Buffer, checks: HarnessConfig["contract"]["imageChecks"]) {
  const png = PNG.sync.read(bytes);
  const pixels = png.width * png.height;
  requireCondition(pixels > 0, "Empty screenshot");
  let opaque = 0;
  const sums = [0, 0, 0];
  const squares = [0, 0, 0];
  const colors = new Map<number, number>();
  for (let i = 0; i < png.data.length; i += 4) {
    if (png.data[i + 3] >= 250) opaque++;
    for (let channel = 0; channel < 3; channel++) {
      sums[channel] += png.data[i + channel];
      squares[channel] += png.data[i + channel] ** 2;
    }
    const key = (png.data[i] >> 3) << 10 | (png.data[i + 1] >> 3) << 5 | png.data[i + 2] >> 3;
    colors.set(key, (colors.get(key) ?? 0) + 1);
  }
  const opaqueFraction = opaque / pixels;
  const dominantColorFraction = Math.max(...colors.values()) / pixels;
  const channelStdDev = sums.map((sum, i) => Math.sqrt(Math.max(0, squares[i] / pixels - (sum / pixels) ** 2)));
  requireCondition(opaqueFraction >= checks.minOpaqueFraction
    && dominantColorFraction <= checks.maxDominantColorFraction
    && Math.max(...channelStdDev) >= checks.minChannelStdDev,
  `Blank/nearly uniform screenshot: opaque=${opaqueFraction.toFixed(4)}, dominant=${dominantColorFraction.toFixed(4)}`);
  return { width: png.width, height: png.height, opaqueFraction, dominantColorFraction, channelStdDev };
}
