import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import type { TestContext } from "node:test";
import {
  createAudio, readAudioState, applySourceAudioPreferences, sourceAudioPreferenceDefaults,
  toggleSourceAudioMute, setSourceAudioPercent,
} from "./index.ts";
import { AudioFailure } from "./errors.ts";
import { SourceControlInput } from "./control-input.ts";
import { AUDIO_INPUTS, SOURCE_RATE } from "./source.ts";
import { audioFixtureWorld } from "../app/tests/player-audio-fixture.ts";
import type { AudioHandle, ClientAssets } from "../shared/contracts.ts";

const cueId = "asset.source.osrs.cache2695.audio-runtime.sfx.2266";
const root = new URL("../../", import.meta.url);
const cue = await readFile(new URL("assets/source/osrs/audio-runtime/sfx/2266.flac", root));
const metadata = new Map(await Promise.all(Object.values(AUDIO_INPUTS).map(async (input) =>
  [input.path, await readFile(new URL(input.path, root))] as const)));

class Param {
  value = 1;
  setValueAtTime(value: number): void { this.value = value; }
}
class Gain {
  gain = new Param();
  connected = false;
  connect(): void { this.connected = true; }
  disconnect(): void { this.connected = false; }
}
class Buffer {
  readonly length = 2756;
  readonly sampleRate = SOURCE_RATE;
  readonly numberOfChannels = 1;
  readonly duration = this.length / SOURCE_RATE;
  private readonly samples = new Float32Array(this.length);
  constructor() { this.samples[1] = 0.001; }
  getChannelData(): Float32Array { return this.samples; }
}
class Source extends EventTarget {
  buffer: Buffer | null = null;
  loop = false;
  loopStart = 0;
  loopEnd = 0;
  connected = false;
  stopped = false;
  when: number | null = null;
  onended: (() => void) | null = null;
  connect(): void { this.connected = true; }
  disconnect(): void { this.connected = false; }
  start(when: number): void { this.when = when; }
  stop(): void { this.stopped = true; }
}

function host(t: TestContext, options: {
  response?: "missing" | "corrupt";
  decode?: () => Promise<Buffer>;
} = {}) {
  const requests: string[] = [], contexts: Context[] = [], timers = new Map<number, () => void>();
  const decodeEntered = Promise.withResolvers<void>();
  let timerId = 0, decodeCalls = 0, resumes = 0;
  class Context extends EventTarget {
    state: AudioContextState = "suspended";
    currentTime = 0;
    sampleRate = SOURCE_RATE;
    baseLatency = 0.042675736961451244;
    destination = { channelCount: 2 };
    gains: Gain[] = [];
    sources: Source[] = [];
    constructor() { super(); contexts.push(this); }
    createGain(): Gain { const gain = new Gain(); this.gains.push(gain); return gain; }
    createBufferSource(): Source { const source = new Source(); this.sources.push(source); return source; }
    async decodeAudioData(): Promise<Buffer> {
      decodeCalls++;
      decodeEntered.resolve();
      return options.decode ? options.decode() : new Buffer();
    }
    resume(): Promise<void> {
      resumes++;
      this.state = "running";
      this.dispatchEvent(new Event("statechange"));
      return Promise.resolve();
    }
    close(): Promise<void> {
      this.state = "closed";
      this.dispatchEvent(new Event("statechange"));
      return Promise.resolve();
    }
  }
  const globals: Record<string, unknown> = {
    AudioContext: Context,
    isSecureContext: true,
    location: { href: "https://audio.test/", origin: "https://audio.test" },
    navigator: { userActivation: { isActive: true } },
    setInterval: (callback: () => void) => { timers.set(++timerId, callback); return timerId; },
    clearInterval: (id: number) => { timers.delete(id); },
    fetch: async (input: string | URL | Request) => {
      const url = new URL(input instanceof Request ? input.url : String(input));
      const id = decodeURIComponent(url.pathname.slice("/asset/".length));
      requests.push(id);
      const bytes = metadata.get(id) ?? (id === cueId ? cue : null);
      assert(bytes, `Unexpected/unrelated audio download: ${id}`);
      if (id === cueId && options.response === "missing") return new Response("unavailable", { status: 503 });
      const body = Uint8Array.from(bytes);
      if (id === cueId && options.response === "corrupt") body[0] = body[0]! ^ 1;
      return new Response(body, { headers: { "content-length": String(body.length) } });
    },
  };
  const original = new Map(Object.keys(globals).map((key) => [key, Object.getOwnPropertyDescriptor(globalThis, key)]));
  for (const [key, value] of Object.entries(globals)) Object.defineProperty(globalThis, key, { value, configurable: true, writable: true });
  t.after(() => {
    for (const [key, descriptor] of original) {
      if (descriptor) Object.defineProperty(globalThis, key, descriptor);
      else Reflect.deleteProperty(globalThis, key);
    }
  });
  const assets: ClientAssets = {
    baseUrl: "https://audio.test",
    url: (id) => `https://audio.test/asset/${encodeURIComponent(id)}`,
    image: async () => { throw new Error("No image input belongs to control readiness."); },
    json: async () => { throw new Error("Native audio metadata must be fetched and hash checked as bytes."); },
  };
  return { assets, requests, contexts, timers, decodeEntered: decodeEntered.promise,
    get decodeCalls() { return decodeCalls; }, get resumes() { return resumes; },
    tick(time: number) {
      for (const context of contexts) context.currentTime = time;
      for (const callback of [...timers.values()]) callback();
    } };
}

