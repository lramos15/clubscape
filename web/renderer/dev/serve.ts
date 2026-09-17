/**
 * Minimal static file server for the developer fixture page. Serves the repository root read-only
 * on 127.0.0.1 so `/web/renderer/dev/index.html` can fetch `/assets/compiled/render/...`.
 * Usage: node web/renderer/dev/serve.ts [port]   (run from the repository root)
 */
import { createServer } from "node:http";
import { createReadStream } from "node:fs";
import { stat } from "node:fs/promises";
import path from "node:path";

const root = process.cwd();
const port = Number(process.argv[2] ?? "4173");
const types: Record<string, string> = {
  ".html": "text/html; charset=utf-8", ".js": "text/javascript; charset=utf-8", ".mjs": "text/javascript; charset=utf-8",
  ".wasm": "application/wasm", ".json": "application/json", ".bin": "application/octet-stream", ".png": "image/png",
  ".css": "text/css; charset=utf-8", ".map": "application/json",
};

const server = createServer(async (request, response) => {
  try {
    const url = new URL(request.url ?? "/", `http://${request.headers.host ?? "127.0.0.1"}`);
    let pathname = decodeURIComponent(url.pathname);
    if (pathname === "/") pathname = "/web/renderer/dev/index.html";
    const file = path.resolve(root, `.${pathname}`);
    if (!file.startsWith(root + path.sep) || /(^|\/)\.(git|local|worktrees)(\/|$)/.test(pathname)) {
      response.writeHead(403); response.end("forbidden"); return;
    }
    const info = await stat(file).catch(() => null);
    if (!info || !info.isFile()) { response.writeHead(404); response.end(`not found: ${pathname}`); return; }
    response.writeHead(200, {
      "content-type": types[path.extname(file)] ?? "application/octet-stream",
      "content-length": info.size,
      "cache-control": "no-store",
    });
    createReadStream(file).pipe(response);
  } catch (error) {
    response.writeHead(500); response.end(String(error));
  }
});

server.listen(port, "127.0.0.1", () => {
  console.log(`fixture server http://127.0.0.1:${port}/ root=${root}`);
});
