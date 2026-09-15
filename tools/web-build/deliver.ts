import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { copyFile, lstat, mkdir, readFile, readdir, realpath, writeFile } from "node:fs/promises";
import { dirname, extname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { BENCHMARK_CONTRACT_SHA256, SOURCE_PACK_SHA256 } from "../../web/shared/contracts.ts";
import { canonicalJson, publicPath } from "../../web/app/identity.ts";
import { parseContentManifest } from "../../web/app/manifest.ts";
import type { AssetRecord } from "../../web/app/manifest.ts";

export interface PublicFile { url: string; path: string; sha256: string; content_type: string }
const digest = (bytes: Uint8Array | string): string => createHash("sha256").update(bytes).digest("hex");
export const mime: Record<string, string> = {
  ".html": "text/html; charset=utf-8", ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8", ".wasm": "application/wasm", ".json": "application/json",
  ".png": "image/png", ".jpg": "image/jpeg", ".jpeg": "image/jpeg", ".webp": "image/webp",
  ".flac": "audio/flac", ".ogg": "audio/ogg", ".wav": "audio/wav", ".glb": "model/gltf-binary",
  ".bin": "application/octet-stream", ".ktx2": "application/octet-stream",
  ".woff": "font/woff", ".woff2": "font/woff2", ".ttf": "font/ttf",
};
const root = resolve(fileURLToPath(new URL("../../", import.meta.url)));

export async function publicFile(rootPath: string, path: string, url: string, expected?: AssetRecord): Promise<PublicFile> {
  publicPath(url);
  publicPath(`/${path}`);
  const resolved = await realpath(rootPath);
  if (resolved !== resolve(rootPath)) throw new Error("Public asset roots cannot traverse symlinks.");
  let current = resolved;
  for (const segment of path.split("/")) {
    current = join(current, segment);
    if ((await lstat(current)).isSymbolicLink()) throw new Error("Public files cannot traverse symlinks.");
  }
  const stat = await lstat(current);
  if (!stat.isFile() || stat.size > 64 * 1024 * 1024) throw new Error("Public file is not a bounded regular file.");
  const contentType = mime[extname(path)];
  if (!contentType) throw new Error("Refusing to publish a nonpublic file extension.");
  const bytes = await readFile(current);
  const hash = digest(bytes);
  if (expected && (hash !== expected.sha256 || bytes.length !== expected.bytes
    || contentType !== expected.contentType)) throw new Error(`Compiled source asset pin/MIME mismatch: ${expected.id}.`);
  return { url, path, sha256: hash, content_type: contentType };
}

export async function collectBuild(directory: string): Promise<PublicFile[]> {
  const files: PublicFile[] = [];
  async function visit(subpath: string): Promise<void> {
    for (const entry of await readdir(join(directory, subpath), { withFileTypes: true })) {
      const path = subpath ? `${subpath}/${entry.name}` : entry.name;
      if (["clubscape-web.json", "client/build.json", "client/build-artifact.json"].includes(path)) continue;
      if (entry.isSymbolicLink() || entry.name.startsWith(".")) throw new Error("Nonpublic output in browser build.");
      if (entry.isDirectory()) await visit(path);
      else {
        const url = path === "index.html" ? "/" : `/${path}`;
        // / is the one intentional route alias in the account-server contract.
        const file = await publicFile(directory, path, path === "index.html" ? "/index.html" : url);
        files.push({ ...file, url });
      }
    }
  }
  await visit("");
  if (!files.some((file) => file.url === "/")) throw new Error("Browser build has no index.html.");
  if (files.length > 20_000) throw new Error("Public browser build exceeds its file-count limit.");
  return files.sort((a, b) => a.url.localeCompare(b.url, "en"));
}

function repoInput(value: string): string {
  const path = resolve(root, value);
  if (path === root || !path.startsWith(root + sep)) throw new Error("Build inputs must remain inside this repository/worktree.");
  return path;
}

export async function deliver(): Promise<void> {
  const dist = resolve(root, "web/dist");
  const approval = JSON.parse(await readFile(resolve(root, "milestones/approvals/m1-reference-pack-v1.3.0.json"), "utf8")) as {
    authority: string; decision: string; reference_pack: string; reference_pack_sha256: string;
  };
  if (approval.authority !== "owner" || approval.decision !== "approved"
    || approval.reference_pack_sha256 !== SOURCE_PACK_SHA256) throw new Error("Exact source-pack approval is absent.");
  const pack = await readFile(repoInput(approval.reference_pack));
  if (digest(pack) !== SOURCE_PACK_SHA256) throw new Error("Approved source pack digest changed.");
  const contractBytes = await readFile(resolve(root, "spec/m1-benchmark-contract.json"));
  const contract = JSON.parse(contractBytes.toString("utf8")) as { visual_settings: Record<string, unknown> };
  if (digest(contractBytes) !== BENCHMARK_CONTRACT_SHA256) throw new Error("Benchmark source contract digest changed.");
  await mkdir(join(dist, "client/wasm"), { recursive: true });
  for (const file of ["clubscape_wasm.js", "clubscape_wasm_bg.wasm"]) {
    await copyFile(resolve(root, "web/generated/protocol", file), join(dist, "client/wasm", file));
  }
  let content: { path: string; sha256: string; owner: "web" | "game" } | null = null;
  const external: PublicFile[] = [];
  const manifestInput = process.env.CLUBSCAPE_CLIENT_MANIFEST;
  if (manifestInput) {
    const input = repoInput(manifestInput);
    if (await realpath(input) !== input) throw new Error("Content manifest inputs cannot traverse symlinks.");
    const bytes = await readFile(input);
    if (bytes.length > 8 * 1024 * 1024) throw new Error("Public ContentManifest exceeds its byte budget.");
    const manifest = parseContentManifest(JSON.parse(bytes.toString("utf8")));
    const owner = process.env.CLUBSCAPE_CONTENT_OWNER ?? "game";
    if (owner !== "web" && owner !== "game") throw new Error("Content owner must be web or game.");
    const manifestRoute = process.env.CLUBSCAPE_CONTENT_PATH ?? "/content/manifest.json";
    publicPath(manifestRoute, "/content/");
    const assetRoot = repoInput(process.env.CLUBSCAPE_CLIENT_ASSET_ROOT ?? dirname(relative(root, repoInput(manifestInput))));
    for (const asset of manifest.assets) {
      const path = asset.url.slice(1);
      const record = await publicFile(assetRoot, path, asset.url, asset);
      if (owner === "web") {
        const output = join(dist, path);
        await mkdir(dirname(output), { recursive: true });
        await copyFile(join(assetRoot, path), output);
        await publicFile(dist, path, asset.url, asset);
      } else external.push(record);
    }
    if (owner === "web") {
      const output = join(dist, manifestRoute.slice(1));
      await mkdir(dirname(output), { recursive: true });
      await writeFile(output, bytes);
    } else {
      external.push({ url: manifestRoute, path: manifestRoute.slice(1), sha256: digest(bytes), content_type: "application/json" });
    }
    content = { path: manifestRoute, sha256: digest(bytes), owner };
  }
  const files = await collectBuild(dist);
  let total = 0;
  for (const file of files) total += (await lstat(join(dist, file.path))).size;
  if (total > 512 * 1024 * 1024) throw new Error("Browser build exceeds the server's total byte limit.");
  const artifact = canonicalJson({
    schemaVersion: 1, sourcePackSha256: SOURCE_PACK_SHA256,
    benchmarkContractSha256: BENCHMARK_CONTRACT_SHA256, files, gameFiles: external, content,
    exclusions: ["client/build.json", "client/build-artifact.json", "clubscape-web.json"],
  }) + "\n";
  if (Buffer.byteLength(artifact) > 2 * 1024 * 1024) throw new Error("Build identity manifest exceeds its byte budget.");
  const artifactSha256 = digest(artifact);
  await writeFile(join(dist, "client/build-artifact.json"), artifact);
  files.push(await publicFile(dist, "client/build-artifact.json", "/client/build-artifact.json"));
  const components = Object.fromEntries(await Promise.all(["renderer", "ui", "audio"].map(async (name) => {
    const path = join(root, "web", name, "index.ts");
    return [name, await lstat(path).then((stat) => stat.isFile() && !stat.isSymbolicLink()).catch(() => false)];
  })));
  const revision = execFileSync("git", ["rev-parse", "HEAD"], { cwd: root, encoding: "utf8" }).trim();
  await writeFile(join(dist, "client/build.json"), canonicalJson({
    schemaVersion: 1, buildId: `clubscape-${revision}-${artifactSha256.slice(0, 16)}`,
    buildArtifactSha256: artifactSha256, artifactPath: "/client/build-artifact.json",
    sourcePackSha256: SOURCE_PACK_SHA256, benchmarkContractSha256: BENCHMARK_CONTRACT_SHA256,
    content, visualSettings: contract.visual_settings, components,
  }) + "\n");
  files.push(await publicFile(dist, "client/build.json", "/client/build.json"));
  if (files.length > 20_000) throw new Error("Public browser delivery exceeds its file-count limit.");
  const output = JSON.stringify({ schema_version: 1, files }, null, 2) + "\n";
  if (Buffer.byteLength(output) > 2 * 1024 * 1024) throw new Error("Web delivery manifest exceeds its byte limit.");
  await writeFile(join(dist, "clubscape-web.json"), output);
  console.log(JSON.stringify({
    kind: "browser-build-delivery", publicFiles: files.length, buildArtifactSha256: artifactSha256,
    components, contentConfigured: content !== null, gameplayAccepted: false, presentationAccepted: false,
  }));
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await deliver();
