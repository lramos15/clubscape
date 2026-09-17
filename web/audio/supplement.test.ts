import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { AUDIO_INPUTS, sourceAssetForLevel } from "./source.ts";
import type { SourceAsset, SourceCatalog } from "./source.ts";
import { sourceMixerToAssetGain } from "./native-policy.ts";

const root = new URL("../../", import.meta.url);
const load = async (path: string) => JSON.parse(await readFile(new URL(path, root), "utf8"));
const base = await load(AUDIO_INPUTS.manifest.path);
const supplement = await load(AUDIO_INPUTS.supplement.path);
const needs = await load("research/browser-audio-policy/publication-needs.json");

function signal(asset: any, nativeMixerLevel: 128 | 255): SourceAsset {
  return {
    id: asset.asset_id, kind: asset.kind, sourceId: asset.source_group, path: asset.path,
    sha256: asset.sha256, bytes: asset.size_bytes, playable: true, frames: asset.signal.frames,
    channels: asset.signal.channels, peak: asset.signal.peak, firstNonzeroFrame: asset.signal.first_nonzero_frame,
    endFrame: asset.loop.source_engine_end_frame, loopStart: 0, loopEnd: 0, inputGain: 1, nativeMixerLevel,
  };
}
function catalog(): SourceCatalog {
  const assets = new Map<string, SourceAsset>(), groups = new Map<string, SourceAsset>();
  const native255 = new Map<string, SourceAsset>();
  for (const raw of base.assets.filter((asset: any) => asset.kind !== "sfx")) {
    const asset = signal(raw, 128);
    assets.set(asset.id, asset); groups.set(`${asset.kind}:${asset.sourceId}`, asset);
  }
  for (const raw of supplement.assets) {
    const asset = signal(raw, 255), key = `${asset.kind}:${asset.sourceId}`;
    assets.set(asset.id, asset); native255.set(key, asset);
    if (!groups.has(key)) groups.set(key, asset);
  }
  return { assets, groups, native255, sequences: new Map(), ambient: new Map() };
}

test("the additive manifest keeps all base identities and publishes only the exact nine native inputs", async () => {
  assert.equal(supplement.base_manifest.sha256, AUDIO_INPUTS.manifest.sha256);
  assert.equal(supplement.base_manifest.path, AUDIO_INPUTS.manifest.path);
  assert.equal(supplement.assets.length, 9);
  assert.equal(supplement.frozen_files_modified, false);
  assert.equal(supplement.reencoded_base_files, 0);
  const ids = new Set(base.assets.map((a: any) => a.asset_id));
  for (const asset of supplement.assets) {
    assert.equal(ids.has(asset.asset_id), false);
    assert.equal(asset.native_mixer_level, 255);
    assert.equal(asset.encoding.bits_per_sample, 24);
    assert.equal(asset.encoding.effective_source_bits, 16);
    assert.equal(asset.encoding.gain_numerator, 1);
    assert.equal(asset.encoding.gain_denominator, 1);
    assert.equal(asset.loop.export_native_loop, false);
    assert.equal(asset.loop.native_midi_loop, false);
    assert.equal(asset.signal.frames, asset.loop.source_engine_end_frame + 22050);
    assert.equal(asset.native_render.native_startup_percussion_bank, 128);
    assert.equal(asset.native_render.native_startup_percussion_channel, 9);
    assert.equal(asset.native_render.independent_render_passes, 2);
    const bytes = await readFile(new URL(asset.path, root));
    assert.equal(bytes.length, asset.size_bytes);
    assert.equal(createHash("sha256").update(bytes).digest("hex"), asset.sha256);
    if (asset.kind === "jingle") {
      const expected = needs.requirements.find((item: any) => item.kind === "native_mixer_representation" && item.group === asset.source_group);
      assert.equal(asset.encoding.source_pcm_s16le_sha256, expected.required_native_255_pcm_sha256);
      assert.equal(asset.signal.frames, expected.frames);
      assert.equal(asset.native_render.source_device_saturation_samples, expected.source_native_clip_samples);
    }
  }
});

test("native representation selection preserves old IDs at safe levels and resolves native255 only when required", () => {
  const source = catalog();
  for (const id of [40, 54, 58, 64, 65]) {
    const baseId = `asset.source.osrs.cache2695.audio-runtime.jingle.${id}`;
    const highId = `asset.source.osrs.cache2695.audio-supplement.jingle.${id}.native255`;
    assert.equal(sourceAssetForLevel(source, "jingle", id, null, 44).id, baseId);
    assert.equal(sourceAssetForLevel(source, "jingle", id, baseId, 255).id, highId);
    assert.equal(sourceAssetForLevel(source, "jingle", id, highId, 44).id, highId);
    assert.equal(source.assets.get(baseId)!.nativeMixerLevel, 128);
  }
  assert.equal(sourceAssetForLevel(source, "jingle", 33, null, 255).nativeMixerLevel, 128);
  for (const id of [64, 327, 163, 145]) {
    assert.equal(sourceAssetForLevel(source, "music", id, null, 255).id,
      `asset.source.osrs.cache2695.audio-supplement.music.${id}.native255`);
  }
  assert.notEqual(sourceAssetForLevel(source, "music", 64, null, 255).id,
    sourceAssetForLevel(source, "jingle", 64, null, 255).id);
  assert.throws(() => sourceAssetForLevel(source, "music", 64,
    "asset.source.osrs.cache2695.audio-supplement.jingle.64.native255", 255));
  assert.throws(() => sourceAssetForLevel(source, "music", 65000, null, 255));
});

test("native255 PCM is never blindly amplified as a native128 render", () => {
  assert.equal(sourceMixerToAssetGain(255, 255), 1);
  assert.equal(sourceMixerToAssetGain(44, 255), 44 / 255);
  assert.equal(sourceMixerToAssetGain(127), 127 / 128);
  for (let level = 0; level <= 255; level++) assert.ok(sourceMixerToAssetGain(level, 255) <= 1);
});

test("independent native44/native136 controls meet the pre-existing mixer-gain and clipping bounds", async () => {
  const evidence = await load("research/browser-audio-policy/supplement-level-calibration.json");
  assert.equal(evidence.result, "passed");
  assert.equal(evidence.cases.length, 18);
  for (const result of evidence.cases) {
    assert.ok(Math.abs(result.gain_error_db) <= 0.25);
    assert.equal(result.additional_clip_samples, 0);
  }
});
