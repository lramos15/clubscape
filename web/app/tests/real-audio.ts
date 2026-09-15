import assert from "node:assert/strict";
import type { Page } from "playwright-core";

/** Actual title/factory composition only; no synthetic world or game cue loop. */
export async function checkTitleAudio(page: Page): Promise<unknown> {
  const configured = await page.evaluate(async () => {
    const path = "/client/bridge.js";
    const api = await import(path) as typeof import("../bridge.ts");
    const build = await api.loadBuild();
    if (!build.content) return false;
    const loaded = await api.verifiedJson(build.content.path, build.content.sha256, 8 * 1024 * 1024, fetch.bind(globalThis));
    const manifest = api.parseContentManifest(loaded.value);
    if (!manifest.aliases?.[api.AUDIO_INPUTS.manifest.path]) return false;
    const assets = new api.AssetLoader(manifest, loaded.sha256);
    let audio: import("../audio.ts").SourceAudioSession | null = null;
    const errors: Array<{ message: string; recoverable: boolean }> = [];
    const app = new api.BrowserApp(await api.createProtocolClient(), new api.RpcTransport(), {
      content: async () => { throw new Error("Title audio check does not enter a game world."); },
      prepareWorld: async () => { throw new Error("Title audio check has no renderer substitute."); },
      events: (world, events) => audio?.update(world, events),
      unlockAudio: () => {
        if (!audio) return Promise.reject(new Error("Audio is not initialized."));
        return audio.unlock();
      },
      audioEnabled: () => audio?.enabled() === true,
      volume: (channel, value) => audio?.volume(channel, value),
      disconnected: () => audio?.disconnected(),
    });
    audio = await api.SourceAudioSession.create(assets, manifest.assets,
      (error) => { errors.push({ message: error.message, recoverable: error.recoverable }); app.report(error); },
      (state) => app.audioStatus(api.playbackEnabled(state)));
    await app.start();
    audio.update(null, []);
    const button = document.createElement("button");
    button.id = "source-audio-contract-unlock";
    button.textContent = "Enable source audio — contract check, not game UI";
    const state = { app, audio, assets, errors, button, trusted: false };
    button.addEventListener("click", () => {
      state.trusted = navigator.userActivation.isActive;
      void app.unlockAudio().catch(() => {});
    });
    Object.assign(button.style, { position: "fixed", left: "8px", top: "8px", zIndex: "20" });
    document.body.append(button);
    Object.assign(window, { __clubscapeAudioShellCheck: state });
    return true;
  });
  if (!configured) return null;
  type Harness = {
    app: import("../client.ts").BrowserApp; audio: import("../audio.ts").SourceAudioSession;
    assets: import("../assets.ts").AssetLoader; errors: Array<{ message: string; recoverable: boolean }>;
    button: HTMLButtonElement; trusted: boolean;
  };
  try {
    const before = await page.evaluate(() => {
      const state = (window as unknown as { __clubscapeAudioShellCheck: Harness }).__clubscapeAudioShellCheck;
      return { audio: state.audio.snapshot(), app: state.app.state(), errors: state.errors };
    });
    assert.equal(before.app.phase, "title");
    assert.equal(before.app.soundEnabled, false);
    assert.equal(before.audio.pendingGesture, true);
    assert.equal(before.audio.cache.cached, 0, "Factory initialization cannot eagerly decode the whole soundtrack.");
    assert(before.errors.some((error) => error.message.includes("AUDIO_GESTURE_REQUIRED") && error.recoverable));
    await page.locator("#source-audio-contract-unlock").click();
    await page.waitForFunction(() => {
      const state = (window as unknown as { __clubscapeAudioShellCheck: Harness }).__clubscapeAudioShellCheck;
      return state.audio.enabled() && state.app.state().soundEnabled
        && state.audio.snapshot().voices.some((voice) => voice.kind === "music" && voice.sourceId === 0)
        && state.audio.observations().some((asset) => asset.decoded);
    }, undefined, { timeout: 30_000 });
    const running = await page.evaluate(() => {
      const state = (window as unknown as { __clubscapeAudioShellCheck: Harness }).__clubscapeAudioShellCheck;
      return { audio: state.audio.snapshot(), assets: state.audio.observations(), trusted: state.trusted };
    });
    await page.waitForTimeout(120);
    const later = await page.evaluate(() => (window as unknown as { __clubscapeAudioShellCheck: Harness }).__clubscapeAudioShellCheck.audio.snapshot().currentTime);
    assert(later > running.audio.currentTime);
    assert.equal(running.trusted, true);
    assert.equal(running.audio.sampleRate, 22050);
    const stopped = await page.evaluate(() => {
      const state = (window as unknown as { __clubscapeAudioShellCheck: Harness }).__clubscapeAudioShellCheck;
      state.audio.disconnected();
      return { audio: state.audio.snapshot(), enabled: state.app.state().soundEnabled };
    });
    assert.equal(stopped.audio.connected, false);
    assert.equal(stopped.audio.voices.length, 0);
    assert.equal(stopped.enabled, false);
    assert.deepEqual(stopped.audio.background.groups, running.audio.background.groups);
    await page.evaluate(() => (window as unknown as { __clubscapeAudioShellCheck: Harness }).__clubscapeAudioShellCheck.app.setScreen("title"));
    await page.waitForFunction(() =>
      (window as unknown as { __clubscapeAudioShellCheck: Harness }).__clubscapeAudioShellCheck.audio.snapshot().voices.some((voice) => voice.sourceId === 0),
    undefined, { timeout: 10_000 });
    return {
      kind: "real-title-audio-shell-composition", result: "passed", trustedGesture: true,
      sampleRate: running.audio.sampleRate, contextClockAdvanced: true,
      loaded: running.assets, backgroundSourceIds: running.audio.background.groups,
      sourceDefaultsChanged: false, disconnectPreservedSelection: true,
      explicitTitleReset: true, gameCuesOrJourneyTested: false, presentationAccepted: false,
    };
  } finally {
    await page.evaluate(async () => {
      const state = (window as unknown as { __clubscapeAudioShellCheck?: Harness }).__clubscapeAudioShellCheck;
      if (!state) return;
      try { await state.audio.dispose(); } finally { await state.app.dispose(); state.assets.dispose(); state.button.remove(); }
      Reflect.deleteProperty(window, "__clubscapeAudioShellCheck");
    });
  }
}
