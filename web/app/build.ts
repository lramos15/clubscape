import { BENCHMARK_CONTRACT_SHA256, SOURCE_PACK_SHA256 } from "../shared/contracts.ts";
import { invariant } from "./errors.ts";
import { isHash, publicPath, sha256 } from "./identity.ts";
import { boundedBytes } from "./transport.ts";
import type { Fetch } from "./transport.ts";

export interface BuildConfig {
  schemaVersion: 1;
  buildId: string;
  buildArtifactSha256: string;
  artifactPath: string;
  sourcePackSha256: string;
  benchmarkContractSha256: string;
  content: { path: string; sha256: string; owner: "web" | "game" } | null;
  visualSettings: Readonly<Record<string, unknown>>;
  components: { renderer: boolean; ui: boolean; audio: boolean };
}

export async function verifiedJson(path: string, hash: string | null, maxBytes: number, fetcher: Fetch): Promise<{ value: unknown; sha256: string }> {
  publicPath(path);
  const response = await fetcher(path, {
    credentials: "omit", mode: "same-origin", redirect: "error", referrerPolicy: "no-referrer",
    signal: AbortSignal.timeout(30_000), cache: "no-cache",
  });
  invariant(response.ok && !response.redirected, `Public metadata could not be fetched (${response.status}): ${path}.`);
  invariant(response.headers.get("content-type")?.split(";")[0]?.trim() === "application/json", "Invalid public metadata MIME type.");
  const bytes = await boundedBytes(response, maxBytes);
  const actual = await sha256(bytes);
  invariant(hash === null || actual === hash, "Public metadata digest mismatch.");
  let value: unknown;
  try { value = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes)); }
  catch { throw new Error("Public metadata could not be decoded."); }
  return { value, sha256: actual };
}

export async function loadBuild(fetcher: Fetch = globalThis.fetch.bind(globalThis)): Promise<BuildConfig> {
  const { value } = await verifiedJson("/client/build.json", null, 64 * 1024, fetcher);
  const config = value as BuildConfig;
  invariant(config?.schemaVersion === 1 && typeof config.buildId === "string"
    && config.buildId.length <= 192 && isHash(config.buildArtifactSha256)
    && config.sourcePackSha256 === SOURCE_PACK_SHA256
    && config.benchmarkContractSha256 === BENCHMARK_CONTRACT_SHA256, "Unsupported or source-mismatched browser build.");
  publicPath(config.artifactPath, "/client/");
  if (config.content !== null) {
    publicPath(config.content.path, "/content/");
    invariant(isHash(config.content.sha256) && ["web", "game"].includes(config.content.owner), "Invalid content deployment pin.");
  }
  const artifact = await verifiedJson(config.artifactPath, config.buildArtifactSha256, 2 * 1024 * 1024, fetcher);
  const identity = artifact.value as { sourcePackSha256?: string; benchmarkContractSha256?: string };
  invariant(identity.sourcePackSha256 === config.sourcePackSha256
    && identity.benchmarkContractSha256 === config.benchmarkContractSha256, "Build artifact/source identity mismatch.");
  return config;
}
