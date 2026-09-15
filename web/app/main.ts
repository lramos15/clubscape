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
import type { SourceAudioSession } from "./audio.ts";
import { presentationOptions } from "./presentation.ts";

declare global {
  interface Window {
    __clubscapeClientStateV1?: {
      read(): Readonly<AppState>; gameplayUi(): Readonly<GameplayUiSupport>;
      audioControls(): ReturnType<SourceAudioSession["controls"]> | null;
    };
    __clubscapePresentationV1?: Readonly<{
      mode: "live" | "early_fixture" | "recorded_camera"; sceneId: string | null; cameraInput: string | null;
      projection: "renderer-viewport-only-helper"; fullHudProjectionMatched: false;
    }>;
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
  const { earlyScene, recordedCamera } = presentationOptions(location.search);
  window.__clubscapePresentationV1 = Object.freeze({
    mode: earlyScene ? "early_fixture" : recordedCamera ? "recorded_camera" : "live",
    sceneId: earlyScene, cameraInput: earlyScene ?? recordedCamera,
    projection: "renderer-viewport-only-helper", fullHudProjectionMatched: false,
  });
  const ownedBridge = bridge;
  bridge = null;
  application = await mountApplication({
    build, benchmark, bridge: ownedBridge, components, status, earlyScene, recordedCamera,
    worldCanvas: document.querySelector<HTMLCanvasElement>("#world")!,
    uiCanvas: document.querySelector<HTMLCanvasElement>("#overlay")!,
  });
  window.__clubscapeClientStateV1 = Object.freeze({
    read: () => application!.app.state(), gameplayUi: () => application!.app.gameplayUi(),
    audioControls: () => application!.app.audioControls(),
  });
  if (earlyScene !== null || recordedCamera !== null) {
    status.hidden = false;
    status.textContent = earlyScene ? `Early presentation fixture: ${earlyScene}. Not a legitimate journey or acceptance run.`
      : `Real streamed source region, explicit recorded camera: ${recordedCamera}. Viewport-only projection; matched full-HUD projection awaits the renderer. Not a journey or fidelity acceptance run.`;
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
