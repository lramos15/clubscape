import { createProtocolClient } from "./bridge.ts";
import type { BrowserClient } from "../generated/protocol/clubscape_wasm.js";
import { Benchmark } from "./benchmark.ts";
import { loadBuild } from "./build.ts";
import { loadComponents } from "./components.ts";
import { mountApplication } from "./composition.ts";
import type { ApplicationHandle } from "./composition.ts";
import { appError } from "./errors.ts";
import "./style.css";
import type { AppState } from "../shared/contracts.ts";
import { installSourceFetch } from "./source-fetch.ts";
import type { GameplayUiSupport } from "./gameplay-ui.ts";

declare global {
  interface Window {
    __clubscapeClientStateV1?: { read(): Readonly<AppState>; gameplayUi(): Readonly<GameplayUiSupport> };
    __clubscapePresentationV1?: Readonly<{ mode: "live" | "early_fixture"; sceneId: string | null }>;
  }
}

let application: ApplicationHandle | null = null;
let bridge: BrowserClient | null = null;
const status = document.querySelector<HTMLElement>("#bootstrap-status")!;
const restoreFetch = installSourceFetch();

async function start(): Promise<void> {
  const build = await loadBuild();
  const benchmark = new Benchmark(build);
  window.__clubscapeBenchmarkV1 = benchmark;
  const startedAt = performance.now();
  bridge = await createProtocolClient();
  benchmark.startup(performance.now() - startedAt);
  const components = await loadComponents();
  const earlyScene = new URLSearchParams(location.search).get("presentation_scene");
  window.__clubscapePresentationV1 = Object.freeze({ mode: earlyScene === null ? "live" : "early_fixture", sceneId: earlyScene });
  const ownedBridge = bridge;
  bridge = null;
  application = await mountApplication({
    build, benchmark, bridge: ownedBridge, components, status, earlyScene,
    worldCanvas: document.querySelector<HTMLCanvasElement>("#world")!,
    uiCanvas: document.querySelector<HTMLCanvasElement>("#overlay")!,
  });
  window.__clubscapeClientStateV1 = Object.freeze({
    read: () => application!.app.state(), gameplayUi: () => application!.app.gameplayUi(),
  });
  if (earlyScene !== null) {
    status.hidden = false;
    status.textContent = `Early presentation fixture: ${earlyScene}. Not a legitimate journey or acceptance run.`;
  }
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

window.addEventListener("pagehide", () => {
  delete window.__clubscapeClientStateV1;
  delete window.__clubscapePresentationV1;
  restoreFetch();
  void application?.dispose();
}, { once: true });