test("pending control readiness cannot admit a deadline and duplicate preparation shares one load", async () => {
  const decoded = Promise.withResolvers<number>(), states: string[] = [];
  let loads = 0;
  const input = new SourceControlInput(async () => { loads++; return decoded.promise; }, (state) => states.push(state.phase));
  const first = input.prepare(), second = input.prepare();
  assert.equal(first, second);
  assert.throws(() => input.requireReady(), { code: "AUDIO_CONTROL_INPUT_PENDING" });
  await Promise.resolve();
  assert.equal(loads, 1);
  decoded.resolve(2266);
  await first;
  assert.equal(input.requireReady(), 2266);
  await input.prepare();
  assert.equal(loads, 1);
  assert.deepEqual(states, ["pending", "ready"]);
  input.dispose();
  assert.throws(() => input.requireReady(), { code: "AUDIO_DISPOSED" });
});

test("failed control preparation stays explicit and cannot silently retry or fabricate ready input", async () => {
  let loads = 0;
  const problem = new AudioFailure("AUDIO_NETWORK", "Required original control input is missing.");
  const input = new SourceControlInput<number>(async () => { loads++; throw problem; }, () => {});
  await assert.rejects(input.prepare(), (error) => error === problem);
  assert.deepEqual(input.state, { phase: "failed", errorCode: "AUDIO_NETWORK" });
  assert.throws(() => input.requireReady(), (error) => error === problem);
  await assert.rejects(input.prepare(), (error) => error === problem);
  assert.equal(loads, 1);
});

test("disposing pending control preparation prevents late completion from resurrecting readiness", async () => {
  const decoded = Promise.withResolvers<number>();
  const input = new SourceControlInput(() => decoded.promise, () => {});
  const pending = input.prepare();
  input.dispose();
  decoded.resolve(2266);
  await assert.rejects(pending, { code: "AUDIO_CANCELLED" });
  assert.equal(input.state.phase, "disposed");
  assert.throws(() => input.requireReady(), { code: "AUDIO_DISPOSED" });
});

test("factory resolution waits for actual verified2266 decode without resuming or loading any other playable input", async (t) => {
  const decoded = Promise.withResolvers<Buffer>(), environment = host(t, { decode: () => decoded.promise });
  const errors: Error[] = [];
  let resolved = false;
  const pending = createAudio(environment.assets, (error) => errors.push(error)).then((handle) => { resolved = true; return handle; });
  await environment.decodeEntered;
  assert.equal(resolved, false);
  assert.equal(environment.contexts[0]!.state, "suspended");
  assert.equal(environment.resumes, 0);
  assert.equal(environment.contexts[0]!.sources.length, 0);
  decoded.resolve(new Buffer());
  const audio = await pending;
  t.after(() => audio.dispose());
  assert.deepEqual(environment.requests.filter((id) => !metadata.has(id)), [cueId]);
  const state = readAudioState(audio);
  assert.equal(state.controlInput?.phase, "ready");
  assert.equal(state.pendingGesture, true);
  assert.equal(state.unlocked, false);
  assert.equal(state.queueSize, 0);
  assert.equal(state.cache.decodedBytes, 2756 * 4);
  assert.deepEqual(state.traces.filter((trace) => trace.type.startsWith("control_input_")).map((trace) => trace.type),
    ["control_input_pending", "control_input_ready"]);
  assert(state.traces.findIndex((trace) => trace.type === "decoded") < state.traces.findIndex((trace) => trace.type === "control_input_ready"));
  await audio.dispose();
  assert.equal(readAudioState(audio).controlInput?.phase, "disposed");
  assert.equal(readAudioState(audio).cache.decodedBytes, 0);
  assert.equal(environment.timers.size, 0);
});

