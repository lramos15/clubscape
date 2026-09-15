import assert from "node:assert/strict";
import { test } from "node:test";
import { AudioFailure, sourceAudioDefaults, sourceSliderToMixer, sourceMixerToAssetGain, SOURCE_MUSIC_MODE_IDS } from "../../audio/index.ts";
import type { AudioSnapshot } from "../../audio/index.ts";
import type { SourceAudioScene, SourceMusicState } from "../../audio/index.ts";
import type { AudioEvent, AudioHandle, ClientAssets, WorldView } from "../../shared/contracts.ts";
import { SourceAudioSession, audioProblem, playbackEnabled, sourceControlState } from "../audio.ts";
import type { AudioAdapter } from "../audio.ts";
import { Settings } from "../settings.ts";

function snapshot(): AudioSnapshot {
  return {
    sourcePackSha256: "fixture", contextState: "suspended", currentTime: 0, sampleRate: 22050,
    pendingGesture: true, unlocked: false, muted: false, connected: true, disposed: false, outputEnabled: true,
    volumes: { music: 0.25, effects: 0.5, area: 0.75 }, queueSize: 0,
    nativeMixer: { music: sourceSliderToMixer("music", 25), effects: sourceSliderToMixer("effects", 50), area: sourceSliderToMixer("area", 75) },
    masterPercent: 100,
    background: { groups: [0], cursor: 0, mode: "once", exhausted: false, failed: false },
    voices: [], cache: { decodedBytes: 0, cached: 0, pending: 0 }, policyLimits: [], traces: [],
  };
}

test("audio permission is recoverable and enabled state describes real output, not factory resolution", () => {
  const error = audioProblem(new AudioFailure("AUDIO_GESTURE_REQUIRED", "Trusted gesture required.", false));
  assert.equal(error.recoverable, true);
  assert.equal(error.kind, "audio");
  assert.match(error.message, /AUDIO_GESTURE_REQUIRED/);
  assert.equal(playbackEnabled(snapshot()), false);
  const running = { ...snapshot(), contextState: "running", unlocked: true, pendingGesture: false };
  assert.equal(playbackEnabled(running), true);
  assert.equal(playbackEnabled({ ...running, outputEnabled: false }), false);
  assert.equal(playbackEnabled({ ...running, connected: false }), false);
  assert.equal(playbackEnabled({ ...running, muted: true }), false);
});

test("factory initialization failures retain their actual audio code and recovery classification", async () => {
  const api: AudioAdapter = {
    create: async () => { throw new AudioFailure("AUDIO_INTEGRITY", "Pinned bytes changed.", true); },
    read: () => snapshot(), observe: () => () => {},
    scene() {}, musicState() {},
  };
  await assert.rejects(SourceAudioSession.create({} as ClientAssets, [], () => {}, () => {}, api),
    (error: unknown) => error instanceof Error && error.message.includes("[AUDIO_INTEGRITY]"));
});

