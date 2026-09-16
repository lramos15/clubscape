import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import type { Page } from "playwright-core";
import type { SourceAudioPreferenceBinding, SourceMusicSkipResult } from "../../audio/index.ts";
import type { PlayerAudioControls } from "../player-audio.ts";
import { audioFixtureWorld } from "./player-audio-fixture.ts";
import { SOURCE_CYCLE_SECONDS, SOURCE_RATE } from "../../audio/source.ts";

interface NativeTimingInstrumentation {
  intervals: Array<{ wallTime: number; audioTime: number; afterWallTime: number; afterAudioTime: number }>;
  starts: Array<{ frames: number; when: number; callTime: number; wallTime: number; state: string }>;
  ends: Array<{ frames: number; audioTime: number; wallTime: number }>;
  longTasks: Array<{ wallTime: number; duration: number }>;
  context: { sampleRate: number; baseLatency: number; outputLatency: number; state: string; currentTime: number } | null;
  pcm: Array<{ when: number; frames: number; observedNonzeroFrames: number; observedFirstNonzero: number;
    nativeGain: number; maximumError: number; clips: number; completeWaveform: boolean; stopCalledAt: number | null }>;
}

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
  timing: NativeTimingInstrumentation;
  finishTiming(): NativeTimingInstrumentation;
  restoreTiming(): void;
}
type PreferenceWindow = Window & { __clubscapePreferenceFixture?: PreferenceFixture };

