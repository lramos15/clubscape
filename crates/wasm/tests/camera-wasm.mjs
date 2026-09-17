import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { gunzipSync } from "node:zlib";
import { test } from "node:test";
import { initSync, NativeCamera } from "../../../web/generated/protocol/clubscape_wasm.js";

// Actual CPU WASM with pinned original terrain and explicitly controlled focus inputs.
// No browser, WebGPU, source capture, account or production-focus assertion.
const root = new URL("../../../", import.meta.url);
const read = (path) => readFileSync(new URL(path, root));
const sha = (bytes) => createHash("sha256").update(bytes).digest("hex");
initSync({ module: read("web/generated/protocol/clubscape_wasm_bg.wasm") });
const manifest = JSON.parse(read("assets/compiled/render/manifest.json"));
const publication = JSON.parse(read("assets/manifests/osrs/cache2695-published.json"));
const objectsPath = "assets/source/osrs/cache2695/collections/object.json.gz";
const objectsBytes = read(objectsPath);
assert.equal(sha(objectsBytes), publication.published_files.find((p) => p.path === objectsPath).sha256);
const objects = Object.values(JSON.parse(gunzipSync(objectsBytes))).map(({ id, raise }) => ({ id, raise }));
assert.equal(objects.find((o) => o.id === 941).raise, 7);
const definitions = JSON.stringify(objects);

function decodeScene(name, generation = 1n) {
  const record = manifest.scenes.find((s) => s.name === name);
  const compressed = read(`assets/compiled/render/${record.file_gz}`);
  assert.equal(sha(compressed), manifest.files[record.file_gz].sha256);
  const bytes = gunzipSync(compressed);
  assert.equal(sha(bytes), record.sha256);
  return NativeCamera.decode_scene(bytes, generation);
}

const sceneJson = decodeScene("lumbridge-castle-plaza");
const scene = JSON.parse(sceneJson);

function sampleFor(data, x = 3215, y = 3218, actor = "actor.controlled-wasm") {
  const local = [(x - data.base.x) * 128 + 64, (y - data.base.y) * 128 + 64];
  return {
    context: { actorId: actor, region: "controlled-source-region", instance: null, revision: "9007199254740993",
      tick: "9007199254740993", sceneId: data.id, sceneGeneration: data.generation },
    renderedActor: { actorId: actor, local, plane: 0, sizeTiles: 1 },
    focus: { identity: 42, world_base: data.base, logical: local, rendered: local, plane: 0, footprint: 0, kind: "Actor" },
    effects: { shake: [null, null, null, null, null], suppress_jitter: false }, missing: [],
  };
}

const sample = sampleFor(scene);
const input = (values = {}) => ({ arrows: { left: false, right: false, up: false, down: false },
  mouse: [0, 0], button: "Released", wheel: null, ...values });
const state = (camera) => JSON.parse(camera.state());
const bind = (camera, at = 0n, source = sample, terrain = sceneJson) => JSON.parse(camera.bind(terrain,
  JSON.stringify(source), definitions, "TutorialStartingHouse", 1920, 1080, at));
const frame = (camera, at, source = sample) => JSON.parse(camera.frame(at, JSON.stringify(source)));
const cameraError = (text) => (error) => typeof error === "string"
  && JSON.parse(error).kind === "camera_unavailable" && JSON.parse(error).message.includes(text);

test("actual WASM applies source triangles/raise while frame following keeps its separate source height", () => {
  const camera = new NativeCamera();
  try {
    const initialized = bind(camera);
    assert.equal(initialized.cycle, 0);
    assert.equal(initialized.output.projection.zoom, 662);
    assert.equal(initialized.output.pitch_native, 2048);
    assert.equal(initialized.output.yaw_native, 0);
    assert.equal(initialized.initialization.observed_osrs_account_defaults, false);
    assert.equal(initialized.output.focal_native[1], -522);
    frame(camera, 20_000_000n);
    assert.equal(state(camera).logical_focus_ground, -471);
    const triangleSample = sampleFor(scene, 3223, 3218, "actor.controlled-triangle");
    bind(camera, 30_000_000n, triangleSample);
    frame(camera, 50_000_000n, triangleSample);
    assert.equal(state(camera).logical_focus_ground, -240);
    assert.equal(state(camera).focal_native[1], -296, "frame bilinear -238 is not fixed-step triangle -240");
  } finally { camera.free(); }
});