test("composition delegates the trusted call synchronously and never substitutes disconnect with title", async () => {
  const calls: unknown[] = [];
  let trusted = true;
  const state = snapshot();
  const handle: AudioHandle = {
    unlock() { calls.push(["unlock", trusted]); return Promise.resolve(); },
    mute(value) { calls.push(["mute", value]); }, volume(channel, value) { calls.push(["volume", channel, value]); },
    update(world, events) { calls.push(["update", world, events]); },
    disconnected() { calls.push(["disconnected"]); }, dispose: async () => { calls.push(["dispose"]); },
  };
  const api: AudioAdapter = {
    create: async () => handle, read: () => state, observe: (_handle, listener) => { listener(state); return () => {}; },
    scene() {}, musicState() {},
  };
  const audio = await SourceAudioSession.create({} as ClientAssets, [], () => {}, () => {}, api);
  assert.equal(calls.length, 0, "No default gain or playback policy is applied by the shell.");
  const pending = audio.unlock();
  trusted = false;
  await pending;
  assert.deepEqual(calls, [["mute", false], ["unlock", true]]);
  const world = { revision: "1", player: { id: "actor.fixture" } } as WorldView;
  const cue: AudioEvent = Object.freeze({
    id: "committed.fixture", kind: "sound", sourceId: 2393, assetId: null, actorId: "actor.fixture", tile: null,
    sourceCycle: 12345, payload: Object.freeze({ committed: true, actionId: "action.fixture", cueId: "cue.fixture", sequenceId: 12526, frame: 1, iteration: 0, delayCycles: 0, repeatCount: 1 }),
  });
  const closed: AudioEvent = Object.freeze({
    id: "close.fixture", kind: "interface_closed", sourceId: 153, assetId: null, actorId: "actor.fixture", tile: null,
    sourceCycle: 12350, payload: Object.freeze({ committed: true, questId: "quest.cooks_assistant", completionId: "complete.fixture", actionId: "action.close" }),
  });
  const events = Object.freeze([cue, closed]);
  audio.update(world, events);
  assert.deepEqual(calls.at(-1), ["update", world, events]);
  const before = calls.length;
  audio.disconnected();
  assert.deepEqual(calls.slice(before), [["disconnected"]]);
  audio.update(null, []);
  assert.deepEqual(calls.at(-1), ["update", null, []]);
  await audio.dispose();
});

test("only actual decoded/evicted audio notices establish source residency", async () => {
  let state = snapshot();
  let observer: (value: AudioSnapshot) => void = () => {};
  const handle = { dispose: async () => {} } as AudioHandle;
  const api: AudioAdapter = {
    create: async () => handle, read: () => state, observe: (_handle, listener) => { observer = listener; listener(state); return () => {}; },
    scene() {}, musicState() {},
  };
  const audio = await SourceAudioSession.create({} as ClientAssets, [{
    id: "asset.fixture", url: "/assets/fixture.flac", bytes: 100, sha256: "a".repeat(64), contentType: "audio/flac",
  }], () => {}, () => {}, api);
  state = { ...state, traces: [{ type: "fetch", audioTime: 0, wallTime: 1, data: { assetId: "asset.fixture", bytes: 100 } }] };
  observer(state);
  assert.equal(audio.observations().length, 0);
  state = { ...state, traces: [...state.traces, { type: "decoded", audioTime: 1, wallTime: 2, data: { assetId: "asset.fixture", bytes: 400 } }] };
  observer(state);
  assert.equal(audio.observations()[0]?.decoded, true);
  assert.equal(audio.observations()[0]?.bytes, 100, "Transfer bytes are not confused with float PCM cache bytes.");
  state = { ...state, traces: [...state.traces, { type: "evicted", audioTime: 2, wallTime: 3, data: { assetId: "asset.fixture" } }] };
  observer(state);
  assert.equal(audio.observations().length, 0);
  await audio.dispose();
});

test("an unset preference does not fabricate source default channel gains", () => {
  const writes: string[] = [];
  const settings = new Settings({ getItem: () => null, setItem: (_key, value) => { writes.push(value); } });
  assert.deepEqual(settings.audioOverrides(), {});
  settings.volume("music", 0.4);
  assert.deepEqual(settings.audioOverrides(), { music: 0.4 });
  assert.deepEqual(JSON.parse(writes[0]!).audio, { music: 0.4 });
});

test("applied native master and mute changes alter settings identity without inventing persisted preferences", async () => {
  const settings = new Settings(null);
  const audio = { masterPercent: 100, volumes: { music: 1, effects: 1, area: 1 },
    nativeMixer: { ...sourceAudioDefaults().mixer }, muted: false };
  const before = await settings.hash({ audio });
  const master = { ...audio, masterPercent: 50, nativeMixer: {
    music: sourceSliderToMixer("music", 100, 50), effects: sourceSliderToMixer("effects", 100, 50),
    area: sourceSliderToMixer("area", 100, 50),
  } };
  assert.notEqual(await settings.hash({ audio: master }), before);
  assert.notEqual(await settings.hash({ audio: { ...audio, muted: true } }), before);
  assert.deepEqual(settings.audioOverrides(), {});
});

