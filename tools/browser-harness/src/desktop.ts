import { spawn } from "node:child_process";
import type { ChildProcess } from "node:child_process";
import { randomBytes, randomUUID } from "node:crypto";
import { mkdir, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { harnessRoot } from "./config.ts";

function authority(display: string, cookie: Buffer): Buffer {
  const word = (value: number) => { const b = Buffer.alloc(2); b.writeUInt16BE(value); return b; };
  const field = (value: Buffer) => Buffer.concat([word(value.length), value]);
  return Buffer.concat([word(65535), field(Buffer.alloc(0)), field(Buffer.from(display)),
    field(Buffer.from("MIT-MAGIC-COOKIE-1")), field(cookie)]);
}

function exited(child: ChildProcess): Promise<void> {
  if (child.exitCode !== null || child.signalCode !== null) return Promise.resolve();
  return new Promise((resolve) => child.once("exit", () => resolve()));
}

async function stop(child?: ChildProcess, graceMs = 3000): Promise<void> {
  if (!child?.pid || child.exitCode !== null || child.signalCode !== null) return;
  const done = exited(child);
  child.kill("SIGTERM");
  const timer = setTimeout(() => child.kill("SIGKILL"), graceMs);
  try { await done; } finally { clearTimeout(timer); }
}

async function main() {
  if (process.argv.length < 3) throw new Error("Usage: node src/desktop.ts <node arguments>");
  const runtime = path.join(harnessRoot, ".runtime", `d-${randomUUID().slice(0, 8)}`);
  await mkdir(runtime, { recursive: true, mode: 0o700 });
  let displayServer: ChildProcess | undefined;
  let command: ChildProcess | undefined;
  let displayLog = "";
  let interrupted = false;
  const interrupt = () => { interrupted = true; void stop(command, 25_000); };
  process.once("SIGINT", interrupt);
  process.once("SIGTERM", interrupt);
  try {
    // Chromium's Unix singleton socket has a short path limit. A relative, owned
    // runtime path avoids that limit without moving files outside this package.
    const env: NodeJS.ProcessEnv = { ...process.env, TMPDIR: path.relative(harnessRoot, runtime), XDG_RUNTIME_DIR: runtime };
    if (!env.DISPLAY && process.platform === "linux") {
      const cookie = randomBytes(16);
      const serverAuth = path.join(runtime, "server.xauthority");
      const clientAuth = path.join(runtime, "client.xauthority");
      await writeFile(serverAuth, authority("0", cookie), { mode: 0o600 });
      displayServer = spawn("Xvfb", [
        "-displayfd", "3", "-screen", "0", "2720x1600x24",
        "-nolisten", "tcp", "-nolisten", "unix", "-nolock", "-auth", serverAuth,
      ], { env, cwd: harnessRoot, stdio: ["ignore", "ignore", "pipe", "pipe"] });
      displayServer.stderr!.on("data", (chunk) => { displayLog = (displayLog + String(chunk)).slice(-8000); });
      const display = await new Promise<string>((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error("Xvfb did not announce a display within 15s")), 15_000);
        let output = "";
        displayServer!.once("error", (error) => { clearTimeout(timer); reject(error); });
        displayServer!.once("exit", () => { clearTimeout(timer); reject(new Error(`Xvfb exited: ${displayLog}`)); });
        const pipe = displayServer!.stdio[3] as NodeJS.ReadableStream;
        pipe.on("data", (chunk) => {
          output += String(chunk);
          if (/^\d+\n$/.test(output)) { clearTimeout(timer); resolve(output.trim()); }
        });
      });
      await writeFile(clientAuth, authority(display, cookie), { mode: 0o600 });
      env.DISPLAY = `:${display}`;
      env.XAUTHORITY = clientAuth;
      env.CLUBSCAPE_DISPLAY_MODE = "Xvfb 2720x1600x24; abstract Unix transport; cookie auth; no TCP/filesystem socket";
    } else {
      env.CLUBSCAPE_DISPLAY_MODE = "existing desktop display (not managed by harness)";
    }
    if (interrupted) throw new Error("Interrupted before browser command");
    command = spawn(process.execPath, process.argv.slice(2), { env, cwd: harnessRoot, stdio: "inherit" });
    process.exitCode = await new Promise<number>((resolve, reject) => {
      command!.once("error", reject);
      command!.once("exit", (code) => resolve(code ?? 1));
    });
  } finally {
    await stop(command, 25_000);
    await stop(displayServer);
    process.removeListener("SIGINT", interrupt);
    process.removeListener("SIGTERM", interrupt);
    await rm(runtime, { recursive: true, force: true });
  }
}

main().catch((error) => { console.error(`Desktop harness failed: ${error.message}`); process.exitCode = 1; });
