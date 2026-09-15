import assert from "node:assert/strict";
import { test } from "node:test";
import { AudioFailure } from "../../audio/index.ts";
import type { AudioSnapshot } from "../../audio/index.ts";
import type { AudioEvent, AudioHandle, ClientAssets, WorldView } from "../../shared/contracts.ts";
import { SourceAudioSession, audioProblem, playbackEnabled } from "../audio.ts";
import type { AudioAdapter } from "../audio.ts";
import { Settings } from "../settings.ts";

function snapshot(): AudioSnapshot {
  return {
    sourcePackSha256: "fixture", contextState: "suspended", currentTime: 0, sampleRate: 22050,
    pendingGesture: true, unlocked: false, muted: false, connected: true, disposed: false, outputEnabled: true,
    volumes: { music: 0.25, effects: 0.5, area: 0.75 }, queueSize: 0,
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
  };
  const audio = await SourceAudioSession.create({} as ClientAssets, [], () => {}, () => {}, api);
  assert.equal(calls.length, 0, "No default gain or playback policy is applied by the shell.");
  const pending = audio.unlock();
  trusted = false;
  await pending;
  assert.deepEqual(calls, [["mute", false], ["unlock", true]]);
  const world = { revision: "1" } as WorldView;
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