test("actual WASM consumes timestamped input once with BigInt clocks above JS integer precision", () => {
  const camera = new NativeCamera();
  const origin = (1n << 60n) + 123n;
  try {
    bind(camera, origin);
    camera.input(origin + 11_000_000n, JSON.stringify(input({
      arrows: { left: false, right: true, up: false, down: false },
    })));
    assert.equal(frame(camera, origin + 10_000_000n).cycle, 0);
    assert.equal(frame(camera, origin + 20_000_000n).cycle, 1);
    assert.deepEqual(state(camera).native_velocity, [96, 0]);
    assert.deepEqual(state(camera).legacy_velocity, [12, 0]);
    const yaw = state(camera).target_yaw;
    assert.equal(frame(camera, origin + 30_000_000n).cycle, 1);
    assert.notEqual(state(camera).target_yaw, yaw);
    camera.input(origin + 31_000_000n, JSON.stringify(input({ wheel: { rotation: 1, route: "Camera" } })));
    assert.equal(frame(camera, origin + 80_000_000n).cycle, 4);
    const fov = state(camera).encoded_fov;
    frame(camera, origin + 100_000_000n);
    assert.deepEqual(state(camera).encoded_fov, fov);
    camera.resize(1024, 768);
    assert.equal(frame(camera, origin + 110_000_000n).output.projection.zoom, state(camera).projection.zoom);
    assert.notEqual(state(camera).projection.zoom, 292, "controlled full-HUD 127/127 must not replace constructor/wheel FOV");
  } finally { camera.free(); }
});

test("actual WASM requires coherent focus, geometry, object raise and explicit active-effect samples", () => {
  const camera = new NativeCamera();
  try {
    assert.throws(() => bind(camera, 0n, { ...sample, focus: null }), cameraError("actual focus"));
    assert.throws(() => bind(camera, 0n, { ...sample, effects: null }), cameraError("explicit source effect"));
    assert.throws(() => camera.bind(sceneJson, JSON.stringify(sample), "[]", "TutorialStartingHouse", 1920, 1080, 0n),
      cameraError("object 941"));
    bind(camera);
    const prior = camera.state();
    const invalid = structuredClone(sample);
    invalid.effects.shake[0] = { random_radius: 2, sine_amplitude: 4, sine_frequency: 10, phase: 5, random_sample: null };
    assert.throws(() => frame(camera, 20_000_000n, invalid), cameraError("random draw"));
    assert.equal(camera.state(), prior);
    invalid.effects.shake[0].random_sample = 0.25;
    assert.equal(frame(camera, 20_000_000n, invalid).cycle, 1);
    const stale = structuredClone(sample);
    stale.context.sceneGeneration = "2";
    assert.throws(() => frame(camera, 40_000_000n, stale), cameraError("stale/malformed"));
  } finally { camera.free(); }
});

test("actual WASM reconnect preserves camera state, clears queued wheel input and switches to genuine new source terrain", () => {
  const camera = new NativeCamera();
  try {
    bind(camera);
    camera.input(1n, JSON.stringify(input({ mouse: [120, 240], arrows: { left: true, right: false, up: false, down: false } })));
    frame(camera, 20_000_000n);
    const prior = state(camera);
    camera.input(21_000_000n, JSON.stringify(input({ mouse: [120, 240], wheel: { rotation: 5, route: "Camera" } })));
    camera.suspend();
    assert.throws(() => frame(camera, 22_000_000n), cameraError("suspended"));
    bind(camera, 5_000_000_000n);
    assert.equal(state(camera).target_yaw, prior.target_yaw);
    assert.deepEqual(state(camera).native_velocity, prior.native_velocity);
    frame(camera, 5_020_000_000n);
    assert.deepEqual(state(camera).previous_mouse, [120, 240]);
    assert.deepEqual(state(camera).encoded_fov, prior.encoded_fov);
    const nextTerrain = decodeScene("tutorial-starting-house", 2n);
    const next = sampleFor(JSON.parse(nextTerrain), 3094, 3107);
    const yaw = state(camera).target_yaw;
    bind(camera, 6_000_000_000n, next, nextTerrain);
    assert.equal(state(camera).target_yaw, yaw);
    assert.deepEqual(state(camera).base, JSON.parse(nextTerrain).base);
    assert.equal(frame(camera, 6_020_000_000n, next).context.actorId, sample.context.actorId);
  } finally { camera.free(); }
});

test("actual WASM rejects malformed scene headers and oversized model counts as explicit camera errors", () => {
  const record = manifest.scenes.find((s) => s.name === "lumbridge-castle-plaza");
  const bytes = gunzipSync(read(`assets/compiled/render/${record.file_gz}`));
  const chunks = new Map();
  for (let offset = 8; offset < bytes.length;) {
    const size = bytes.readUInt32LE(offset + 4);
    chunks.set(bytes.toString("ascii", offset, offset + 4), offset + 8);
    offset += 8 + size;
  }
  const invalidHeader = Buffer.from(bytes);
  invalidHeader.writeInt32LE(-1, chunks.get("SCHD") + 2 * 4);
  assert.throws(() => NativeCamera.decode_scene(invalidHeader, 1n), cameraError("header is out of bounds"));
  const invalidModel = Buffer.from(bytes);
  invalidModel.writeInt32LE(-1, chunks.get("TMOD") + 6 * 4);
  assert.throws(() => NativeCamera.decode_scene(invalidModel, 1n), cameraError("model counts are out of bounds"));
  assert.throws(() => NativeCamera.decode_scene(bytes.subarray(0, 7), 1n), cameraError("CSRC"));
});
