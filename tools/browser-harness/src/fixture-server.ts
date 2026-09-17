import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";
import path from "node:path";
import { parseArgs } from "node:util";
import { canonicalJson, harnessRoot, loadConfig, requireCondition, sha256 } from "./config.ts";
import type { HarnessConfig } from "./config.ts";
import { expectedIdentity } from "./metrics.ts";

export async function startFixture(config: HarnessConfig, port = 0, forbiddenOrigin?: string) {
  requireCondition(config.purpose === "tool-fixture", "The fixture server cannot serve candidate/source configurations");
  const [html, shader] = await Promise.all([
    readFile(path.join(harnessRoot, "fixtures/fixture.html")),
    readFile(path.join(harnessRoot, "fixtures/triangle.wgsl")),
  ]);
  requireCondition(config.contract.requiredAssets[0].sha256 === sha256(shader), "Fixture asset pin differs from actual shader");
  let requestCount = 0;
  const server = createServer((request, response) => {
    requestCount++;
    const url = new URL(request.url ?? "/", "http://127.0.0.1");
    response.setHeader("Cache-Control", "no-store");
    response.setHeader("Cross-Origin-Opener-Policy", "same-origin");
    response.setHeader("Cross-Origin-Embedder-Policy", "require-corp");
    if (url.pathname === "/health") { response.writeHead(200, { "Content-Type": "text/plain" }); response.end("tool-fixture-only\n"); }
    else if (url.pathname === "/") { response.writeHead(200, { "Content-Type": "text/html" }); response.end(html); }
    else if (url.pathname === "/triangle.wgsl") { response.writeHead(200, { "Content-Type": "text/plain" }); response.end(shader); }
    else if (url.pathname === "/asset-manifest.json") {
      response.writeHead(200, { "Content-Type": "application/json" });
      response.end(canonicalJson(config.contract.requiredAssets));
    }
    else if (url.pathname === "/fixture-config.json") {
      response.writeHead(200, { "Content-Type": "application/json" });
      response.end(JSON.stringify({
        identity: expectedIdentity(config), assets: config.contract.requiredAssets, entities: config.contract.minimumEntities,
        forbiddenUrl: forbiddenOrigin ? `${forbiddenOrigin}/forbidden-owned-test-server` : null,
      }));
    } else { response.writeHead(404); response.end("Intentional/unknown fixture route"); }
  });
  await new Promise<void>((resolve, reject) => { server.once("error", reject); server.listen(port, "127.0.0.1", resolve); });
  const address = server.address();
  requireCondition(address && typeof address !== "string", "Fixture did not bind a loopback TCP address");
  const origin = `http://127.0.0.1:${address.port}`;
  let closed = false;
  const close = async () => {
    if (closed) return;
    closed = true;
    server.closeAllConnections();
    await new Promise<void>((resolve, reject) => server.close((error) => error ? reject(error) : resolve()));
  };
  try {
    requireCondition(await (await fetch(`${origin}/health`, { signal: AbortSignal.timeout(5000) })).text() === "tool-fixture-only\n", "Fixture health check failed");
  } catch (error) {
    await close();
    throw error;
  }
  return {
    origin,
    requestCount: () => requestCount,
    close,
  };
}

async function main() {
  const { values } = parseArgs({ options: { port: { type: "string", default: "4173" }, config: { type: "string", default: "config/fixture.json" } } });
  const port = Number(values.port);
  requireCondition(Number.isInteger(port) && port >= 1024 && port <= 65535, "Fixture port must be 1024..65535");
  const server = await startFixture(await loadConfig(values.config!), port);
  console.log(`TOOL FIXTURE ONLY responsive at ${server.origin}; stop with Ctrl-C`);
  const stop = async () => { await server.close(); };
  process.once("SIGINT", stop);
  process.once("SIGTERM", stop);
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  main().catch((error) => { console.error(`Fixture failed: ${error.message}`); process.exitCode = 1; });
}
