import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { AUDIO_RENDER_QUANTUM_FRAMES, sourceSchedulingLookahead } from "./clock.ts";
import { SourceQueue } from "./queue.ts";
import { SOURCE_CYCLE_SECONDS, SOURCE_RATE } from "./source.ts";

const before = JSON.parse(await readFile(new URL("../../research/browser-audio-policy/cue-timing/before.json", import.meta.url), "utf8"));

test("the recorded composed overrun is prepared before its delayed callback, without shifting its recorded deadline", () => {
  const missed = before.fixture.nativeCueTiming.dispatches.find((entry: { lateMs: number }) => entry.lateMs > 20);
  assert.ok(missed);
  const ticks: { audioTime: number; wallTime: number }[] = before.instrumentation.ticks;
  const previous = ticks.filter((tick) => tick.audioTime < missed.dueAt).at(-1)!;
  const gap = missed.dueAt - previous.audioTime;
  assert.ok(gap > 0.01, "The historical 10ms window missed this actual source boundary.");
  const horizon = sourceSchedulingLookahead(before.instrumentation.contexts[0].baseLatency);
  assert.ok(gap < horizon);
  assert.ok(horizon <= SOURCE_CYCLE_SECONDS);
  assert.equal(AUDIO_RENDER_QUANTUM_FRAMES, 128);
});

test("device scheduling stays within one source cycle and rejects corrupt latency", () => {
  assert.equal(sourceSchedulingLookahead(0), 0.01);
  assert.equal(sourceSchedulingLookahead(0.01070294784580499), 0.01070294784580499 + 128 / 22050);
  assert.equal(sourceSchedulingLookahead(1), 0.02);
  for (const value of [NaN, Infinity, -1]) assert.throws(() => sourceSchedulingLookahead(value));
});

test("every sample phase retains the native one/three/four-call absolute onset bound", () => {
  const horizon = sourceSchedulingLookahead(before.instrumentation.contexts[0].baseLatency);
  for (let sample = 0; sample < 441; sample++) {
    const now = 1 + sample / SOURCE_RATE;
    const nextCycle = (Math.floor((now + horizon) / SOURCE_CYCLE_SECONDS) + 1) * SOURCE_CYCLE_SECONDS;
    for (const delay of [0, 2, 3]) {
      const dueAt = Math.max(nextCycle, now + SOURCE_CYCLE_SECONDS) + delay * SOURCE_CYCLE_SECONDS;
      const independentSourceTime = now + (delay + 1) * SOURCE_CYCLE_SECONDS;
      assert.ok(Math.abs(dueAt - independentSourceTime) <= SOURCE_CYCLE_SECONDS + 1 / SOURCE_RATE);
      const queue = new SourceQueue<number>();
      queue.enqueue(2266, delay);
      let dispatched = 0;
      for (let call = 1; call <= delay + 1; call++) {
        queue.process(() => { dispatched++; return true; });
        assert.equal(dispatched, call === delay + 1 ? 1 : 0);
      }
      assert.equal(queue.size, 1);
      queue.process(() => { throw new Error("A dispatched tombstone must not replay."); });
      assert.equal(queue.size, 0);
    }
  }
});

test("lookahead does not turn an undecoded future boundary into an already missed loading cycle", () => {
  const queue = new SourceQueue<{ decoded: boolean }>();
  const value = { decoded: false };
  queue.enqueue(value, 2);
  const ready = (entry: typeof value) => entry.decoded;
  assert.equal(queue.readyForNextCycle(ready), true);
  queue.process(() => { throw new Error("Delay2 dispatched on call1."); });
  assert.equal(queue.readyForNextCycle(ready), true);
  queue.process(() => { throw new Error("Delay2 dispatched on call2."); });
  assert.equal(queue.readyForNextCycle(ready), false);
  assert.equal(queue.readyForNextCycle(ready), false);
  value.decoded = true;
  assert.equal(queue.readyForNextCycle(ready), true);
  let calls = 0;
  queue.process(() => { calls++; return true; });
  assert.equal(calls, 1);
});
