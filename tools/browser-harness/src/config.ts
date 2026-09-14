import { createHash } from "node:crypto";
import { readFile, realpath } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { z } from "zod";

export const harnessRoot = fileURLToPath(new URL("../", import.meta.url));
export const repositoryRoot = path.resolve(harnessRoot, "../..");
const text = z.string().trim().min(1).max(1024);
export const digest = z.string().regex(/^[a-f0-9]{64}$/);
const size = z.strictObject({
  width: z.number().int().min(640).max(2560),
  height: z.number().int().min(480).max(1440),
});
export const viewportSchema = size.extend({ deviceScaleFactor: z.literal(1) });
const adapter = z.strictObject({
  vendor: z.string(),
  architecture: z.string(),
  device: z.string(),
  description: z.string(),
});
const pin = z.strictObject({
  id: text,
  path: text,
  sha256: digest,
  ownerApprovalRef: text,
});
const counts = z.record(text, z.number().int().nonnegative().max(1_000_000));
const point = { x: z.number().min(0).max(1919), y: z.number().min(0).max(1079) };
const action = z.discriminatedUnion("type", [
  z.strictObject({ type: z.literal("key-press"), key: z.string().min(1).max(64) }),
  z.strictObject({ type: z.literal("mouse-click"), ...point, button: z.enum(["left", "right", "middle"]) }),
  z.strictObject({ type: z.literal("mouse-move"), ...point }),
  z.strictObject({ type: z.literal("mouse-wheel"), deltaX: z.number().int().min(-10_000).max(10_000), deltaY: z.number().int().min(-10_000).max(10_000) }),
]);

export const contractSchema = z.strictObject({
  id: text,
  ownerApprovalRef: text.nullable(),
  sourcePack: pin.nullable(),
  buildId: text,
  sceneId: text,
  routeId: text,
  workloadId: text,
  settingsSha256: digest,
  assetManifestSha256: digest,
  requiredAssets: z.array(z.strictObject({ id: text, sha256: digest })).min(1).max(10_000),
  minimumEntities: counts.refine((v) => Object.values(v).some((n) => n > 0), "An actual workload is required"),
  viewport: viewportSchema,
  resizeRange: z.strictObject({ min: size, max: size }),
  resizeChecks: z.array(size).min(2).max(8),
  surfaceSelector: text,
  setupActions: z.array(action).max(64),
  imageChecks: z.strictObject({
    minSurfaceAreaFraction: z.number().min(0.5).max(1),
    minOpaqueFraction: z.number().min(0.99).max(1),
    maxDominantColorFraction: z.number().min(0.5).max(0.995),
    minChannelStdDev: z.number().min(2).max(128),
  }),
  measurement: z.strictObject({
    warmupMs: z.number().int().min(0).max(60_000),
    durationMs: z.number().int().min(500).max(300_000),
    pollMs: z.number().int().min(50).max(500),
    staleAfterMs: z.number().int().min(100).max(1000),
    readinessTimeoutMs: z.number().int().min(1000).max(60_000),
    minimumFps: z.literal(60),
    p95FrameMs: z.number().positive().max(20),
    p99FrameMs: z.number().positive().max(1000 / 30),
    stallThresholdMs: z.literal(50),
    maxStallsPerMinute: z.number().int().min(0).max(1),
    maxGapMs: z.number().positive().max(100),
  }),
  hardware: z.strictObject({
    id: text,
    representativeIntegratedGraphics: z.boolean(),
    ownerApprovalRef: text.nullable(),
    evidenceRefs: z.array(text).max(20),
    gpu: text,
    cpu: text,
    memory: text,
    os: text,
    driver: text,
    expectedAdapter: adapter.nullable(),
  }),
});

const configSchema = z.strictObject({
  version: z.literal(1),
  purpose: z.enum(["tool-fixture", "candidate", "source-observation"]),
  mode: z.enum(["capture", "benchmark"]),
  url: text,
  allowedOrigins: z.array(text).min(1).max(8),
  captureCaseId: text,
  browser: z.strictObject({
    product: z.enum(["chrome", "edge"]),
    expectedVersion: z.string().regex(/^\d+\.\d+\.\d+\.\d+$/),
    executableEnv: z.string().regex(/^[A-Z][A-Z0-9_]*$/),
    graphicsProfile: z.enum(["sparky-vulkan-x11", "desktop-default"]),
  }),
  sourceObservation: z.strictObject({
    referenceBuild: text,
    sourceSnapshot: text,
    provenance: text,
  }).optional(),
  contract: contractSchema,
});

export type HarnessConfig = z.infer<typeof configSchema>;
export type Contract = HarnessConfig["contract"];
export type Viewport = { width: number; height: number; deviceScaleFactor: number };

