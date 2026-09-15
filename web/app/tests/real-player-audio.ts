import assert from "node:assert/strict";
import type { Page } from "playwright-core";
import type { SourceAudioPreferenceBinding, SourceMusicSkipResult } from "../../audio/index.ts";
import type { PlayerAudioControls } from "../player-audio.ts";
import { audioFixtureWorld } from "./player-audio-fixture.ts";
import { SOURCE_CYCLE_SECONDS, SOURCE_RATE } from "../../audio/source.ts";

interface PreferenceFixture {
  audio: import("../audio.ts").SourceAudioSession;
  composition: import("../player-audio-composition.ts").PlayerAudioComposition;
  assets: import("../assets.ts").AssetLoader;
  controls: PlayerAudioControls | null;
  world: import("../../shared/contracts.ts").WorldView;
  preparing: Promise<void>;
  releaseRead(): void;
  holdWrites(): void;
  releaseWrites(): void;
  observePreferences(enabled: boolean): void;
  reads: number;
  writes: number[];
  writing: number;
  maxWriting: number;
  order: string[];
  projected: SourceAudioPreferenceBinding | null;
  trusted: boolean;
  action: "unlock" | "skip";
  pending: Promise<void> | null;
  skip: SourceMusicSkipResult | null;
  feedback: Array<{ kind: string; message: string }>;
  button: HTMLButtonElement;
  storageKey: string;
}
type PreferenceWindow = Window & { __clubscapePreferenceFixture?: PreferenceFixture };

