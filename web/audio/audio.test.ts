import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import test from "node:test";
import type { AudioEvent } from "../shared/contracts.ts";
import { repeatedEffect } from "./buffers.ts";
import { cueKey, OBSERVED_SELECTORS, validateEvent } from "./events.ts";
import { EventLedger, SourceQueue } from "./queue.ts";
import { APPROVED_PACK, AUDIO_INPUTS, regionalTrack, selectWeighted } from "./source.ts";
import type { FrameCue, SourceAsset } from "./source.ts";

const root = new URL("../../", import.meta.url);
const file = (path: string) => readFile(new URL(path, root));
const json = async (path: string) => JSON.parse((await file(path)).toString());
const event = (override: Partial<AudioEvent> = {}): AudioEvent => ({
  id: "event/1", kind: "sound", sourceId: 2735, assetId: null, actorId: "player.test",
  tile: null, sourceCycle: 12, payload: { committed: true, repeatCount: 1, delayCycles: 2 },
  ...override,
});

test("the active pack, corrected manifest, source map and all 266 original payloads retain their hashes", async () => {
  assert.equal(createHash("sha256").update(await file("research/reference-pack/v1/manifest.json")).digest("hex"), APPROVED_PACK);
  for (const input of Object.values(AUDIO_INPUTS)) {
    const data = await file(input.path);
    assert.equal(data.length, input.bytes);
    assert.equal(createHash("sha256").update(data).digest("hex"), input.sha256);
  }
  const manifest = await json(AUDIO_INPUTS.manifest.path);
  const reference = await json(AUDIO_INPUTS.reference.path);
  assert.equal(manifest.assets.length, 264);
  assert.equal(manifest.settings.native_startup_percussion_bank, 128);
  assert.equal(manifest.settings.native_startup_percussion_channel, 9);
  for (const asset of [...manifest.assets, ...reference.reference_templates]) {
    const data = await file(asset.path);
    assert.equal(data.length, asset.size_bytes, asset.path);
    assert.equal(createHash("sha256").update(data).digest("hex"), asset.sha256, asset.path);
  }
  assert.deepEqual(reference.reference_templates.map((a: { source_group: number }) => a.source_group).sort((a: number, b: number) => a - b), [710, 2693]);
  assert.equal(manifest.source_silences[0].source_group, 2411);
  assert.equal(manifest.source_silences[0].playable_output, null);
});

test("native delay 2 dispatches on processing call 3 and occupies capacity until call 4", () => {
  const queue = new SourceQueue<number>();
  const calls: number[] = [];
  queue.enqueue(2393, 2);
  const dispatch = (value: number) => { calls.push(value); return true; };
  queue.process(dispatch);
  queue.process(dispatch);
  assert.deepEqual(calls, []);
  queue.process(dispatch);
  assert.deepEqual(calls, [2393]);
  assert.equal(queue.size, 1);
  queue.process(dispatch);
  assert.equal(queue.size, 0);
});

test("native 50-entry FIFO drops only the new overflow request, never reprioritizes existing entries", () => {
  const queue = new SourceQueue<number>();
  for (let i = 0; i < 50; i++) assert.equal(queue.enqueue(i, 0), true);
  assert.equal(queue.enqueue(50, 0), false);
  const calls: number[] = [];
  queue.process((value) => { calls.push(value); return true; });
  assert.deepEqual(calls, Array.from({ length: 50 }, (_, i) => i));
  assert.equal(queue.enqueue(51, 0), false);
  queue.process(() => { assert.fail("Dispatched entries must not run twice"); });
  assert.equal(queue.enqueue(51, 0), true);
});

test("pending decode does not dispatch a placeholder, and cancellation removes only matching actions", () => {
  const queue = new SourceQueue<string>();
  queue.enqueue("loading", 0);
  queue.enqueue("other", 8);
  queue.process(() => false);
  assert.equal(queue.size, 2);
  queue.remove((id) => id === "loading");
  assert.deepEqual(queue.values(), ["other"]);
  queue.clear();
  assert.equal(queue.size, 0);
  for (const delay of [-1, NaN, Infinity, 0.25, 65536]) {
    assert.throws(() => queue.enqueue("bad", delay), /client-cycle delay/);
  }
});

test("frame-1 eating after four cycles cannot inherit the additional packet-delay-2 convention", () => {
  const queue = new SourceQueue<number>();
  queue.enqueue(2393, 4 - 1);
  let calls = 0;
  for (let cycle = 1; cycle <= 4; cycle++) {
    queue.process(() => { calls++; return true; });
    assert.equal(calls, cycle === 4 ? 1 : 0);
  }
});