for (const response of ["missing", "corrupt"] as const) {
  test(`factory rejects ${response} required control bytes before admission and cleans its context`, async (t) => {
    const environment = host(t, { response }), errors: Error[] = [];
    const code = response === "missing" ? "AUDIO_NETWORK" : "AUDIO_INTEGRITY";
    await assert.rejects(createAudio(environment.assets, (error) => errors.push(error)), { code });
    assert(errors.some((error) => error instanceof AudioFailure && error.code === code));
    assert.equal(environment.decodeCalls, 0);
    assert.equal(environment.contexts[0]!.state, "closed");
    assert(environment.contexts[0]!.gains.every((gain) => !gain.connected));
    assert.equal(environment.contexts[0]!.sources.length, 0);
    assert.equal(environment.timers.size, 0);
  });
}

test("required-control decoder rejection remains a failed factory, never a silent-ready handle", async (t) => {
  const environment = host(t, { decode: async () => { throw new DOMException("Controlled decoder failure", "EncodingError"); } });
  await assert.rejects(createAudio(environment.assets, () => {}), { code: "AUDIO_DECODE" });
  assert.equal(environment.decodeCalls, 1);
  assert.equal(environment.contexts[0]!.state, "closed");
  assert(environment.contexts[0]!.gains.every((gain) => !gain.connected));
});

test("ready input survives re-entry and reservations retain volume, queue and cancellation semantics", async (t) => {
  const environment = host(t);
  const audio: AudioHandle = await createAudio(environment.assets, () => {});
  t.after(() => audio.dispose());
  const world = audioFixtureWorld("actor.readiness-test");
  const record = sourceAudioPreferenceDefaults();
  const preferences = { ...record, volumes: { ...record.volumes,
    current: { master: 100, music: 0, effects: 100, area: 100 } } };
  audio.update(world, []);
  applySourceAudioPreferences(audio, world.player.id, preferences, [62, 76]);
  const unlocking = audio.unlock();
  assert.equal(environment.resumes, 1, "Native resume must begin synchronously before awaits.");
  await unlocking;
  environment.tick(0);
  const beforeRequests = environment.requests.length;
  toggleSourceAudioMute(audio, world.player.id, "area");
  await Promise.resolve();
  const submitted = readAudioState(audio);
  assert.equal(submitted.queueSize, 1);
  assert.equal(submitted.voices.length, 1);
  const queued = submitted.traces.find((trace) => trace.type === "queued")!;
  const source = environment.contexts[0]!.sources.at(-1)!;
  assert.equal(source.when, queued.data.dueAt);
  assert.equal(submitted.voices[0]!.appliedNativeLevel, 127);
  setSourceAudioPercent(audio, world.player.id, "effects", 50);
  assert.equal(readAudioState(audio).voices[0]!.appliedNativeLevel, 22);
  setSourceAudioPercent(audio, world.player.id, "effects", 0);
  assert.equal(source.stopped, true);
  assert.equal(readAudioState(audio).voices.length, 0);
  audio.disconnected();
  assert.equal(readAudioState(audio).queueSize, 0);
  assert.equal(readAudioState(audio).controlInput?.phase, "ready");
  audio.update(world, []);
  applySourceAudioPreferences(audio, world.player.id, preferences, [62, 76]);
  toggleSourceAudioMute(audio, world.player.id, "area");
  await Promise.resolve();
  assert.equal(environment.requests.length, beforeRequests);
  assert.equal(readAudioState(audio).voices.length, 1);
  environment.tick(0.02);
  assert.equal(readAudioState(audio).traces.filter((trace) => trace.type === "effect_dispatched").length, 1);
  audio.update(null, []);
  assert.equal(readAudioState(audio).voices.length, 0);
  assert.equal(readAudioState(audio).queueSize, 0);
  assert.equal(readAudioState(audio).controlInput?.phase, "ready");
  await audio.dispose();
  assert.equal(readAudioState(audio).controlInput?.phase, "disposed");
  assert.equal(readAudioState(audio).cache.decodedBytes, 0);
  assert.equal(environment.timers.size, 0);
});
