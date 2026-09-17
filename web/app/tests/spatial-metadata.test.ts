import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { gunzipSync } from "node:zlib";
import { AUDIO_INPUTS } from "../../audio/index.ts";
import { SOURCE_PACK_SHA256 } from "../../shared/contracts.ts";
import { AssetLoader } from "../assets.ts";
import { AppError } from "../errors.ts";
import type { AssetRecord, ContentManifest } from "../manifest.ts";
import { SourceSpatialMetadata } from "../spatial-metadata.ts";
import type { SpatialGeometry, SpatialWorld } from "../spatial-metadata.ts";

const repo = new URL("../../../", import.meta.url);
const originalRenderBytes = await readFile(new URL("assets/compiled/render/manifest.json", repo));
const originalRender = JSON.parse(originalRenderBytes.toString("utf8"));
const originalMap = await readFile(new URL("research/audio-source/source-map.json", repo));
const originalRegion = gunzipSync(await readFile(new URL("assets/source/osrs/cache2695/world/12336.json.gz", repo)));
const originalOffice = gunzipSync(await readFile(new URL("assets/source/osrs/cache2695/world/12633.json.gz", repo)));
const originalObjects = JSON.parse(gunzipSync(await readFile(new URL("assets/source/osrs/cache2695/collections/object.json.gz", repo))).toString("utf8"));
const digest = (value: Uint8Array | string) => createHash("sha256").update(value).digest("hex");

// Source records under a controlled metadata-only transport, never a seeded game/server.
function fixtures(changeRegion?: (region: Record<string, unknown>) => void) {
  const assets: AssetRecord[] = [];
  const files = new Map<string, Uint8Array>();
  const entities: ContentManifest["catalog"]["entities"] = {};
  const add = (id: string, bytes: Uint8Array) => {
    const url = `/assets/metadata-fixture/${assets.length}.json`;
    assets.push({ id, url, sha256: digest(bytes), bytes: bytes.length, contentType: "application/json" });
    files.set(url, bytes);
    return id;
  };
  const rendererId = add("asset.fixture.renderer", originalRenderBytes);
  const mapId = add("asset.fixture.audio_map", originalMap);
  const region = JSON.parse(originalRegion.toString("utf8"));
  changeRegion?.(region);
  const regionId = add("asset.source.osrs.cache2695.region.12336", changeRegion
    ? Buffer.from(JSON.stringify(region)) : originalRegion);
  const officeId = add("asset.source.osrs.cache2695.region.12633", originalOffice);
  const selected = new Set<number>(originalRender.dynamic_objects.map((value: { object_id: number }) => value.object_id));
  selected.add(683);
  selected.add(34815);
  selected.add(34817);
  for (const [id, raw] of Object.entries(originalObjects)) {
    if (!raw || typeof raw !== "object" || !("id" in raw) || typeof raw.id !== "number" || !selected.has(raw.id)) continue;
    add(id, Buffer.from(JSON.stringify(raw)));
    entities[`object.fixture.${raw.id}`] = { name: `Source object ${raw.id}`, sourceId: raw.id, asset: id };
  }
  const manifest: ContentManifest = {
    schemaVersion: 1, sourcePackSha256: SOURCE_PACK_SHA256, contentRevision: "metadata-fixture-only",
    artifactSha256: "0".repeat(64), assets, bootstrap: [],
    catalog: { contentRevision: "metadata-fixture-only", entities, equipmentSlots: [], items: {}, skills: {}, quests: {} },
    aliases: { [AUDIO_INPUTS.map.path]: mapId },
    rendererManifest: rendererId,
    renderer: { coverage: "source_world_blocks", assetBaseUrl: "/assets/compiled/render/",
      manifestSha256: assets[0]!.sha256, assetIds: {}, fixtures: {}, commonAssets: [],
      regions: { "region.osrs.12336": { square: 12336, requiredAssets: [] },
        "region.osrs.12633": { square: 12633, requiredAssets: [] } } },
    regions: { "region.osrs.12336": { sceneId: "region.osrs.12336", sceneAsset: regionId,
      requiredAssets: [], routeId: "region.osrs.12336", workloadId: "metadata-fixture-only", camera: null, controls: null },
    "region.osrs.12633": { sceneId: "region.osrs.12633", sceneAsset: officeId,
      requiredAssets: [], routeId: "region.osrs.12633", workloadId: "metadata-fixture-only", camera: null, controls: null } },
    instanceLayouts: { "instance_template.death.office": { chunkSize: 8,
      chunks: [[3168, 5720], [3168, 5728], [3176, 5720], [3176, 5728]].map(([x, y]) => ({
        sourceRegion: "region.osrs.12633", destinationRegion: "region.osrs.12633",
        sourceOrigin: { x: x!, y: y!, plane: 0 }, destinationOrigin: { x: x!, y: y!, plane: 0 }, quarterTurns: 0,
      })) } },
  };
  const fetched: string[] = [];
  const loader = new AssetLoader(manifest, "0".repeat(64), { fetch: async input => {
    const url = String(input), bytes = files.get(url);
    fetched.push(url);
    return bytes ? new Response(new Uint8Array(bytes).buffer, { headers: { "content-type": "application/json" } })
      : new Response("missing metadata fixture", { status: 404 });
  } });
  return { loader, manifest, fetched };
}