test("source slider positions and observed native mixer levels are not double-normalized", async () => {
  const source = sourceAudioDefaults();
  assert.deepEqual(source.mixer, { music: 255, effects: 127, area: 127 });
  assert.deepEqual(SOURCE_MUSIC_MODE_IDS, { area: 0, shuffle: 1, single: 2 });
  const observed = snapshot();
  const controls = sourceControlState(observed);
  assert.equal(controls.channels.music.normalizedPosition, 0.25);
  assert.equal(controls.channels.music.percent, 25);
  assert.equal(controls.channels.music.nativeMixer, sourceSliderToMixer("music", 25));
  assert.equal(controls.channels.music.referenceGains.native128, sourceMixerToAssetGain(observed.nativeMixer.music, 128));
  assert.equal(controls.channels.music.referenceGains.native255, sourceMixerToAssetGain(observed.nativeMixer.music, 255));
  let received: number | undefined;
  const handle = { volume: (_channel: string, value: number) => { received = value; }, dispose: async () => {} } as AudioHandle;
  const api: AudioAdapter = { create: async () => handle, read: () => observed,
    observe: () => () => {}, scene() {}, musicState() {} };
  const audio = await SourceAudioSession.create({} as ClientAssets, [], () => {}, () => {}, api);
  audio.volume("music", 0.5);
  assert.equal(received, 0.5, "AudioHandle receives source normalized position, never a mixer level or linear gain.");
  assert.equal(audio.controls().sourceSceneSupplied, false);
  assert.equal(audio.controls().sourceMusicStateSupplied, false);
  assert.equal(audio.controls().providedMusicState, null);
  assert.equal(audio.controls().musicContinuation, "native-bound");
  await audio.dispose();
});

test("actual scene/music inputs bind after actor reset, before later cues, without guessed callbacks", async () => {
  const calls: unknown[][] = [];
  const errors: string[] = [];
  const scene: SourceAudioScene = {
    listener: { x: 3200 * 128 + 37, y: 3201 * 128 + 92 }, plane: 2, instance: "instance.actual",
    owner: { id: "source.owner", exteriorPlane: 0, audibleInteriorPlane: 2 },
    varps: new Map([[281, 17]]),
    emitters: [{
      id: "placement.original", objectId: 23, tile: { x: 3202, y: 3201, plane: 2 }, orientation: 1,
      instance: "instance.actual", owner: { id: "source.owner", exteriorPlane: 0, audibleInteriorPlane: 2 }, present: true,
    }],
  };
  const handle = { update: (world: unknown, events: unknown) => { calls.push(["world", world, events]); },
    disconnected: () => { calls.push(["disconnected"]); }, dispose: async () => {} } as AudioHandle;
  const api: AudioAdapter = {
    create: async () => handle, read: snapshot, observe: () => () => {},
    scene: (_handle, input) => { calls.push(["scene", input]); },
    musicState: (_handle, state) => { calls.push(["music", state]); },
  };
  const audio = await SourceAudioSession.create({} as ClientAssets, [], (error) => { errors.push(error.message); }, () => {}, api);
  const world = { revision: "44", player: { id: "actor.fixture" } } as WorldView;
  const unlocked = [76];
  const music: SourceMusicState = {
    mode: "single", areaMode: "classic", unlockedGroups: unlocked, selectedGroup: 76,
    playlistGroups: [], loopEnabled: false,
  };
  audio.update(world, [], scene, music);
  assert.equal(calls[0]![1], world, "The audio owner initializes/reset its actor before accepting scene/music inputs.");
  assert.equal(calls[1]![1], scene);
  assert.deepEqual(calls[2], ["music", music]);
  assert.equal(audio.controls().sourceSceneSupplied, true);
  assert.equal(audio.controls().sourceMusicStateSupplied, true);
  assert.deepEqual(audio.controls().providedMusicState, music);
  unlocked.push(64);
  assert.deepEqual(audio.controls().providedMusicState?.unlockedGroups, [76]);
  audio.update(world, [], scene);
  assert.deepEqual(calls.at(-2), ["scene", scene], "Continuing-actor spatial context precedes its coherent event batch.");
  assert.deepEqual(calls.at(-1), ["world", world, []]);
  audio.disconnected();
  assert.deepEqual(calls.at(-1), ["disconnected"], "Disconnect does not synthesize a title or replacement scene.");
  audio.update(world, []);
  assert.deepEqual(calls.at(-2), ["scene", null], "Unavailable new scene stops stale ambience.");
  assert.equal(audio.controls().sourceSceneSupplied, false);
  assert(errors.some((error) => error.includes("AUDIO_SOURCE_SCENE_REQUIRED")));
  assert.equal(scene.listener.x, 3200 * 128 + 37);
  assert.equal(scene.varps.get(281), 17);
  audio.update({ ...world, player: { ...world.player, id: "actor.other" } }, []);
  assert.equal(audio.controls().sourceMusicStateSupplied, false, "A new actor cannot inherit another actor's unlock/selection inputs.");
  assert.equal(audio.controls().providedMusicState, null);
  await audio.dispose();
});