test("all 100 original sequence 13612 weights include the 74 silent outcomes", async () => {
  const map = await json(AUDIO_INPUTS.map.path);
  const cues: FrameCue[] = map.sequence_sound_events
    .filter((c: { sequence_id: number }) => c.sequence_id === 13612)
    .map((c: Record<string, number>) => ({
      sequence: c.sequence_id, frame: c.frame, cycle: c.source_frame_start_cycle_sum,
      sourceId: c.id, repeats: c.loops, range: c.location, retain: c.retain, weight: c.weight,
    }));
  const selected = new Map<number, number>();
  for (let roll = 0; roll < 100; roll++) {
    const id = selectWeighted(cues, roll).sourceId;
    selected.set(id, (selected.get(id) ?? 0) + 1);
  }
  assert.deepEqual(Object.fromEntries(selected), { 2411: 74, 10983: 5, 10984: 6, 10985: 5, 10986: 5, 10987: 5 });
  assert.throws(() => selectWeighted(cues, 100));
  assert.throws(() => selectWeighted(cues.filter((c) => c.sourceId !== 2411), 0));
});

test("source frame correlation deduplicates server and animation callbacks, independent of event IDs", () => {
  const start = event({ id: "server/17", payload: { actionId: "action/17", iteration: 0 } });
  const callback = event({ id: "renderer/17/1", payload: { actionId: "action/17", iteration: 0 } });
  assert.equal(cueKey(start, 12526, 1), cueKey(callback, 12526, 1));
  const ledger = new EventLedger();
  assert.equal(ledger.cue(cueKey(start, 12526, 1)), true);
  assert.equal(ledger.cue(cueKey(callback, 12526, 1)), false);
  assert.equal(ledger.event(start.id), true);
  assert.equal(ledger.event(start.id), false);
  assert.equal(ledger.cue(cueKey({ ...start, payload: { actionId: "action/17", iteration: 1 } }, 12526, 1)), true);
});

test("events reject unsafe IDs, nonfinite values, invalid positions, and mutable compound payloads", () => {
  for (const id of ["", "https://evil.test/?x=1", "<script>", "x".repeat(257)]) {
    assert.throws(() => validateEvent(event({ id })));
  }
  for (const sourceCycle of [-1, 0.5, NaN, Infinity]) assert.throws(() => validateEvent(event({ sourceCycle })));
  for (const payload of [{ gain: NaN }, { nested: {} }, [], { bad_key: () => 1 }]) {
    assert.throws(() => validateEvent(event({ payload: payload as AudioEvent["payload"] })));
  }
  assert.throws(() => validateEvent(event({ tile: { x: -1, y: 1, plane: 0 } })));
  const input = event({ tile: { x: 3200, y: 3200, plane: 0 } });
  const output = validateEvent(input);
  input.tile!.x = 1;
  assert.equal(output.tile!.x, 3200);
  assert.equal(Object.isFrozen(output.payload), true);
  for (const unsupported of ["gain", "fade", "loop", "trim", "offset", "priority"]) {
    assert.throws(() => validateEvent(event({ payload: { committed: true, [unsupported]: 1 } })));
  }
  assert.throws(() => validateEvent(event({ kind: "jingle", payload: { committed: true, auxiliary: 0.5 } })));
});

test("named selectors use actual shortbow/rat/goblin/smelting IDs; no generic UI click binding", () => {
  assert.deepEqual(OBSERVED_SELECTORS, {
    shortbow_release: 2693, rat_attack: 710, rat_hit: 713, rat_death: 711,
    goblin_attack: 469, goblin_hit: 472, goblin_death: 471, bronze_smelt_start: 2725,
  });
  assert.equal(Object.hasOwn(OBSERVED_SELECTORS, "click"), false);
});

test("journey region routing does not call the mill extraction seed the Autumn Classic square", () => {
  assert.equal(regionalTrack("region.osrs.12336"), 62);
  assert.equal(regionalTrack("region.osrs.12592"), 62);
  assert.equal(regionalTrack("region.osrs.12436"), 144);
  assert.equal(regionalTrack("region.osrs.12850"), 76);
  assert.equal(regionalTrack("region.osrs.12851"), 2);
  assert.equal(regionalTrack("region.osrs.12595"), null);
  assert.equal(regionalTrack("unknown"), null);
});

test("finite SFX repetition copies native loop samples and retains the actual tail without resampling", () => {
  const make = (samples: number[]) => {
    const data = Float32Array.from(samples);
    return { length: data.length, numberOfChannels: 1, sampleRate: 22050, getChannelData: () => data };
  };
  const input = make([0, 1, 2, 3, 4, 5]);
  const context = { createBuffer: (_channels: number, size: number) => make(Array(size).fill(0)) };
  const asset = { loopStart: 2, loopEnd: 4 } as SourceAsset;
  const output = repeatedEffect(context as unknown as BaseAudioContext, input as unknown as AudioBuffer, asset, 3);
  assert.deepEqual([...output.getChannelData(0)], [0, 1, 2, 3, 2, 3, 2, 3, 4, 5]);
  assert.equal(repeatedEffect(context as unknown as BaseAudioContext, input as unknown as AudioBuffer,
    { loopStart: 0, loopEnd: 0 } as SourceAsset, 4), input);
});
