import assert from "node:assert/strict";
import { createHash, randomUUID } from "node:crypto";
import { mkdir, readFile, rm } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { AUDIO_INPUTS } from "../../../web/audio/index.ts";
import { deliverAudioAssets } from "../audio-assets.ts";
import { publicFile } from "../deliver.ts";

test("audio delivery preserves all frozen IDs and ships the required fourth metadata route and exact nine supplements", async () => {
  const root = fileURLToPath(new URL("../../../", import.meta.url));
  const directory = resolve(root, ".local/audio-delivery-checks", randomUUID());
  await mkdir(directory, { recursive: true });
  try {
    const data = await deliverAudioAssets(directory);
    assert.equal(data.metadata.length, 4);
    assert.equal(data.assets.length, 279);
    assert.equal(data.files.length, 279);
    const supplementId = data.aliases[AUDIO_INPUTS.supplement.path];
    assert.equal(supplementId, "asset.source.audio.metadata.supplement");
    assert(data.metadata.includes(supplementId));
    const meta = data.assets.find((asset) => asset.id === supplementId)!;
    assert.equal(meta.url, "/content/audio/supplement.json");
    assert.equal(meta.sha256, "840aef91bac9a1fd042bdb1c3662ff92a378e279f48335108730e168af550d91");
    const original = JSON.parse(await readFile(resolve(root, AUDIO_INPUTS.manifest.path), "utf8")) as {
      assets: Array<{ asset_id: string; sha256: string; kind: string; source_group: number }>;
    };
    for (const asset of original.assets) {
      const delivered = data.assets.find((value) => value.id === asset.asset_id)!;
      assert.equal(delivered.sha256, asset.sha256);
      assert.equal(delivered.url, `/assets/audio/${asset.kind}-${asset.source_group}.flac`);
    }
    const supplement = JSON.parse(await readFile(resolve(root, AUDIO_INPUTS.supplement.path), "utf8")) as {
      assets: Array<{ asset_id: string; path: string; sha256: string; size_bytes: number }>;
    };
    assert.equal(supplement.assets.reduce((sum, asset) => sum + asset.size_bytes, 0), 45_199_584);
    for (const asset of supplement.assets) {
      const delivered = data.assets.find((value) => value.id === asset.asset_id)!;
      assert.equal(data.aliases[asset.path], asset.asset_id);
      assert.equal(delivered.url, `/${asset.path}`);
      assert.equal(delivered.contentType, "audio/flac");
      await publicFile(directory, asset.path, delivered.url, delivered);
      assert.equal(createHash("sha256").update(await readFile(resolve(root, asset.path))).digest("hex"), asset.sha256);
    }
    assert.notEqual(data.assets.find((asset) => asset.id.endsWith("music.64.native255"))?.url,
      data.assets.find((asset) => asset.id.endsWith("jingle.64.native255"))?.url);
    assert.equal(new Set(data.assets.map((asset) => asset.id)).size, data.assets.length);
  } finally { await rm(directory, { recursive: true, force: true }); }
});
