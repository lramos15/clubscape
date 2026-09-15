import http from "node:http";
import { readFile, mkdir } from "node:fs/promises";
import { resolve, extname, sep } from "node:path";
import { stripTypeScriptTypes } from "node:module";
import { pathToFileURL } from "node:url";

const root = resolve(import.meta.dirname, "../../..");
export const results = resolve(root, "web/ui/test-results");

export async function browserHost({ audioAssets = false } = {}) {
  await mkdir(results, { recursive: true });
  const audio = new Map();
  if (audioAssets) {
    for (const path of ["assets/manifests/osrs/audio-runtime.json", "research/audio-source/source-map.json",
      "research/reference-pack/v1/audio-reference.json", "assets/manifests/osrs/audio-m1-supplement.json"])
      audio.set(path, resolve(root, path));
    const manifest = JSON.parse(await readFile(audio.get("assets/manifests/osrs/audio-runtime.json"), "utf8"));
    const reference = JSON.parse(await readFile(audio.get("research/reference-pack/v1/audio-reference.json"), "utf8"));
    const supplement = JSON.parse(await readFile(audio.get("assets/manifests/osrs/audio-m1-supplement.json"), "utf8"));
    for (const entry of [...manifest.assets, ...reference.reference_templates, ...supplement.assets]) {
      const path = resolve(root, entry.path);
      if (!path.startsWith(root + sep)) throw new Error("Invalid original audio fixture path.");
      audio.set(entry.asset_id ?? entry.id, path);
    }
  }
  const server = http.createServer(async (request, response) => {
    const url = new URL(request.url, "http://127.0.0.1");
    if (url.pathname === "/") {
      response.setHeader("Content-Type", "text/html");
      response.end(`<!doctype html><html><head><meta charset="utf-8"><title>UI component/source fixture — not gameplay</title>
        <style>html,body{margin:0;background:#000;overflow:hidden}main{position:relative}canvas{display:block}</style>
        </head><body><main><canvas aria-label="ClubScape source UI"></canvas></main>
        <script type="module">import{sourceFixture,ownerFixture,flameFixture,filterProjection,recoveryProjection,reconnectFixture}from"/web/ui/tests/source-fixture.ts";
        window.sourceFixture=sourceFixture;window.ownerFixture=ownerFixture;window.flameFixture=flameFixture;window.filterProjection=filterProjection;window.recoveryProjection=recoveryProjection;window.reconnectFixture=reconnectFixture;</script></body></html>`);
      return;
    }
    if (url.pathname.startsWith("/audio-asset/")) {
      const path = audio.get(decodeURIComponent(url.pathname.slice(13)));
      if (!path) { response.writeHead(404).end(); return; }
      try {
        const data = await readFile(path);
        response.writeHead(200, { "Content-Type": path.endsWith(".json") ? "application/json" : "application/octet-stream",
          "Content-Length": data.byteLength, "Cache-Control": "no-store" });
        response.end(data);
      } catch (error) { response.writeHead(500).end(`Original audio fixture input failed: ${String(error)}`); }
      return;
    }
    const directories = { "/assets/": resolve(root, "assets/compiled"), "/web/ui/": resolve(root, "web/ui"),
      "/web/shared/": resolve(root, "web/shared"), "/web/audio/": resolve(root, "web/audio") };
    const entry = Object.entries(directories).find(([prefix]) => url.pathname.startsWith(prefix));
    if (!entry) { response.writeHead(404).end(); return; }
    const path = resolve(entry[1], "." + url.pathname.slice(entry[0].length - 1));
    if (!path.startsWith(entry[1] + sep)) { response.writeHead(403).end(); return; }
    try {
      let data = await readFile(path);
      const extension = extname(path);
      if (extension === ".ts") data = stripTypeScriptTypes(data.toString(), { mode: "transform", sourceMap: false });
      const mime = { ".ts": "text/javascript", ".mjs": "text/javascript", ".json": "application/json", ".png": "image/png" };
      response.setHeader("Content-Type", mime[extension] ?? "application/octet-stream");
      response.setHeader("Cache-Control", "no-store");
      response.end(data);
    } catch (error) { response.writeHead(404).end(String(error)); }
  });
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
  return { url: `http://127.0.0.1:${server.address().port}`,
    close: () => new Promise(resolve => server.close(resolve)) };
}

export async function launchBrowser({ muteAudio = false } = {}) {
  const { chromium } = await import("playwright-core");
  // Chromium's Unix socket path must fit sockaddr_un, including its generated suffix.
  const scratch = resolve(root, "web/ui/.s");
  await mkdir(scratch, { recursive: true });
  process.env.TMPDIR = scratch; process.env.TEMP = scratch; process.env.TMP = scratch;
  return chromium.launch({
    executablePath: process.env.CLUBSCAPE_CHROME ?? `${process.env.HOME}/.cache/ms-playwright/chromium-1243/chrome-linux-arm64/chrome`,
    chromiumSandbox: true, headless: false,
    args: ["--enable-unsafe-webgpu", "--enable-features=Vulkan", "--use-angle=vulkan",
      "--enable-gpu", "--ignore-gpu-blocklist", "--ozone-platform=x11", ...(muteAudio ? ["--mute-audio"] : [])],
  });
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const host = await browserHost();
  console.log(host.url);
}
