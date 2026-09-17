import { execFileSync } from "node:child_process";
import { lstat, readFile, realpath } from "node:fs/promises";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { gunzipSync } from "node:zlib";
import type { ContentValidation, DisplayCatalog } from "../../web/app/manifest.ts";
import type { SourceInstanceLayouts } from "../../web/app/instance-layout.ts";

const root = resolve(fileURLToPath(new URL("../../", import.meta.url)));
const maximum = 64 * 1024 * 1024;
export interface ContentProjection {
  schemaVersion: 1;
  artifactSha256: string;
  catalog: DisplayCatalog;
  regions: Record<string, { name: string; sceneAsset: string | null }>;
  referencedAssets: string[];
  contentValidation: ContentValidation;
  instanceLayouts: SourceInstanceLayouts;
}

export async function readArtifact(path: string): Promise<Buffer> {
  const input = resolve(root, path);
  if (!input.startsWith(root + sep) || await realpath(input) !== input) {
    throw new Error("Compiled content inputs must remain regular files inside the worktree.");
  }
  const stat = await lstat(input);
  if (!stat.isFile() || stat.size > maximum) throw new Error("Compiled content exceeds its byte budget.");
  const bytes = await readFile(input);
  if (bytes.length > maximum) throw new Error("Compiled content exceeds its byte budget.");
  if (!input.endsWith(".gz")) return bytes;
  try { return gunzipSync(bytes, { maxOutputLength: maximum }); }
  catch { throw new Error("Compiled content gzip is invalid or exceeds the decoded byte budget."); }
}

export async function projectArtifact(path: string): Promise<ContentProjection> {
  const bytes = await readArtifact(path);
  const result = execFileSync("cargo", [
    "run", "--quiet", "-p", "clubscape-wasm", "--bin", "project-content", "--", "-",
  ], { cwd: root, input: bytes, encoding: "utf8", maxBuffer: 16 * 1024 * 1024 });
  return JSON.parse(result) as ContentProjection;
}
