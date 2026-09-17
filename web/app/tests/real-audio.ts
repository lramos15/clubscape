import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import type { Page } from "playwright-core";
import { sourceAudioDefaults, sourceSliderToMixer, sourceMixerToAssetGain } from "../../audio/index.ts";
import type { SourceMusicState } from "../../audio/index.ts";

type Harness = {
  app: import("../client.ts").BrowserApp; audio: import("../audio.ts").SourceAudioSession;
  assets: import("../assets.ts").AssetLoader; errors: Array<{ message: string; recoverable: boolean }>;
  button: HTMLButtonElement; trusted: boolean;
};
type AudioWindow = Window & { __clubscapeAudioShellCheck?: Harness };
interface SupplementOracle {
  asset_id: string; frames: number; sample_rate: number; channels: number;
  float32_channel_sha256: string[]; full_scale_samples: number;
}

async function checkSupplementInputs(page: Page): Promise<unknown> {
  const oracles = JSON.parse(await readFile(new URL("../../../research/browser-audio-policy/supplement-browser-oracles.json", import.meta.url), "utf8")) as {
    manifest_sha256: string; assets: SupplementOracle[];
  };
  assert.equal(oracles.manifest_sha256, "840aef91bac9a1fd042bdb1c3662ff92a378e279f48335108730e168af550d91");
  const decoded = [];
  for (const oracle of oracles.assets) {
    const actual = await page.evaluate(async (id) => {
      const { assets } = (window as AudioWindow).__clubscapeAudioShellCheck!;
      const input = await assets.bytes(id);
      const context = new OfflineAudioContext(2, 1, 22050);
      const buffer = await context.decodeAudioData(new Uint8Array(input).buffer);
      const hashes: string[] = [];
      let fullScale = 0;
      for (let channel = 0; channel < buffer.numberOfChannels; channel++) {
        const samples = buffer.getChannelData(channel);
        for (const sample of samples) {
          if (!Number.isFinite(sample) || Math.abs(sample) > 1) throw new Error("Native255 decode violated the original finite PCM bounds.");
          if (sample === -1 || sample === 32767 / 32768) fullScale++;
        }
        hashes.push(Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", samples)),
          (byte) => byte.toString(16).padStart(2, "0")).join(""));
      }
      return { assetId: id, frames: buffer.length, channels: buffer.numberOfChannels, sampleRate: buffer.sampleRate, hashes, fullScale };
    }, oracle.asset_id);
    assert.equal(actual.frames, oracle.frames);
    assert.equal(actual.channels, oracle.channels);
    assert.equal(actual.sampleRate, oracle.sample_rate);
    assert.deepEqual(actual.hashes, oracle.float32_channel_sha256);
    assert.equal(actual.fullScale, oracle.full_scale_samples);
    decoded.push(actual);
  }
  const playback = [];
  for (const group of [64, 327, 163, 145]) {
    // Playback-control fixture only: no account/world, unlock grant, or fabricated committed event.
    const input: SourceMusicState = { mode: "single", areaMode: "modern", unlockedGroups: [group],
      selectedGroup: group, playlistGroups: [], loopEnabled: false };
    await page.evaluate((state) => {
      const { audio } = (window as AudioWindow).__clubscapeAudioShellCheck!;
      audio.volume("music", 1);
      audio.setMusicState(state);
    }, input);
    await page.waitForFunction((group) => {
      const { audio } = (window as AudioWindow).__clubscapeAudioShellCheck!;
      return audio.snapshot().voices.some((voice) => voice.kind === "music" && voice.sourceId === group
        && voice.renderedNativeLevel === 255 && voice.appliedNativeLevel === 255 && voice.gain === 1);
    }, group, { timeout: 30_000 });
    const current = await page.evaluate((group) => {
      const { audio } = (window as AudioWindow).__clubscapeAudioShellCheck!;
      const snapshot = audio.snapshot();
      return { voice: snapshot.voices.find((voice) => voice.kind === "music" && voice.sourceId === group)!,
        controls: audio.controls(), loaded: audio.observations() };
    }, group);
    assert.equal(current.voice.assetId, `asset.source.osrs.cache2695.audio-supplement.music.${group}.native255`);
    assert(current.loaded.some((asset) => asset.id === current.voice.assetId && asset.decoded));
    assert.equal(current.controls.musicContinuation, "native-bound");
    assert.deepEqual(current.controls.providedMusicState, input);
    await page.evaluate((state) => {
      const { audio } = (window as AudioWindow).__clubscapeAudioShellCheck!;
      for (let index = 0; index < 5; index++) audio.setMusicState(state);
    }, input);
    await page.waitForTimeout(100);
    assert.equal(await page.evaluate((group) => (window as AudioWindow).__clubscapeAudioShellCheck!.audio.snapshot()
      .voices.find((voice) => voice.kind === "music" && voice.sourceId === group)?.id, group), current.voice.id);
    playback.push({ sourceId: group, assetId: current.voice.assetId, renderedNativeLevel: current.voice.renderedNativeLevel,
      appliedNativeLevel: current.voice.appliedNativeLevel, gain: current.voice.gain, idempotentState: true });
  }
  const rejection = await page.evaluate(() => {
    const { audio } = (window as AudioWindow).__clubscapeAudioShellCheck!;
    try {
      audio.setMusicState({ mode: "single", areaMode: "modern", unlockedGroups: [145],
        selectedGroup: 64, playlistGroups: [], loopEnabled: false });
      return null;
    } catch (error) { return error instanceof Error ? error.message : "non-error rejection"; }
  });
  assert.match(rejection ?? "", /AUDIO_SOURCE_MUSIC/);
  return { kind: "native-audio-publication-controls-fixture-only", decoded, playback,
    fixtureMusicStateNotAccountUnlocks: true, nextSongCallbackRequired: false, lockedSelectionRejected: true,
    syntheticWorld: false, committedGameEventsFabricated: false, physicalSpeakersAccepted: false, gameJourney: false };
}

