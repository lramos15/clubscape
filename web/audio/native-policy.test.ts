import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import {
  NATIVE_AUDIO_RUNTIME_SHA256, NATIVE_EFFECT_LEVELS, NATIVE_MUSIC_LEVELS,
  sourceAmbientFadeDuration, sourceAmbientFadeVolume, sourceAmbientSpatial, sourceAmbientVisible,
  sourceAudioDefaults, sourceMixerToAssetGain, sourceMusicFade, sourceObjectBounds,
  sourcePacketOwnerVisible, sourcePacketSpatial, sourceSliderToMixer,
} from "./native-policy.ts";
import {
  resolveSourceObject, sourceObjectDefinition, sourceMusicRegion, SOURCE_LUMBRIDGE_GROUPS,
  sourceMusicDurationSeconds, SOURCE_MUSIC_MODE_IDS, SOURCE_MIX_REPRESENTATION_NEEDS,
} from "./native-scene.ts";

const root = new URL("../../", import.meta.url);
async function evidence(name: string): Promise<any> {
  const value = JSON.parse(await readFile(new URL(`research/browser-audio-policy/native-${name}.json`, root), "utf8"));
  assert.equal(value.result, "passed");
  assert.equal(value.input_artifacts.find((v: { name: string }) => v.name === "injected-client-1.12.38.jar").sha256,
    NATIVE_AUDIO_RUNTIME_SHA256);
  return value;
}

test("native constructor/refresh defaults and exact nonlinear lookup tables", async () => {
  const native = await evidence("preferences");
  const tables = native.cases.find((v: any) => v.case === "native-nonlinear-lookup-tables");
  assert.deepEqual(NATIVE_MUSIC_LEVELS, tables.music);
  assert.deepEqual(NATIVE_EFFECT_LEVELS, tables.effect_and_area);
  const fresh = native.cases.find((v: any) => v.case === "fresh-native-preferences-after-em-fa").observed;
  assert.deepEqual(sourceAudioDefaults().mixer, {
    music: fresh.music_mixer, effects: fresh.effects_mixer, area: fresh.area_mixer,
  });
  assert.equal(sourceAudioDefaults().assetGain.music, 255 / 128);
  assert.equal(sourceAudioDefaults().assetGain.effects, 127 / 128);
  for (const row of native.cases.find((v: any) => v.case === "native-slider-setter-mixer").observed) {
    if (row.input < 0 || row.input > 100) continue;
    for (const channel of ["music", "effects", "area"] as const) {
      assert.equal(sourceSliderToMixer(channel, row.input), row.observed[`${channel}_mixer`]);
    }
  }
  assert.equal(sourceSliderToMixer("effects", 100, 50), 22);
  assert.equal(sourceSliderToMixer("music", 100, 50), 44);
  assert.notEqual(sourceSliderToMixer("effects", 100, 50), 127 * 0.5);
  assert.throws(() => sourceSliderToMixer("effects", 101));
  assert.throws(() => sourceSliderToMixer("music", NaN));
});

test("original source-cycle queue position and PCM-calibrated integer gains", async () => {
  const native = await evidence("position");
  for (const row of native.cases.find((v: any) => v.case === "actual-native-client-ib-position-and-pcm").observed) {
    const result = sourcePacketSpatial({ x: row.listener[0], y: row.listener[1] },
      { x: row.source_tile[0] * 128, y: row.source_tile[1] * 128 }, row.range_field, row.retain_field, 127);
    assert.equal(result.distance, row.distance_source_units);
    assert.equal(result.retainedRadius, row.retained_source_units);
    assert.equal(result.volume, row.expected_native_volume);
    assert.equal(row.all_stereo_samples_integer_error, 0);
    assert.equal(sourceMixerToAssetGain(result.volume), row.expected_native_volume / 128);
  }
  assert.equal(sourcePacketSpatial({ x: 2112, y: 1344 }, { x: 1280, y: 1280 }, 5, 31, 127).audible, false);
});

