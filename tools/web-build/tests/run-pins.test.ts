import assert from "node:assert/strict";
import { createHash, randomUUID } from "node:crypto";
import { mkdir, rm, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { SOURCE_PACK_SHA256 } from "../../../web/shared/contracts.ts";
import { assertSourceRunPin, captureSourceRunPin } from "../run-pins.ts";

const root = fileURLToPath(new URL("../../../", import.meta.url));
const hash = (value: string): string => createHash("sha256").update(value).digest("hex");
async function fixture(directory: string, revision: string, worldId: string): Promise<void> {
  const artifact = `Identity fixture only, not a compiled game: ${revision}`;
  const content = JSON.stringify({
    schemaVersion: 1, sourcePackSha256: SOURCE_PACK_SHA256, contentRevision: revision, artifactSha256: hash(artifact),
    catalog: { contentRevision: revision, items: {}, entities: {}, skills: {}, quests: {}, equipmentSlots: [] },
    assets: [], bootstrap: [], rendererManifest: null, regions: {},
  });
  await mkdir(resolve(directory, "content"), { recursive: true });
  await writeFile(resolve(directory, "world.csc"), artifact);
  await writeFile(resolve(directory, "content/manifest.json"), content);
  await writeFile(resolve(directory, "clubscape-game.json"), JSON.stringify({
    schema_version: 1, world_id: worldId, artifact: "world.csc",
    sha256: hash(artifact), content_manifest_path: "/content/manifest.json",
  }));
  await writeFile(resolve(directory, "clubscape-game-assets.json"), JSON.stringify({
    schema_version: 1, files: [{ url: "/content/manifest.json", path: "content/manifest.json", sha256: hash(content) }],
  }));
}

test("old diagnostic run pins do not consult or adopt current canonical revision", async () => {
  const directory = resolve(root, ".local/run-pin-tests", randomUUID());
  try {
    await fixture(directory, "old-diagnostic-fixture", randomUUID());
    const pin = await captureSourceRunPin(directory);
    assert.equal(pin.contentRevision, "old-diagnostic-fixture");
    await assertSourceRunPin(directory, pin);
    await fixture(directory, "new-coherent-but-unapproved-replacement", pin.worldId);
    await assert.rejects(assertSourceRunPin(directory, pin), /Pinned run identity changed/);
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test("artifact/public metadata mismatches cannot be disguised as a restart", async () => {
  const directory = resolve(root, ".local/run-pin-tests", randomUUID());
  try {
    await fixture(directory, "unchanged-fixture", randomUUID());
    await writeFile(resolve(directory, "world.csc"), "different bytes");
    await assert.rejects(captureSourceRunPin(directory), /identities do not match/);
  } finally { await rm(directory, { recursive: true, force: true }); }
});