/** Actual title/factory composition only; no synthetic world or game cue loop. */
export async function checkTitleAudio(page: Page): Promise<unknown> {
  const configured = await page.evaluate(async () => {
    const path = "/client/bridge.js";
    const api = await import(path) as typeof import("../bridge.ts");
    const build = await api.loadBuild();
    if (!build.content) return false;
    const loaded = await api.verifiedJson(build.content.path, build.content.sha256, 8 * 1024 * 1024, fetch.bind(globalThis));
    const manifest = api.parseContentManifest(loaded.value);
    if (!Object.values(api.AUDIO_INPUTS).every((input) => manifest.aliases?.[input.path])) {
      throw new Error("A required source audio metadata route is missing, including the native supplement.");
    }
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
      componentFailure(error) {
        errors.push({ message: error.message, recoverable: error.recoverable });
        app.report(error);
      },
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
  try {
    const before = await page.evaluate(() => {
      const state = (window as unknown as { __clubscapeAudioShellCheck: Harness }).__clubscapeAudioShellCheck;
      return { audio: state.audio.snapshot(), app: state.app.state(), errors: state.errors };
    });
    assert.equal(before.app.phase, "title");
    assert.equal(before.app.soundEnabled, false);
    assert.equal(before.audio.pendingGesture, true);
    assert.deepEqual(before.audio.nativeMixer, sourceAudioDefaults().mixer);
    assert.equal(before.audio.masterPercent, sourceAudioDefaults().sliders.master);
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
      return { audio: state.audio.snapshot(), controls: state.audio.controls(), assets: state.audio.observations(), trusted: state.trusted };
    });
    await page.waitForTimeout(120);
    const later = await page.evaluate(() => (window as unknown as { __clubscapeAudioShellCheck: Harness }).__clubscapeAudioShellCheck.audio.snapshot().currentTime);
    assert(later > running.audio.currentTime);
    assert.equal(running.trusted, true);
    assert.equal(running.audio.sampleRate, 22050);
    const adjusted = await page.evaluate(() => {
      const state = (window as unknown as { __clubscapeAudioShellCheck: Harness }).__clubscapeAudioShellCheck;
      state.app.audioVolume("music", 0.5);
      return state.audio.controls();
    });
    assert.equal(adjusted.channels.music.normalizedPosition, 0.5);
    assert.equal(adjusted.channels.music.percent, 50);
    assert.equal(adjusted.channels.music.nativeMixer, sourceSliderToMixer("music", 50));
    assert.equal(adjusted.channels.music.referenceGains.native128, sourceMixerToAssetGain(sourceSliderToMixer("music", 50), 128));
    assert.equal(adjusted.channels.music.referenceGains.native255, sourceMixerToAssetGain(sourceSliderToMixer("music", 50), 255));
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
    const supplement = await checkSupplementInputs(page);
    return {
      kind: "real-title-audio-shell-composition", result: "passed", trustedGesture: true,
      sampleRate: running.audio.sampleRate, contextClockAdvanced: true,
      nativeControls: running.controls,
      changedSourceSlider: adjusted,
      loaded: running.assets, backgroundSourceIds: running.audio.background.groups,
      sourceDefaultsChanged: false, disconnectPreservedSelection: true,
      explicitTitleReset: true, supplement, gameCuesOrJourneyTested: false, presentationAccepted: false,
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