test("control observations distinguish configured mixer levels from each actual applied representation", () => {
  const state: AudioSnapshot = {
    ...snapshot(), nativeMixer: { music: 255, effects: 127, area: 127 }, volumes: { music: 1, effects: 1, area: 1 },
    voices: [{
      id: 7, sourceId: 54, kind: "jingle", channel: "music", eventId: "fixture.jingle", when: 0,
      gain: 44 / 128, loop: false, loopEnd: 0, renderedNativeLevel: 128, appliedNativeLevel: 44,
      assetId: "asset.source.osrs.cache2695.audio-runtime.jingle.54",
    }],
  };
  const pending = sourceControlState(state);
  assert.equal(pending.channels.music.nativeMixer, 255);
  assert.deepEqual(pending.channels.music.referenceGains, { native128: 255 / 128, native255: 1 });
  assert.equal(pending.voices[0]!.appliedNativeLevel, 44);
  assert.equal(pending.voices[0]!.gain, 44 / 128, "Pending native255 loading cannot be advertised as an already-applied gain.");
  const changed = sourceControlState({ ...state, voices: [{ ...state.voices[0]!,
    renderedNativeLevel: 255, appliedNativeLevel: 255, gain: 1,
    assetId: "asset.source.osrs.cache2695.audio-supplement.jingle.54.native255",
  }] });
  assert.equal(changed.voices[0]!.gain, 1);
  assert.equal(changed.voices[0]!.renderedNativeLevel, 255);
  assert(Object.isFrozen(changed.voices[0]));
});

test("UI binding receives the real audio handle and reads applied UI preferences without duplicating world updates", async () => {
  let updates = 0, disposed = false, stopped = false;
  let applied: SourceMusicState | null = null;
  const handle: AudioHandle = {
    update() { updates++; }, unlock: async () => {}, mute() {}, volume() {}, disconnected() {},
    dispose: async () => { disposed = true; },
  };
  const api: AudioAdapter = {
    create: async () => handle, read: snapshot, observe: () => () => {}, scene() {}, musicState() {},
    readMusicState: () => applied,
  };
  const audio = await SourceAudioSession.create({} as ClientAssets, [], () => {}, () => {}, api);
  const stop = await audio.bindUi(async (bound) => {
    assert.equal(bound, handle);
    return () => { stopped = true; };
  });
  assert.equal(updates, 0, "Binding is an observer, not another audio world feed.");
  applied = { mode: "single", areaMode: "modern", unlockedGroups: [163], selectedGroup: 163,
    playlistGroups: [], loopEnabled: false };
  assert.deepEqual(audio.controls().providedMusicState, applied);
  assert(Object.isFrozen(audio.controls().providedMusicState));
  stop();
  assert(stopped && !disposed);
  await audio.dispose();
  assert(disposed);
});

