import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { AUDIO_RENDER_QUANTUM_FRAMES, sourceSchedulingLookahead, sourceRenderReservationDue } from "./clock.ts";
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

test("the actual 42ms device batch needs render submission without advancing the 20ms source queue", () => {
  const baseLatency = 0.042675736961451244;
  const recorded = [
    { request: 0.7256235827664399, due: 0.7495691609977326, late: 0.7720634920634921 },
    { request: 1.3235374149659864, due: 1.3481179138321997, late: 1.3699773242630386 },
    { request: 1.4512471655328798, due: 1.4751927437641725, late: 1.497687074829932 },
  ];
  assert.equal(sourceSchedulingLookahead(baseLatency), 0.02);
  for (const sample of recorded) {
    const queue = new SourceQueue<number>();
    queue.enqueue(2266, 0);
    assert.ok(sample.due - sample.request > sourceSchedulingLookahead(baseLatency));
    assert.ok((sample.late - sample.due) * 1000 > 20 + 1000 / SOURCE_RATE);
    assert.equal(sourceRenderReservationDue(sample.request, sample.due, baseLatency), true);
    assert.equal(sourceRenderReservationDue(sample.late, sample.due, baseLatency), false);
    assert.equal(queue.size, 1);
    assert.deepEqual(queue.values(), [2266]);
    let calls = 0;
    queue.process(() => { calls++; return true; });
    assert.equal(calls, 1);
    assert.equal(queue.size, 1, "A pre-submitted cue still occupies its original tombstone slot.");
    queue.process(() => { assert.fail("The native dispatch must not create a second source."); });
    assert.equal(queue.size, 0);
  }
});

test("a render reservation cannot postpone a past deadline or widen native client-cycle timing", () => {
  for (const baseLatency of [0, 0.01070294784580499, 0.042675736961451244]) {
    const horizon = baseLatency + 128 / 22050;
    assert.equal(sourceRenderReservationDue(0, horizon, baseLatency), true);
    assert.equal(sourceRenderReservationDue(0, horizon + 1 / 22050, baseLatency), false);
    assert.equal(sourceRenderReservationDue(1, 1 - 1 / 22050, baseLatency), false);
    assert.ok(sourceSchedulingLookahead(baseLatency) <= 0.02);
  }
  for (const invalid of [NaN, Infinity, -1]) {
    assert.throws(() => sourceRenderReservationDue(invalid, 1, 0.04));
    assert.throws(() => sourceRenderReservationDue(0, invalid, 0.04));
    assert.throws(() => sourceRenderReservationDue(0, 1, invalid));
  }
});

test("a cold zero-delay cue keeps native retry counts without disguising its late render deadline", () => {
  const queue = new SourceQueue<number>();
  queue.enqueue(2266, 0);
  for (let call = 1; call <= 3; call++) queue.process(() => false);
  let dispatches = 0;
  queue.process(() => { dispatches++; return true; });
  assert.equal(dispatches, 1);
  assert.equal(queue.size, 1);
  const due = 0.6295691609977325, decoded = 0.6849886621315193, actual = 0.6895691609977326;
  assert.equal(sourceRenderReservationDue(decoded, due, 0.042675736961451244), false);
  assert.ok((actual - due) * 1000 > 20 + 1000 / SOURCE_RATE);
  assert.ok(Math.abs((actual - due) - 0.06) < 1 / SOURCE_RATE);
});