function world(): SpatialWorld {
  return {
    player: { id: "actor.metadata.fixture", region: "region.osrs.12336", tile: { x: 3094, y: 3107, plane: 0 }, instance: null },
    scene: { region: "region.osrs.12336", instance: null, instanceTemplate: null },
    ui: { version: 1, appearance: { base: { sourceNpc: 2063, asset: "asset.source.osrs.cache2695.npc.2063",
      adaptation: "milestones/approvals/m1-reference-pack-v1.3.0.json" } } },
    dynamicObjects: [],
    audioAuthority: { version: 1, profile: "spatial-fixture-no-music-grants",
      music: { history: "legacy_untracked", complete: false, trackedFromTick: "9007199254740993",
        revision: "18446744073709551615", unlockedGroups: [],
        tracks: [{ group: 62, status: "unknown", confirmedAtTick: null, rule: null }] },
      varps: [{ id: 491, value: 0, knownBits: 20, binding: "source491.bits2and4", unavailableReason: null }] },
  };
}

const geometry: SpatialGeometry = {
  sceneId: "blocks@3072,3072", placement: { baseX: 3072, baseY: 3072, sizeTiles: 104, blocks: true },
  loadedSquares: [12336],
  owner: null,
};

test("original rows produce stable placed ambience, not an interactable-entity approximation", async () => {
  const { loader } = fixtures();
  try {
    const metadata = await SourceSpatialMetadata.load(loader);
    const current = world(), before = JSON.stringify(current);
    const scene = metadata.scene(current, geometry);
    const region = JSON.parse(originalRegion.toString("utf8"));
    const ordinal = region.placements.findIndex((row: number[]) =>
      row[0] === 683 && row[1] === 3092 && row[2] === 3101 && row[3] === 0);
    assert(ordinal >= 0);
    assert.deepEqual(scene.emitters.find(emitter => emitter.id === `asset.source.osrs.cache2695.region.12336:placement:${ordinal}`), {
      id: `asset.source.osrs.cache2695.region.12336:placement:${ordinal}`, objectId: 683,
      tile: { x: 3092, y: 3101, plane: 0 }, orientation: 3, instance: null, owner: null, present: true,
    });
    assert(scene.emitters.some(emitter => emitter.objectId === 16446), "Invisible original audio locs are retained.");
    assert.deepEqual(scene.listener, { x: 396096, y: 397760 });
    assert.equal(scene.owner, null);
    assert(Object.isFrozen(scene.emitters));
    assert.equal(JSON.stringify(current), before, "Unknown music/history and authoritative variables are not rewritten.");
    assert.equal(current.audioAuthority!.music.tracks[0]!.status, "unknown");
    assert(metadata.requiredAssets.includes("asset.source.osrs.cache2695.region.12336"));
  } finally { loader.dispose(); }
});