test("native music-state rejection preserves its actual error code and is not treated as supplied state", async () => {
  const errors: string[] = [];
  const delivered: WorldView[] = [];
  const handle: AudioHandle = {
    update(world) { if (world !== null) delivered.push(world); },
    unlock: async () => {}, mute() {}, volume() {}, disconnected() {}, dispose: async () => {},
  };
  const api: AudioAdapter = {
    create: async () => handle, read: snapshot, observe: () => () => {}, scene() {},
    musicState() { throw new AudioFailure("AUDIO_SOURCE_MUSIC", "A locked source track was selected."); },
  };
  const audio = await SourceAudioSession.create({} as ClientAssets, [], (error) => errors.push(error.message), () => {}, api);
  const world = { revision: "45", player: { id: "actor.fixture" } } as WorldView;
  audio.update(world, [], undefined, { mode: "single", areaMode: "modern", unlockedGroups: [],
    selectedGroup: 64, playlistGroups: [], loopEnabled: false });
  assert.equal(delivered[0], world, "An invalid control cannot discard or split the committed world/event batch.");
  assert(errors.some((message) => message.includes("[AUDIO_SOURCE_MUSIC]")));
  assert.equal(audio.controls().sourceMusicStateSupplied, false);
  await audio.dispose();
});

test("committed before/after Cook batches and source-selected reward metadata are never rewritten", async () => {
  const delivered: Array<{ world: WorldView | null; events: readonly AudioEvent[] }> = [];
  const handle = { update: (world: WorldView | null, events: readonly AudioEvent[]) => { delivered.push({ world, events }); },
    dispose: async () => {} } as AudioHandle;
  const api: AudioAdapter = { create: async () => handle, read: snapshot, observe: () => () => {}, scene() {}, musicState() {} };
  const audio = await SourceAudioSession.create({} as ClientAssets, [], () => {}, () => {}, api);
  const before = { revision: "50", player: { id: "actor.original", skills: [{ id: "skill.cooking", baseLevel: 10, xpTenths: "9007199254740993" }] } } as unknown as WorldView;
  const after = { revision: "51", player: { id: "actor.original", skills: [{ id: "skill.cooking", baseLevel: 11, xpTenths: "9007199254743993" }] } } as unknown as WorldView;
  const level: AudioEvent = Object.freeze({
    id: "level.original", kind: "level_up", sourceId: 54, assetId: null, actorId: "actor.original",
    sourceCycle: 500, tile: null, payload: Object.freeze({
      committed: true, skillId: "skill.cooking", causeQuestId: "quest.cooks_assistant",
      previousLevel: 10, level: 11, completionId: "completion.original", actionId: "action.reward", cueId: "cue.original",
    }),
  });
  const completion: AudioEvent = Object.freeze({
    id: "completion.original", kind: "quest_complete", sourceId: 152, assetId: null, actorId: "actor.original",
    sourceCycle: 500, tile: null, payload: Object.freeze({ committed: true, questId: "quest.cooks_assistant" }),
  });
  const events = Object.freeze([level, completion]);
  audio.update(before, []);
  audio.update(after, events);
  assert.equal(delivered.length, 2);
  assert.equal(delivered[0]!.world, before);
  assert.equal(delivered[1]!.world, after);
  assert.equal(delivered[1]!.events, events);
  assert.equal(delivered[1]!.events[0]!.sourceId, 54, "Pre-trained levels do not force the golden-case group33.");
  assert.equal(after.player.skills[0]!.xpTenths, "9007199254743993");
  await audio.dispose();
});
