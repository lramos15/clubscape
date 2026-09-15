import { defineConfig } from "vite";
import { fileURLToPath } from "node:url";

export default defineConfig({
  publicDir: false,
  build: {
    target: "es2024",
    outDir: "dist",
    emptyOutDir: true,
    assetsDir: "client",
    assetsInlineLimit: 0,
    sourcemap: false,
    cssCodeSplit: true,
    rolldownOptions: {
      preserveEntrySignatures: "strict",
      input: {
        app: fileURLToPath(new URL("./index.html", import.meta.url)),
        bridge: fileURLToPath(new URL("./app/bridge.ts", import.meta.url)),
      },
      output: {
        entryFileNames: (entry) => entry.name === "bridge" ? "client/bridge.js" : "client/[name]-[hash].js",
      },
    },
  },
  server: {
    host: "127.0.0.1",
    strictPort: true,
    cors: false,
    // Development transport is the real account server serving dist, not a proxy.
    fs: { strict: true },
  },
});