/** Real native audio/storage; the world/unlocks/UI port are explicit contract fixtures, never a game journey. */
export async function checkPlayerAudioPreferences(page: Page) {
  await page.evaluate(async (fixtureWorld) => {
    const path = "/client/bridge.js";
    const api = await import(path) as typeof import("../bridge.ts");
    const build = await api.loadBuild();
    if (!build.content) throw new Error("Native preference fixture needs real hash-pinned source delivery.");
    const document = await api.verifiedJson(build.content.path, build.content.sha256, 8 * 1024 * 1024, fetch.bind(globalThis));
    const manifest = api.parseContentManifest(document.value);
    const assets = new api.AssetLoader(manifest, document.sha256);
    const id = `actor.audio_contract_${crypto.randomUUID()}`;
    const world = Object.freeze({ ...fixtureWorld, player: Object.freeze({ ...fixtureWorld.player, id }) });
    const storageKey = `${api.PLAYER_AUDIO_PREFERENCE_PREFIX}${id}`;
    if (localStorage.getItem(storageKey) !== null) throw new Error("The isolated fixture key unexpectedly exists.");
    const storage = api.browserPlayerAudioStorage();
    const read = Promise.withResolvers<void>();
    let write: PromiseWithResolvers<void> | null = null;
    const order: string[] = [], feedback: PreferenceFixture["feedback"] = [];
    let composition: PreferenceFixture["composition"] | null = null;
    let observePreferences = true;
    const audio = await api.SourceAudioSession.create(assets, manifest.assets,
      (error) => feedback.push({ kind: error.errorId, message: error.message }),
      (snapshot) => { if (observePreferences) composition?.changed(snapshot.preferences); });
    const button = window.document.createElement("button");
    button.id = "native-player-preference-contract";
    button.textContent = "Native audio preference contract fixture (not game UI)";
    Object.assign(button.style, { position: "fixed", left: "8px", top: "8px", zIndex: "30" });
    const native = audio.preferenceApi();
    const state: PreferenceFixture = {
      audio, assets, controls: null, world, projected: null, reads: 0, writes: [], writing: 0, maxWriting: 0,
      preparing: Promise.resolve(), order, trusted: false, action: "unlock", pending: null, skip: null,
      feedback, button, storageKey,
      releaseRead: () => read.resolve(),
      holdWrites: () => { if (write) throw new Error("Fixture write gate already active."); write = Promise.withResolvers<void>(); },
      releaseWrites: () => { const held = write; write = null; held?.resolve(); },
      observePreferences: (enabled) => { observePreferences = enabled; },
      composition: new api.PlayerAudioComposition(new api.PlayerAudioPreferenceStore({
        async read(player) {
          state.reads++; order.push("read-start");
          await read.promise;
          const value = await storage.read(player);
          order.push("read-complete");
          return value;
        },
        async write(player, text) {
          state.writing++; state.maxWriting = Math.max(state.maxWriting, state.writing);
          state.writes.push(api.deserializeSourceAudioPreferences(text).volumes.current.music);
          try { if (write) await write.promise; await storage.write(player, text); }
          finally { state.writing--; }
        },
      }), {
        update(world, events, scene) { order.push(world ? "world" : "title"); audio.update(world, events, scene); },
        disconnected: () => audio.disconnected(),
        preferences: { ...native, apply(player, record, unlocked) {
          order.push("apply"); return native.apply(player, record, unlocked);
        } },
      }, {
        bindAudio: async () => () => {},
        bindPreferences(controls) { state.controls = controls; return () => { if (state.controls === controls) state.controls = null; }; },
        project(binding) { order.push("project"); state.projected = binding; },
      }, { unlockedGroups: () => [62, 76] },
      (error) => feedback.push({ kind: error.errorId, message: error.message })),
    };
    composition = state.composition;
    button.addEventListener("click", () => {
      state.trusted = navigator.userActivation.isActive;
      // Real resume/Skip starts inside this trusted handler, not after storage or another promise.
      const operation = state.action === "skip"
        ? state.controls!.skip(id).then((result) => { state.skip = result; })
        : audio.unlock();
      state.pending = operation.catch((error: unknown) => {
        const problem = api.audioProblem(error);
        feedback.push({ kind: problem.errorId, message: problem.message });
        throw problem;
      });
      void state.pending.catch(() => {}); // The fixture awaits/asserts this recorded result below.
    });
    window.document.body.append(button);
    Object.assign(window, { __clubscapePreferenceFixture: state });
    state.preparing = composition.prepare(world);
  }, audioFixtureWorld());
  const checks: string[] = [];
  try {
    await page.waitForFunction(() => (window as PreferenceWindow).__clubscapePreferenceFixture!.reads === 1);
    const loading = await page.evaluate(() => {
      const state = (window as PreferenceWindow).__clubscapePreferenceFixture!;
      return { order: state.order, preferences: state.audio.snapshot().preferences, status: state.composition.observe() };
    });
    assert.equal(loading.status.preferences.phase, "loading");
    assert.equal(loading.preferences, null);
    assert(!loading.order.includes("world") && !loading.order.includes("apply"));
    const initial = await page.evaluate(async () => {
      const state = (window as PreferenceWindow).__clubscapePreferenceFixture!;
      state.releaseRead(); await state.preparing;
      state.composition.events(state.world, []);
      await state.controls!.persistCurrent(state.world.player.id);
      return { order: state.order, status: state.composition.observe(), projected: state.projected,
        record: JSON.parse(localStorage.getItem(state.storageKey)!), native: state.audio.snapshot().preferences };
    });
    assert.deepEqual(initial.order, ["read-start", "read-complete", "world", "apply", "project"]);
    assert.equal(initial.status.preferences.origin, "confirmed_absent");
    assert.equal(initial.status.preferences.phase, "ready");
    assert.equal(initial.status.preferences.save, "stored");
    assert.deepEqual(initial.native, initial.projected);
    assert.deepEqual(initial.record, initial.native!.preferences);
    checks.push("genuine browser storage completes before the real native world/apply/projection boundary");
    await page.locator("#native-player-preference-contract").click();
    await page.evaluate(() => (window as PreferenceWindow).__clubscapePreferenceFixture!.pending);
    await page.waitForFunction(() => {
      const state = (window as PreferenceWindow).__clubscapePreferenceFixture!;
      return state.audio.snapshot().voices.some((voice) => voice.kind === "music" && voice.sourceId === 62);
    }, undefined, { timeout: 30_000 });
    assert.equal(await page.evaluate(() => (window as PreferenceWindow).__clubscapePreferenceFixture!.trusted), true);
    checks.push("trusted handler starts real WebAudio resume and verified original source62 playback");
    const configured = await page.evaluate(async () => {
      const state = (window as PreferenceWindow).__clubscapePreferenceFixture!, controls = state.controls!, player = state.world.player.id;
      const settle = async () => {
        const started = performance.now();
        do {
          await new Promise((resolve) => setTimeout(resolve, 20));
          if (performance.now() - started > 3000) throw new Error("The real source control queue did not drain.");
        } while (state.audio.snapshot().queueSize !== 0);
      };
      for (const channel of ["master", "music", "effects", "area"] as const) {
        controls.setPercent(player, channel, 0); await settle();
      }
      for (const channel of ["master", "music", "effects", "area"] as const) {
        controls.toggleMute(player, channel); await settle();
      }
      const fallback = controls.read()!.preferences.volumes;
      for (const [channel, percent] of [["master", 37], ["music", 21], ["effects", 66], ["area", 83]] as const) {
        controls.setPercent(player, channel, percent);
        await settle();
        controls.toggleMute(player, channel); await settle();
        controls.toggleMute(player, channel); await settle();
      }
      const slots = [...controls.read()!.preferences.music.savedPlaylist2];
      slots[2] = 62; slots[99] = 76;
      controls.setSavedPlaylist(player, 2, slots);
      controls.editSavedPlaylist(player, 2, { kind: "remove", group: 62 });
      controls.editSavedPlaylist(player, 2, { kind: "add", group: 62 });
      controls.selectPlaylist(player, 2);
      await settle();
      controls.setMusic(player, { ...controls.read()!.preferences.music,
        mode: "single", selectedGroup: 62, rememberModeOnLogin: true, keepPlayingOnPlaylistChange: true });
      await controls.persistCurrent(player);
      return { fallback, binding: controls.read(), record: JSON.parse(localStorage.getItem(state.storageKey)!),
        controls: state.audio.controls(), status: state.composition.observe() };
    });
    assert.deepEqual(configured.fallback.current, { master: 100, music: 20, effects: 45, area: 25 });
    assert.deepEqual(configured.binding!.preferences.volumes.current, { master: 37, music: 21, effects: 66, area: 83 });
    assert.deepEqual(configured.binding!.preferences.volumes.remembered, { master: 37, music: 21, effects: 66, area: 83 });
    assert.deepEqual(["music", "effects", "area"].map((channel) => configured.controls.channels[channel as "music" | "effects" | "area"].nativeMixer),
      [3, 7, 10]);
    assert.deepEqual(configured.record, configured.binding!.preferences);
    assert.equal(configured.record.music.savedPlaylist2.length, 100);
    assert.deepEqual([configured.record.music.savedPlaylist2[0], configured.record.music.savedPlaylist2[1],
      configured.record.music.savedPlaylist2[2], configured.record.music.savedPlaylist2[99]], [62, null, null, 76]);
    assert.equal(configured.record.music.currentPlaylist, 2);
    assert(!/playerId|unlockedGroups|playhead|privacyMute|token|account|password/.test(JSON.stringify(configured.record)));
    checks.push("actual native fallback/remembered mute, nonlinear mixers and exact saved-slot holes persist without authority");
    await page.evaluate(() => { (window as PreferenceWindow).__clubscapePreferenceFixture!.action = "skip"; });
    await page.locator("#native-player-preference-contract").click();
    await page.evaluate(() => (window as PreferenceWindow).__clubscapePreferenceFixture!.pending);
    assert.equal(await page.evaluate(() => (window as PreferenceWindow).__clubscapePreferenceFixture!.skip?.status), "disabled_mode");
    checks.push("Single Skip uses the actual typed disabled_mode command, never a mode flip");
    const reentered = await page.evaluate(async () => {
      const state = (window as PreferenceWindow).__clubscapePreferenceFixture!, old = state.controls!;
      await state.composition.title();
      const titleCleared = state.audio.snapshot().preferences === null;
      await state.composition.prepare(state.world);
      state.composition.events(state.world, []);
      let blocked = false;
      try { old.toggleMute(state.world.player.id, "music"); } catch { blocked = true; }
      await state.controls!.persistCurrent(state.world.player.id);
      return { titleCleared, blocked, binding: state.controls!.read(), status: state.composition.observe(), reads: state.reads };
    });
    assert.equal(reentered.titleCleared, true); assert.equal(reentered.blocked, true);
    assert.equal(reentered.reads, 2);
    assert.equal(reentered.status.preferences.origin, "stored");
    assert.deepEqual(reentered.binding!.preferences, configured.binding!.preferences);
    checks.push("genuine title clears native binding; same-actor re-entry reloads storage and fences prior controls");
    await page.evaluate(() => {
      const state = (window as PreferenceWindow).__clubscapePreferenceFixture!;
      state.holdWrites();
      state.controls!.setPercent(state.world.player.id, "music", 11);
    });
    await page.waitForFunction(() => (window as PreferenceWindow).__clubscapePreferenceFixture!.writing === 1);
    const queued = await page.evaluate(() => {
      const state = (window as PreferenceWindow).__clubscapePreferenceFixture!;
      state.controls!.setPercent(state.world.player.id, "music", 22);
      state.controls!.setPercent(state.world.player.id, "music", 33);
      const pending = state.controls!.persistCurrent(state.world.player.id);
      state.pending = pending;
      void pending.catch(() => {});
      return { writes: state.writes, status: state.composition.observe() };
    });
    assert.equal(queued.writes.at(-1), 11);
    assert.equal(queued.status.preferences.save, "pending");
    const saved = await page.evaluate(async () => {
      const state = (window as PreferenceWindow).__clubscapePreferenceFixture!;
      state.releaseWrites();
      await state.pending;
      return { record: JSON.parse(localStorage.getItem(state.storageKey)!), writes: state.writes.slice(-2),
        maxWriting: state.maxWriting, status: state.composition.observe() };
    });
    assert.deepEqual(saved.writes, [11, 33]);
    assert.equal(saved.maxWriting, 1);
    assert.equal(saved.record.volumes.current.music, 33);
    assert.equal(saved.status.preferences.save, "stored");
    checks.push("delayed real browser writes are serialized/coalesced per actor; the older save cannot overwrite33");
    const timingComparison = await page.evaluate(async () => {
      const state = (window as PreferenceWindow).__clubscapePreferenceFixture!, player = state.world.player.id;
      const samples = [];
      for (const path of ["coordinated", "native-api-without-preference-coordinator"] as const) {
        const before = new Set(state.audio.snapshot().traces.filter((trace) => trace.type === "effect_dispatched")
          .map((trace) => trace.data.eventId));
        const durations = [];
        state.observePreferences(path === "coordinated");
        try {
          for (let count = 0; count < 6; count++) {
            const started = performance.now();
            if (path === "coordinated") state.controls!.toggleMute(player, "area");
            else state.audio.preferenceApi().toggle(player, "area");
            durations.push(performance.now() - started);
            const waitStarted = performance.now();
            do {
              await new Promise((resolve) => setTimeout(resolve, 20));
              if (performance.now() - waitStarted > 3000) throw new Error("Native comparison queue did not drain.");
            } while (state.audio.snapshot().queueSize !== 0);
          }
          samples.push({ path, apiDurationsMs: durations,
            dispatches: state.audio.snapshot().traces.filter((trace) =>
              trace.type === "effect_dispatched" && !before.has(trace.data.eventId)).map((trace) => trace.data) });
        } finally { state.observePreferences(true); }
      }
      state.composition.changed(state.audio.preferenceApi().read());
      await state.controls!.persistCurrent(player);
      return samples;
    });
    const final = await page.evaluate(() => {
      const state = (window as PreferenceWindow).__clubscapePreferenceFixture!;
      return { assets: state.audio.observations(), audio: state.audio.controls(), feedback: state.feedback,
        dispatches: state.audio.snapshot().traces.filter((trace) => trace.type === "effect_dispatched").map((trace) => trace.data) };
    });
    assert(final.assets.some((asset) => asset.decoded));
    const timingFailures = final.feedback.filter((item) =>
      ["AUDIO_LOADING_LATE", "AUDIO_TIMING_LATE", "AUDIO_CLOCK_LATE", "AUDIO_SOURCE_QUEUE_EXPIRED"].includes(item.kind));
    const dispatchToleranceMs = (SOURCE_CYCLE_SECONDS + 1 / SOURCE_RATE) * 1000;
    const dispatchOverruns = final.dispatches.filter((dispatch) =>
      typeof dispatch.lateMs === "number" && dispatch.lateMs > dispatchToleranceMs).length;
    assert(final.feedback.every((item) => ["AUDIO_GESTURE_REQUIRED", "AUDIO_SOURCE_SCENE_REQUIRED"].includes(item.kind)
      || timingFailures.includes(item)),
      `Unexpected native fixture feedback: ${JSON.stringify(final.feedback)}`);
    return { kind: "native-player-audio-storage-composition-contract-fixture", result: "contract_passed", checks,
      actualNativeAudio: true, actualBrowserStorage: true, trustedGesture: true,
      syntheticWorld: true, fixtureUnlockedGroups: [62, 76], fixtureUiPort: true,
      actualGameUiPreferenceRouting: false, actualSourceSceneSupplied: false, gameAuthorityModified: false,
      committedGameEventsFabricated: false, record: saved.record, controls: final.audio, loaded: final.assets,
      feedback: final.feedback,
      nativeCueTiming: {
        passed: timingFailures.length === 0 && dispatchOverruns === 0, failures: timingFailures,
        dispatchToleranceMs, dispatchOverruns,
        dispatches: final.dispatches, comparison: timingComparison,
      },
      fixtureCadence: "Serial controls separated by actual source queue drain; not a bulk synthetic-input burst.",
      gameJourney: false, presentationAccepted: false, physicalSpeakersAccepted: false };
  } finally {
    await page.evaluate(async () => {
      const state = (window as PreferenceWindow).__clubscapePreferenceFixture;
      if (!state) return;
      state.releaseRead(); state.releaseWrites();
      try { await state.composition.dispose(); }
      finally {
        await state.audio.dispose();
        state.assets.dispose(); state.button.remove();
        localStorage.removeItem(state.storageKey);
        Reflect.deleteProperty(window, "__clubscapePreferenceFixture");
      }
    });
  }
}
