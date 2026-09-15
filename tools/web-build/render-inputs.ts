import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFile, lstat, mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import type { RenderAssetManifest } from "../../web/renderer/src/index.ts";
import { RENDER_MANIFEST_SHA256 } from "../../web/app/render-identity.ts";
import { DEFAULT_RENDER_INPUTS, verifyReproductionManifest } from "./render-data.ts";

const root = resolve(fileURLToPath(new URL("../../", import.meta.url)));
const source = resolve(root, "assets/compiled/render");
const output = resolve(root, process.argv[2] ?? DEFAULT_RENDER_INPUTS);
if (!output.startsWith(root + sep) || output === source) throw new Error("Reproduction needs a separate owned directory inside the worktree.");
const hash = (bytes: Uint8Array): string => createHash("sha256").update(bytes).digest("hex");
const absent = (error: unknown): null => {
  if (error instanceof Error && "code" in error && error.code === "ENOENT") return null;
  throw error;
};
const bytes = await readFile(resolve(source, "manifest.json"));
if (hash(bytes) !== RENDER_MANIFEST_SHA256) throw new Error("Published render manifest changed.");
const manifest = JSON.parse(bytes.toString("utf8")) as RenderAssetManifest;
await mkdir(output, { recursive: true });
const prior = await readFile(resolve(output, "manifest.json")).catch(absent);
if (prior) verifyReproductionManifest(manifest, JSON.parse(prior.toString("utf8")) as RenderAssetManifest);
if (!prior) await writeFile(resolve(output, "manifest.json"), bytes, { flag: "wx" });
let needBlocks = false;
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
  if (name.startsWith("blocks/")) { needBlocks = true; continue; }
  const original = resolve(source, name);
  const data = await readFile(original);
  if (data.length !== pin.size_bytes || hash(data) !== pin.sha256) throw new Error(`Published renderer input mismatch: ${name}`);
  await mkdir(dirname(target), { recursive: true });
  await copyFile(original, target);
}
if (needBlocks) {
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
}
const report = { kind: "original-world-block-reproduction", manifestSha256: RENDER_MANIFEST_SHA256,
  invokedOriginalExporter: needBlocks, verifiedInventoryManifestSha256: hash(exporterBytes),
  exporterOmittedValidationOrRawTwins: omitted,
  directory: output, blocks: manifest.blocks?.length ?? 0, files: count, bytes: total,
  browserNeedsJdkOrCache: false, presentationAccepted: false };
await writeFile(resolve(root, ".local/evidence/render-block-reproduction.json"), JSON.stringify(report, null, 2) + "\n");
console.log(JSON.stringify(report));
