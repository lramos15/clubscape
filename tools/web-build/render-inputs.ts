import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFile, lstat, mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import type { RenderAssetManifest } from "../../web/renderer/src/index.ts";
import { RENDER_MANIFEST_SHA256 } from "../../web/app/render-identity.ts";
import { DEFAULT_RENDER_INPUTS, verifyReproductionManifest } from "./render-data.ts";
import { validateRenderBlockPackage } from "./render-package.ts";

const root = resolve(fileURLToPath(new URL("../../", import.meta.url)));
const source = resolve(root, "assets/compiled/render");
const output = resolve(root, process.argv[2] ?? DEFAULT_RENDER_INPUTS);
const reuse = process.env.CLUBSCAPE_RENDER_REUSE_INPUTS ? resolve(root, process.env.CLUBSCAPE_RENDER_REUSE_INPUTS) : null;
const packagePath = process.env.CLUBSCAPE_RENDER_BLOCK_PACKAGE ? resolve(root, process.env.CLUBSCAPE_RENDER_BLOCK_PACKAGE) : null;
if (!output.startsWith(root + sep) || output === source) throw new Error("Reproduction needs a separate owned directory inside the worktree.");
if (reuse !== null && !reuse.startsWith(root + sep)) throw new Error("Reused renderer inputs must remain inside the worktree.");
if (packagePath !== null && !packagePath.startsWith(root + sep)) throw new Error("Copy the authorized block package into this worktree before installation.");
const hash = (bytes: Uint8Array): string => createHash("sha256").update(bytes).digest("hex");
const absent = (error: unknown): null => {
  if (error instanceof Error && "code" in error && error.code === "ENOENT") return null;
  throw error;
};
const bytes = await readFile(resolve(source, "manifest.json"));
if (hash(bytes) !== RENDER_MANIFEST_SHA256) throw new Error("Published render manifest changed.");
const manifest = JSON.parse(bytes.toString("utf8")) as RenderAssetManifest;
const indexBytes = await readFile(resolve(source, "blocks.index.json"));
const index = validateRenderBlockPackage(JSON.parse(indexBytes.toString("utf8")), manifest, RENDER_MANIFEST_SHA256);
if (packagePath !== null) {
  const stat = await lstat(packagePath);
  if (!stat.isFile() || stat.isSymbolicLink() || stat.size !== index.pack.size_bytes
    || hash(await readFile(packagePath)) !== index.pack.sha256) {
    throw new Error("The supplied archive is not the exact current block/MICN package. Older packs cannot substitute for its sidecars.");
  }
}
const packaged = new Set(index.files.map((entry) => entry.file));
await mkdir(output, { recursive: true });
const prior = await readFile(resolve(output, "manifest.json")).catch(absent);
if (prior) verifyReproductionManifest(manifest, JSON.parse(prior.toString("utf8")) as RenderAssetManifest);
if (!prior) await writeFile(resolve(output, "manifest.json"), bytes, { flag: "wx" });
const priorIndex = await readFile(resolve(output, "blocks.index.json")).catch(absent);
if (priorIndex && !priorIndex.equals(indexBytes)) throw new Error("Output already contains a different block-package index; use a new input directory.");
if (!priorIndex) await writeFile(resolve(output, "blocks.index.json"), indexBytes, { flag: "wx" });
let needBlocks = false;
let reused = 0;
for (const [name, pin] of Object.entries(manifest.files)) {
  if (name === "tables.bin" || name.startsWith("models/baked/")) continue;
  if ((name.startsWith("scenes/") || name.startsWith("blocks/")) && !name.endsWith(".gz")) continue;
  const target = resolve(output, name);
  if (!target.startsWith(output + sep)) throw new Error("Unsafe original renderer input path.");
  const exists = await lstat(target).catch(absent);
  if (exists) {
    if (!exists.isFile() || exists.isSymbolicLink() || exists.size !== pin.size_bytes || hash(await readFile(target)) !== pin.sha256) {
      throw new Error(`Reproduced input changed: ${name}`);
    }
    continue;
  }
  if (packagePath !== null && packaged.has(name)) continue;
  if (name.startsWith("blocks/")) {
    const previous = reuse === null ? null : await readFile(resolve(reuse, name)).catch(absent);
    if (previous !== null) {
      if (previous.length !== pin.size_bytes || hash(previous) !== pin.sha256) throw new Error(`Reused block differs from the current published pin: ${name}`);
      await mkdir(dirname(target), { recursive: true });
      await writeFile(target, previous, { flag: "wx" });
      reused++;
    } else needBlocks = true;
    continue;
  }
  const original = resolve(source, name);
  const data = await readFile(original);
  if (data.length !== pin.size_bytes || hash(data) !== pin.sha256) throw new Error(`Published renderer input mismatch: ${name}`);
  await mkdir(dirname(target), { recursive: true });
  await copyFile(original, target);
}
if (packagePath !== null) {
  execFileSync("python3", ["-B", "tools/render-assets/export.py", "--profile", "unpack-blocks", "--output", output, packagePath], {
    cwd: root, stdio: "inherit", timeout: 180_000,
  });
} else if (needBlocks) {
  execFileSync("python3", ["-B", "tools/render-assets/export.py", "--profile", "blocks", "--output", output], {
    cwd: root, stdio: "inherit", timeout: 1_800_000,
  });
}
let count = 0, total = 0;
for (const block of manifest.blocks ?? []) {
  for (const name of [block.file_gz ?? block.file, block.models_file_gz ?? block.models_file]) {
    const pin = manifest.files[name];
    if (!pin) throw new Error("A reproduced block lost its published identity.");
    const data = await readFile(resolve(output, name));
    if (data.length !== pin.size_bytes || hash(data) !== pin.sha256) throw new Error(`Original block reproduction differs: ${name}`);
    count++; total += data.length;
  }
}
const exporterBytes = await readFile(resolve(output, "manifest.json"));
const omitted = verifyReproductionManifest(manifest, JSON.parse(exporterBytes.toString("utf8")) as RenderAssetManifest);
await mkdir(resolve(root, ".local/evidence"), { recursive: true });
if (omitted.length > 0) {
  await writeFile(resolve(root, ".local/evidence/render-block-exporter-manifest.json"), exporterBytes);
  await writeFile(resolve(output, "manifest.json"), bytes);
  await writeFile(resolve(output, "blocks.index.json"), indexBytes);
}
execFileSync("python3", ["-B", "tools/render-assets/export.py", "--profile", "verify-blocks", "--output", output], {
  cwd: root, stdio: "inherit", timeout: 180_000,
});
const report = { kind: "original-world-block-reproduction", manifestSha256: RENDER_MANIFEST_SHA256,
  invokedOriginalExporter: packagePath === null && needBlocks, verifiedInventoryManifestSha256: hash(exporterBytes),
  verifiedPublishedPackage: packagePath === null ? null : index.pack,
  verifiedPackageMembers: index.files.length,
  reusedHashIdenticalBlockFiles: reused,
  exporterOmittedValidationOrRawTwins: omitted,
  directory: output, blocks: manifest.blocks?.length ?? 0, files: count, bytes: total,
  browserNeedsJdkOrCache: false, presentationAccepted: false };
await writeFile(resolve(root, ".local/evidence/render-block-reproduction.json"), JSON.stringify(report, null, 2) + "\n");
console.log(JSON.stringify(report));