test("original object rectangle, inner retention and native plane/world-owner policy", async () => {
  const native = await evidence("position");
  for (const row of native.cases.find((v: any) => v.case === "native-ambient-rectangle-manhattan-minus64").observed) {
    const result = sourceAmbientSpatial({ x: row.listener[0], y: row.listener[1] },
      { minX: 1280, minY: 1280, maxX: 1536, maxY: 1536 }, 3, 0, 127);
    assert.equal(result.distance, row.native_distance);
  }
  for (const row of native.cases.find((v: any) => v.case === "actual-original-cooking-range-emitter").observed) {
    const result = sourceAmbientSpatial({ x: row.listener_x, y: 1344 },
      sourceObjectBounds({ x: 10, y: 10 }, 1, 1), 3, 0, 127);
    assert.equal(result.volume, row.initial_stream_volume);
  }
  const owners = [null, { id: "a", exteriorPlane: 0, audibleInteriorPlane: 0 },
    { id: "b", exteriorPlane: 0, audibleInteriorPlane: 0 }] as const;
  for (const row of native.cases.find((v: any) => v.case === "native-rq-world-entity-visibility").observed) {
    assert.equal(sourcePacketOwnerVisible(owners[row.listener_owner]!, owners[row.emitter_owner]!, row.cross_owner_flag),
      row.native_audible);
  }
  for (const row of native.cases.find((v: any) => v.case === "actual-dz-ambient-plane-visibility").observed) {
    assert.equal(sourceAmbientVisible(row.listener_plane, row.source_plane, null, null, row.visibility_id), row.native_visible);
  }
  assert.equal(sourceAmbientVisible(0, 0, owners[1], owners[2], 1), false);
  assert.equal(sourceAmbientVisible(0, 0, owners[1], null, 2), true);
  assert.equal(sourceAmbientVisible(0, 0, null, owners[1], 2), false);
});

test("actual signed ambient fade behavior, including immediate upward changes", async () => {
  const native = await evidence("position");
  for (const row of native.cases.find((v: any) => v.case === "native-wc-signed-fade-and-volume-rounding").observed) {
    const duration = sourceAmbientFadeDuration(row.start, row.target, row.base, row.configured_ms);
    assert.equal(duration, row.native_duration_ms);
    assert.deepEqual(row.elapsed_ms.map((time: number) => sourceAmbientFadeVolume(row.start, row.target, duration, time)),
      row.native_volumes);
  }
});

test("original music fade uses float32 stepped native master levels, not a generic linear ramp", async () => {
  const native = await evidence("music");
  for (const row of native.cases.find((v: any) => v.case === "original-wo-wp-music-fade-steps").observed) {
    assert.deepEqual(sourceMusicFade(row.volume, row.fade_cycles, row.direction), row.native_volumes);
  }
  const jingle = native.cases.find((v: any) => v.case === "source-music-parameters-across-jingle");
  assert.deepEqual(jingle.before, { out_delay: 0, out_duration: 60, in_delay: 60, in_duration: 0 });
  assert.deepEqual(jingle.during, { out_delay: 0, out_duration: 0, in_delay: 0, in_duration: 0 });
  assert.deepEqual(jingle.remembered_groups, [62]);
});

test("all native M1 morph cases use real varbit masks and original target definitions", async () => {
  const native = await evidence("objects");
  for (const row of native.cases.find((v: any) => v.case === "actual-om-dl-morph-selection").observed) {
    const selected = resolveSourceObject(row.object, new Map([[row.varp, row.varpValue]]));
    assert.equal(selected?.id ?? -1, row.native_selected);
  }
  assert.throws(() => resolveSourceObject(34815, new Map()), /source varp 491/);
  assert.deepEqual([sourceObjectDefinition(114)!.sizeX, sourceObjectDefinition(114)!.sizeY], [1, 2]);
});

test("source polygons and native table44 select the real six-track Modern area, not three guessed songs", () => {
  assert.deepEqual(SOURCE_MUSIC_MODE_IDS, { area: 0, shuffle: 1, single: 2 });
  for (const [x, y] of [[3222,3218], [3222,3280], [3166,3300]]) {
    const region = sourceMusicRegion({ x: x!, y: y!, plane: 0 })!;
    assert.equal(region.areaId, 1);
    assert.equal(region.defaultGroup, 76);
    assert.deepEqual(region.groups, SOURCE_LUMBRIDGE_GROUPS);
  }
  assert.equal(sourceMusicRegion({ x: 3222, y: 3280, plane: 0 }, "classic")!.defaultGroup, 2);
  assert.deepEqual(sourceMusicRegion({ x: 3094, y: 3107, plane: 0 })!.groups, [62]);
  assert.deepEqual(sourceMusicRegion({ x: 3094, y: 9507, plane: 0 })!.groups, [144]);
  assert.equal(sourceMusicRegion({ x: 3300, y: 3300, plane: 0 }), null);
  assert.equal(sourceMusicDurationSeconds(2), 229 * 0.6);
});

test("native mix comparisons stay source-qualified and explicitly identify unrepresentable clipping cases", async () => {
  const native = await evidence("pcm");
  const rows = native.cases.find((v: any) => v.case === "immutable128-pcm-native255-mixer-comparison").observed;
  assert.equal(rows.length, 35);
  assert.equal(rows.every((v: any) => Math.abs(v.output_rms_gain_error_db) <= 0.25), true);
  assert.deepEqual(SOURCE_MIX_REPRESENTATION_NEEDS.map((v) => v.group), [40, 54, 58, 64, 65]);
  for (const required of SOURCE_MIX_REPRESENTATION_NEEDS) {
    assert.equal(rows.find((v: any) => v.index === required.index && v.group === required.group).additional_clip_samples,
      required.additionalClipsAt255);
  }
});
