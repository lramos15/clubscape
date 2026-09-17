import { execFileSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";

const root = fileURLToPath(new URL("../../", import.meta.url));
const output = resolve(root, "web/generated/protocol");
const version = execFileSync("wasm-bindgen", ["--version"], { cwd: root, encoding: "utf8" }).trim();
if (version !== "wasm-bindgen 0.2.128") {
  throw new Error(`This crate requires wasm-bindgen CLI 0.2.128; found ${version}. No mismatched glue was emitted.`);
}
const manifest = await readFile(resolve(root, "crates/wasm/Cargo.toml"), "utf8");
if (!manifest.includes('wasm-bindgen = "=0.2.128"')) throw new Error("WASM crate/CLI pin mismatch.");
await mkdir(output, { recursive: true });
execFileSync("cargo", ["build", "--quiet", "--release", "--lib", "-p", "clubscape-wasm", "--target", "wasm32-unknown-unknown"], {
  cwd: root, stdio: "inherit",
});
const metadata = JSON.parse(execFileSync("cargo", ["metadata", "--no-deps", "--format-version=1"], {
  cwd: root, encoding: "utf8",
})) as { target_directory: string };
execFileSync("wasm-bindgen", [
  resolve(metadata.target_directory, "wasm32-unknown-unknown/release/clubscape_wasm.wasm"),
  "--target", "web", "--out-dir", output, "--out-name", "clubscape_wasm",
], { cwd: root, stdio: "inherit" });
await writeFile(resolve(output, "toolchain.json"), JSON.stringify({
  schemaVersion: 1, bindgen: "0.2.128", rust: execFileSync("rustc", ["--version"], { cwd: root, encoding: "utf8" }).trim(),
}) + "\n");
console.log("Built the real client-core/Protobuf WASM bridge (wasm-bindgen 0.2.128).");