/** Real native audio/storage; the world/unlocks/UI port are explicit contract fixtures, never a game journey. */
export async function checkPlayerAudioPreferences(page: Page) {
  const manifest = JSON.parse(await readFile(new URL("../../../assets/manifests/osrs/audio-runtime.json", import.meta.url), "utf8"));
  const cue = manifest.assets.find((asset: { kind: string; source_group: number }) => asset.kind === "sfx" && asset.source_group === 2266);
  assert.equal(cue.sha256, "cd3323c5c52a3e03490fd2b19df7eef1b53ef6429df49ee47b5f994887c17a51");
  assert.equal(cue.signal.frames, 2756);
  await page.evaluate(async ({ fixtureWorld, cueFrames, cueFirstNonzero }) => {
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
    const timing: NativeTimingInstrumentation = { intervals: [], starts: [], ends: [], longTasks: [], context: null, pcm: [] };
    let measuredContext: AudioContext | null = null;
    const createSource = AudioContext.prototype.createBufferSource;
    const interval = window.setInterval;
    const connect = AudioNode.prototype.connect;
    const edges = new WeakMap<AudioNode, AudioNode>();
    const connectObserver = new Proxy(connect, {
      apply(target, receiver: AudioNode, arguments_: unknown[]) {
        const result = Reflect.apply(target, receiver, arguments_);
        if (arguments_[0] instanceof AudioNode) edges.set(receiver, arguments_[0]);
        return result;
      },
    });
    const sourceObserver = function (this: AudioContext): AudioBufferSourceNode {
      const source = createSource.call(this), context = this, start = source.start.bind(source);
      const stop = source.stop.bind(source);
      let analyser: AnalyserNode | null = null;
      let output: GainNode | null = null;
      let scheduledWhen = 0;
      let stopCalledAt: number | null = null;
      source.stop = (when = 0) => { stopCalledAt = context.currentTime; stop(when); };
      measuredContext = context;
      source.start = (when = 0, offset = 0, duration?: number) => {
        if (source.buffer!.length === cueFrames) {
          const target = edges.get(source);
          if (!(target instanceof GainNode)) throw new Error("The real cue has no calibrated voice gain.");
          output = target;
          analyser = context.createAnalyser();
          analyser.fftSize = 8192;
          Reflect.apply(connect, output, [analyser]);
        }
        const callTime = context.currentTime;
        if (duration === undefined) start(when, offset);
        else start(when, offset, duration);
        scheduledWhen = when;
        timing.starts.push({ frames: source.buffer!.length, when, callTime, wallTime: performance.now(), state: context.state });
      };
      source.addEventListener("ended", () => {
        timing.ends.push({ frames: source.buffer!.length, audioTime: context.currentTime, wallTime: performance.now() });
        if (!analyser || !output) return;
        const samples = new Float32Array(analyser.fftSize);
        analyser.getFloatTimeDomainData(samples);
        const observedFirstNonzero = samples.findIndex((value) => value !== 0);
        const startFrame = observedFirstNonzero - cueFirstNonzero;
        const original = source.buffer!.getChannelData(0), nativeGain = output.gain.value;
        const completeWaveform = startFrame >= 0 && startFrame + original.length <= samples.length;
        let maximumError = 0, clips = 0, observedNonzeroFrames = 0;
        for (let frame = 0; frame < samples.length; frame++) {
          const sample = samples[frame]!;
          if (sample !== 0) observedNonzeroFrames++;
          if (Math.abs(sample) > 1) clips++;
          const index = frame - startFrame;
          const expected = index >= 0 && index < original.length ? Math.fround(original[index]! * nativeGain) : 0;
          maximumError = Math.max(maximumError, Math.abs(sample - expected));
        }
        timing.pcm.push({ when: scheduledWhen, frames: original.length, observedNonzeroFrames,
          observedFirstNonzero, nativeGain, maximumError, clips, completeWaveform, stopCalledAt });
        analyser.disconnect();
      });
      return source;
    };
    const intervalObserver = new Proxy(interval, {
      apply(target, receiver, arguments_: unknown[]) {
        const [handler, timeout, ...args] = arguments_;
        if (typeof handler !== "function" || timeout !== 5) return Reflect.apply(target, receiver, arguments_);
        return interval(() => {
          const before = { wallTime: performance.now(), audioTime: measuredContext?.currentTime ?? 0 };
          handler(...args);
          if (timing.intervals.length < 2048) timing.intervals.push({
            ...before, afterWallTime: performance.now(), afterAudioTime: measuredContext?.currentTime ?? 0,
          });
        }, timeout);
      },
    });
    const tasks = new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) timing.longTasks.push({ wallTime: entry.startTime, duration: entry.duration });
    });
    tasks.observe({ type: "longtask" });
    AudioContext.prototype.createBufferSource = sourceObserver;
    AudioNode.prototype.connect = connectObserver;
    window.setInterval = intervalObserver;
    const restoreTiming = () => {
      if (AudioContext.prototype.createBufferSource === sourceObserver) AudioContext.prototype.createBufferSource = createSource;
      if (AudioNode.prototype.connect === connectObserver) AudioNode.prototype.connect = connect;
      if (window.setInterval === intervalObserver) window.setInterval = interval;
      tasks.disconnect();
    };
    const finishTiming = (): NativeTimingInstrumentation => {
      if (measuredContext) timing.context = {
        sampleRate: measuredContext.sampleRate, baseLatency: measuredContext.baseLatency,
        outputLatency: measuredContext.outputLatency, state: measuredContext.state, currentTime: measuredContext.currentTime,
      };
      return timing;
    };
    const audio = await api.SourceAudioSession.create(assets, manifest.assets,
      (error) => feedback.push({ kind: error.errorId, message: error.message }),
      (snapshot) => { if (observePreferences) composition?.changed(snapshot.preferences); })
      .catch((error: unknown) => { restoreTiming(); throw error; });
    const button = window.document.createElement("button");
    button.id = "native-player-preference-contract";
    button.textContent = "Native audio preference contract fixture (not game UI)";
    Object.assign(button.style, { position: "fixed", left: "8px", top: "8px", zIndex: "30" });
    const native = audio.preferenceApi();
    const state: PreferenceFixture = {
      audio, assets, controls: null, world, projected: null, reads: 0, writes: [], writing: 0, maxWriting: 0,
      preparing: Promise.resolve(), order, trusted: false, action: "unlock", pending: null, skip: null,
      feedback, button, storageKey, timing, finishTiming, restoreTiming,
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
  }, { fixtureWorld: audioFixtureWorld(), cueFrames: cue.signal.frames as number, cueFirstNonzero: cue.signal.first_nonzero_frame as number });
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
        dispatches: state.audio.snapshot().traces.filter((trace) => trace.type === "effect_dispatched").map((trace) => trace.data),
        sourceTraces: state.audio.snapshot().traces.filter((trace) =>
          ["queued", "effect_submitted", "effect_dispatched", "decoded", "error", "stopped", "ended"].includes(trace.type)),
        instrumentation: state.finishTiming() };
    });
    if (process.env.CLUBSCAPE_BROWSER_EVIDENCE) {
      const root = fileURLToPath(new URL("../../../", import.meta.url));
      const output = resolve(root, process.env.CLUBSCAPE_BROWSER_EVIDENCE);
      assert(output.startsWith(resolve(root, ".local") + sep), "Native diagnostics must stay in the owned evidence root.");
      await mkdir(output, { recursive: true });
      await writeFile(resolve(output, "native-audio-diagnostics.json"), JSON.stringify(final, null, 2) + "\n");
    }
    assert(final.assets.some((asset) => asset.decoded));
    const timingFailures = final.feedback.filter((item) =>
      ["AUDIO_LOADING_LATE", "AUDIO_TIMING_LATE", "AUDIO_CLOCK_LATE", "AUDIO_SOURCE_QUEUE_EXPIRED"].includes(item.kind));
    assert.equal(final.dispatches.length, 18, "All eighteen original source cue dispatches remain required.");
    for (const comparison of timingComparison) assert.equal(comparison.dispatches.length, 6);
    const dispatchToleranceMs = (SOURCE_CYCLE_SECONDS + 1 / SOURCE_RATE) * 1000;
    const dispatchOverruns = final.dispatches.filter((dispatch) =>
      typeof dispatch.lateMs === "number" && dispatch.lateMs > dispatchToleranceMs).length;
    let actualStartOverruns = 0;
    for (const dispatch of final.dispatches) {
      const start = final.instrumentation.starts.find((start) => start.frames === cue.signal.frames && start.when === dispatch.when);
      assert(start, "A dispatch trace must retain its actual AudioBufferSourceNode.start call.");
      assert.equal(start.state, "running");
      assert.equal(dispatch.sourceId, 2266);
      assert(Number.isInteger(dispatch.processingCalls) && Number(dispatch.processingCalls) >= 1 &&
        Number(dispatch.processingCalls) <= 10, "Loading retries retain the native -10-cycle grace; their timing errors remain failures.");
      if (Math.max(start.callTime - Number(dispatch.dueAt), 0) * 1000 > dispatchToleranceMs) actualStartOverruns++;
    }
    const completedPcm = final.instrumentation.pcm.filter((pcm) => pcm.stopCalledAt === null &&
      final.dispatches.some((dispatch) => dispatch.when === pcm.when));
    assert(completedPcm.length > 0, "Actual composed source2266 produced no captured PCM.");
    for (const pcm of completedPcm) {
      assert(pcm.completeWaveform && pcm.observedNonzeroFrames > 0, "The actual cue waveform is missing or truncated.");
      assert(pcm.maximumError <= 0.000023, `The original cue gain/waveform changed: ${JSON.stringify(pcm)}`);
      assert.equal(pcm.clips, 0);
    }
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
        passed: timingFailures.length === 0 && dispatchOverruns === 0 && actualStartOverruns === 0, failures: timingFailures,
        dispatchToleranceMs, dispatchOverruns,
        actualStartOverruns, completedPcmComparisons: completedPcm.length,
        explicitlyStoppedPcm: final.instrumentation.pcm.filter((pcm) => pcm.stopCalledAt !== null),
        dispatches: final.dispatches, comparison: timingComparison,
        sourceTraces: final.sourceTraces, instrumentation: final.instrumentation,
      },
      fixtureCadence: "Serial controls separated by actual source queue drain; not a bulk synthetic-input burst.",
      gameJourney: false, presentationAccepted: false, physicalSpeakersAccepted: false };
  } finally {
    const cleanup = await page.evaluate(async () => {
      const state = (window as PreferenceWindow).__clubscapePreferenceFixture;
      if (!state) return;
      state.releaseRead(); state.releaseWrites();
      try { await state.composition.dispose(); }
      finally {
        try { await state.audio.dispose(); }
        finally {
          state.restoreTiming();
          state.assets.dispose(); state.button.remove();
          localStorage.removeItem(state.storageKey);
          Reflect.deleteProperty(window, "__clubscapePreferenceFixture");
        }
      }
      const snapshot = state.audio.snapshot();
      return { context: snapshot.contextState, voices: snapshot.voices.length, queue: snapshot.queueSize,
        cachedBytes: snapshot.cache.decodedBytes, pending: snapshot.cache.pending,
        storageRemoved: localStorage.getItem(state.storageKey) === null };
    });
    if (cleanup) assert.deepEqual(cleanup, {
      context: "closed", voices: 0, queue: 0, cachedBytes: 0, pending: 0, storageRemoved: true,
    });
  }
}
