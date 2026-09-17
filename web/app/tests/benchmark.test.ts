import assert from "node:assert/strict";
import { test } from "node:test";
import { Benchmark } from "../benchmark.ts";
import type { BuildConfig } from "../build.ts";

const config: BuildConfig = {
  schemaVersion: 1, buildId: "test-fixture-only", buildArtifactSha256: "a".repeat(64),
  artifactPath: "/client/build-artifact.json", sourcePackSha256: "b".repeat(64),
  benchmarkContractSha256: "c".repeat(64), content: null, visualSettings: {},
  components: { renderer: false, ui: false, audio: false },
};

test("benchmark observation cannot advance counters or mark absent renderer/assets ready", () => {
  const benchmark = new Benchmark(config, () => 100);
  benchmark.device("actual-test-device-event", true, false);
  benchmark.settings("d".repeat(64));
  benchmark.worldReady(true);
  benchmark.scene("scene.fixture", "route.fixture", "workload.fixture", "e".repeat(64), new Map([["asset.fixture", "f".repeat(64)]]));
  for (let index = 0; index < 10; index++) {
    assert.equal(benchmark.read(null).renderedFrames, 0);
    assert.equal(benchmark.read(null).ready, false);
    assert.deepEqual(benchmark.read(null).entities, {});
  }
  benchmark.completed({ sequence: 1, submittedAtMs: 10, completedAtMs: 20, drawCalls: 1, primitives: 1 });
  assert.equal(benchmark.read(null).ready, false, "frames alone do not establish workload/loading evidence");
  assert.equal(benchmark.read(0).frames.length, 1);
  assert.equal(benchmark.read(0).frames.length, 1, "read never drains history");
});

test("out-of-order GPU receipts publish only contiguous completions and preserve exact identities", () => {
  const benchmark = new Benchmark(config, () => 100);
  benchmark.completed({ sequence: 2, submittedAtMs: 20, completedAtMs: 40, drawCalls: 2, primitives: 5 });
  assert.equal(benchmark.read(null).renderedFrames, 0);
  benchmark.completed({ sequence: 1, submittedAtMs: 10, completedAtMs: 30, drawCalls: 1, primitives: 3 });
  assert.equal(benchmark.read(null).renderedFrames, 2);
  assert.deepEqual(benchmark.read(0).frames.map((frame) => frame.sequence), [1, 2]);
  benchmark.bindRun({ contractId: "owner.real.audit", contractSha256: "d".repeat(64) });
  const state = benchmark.read(null);
  assert.equal(state.identity.benchmarkContractSha256, "d".repeat(64));
  assert.equal(state.identity.buildArtifactSha256, config.buildArtifactSha256);
  assert.equal(state.identity.sourcePackSha256, config.sourcePackSha256);
  assert.equal(state.renderedFrames, 2);
  assert.throws(() => benchmark.bindRun({ contractId: "another", contractSha256: "e".repeat(64) }), /rebound/);
});

test("zero draws, timestamp-query guesses, regressing clocks and history loss invalidate measurements", () => {
  const benchmark = new Benchmark(config, () => 10_000);
  assert.throws(() => benchmark.completed({ sequence: 1, submittedAtMs: 1, completedAtMs: 2, drawCalls: 0, primitives: 0 }));
  assert.throws(() => benchmark.completed({ sequence: 1, submittedAtMs: 1, completedAtMs: 2, drawCalls: 1, primitives: 1, gpuDurationMs: 1 }));
  for (let sequence = 1; sequence <= 4097; sequence++) {
    benchmark.completed({ sequence, submittedAtMs: sequence, completedAtMs: sequence + 1, drawCalls: 1, primitives: 1 });
  }
  assert.throws(() => benchmark.read(0), /overrun/);
  assert.equal(benchmark.read(1).frames.length, 4096);
});
