import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";
import { access } from "node:fs/promises";
import { webOutputDirectory } from "./output.mts";

const root = fileURLToPath(new URL("../../", import.meta.url));
const webRoot = webOutputDirectory();
await access(resolve(webRoot, "clubscape-web.json"));
if (!process.env.DATABASE_URL) throw new Error("Set DATABASE_URL to an owned account-server database. No database/account is seeded.");
const server = spawn("cargo", ["run", "--quiet", "-p", "clubscape-server"], {
  cwd: root, stdio: "inherit",
  env: { ...process.env, CLUBSCAPE_WEB_ROOT: webRoot, CLUBSCAPE_BIND: process.env.CLUBSCAPE_BIND ?? "127.0.0.1:4010" },
});
for (const signal of ["SIGINT", "SIGTERM"] as const) process.on(signal, () => server.kill(signal));
server.on("error", () => { process.exitCode = 1; });
server.on("exit", (code) => { process.exitCode = code ?? 1; });