export function requireCondition(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

export function sha256(value: string | Buffer): string {
  return createHash("sha256").update(value).digest("hex");
}

export function canonicalJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  if (value !== null && typeof value === "object") {
    return `{${Object.entries(value).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0)
      .map(([key, entry]) => `${JSON.stringify(key)}:${canonicalJson(entry)}`).join(",")}}`;
  }
  return JSON.stringify(value);
}

export function contractHash(contract: Contract): string {
  return sha256(canonicalJson(contract));
}

export function isAllowedUrl(raw: string, origins: string[], websocket = false): boolean {
  try {
    const normalized = websocket ? raw.replace(/^ws:/, "http:") : raw;
    const url = new URL(normalized);
    return url.protocol === "http:" && url.hostname === "127.0.0.1"
      && !url.username && !url.password && origins.includes(url.origin)
      && normalized.startsWith(`${url.origin}/`);
  } catch {
    return false;
  }
}

export function parseConfig(input: unknown): HarnessConfig {
  const config = configSchema.parse(input);
  const { contract: c } = config;
  for (const origin of config.allowedOrigins) {
    const url = new URL(origin);
    requireCondition(url.origin === origin && url.hostname === "127.0.0.1"
      && url.protocol === "http:" && !url.username && !url.password, `Invalid allowlist origin: ${origin}`);
  }
  requireCondition(new Set(config.allowedOrigins).size === config.allowedOrigins.length, "Duplicate allowed origin");
  requireCondition(isAllowedUrl(config.url, config.allowedOrigins), "Target URL is not explicitly allowlisted loopback HTTP");
  requireCondition(c.viewport.width === 1920 && c.viewport.height === 1080, "Primary viewport must be 1920x1080");
  const inRange = (v: { width: number; height: number }) =>
    v.width >= c.resizeRange.min.width && v.height >= c.resizeRange.min.height
    && v.width <= c.resizeRange.max.width && v.height <= c.resizeRange.max.height;
  requireCondition(inRange(c.viewport) && c.resizeChecks.every(inRange), "Viewport/check outside declared resize range");
  for (const edge of [c.resizeRange.min, c.resizeRange.max]) {
    requireCondition(c.resizeChecks.some((v) => v.width === edge.width && v.height === edge.height),
      "Resize checks must include both range endpoints");
  }
  requireCondition(c.measurement.pollMs < c.measurement.staleAfterMs, "Poll interval must be shorter than stale deadline");
  requireCondition(c.measurement.p95FrameMs <= c.measurement.p99FrameMs, "Frame percentile budgets are out of order");
  requireCondition(c.measurement.maxGapMs >= c.measurement.stallThresholdMs, "Maximum gap is below stall threshold");
  requireCondition(new Set(c.requiredAssets.map((a) => a.id)).size === c.requiredAssets.length, "Duplicate required asset");
  if (config.purpose === "tool-fixture") {
    requireCondition(c.sourcePack === null, "Tool fixtures must not claim a source pack");
    requireCondition(c.buildId.startsWith("harness-fixture/"), "Fixture build identity must be explicit");
  } else if (config.purpose === "source-observation") {
    requireCondition(config.mode === "capture" && config.sourceObservation, "Source observations require provenance and capture-only mode");
  } else {
    requireCondition(c.sourcePack !== null, "Candidate capture requires an owner-approved, pinned source pack");
    requireCondition(!c.buildId.startsWith("harness-fixture/"), "A fixture cannot be a candidate");
    if (config.mode === "benchmark") {
      requireCondition(c.ownerApprovalRef, "Candidate benchmark contract has no owner approval reference");
      requireCondition(c.measurement.durationMs >= 60_000 && c.measurement.warmupMs >= 5000,
        "Candidate benchmarks require at least 5s warmup and 60s measurement");
      requireCondition(c.hardware.representativeIntegratedGraphics && c.hardware.ownerApprovalRef
        && c.hardware.evidenceRefs.length && c.hardware.expectedAdapter,
      "Representative integrated-graphics/browser hardware contract remains unapproved or unpinned");
    }
  }
  return config;
}

export async function verifySourcePack(config: HarnessConfig): Promise<void> {
  const pin = config.contract.sourcePack;
  if (!pin) return;
  requireCondition(!path.isAbsolute(pin.path), "Source pack path must be repository-relative");
  const requested = path.resolve(repositoryRoot, pin.path);
  requireCondition(requested.startsWith(`${repositoryRoot}${path.sep}`), "Source pack path escapes repository");
  const file = await realpath(requested);
  requireCondition(file.startsWith(`${await realpath(repositoryRoot)}${path.sep}`), "Source pack path escapes repository");
  requireCondition(sha256(await readFile(file)) === pin.sha256, "Pinned source pack file digest mismatch");
}

export async function loadConfig(file: string): Promise<HarnessConfig> {
  return parseConfig(JSON.parse(await readFile(file, "utf8")));
}