test("dynamic source identities and real orientations route without inventing a placement", async () => {
  const { loader } = fixtures();
  try {
    const metadata = await SourceSpatialMetadata.load(loader);
    const current = world();
    current.dynamicObjects = [{ id: "dynamic.fixture.fire", objectId: "object.fixture.26185", sourceId: 26185,
      tile: { ...current.player.tile }, instance: null, quarterTurns: 2 }];
    assert.equal(metadata.scene(current, geometry).emitters.find(emitter => emitter.id === "dynamic.fixture.fire")?.orientation, 2);
    current.dynamicObjects = [];
    assert(!metadata.scene(current, geometry).emitters.some(emitter => emitter.id === "dynamic.fixture.fire"));
    current.dynamicObjects = [{ id: "dynamic.fixture.fire", objectId: "object.fixture.26185", sourceId: 26185,
      tile: { ...current.player.tile }, instance: null }];
    assert.throws(() => metadata.scene(current, geometry),
      (error: unknown) => error instanceof AppError && error.errorId === "audio.source.dynamic_orientation");
    current.dynamicObjects[0]!.sourceId = 683;
    assert.throws(() => metadata.scene(current, geometry),
      (error: unknown) => error instanceof AppError && error.errorId === "audio.source.dynamic_identity");
  } finally { loader.dispose(); }
});

test("missing owner, coverage and authority remain explicit metadata gaps", async () => {
  const { loader } = fixtures();
  try {
    const metadata = await SourceSpatialMetadata.load(loader);
    const current = world();
    assert.throws(() => metadata.scene(current, { ...geometry, loadedSquares: [12336, 12337] }),
      (error: unknown) => error instanceof AppError && error.errorId === "audio.source.region_coverage");
    const noOwner = { ...geometry };
    Reflect.deleteProperty(noOwner, "owner");
    assert.throws(() => metadata.scene(current, noOwner),
      (error: unknown) => error instanceof AppError && error.errorId === "audio.source.world_owner_required");
    const absent = { ...current };
    delete absent.audioAuthority;
    assert.throws(() => metadata.scene(absent, geometry), /without audio authority/);
    assert.throws(() => metadata.scene(current, { ...geometry, placement: null }),
      (error: unknown) => error instanceof AppError && error.errorId === "audio.source.scene_geometry");
  } finally { loader.dispose(); }
});

test("the actual declared instance chunks route placement metadata without interpreting the opaque instance ID", async () => {
  const { loader } = fixtures();
  try {
    const metadata = await SourceSpatialMetadata.load(loader);
    const current = world();
    current.player.region = "region.osrs.12633";
    current.player.tile = { x: 3169, y: 5721, plane: 0 };
    current.player.instance = "instance.opaque.fixture";
    current.scene = { region: current.player.region, instance: current.player.instance, instanceTemplate: "instance_template.death.office" };
    const source: SpatialGeometry = { sceneId: "blocks@3120,5672#instance_template.death.office",
      placement: { baseX: 3120, baseY: 5672, sizeTiles: 104, blocks: true }, loadedSquares: [12633], owner: null };
    const scene = metadata.scene(current, source);
    assert.equal(scene.instance, "instance.opaque.fixture");
    assert.equal(scene.owner, null, "Root-view ownership was supplied independently from gameplay instance identity.");
    assert(scene.emitters.every(emitter => emitter.instance === current.scene!.instance
      && emitter.tile.plane === 0 && emitter.tile.x >= 3168 && emitter.tile.x < 3184
      && emitter.tile.y >= 5720 && emitter.tile.y < 5736));
    assert.throws(() => metadata.scene(current, { ...source, sceneId: "blocks@3120,5672" }), /authoritative instance template/);
    current.scene.instanceTemplate = "instance_template.unpublished";
    assert.throws(() => metadata.scene(current, source), /no compiler-validated layout/);
  } finally { loader.dispose(); }
});

test("wrong source-region identity or malformed rows cannot become an empty successful scene", async () => {
  for (const corrupt of [
    (region: Record<string, unknown>) => { region.region_id = 12337; },
    (region: Record<string, unknown>) => { region.placement_columns = ["guessed", "columns"]; },
    (region: Record<string, unknown>) => { region.placements = [[683, 3092, 3101, 4, 10, 0]]; },
  ]) {
    const { loader } = fixtures(corrupt);
    try {
      await assert.rejects(SourceSpatialMetadata.load(loader),
        (error: unknown) => error instanceof AppError && error.kind === "source_spatial_metadata");
    } finally { loader.dispose(); }
  }
});
