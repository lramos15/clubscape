import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("../../", import.meta.url)));
const version = execFileSync("wasm-bindgen", ["--version"], { encoding: "utf8", cwd: root }).trim();
if (version !== "wasm-bindgen 0.2.128") throw new Error("Renderer crate/CLI require wasm-bindgen0.2.128.");
execFileSync("python3", ["-B", "tools/render-assets/export.py", "--profile", "unpack"], {
  cwd: root, stdio: "pipe", maxBuffer: 2 * 1024 * 1024, timeout: 120_000,
});
try {
  execFileSync("bash", ["web/renderer/build.sh"], {
    cwd: root, stdio: "pipe", maxBuffer: 8 * 1024 * 1024, timeout: 300_000,
  });
} catch (value) {
  const error = value as { stderr?: Buffer; stdout?: Buffer };
  throw new Error(`Actual renderer build failed:\n${error.stderr?.toString().slice(-6000) ?? error.stdout?.toString().slice(-6000) ?? "No compiler diagnostic."}`);
}
const sha = async (path: string) => createHash("sha256").update(await readFile(resolve(root, path))).digest("hex");
const report = {
  kind: "actual-renderer-build", bindgen: "0.2.128",
  wasmSha256: await sha("web/renderer/pkg/clubscape_renderer_bg.wasm"),
  javascriptSha256: await sha("web/renderer/pkg/clubscape_renderer.js"),
  manifestSha256: await sha("assets/compiled/render/manifest.json"),
  presentationAccepted: false,
};
await mkdir(resolve(root, ".local/evidence"), { recursive: true });
await writeFile(resolve(root, ".local/evidence/renderer-build.json"), JSON.stringify(report, null, 2) + "\n");
console.log(JSON.stringify(report));
