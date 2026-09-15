import { createProtocolClient } from "./bridge.ts";
import type { BrowserClient } from "../generated/protocol/clubscape_wasm.js";
import { Benchmark } from "./benchmark.ts";
import { loadBuild } from "./build.ts";
import { loadComponents } from "./components.ts";
import { mountApplication } from "./composition.ts";
import type { ApplicationHandle } from "./composition.ts";
import { appError } from "./errors.ts";
import "./style.css";

let application: ApplicationHandle | null = null;
let bridge: BrowserClient | null = null;
const status = document.querySelector<HTMLElement>("#bootstrap-status")!;

async function start(): Promise<void> {
  const build = await loadBuild();
  const benchmark = new Benchmark(build);
  window.__clubscapeBenchmarkV1 = benchmark;
  const startedAt = performance.now();
  bridge = await createProtocolClient();
  benchmark.startup(performance.now() - startedAt);
  const components = await loadComponents();
  const ownedBridge = bridge;
  bridge = null;
  application = await mountApplication({
    build, benchmark, bridge: ownedBridge, components, status,
    worldCanvas: document.querySelector<HTMLCanvasElement>("#world")!,
    uiCanvas: document.querySelector<HTMLCanvasElement>("#overlay")!,
  });
}

void start().catch((value: unknown) => {
  const error = appError(value, "ClubScape could not start. Check the built same-origin assets and required browser capabilities.");
  bridge?.free();
  bridge = null;
  // Only a bootstrap/integration failure diagnostic. Never a substitute game UI.
  status.hidden = false;
  status.setAttribute("role", "alert");
  status.textContent = `${error.message}\nError ID: ${error.errorId}`;
});

window.addEventListener("pagehide", () => { void application?.dispose(); }, { once: true });
