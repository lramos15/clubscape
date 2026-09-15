import type { RenderCamera } from "../shared/contracts.ts";
import { SOURCE_PACK_SHA256 } from "../shared/contracts.ts";
import { deepFreeze, invariant } from "./errors.ts";
import { assetId, isHash, publicPath } from "./identity.ts";

export interface AssetRecord {
  id: string;
  url: string;
  sha256: string;
  bytes: number;
  contentType: string;
}

export interface SourceCameraControls {
  yawUnitsPerPixel: number;
  pitchUnitsPerPixel: number;
  keyboardYawUnitsPerSecond: number;
  keyboardPitchUnitsPerSecond: number;
  minimumPitch: number;
  maximumPitch: number;
  zoomPerWheelStep: number;
  minimumZoom: number;
  maximumZoom: number;
  tileWorldUnits: number;
}

export interface RegionPresentation {
  sceneId: string;
  sceneAsset: string;
  requiredAssets: string[];
  routeId: string;
  workloadId: string;
  camera: RenderCamera | null;
  controls: SourceCameraControls | null;
}

export interface DisplayCatalog {
  contentRevision: string;
  items: Record<string, { name: string; sourceId: number | null; asset: string | null }>;
  skills: Record<string, { name: string; sourceId: number | null; asset: string | null }>;
  entities: Record<string, { name: string; sourceId: number | null; asset: string | null }>;
  quests: Record<string, { name: string; completedStage: string }>;
  equipmentSlots: string[];
  icons?: Record<string, string>;
  shops?: Record<string, { name: string; currency: string }>;
  inventoryActions?: Record<string, string[]>;
}

export interface ContentValidation {
  contentSchemaVersion: number;
  artifactVersion: number;
  unresolvedBindings: string[];
  readinessAuthority: "server_readiness_profile";
  runtimeReadinessEstablished: false;
}

interface RendererDeliveryBase {
  manifestSha256: string;
  assetBaseUrl: string;
  assetIds: Record<string, string>;
  fixtures: Record<string, { sourceInputId: string; camera: RenderCamera; requiredAssets: string[] }>;
}
export type RendererDelivery = RendererDeliveryBase & (
  { coverage: "named_fixture_scenes_only" }
  | { coverage: "source_world_blocks"; commonAssets: string[];
    regions: Record<string, { square: number; requiredAssets: string[] }> }
);

export interface ContentManifest {
  schemaVersion: 1;
  sourcePackSha256: string;
  contentRevision: string;
  artifactSha256: string;
  catalog: DisplayCatalog;
  assets: AssetRecord[];
  bootstrap: string[];
  rendererManifest: string | null;
  regions: Record<string, RegionPresentation>;
  contentValidation?: ContentValidation;
  aliases?: Record<string, string>;
  renderer?: RendererDelivery;
}

export function parseContentManifest(value: unknown): ContentManifest {
  invariant(value !== null && typeof value === "object" && !Array.isArray(value), "Missing public ContentManifest.");
  const manifest = value as ContentManifest;
  invariant(manifest.schemaVersion === 1 && manifest.sourcePackSha256 === SOURCE_PACK_SHA256,
    "The public content manifest does not match the approved source pack.");
  invariant(typeof manifest.contentRevision === "string" && manifest.contentRevision.length > 0
    && manifest.contentRevision.length <= 256 && isHash(manifest.artifactSha256), "Invalid source content identity.");
  invariant(manifest.catalog?.contentRevision === manifest.contentRevision, "Catalog content revision mismatch.");
  if (manifest.contentValidation !== undefined) {
    const validation = manifest.contentValidation;
    invariant(validation.contentSchemaVersion === 3 && validation.artifactVersion === 3
      && validation.readinessAuthority === "server_readiness_profile" && validation.runtimeReadinessEstablished === false
      && Array.isArray(validation.unresolvedBindings) && validation.unresolvedBindings.length <= 20_000
      && validation.unresolvedBindings.every((path) => typeof path === "string" && path.length > 0 && path.length <= 1024),
    "Invalid compiler provenance or fabricated runtime-readiness claim.");
  }
  if (manifest.catalog.shops !== undefined) {
    invariant(typeof manifest.catalog.shops === "object" && !Array.isArray(manifest.catalog.shops)
      && Object.keys(manifest.catalog.shops).length <= 4096, "Invalid source shop display catalog.");
    for (const shop of Object.values(manifest.catalog.shops)) {
      invariant(typeof shop.name === "string" && Object.hasOwn(manifest.catalog.items, shop.currency),
        "A source shop currency is missing from the item catalog.");
    }
  }
  for (const table of [manifest.catalog.items, manifest.catalog.skills, manifest.catalog.entities, manifest.catalog.quests]) {
    invariant(table && typeof table === "object" && !Array.isArray(table) && Object.keys(table).length <= 20_000,
      "Invalid public display catalog.");
  }
  invariant(Array.isArray(manifest.catalog.equipmentSlots) && manifest.catalog.equipmentSlots.length <= 64,
    "Invalid source equipment-slot catalog.");
  invariant(Array.isArray(manifest.assets) && manifest.assets.length <= 20_000, "Invalid content asset count.");
  const ids = new Set<string>();
  const urls = new Set<string>();
  let bytes = 0;
  for (const asset of manifest.assets) {
    assetId(asset.id);
    publicPath(asset.url);
    invariant(asset.url.startsWith("/assets/") || asset.url.startsWith("/content/"),
      "Content assets must use /assets/ or /content/ routes.");
    invariant(!ids.has(asset.id) && !urls.has(asset.url), "Duplicate content asset ID or URL.");
    invariant(isHash(asset.sha256) && Number.isSafeInteger(asset.bytes) && asset.bytes > 0 && asset.bytes <= 64 * 1024 * 1024,
      "Invalid asset hash or byte count.");
    invariant(typeof asset.contentType === "string" && /^(image\/(png|jpeg|webp)|audio\/(flac|wav|ogg)|font\/(woff2?|ttf)|application\/(json|octet-stream)|model\/gltf-binary)(; charset=utf-8)?$/.test(asset.contentType),
      "Unsupported public content MIME type.");
    ids.add(asset.id);
    urls.add(asset.url);
    bytes += asset.bytes;
  }
  invariant(bytes <= 512 * 1024 * 1024, "The public content pack exceeds its deployment byte budget.");
  if (manifest.aliases !== undefined) {
    invariant(manifest.aliases && typeof manifest.aliases === "object" && !Array.isArray(manifest.aliases)
      && Object.keys(manifest.aliases).length <= 20_000, "Invalid source path alias table.");
    for (const [path, id] of Object.entries(manifest.aliases)) {
      publicPath(`/${path}`);
      invariant(!path.startsWith("/") && ids.has(id) && !ids.has(path),
        "Source path aliases must resolve only to declared same-origin assets.");
    }
  }
  for (const [id, asset] of Object.entries(manifest.catalog.icons ?? {})) {
    invariant((Object.hasOwn(manifest.catalog.items, id) || Object.hasOwn(manifest.catalog.skills, id))
      && manifest.assets.some((entry) => entry.id === asset && entry.contentType.startsWith("image/")),
    "Source icon bindings must reference known content IDs and declared raster assets.");
  }
  for (const [id, actions] of Object.entries(manifest.catalog.inventoryActions ?? {})) {
    invariant(Object.hasOwn(manifest.catalog.items, id) && Array.isArray(actions)
      && actions.length <= 16 && actions.every((name) => typeof name === "string" && name.length > 0 && name.length <= 128),
    "Invalid original inventory action-label binding.");
  }
  const assetsExist = (list: unknown): list is string[] => Array.isArray(list) && list.length <= 20_000
    && new Set(list).size === list.length && list.every((id) => ids.has(id));
  invariant(assetsExist(manifest.bootstrap), "Startup refers to missing/duplicate content assets.");
  invariant(manifest.rendererManifest === null || ids.has(manifest.rendererManifest),
    "The renderer manifest asset is absent from ContentManifest.");
  invariant(manifest.regions && typeof manifest.regions === "object" && !Array.isArray(manifest.regions)
    && Object.keys(manifest.regions).length <= 2048, "Invalid source region presentation index.");
  for (const [id, region] of Object.entries(manifest.regions)) {
    assetId(id);
    for (const text of [region.sceneId, region.routeId, region.workloadId]) assetId(text);
    invariant(ids.has(region.sceneAsset) && assetsExist(region.requiredAssets)
      && region.requiredAssets.includes(region.sceneAsset), "A region references absent source scene/assets.");
    if (region.camera !== null) {
      invariant(region.camera.unitsPerTurn === 16384
        && ["x", "height", "y", "pitch", "yaw", "unitsPerTurn", "zoom", "near", "far"].every((key) => {
          const value = region.camera![key as keyof RenderCamera];
          return typeof value === "number" && Number.isFinite(value);
        })
        && region.camera.near > 0 && region.camera.far > region.camera.near && region.camera.zoom > 0,
      "Invalid recorded source camera.");
    }
    if (region.controls !== null) {
      invariant(["yawUnitsPerPixel", "pitchUnitsPerPixel", "keyboardYawUnitsPerSecond", "keyboardPitchUnitsPerSecond",
        "minimumPitch", "maximumPitch", "zoomPerWheelStep", "minimumZoom", "maximumZoom", "tileWorldUnits"].every((key) => {
        const value = region.controls![key as keyof SourceCameraControls];
        return typeof value === "number" && Number.isFinite(value) && value > 0;
      })
        && region.controls.minimumPitch < region.controls.maximumPitch
        && region.controls.maximumPitch <= 16384
        && region.controls.minimumZoom < region.controls.maximumZoom,
      "Invalid source camera input bindings.");
    }
  }
  if (manifest.renderer !== undefined) {
    const renderer = manifest.renderer;
    publicPath(`${renderer.assetBaseUrl.replace(/\/$/, "")}/manifest.json`, "/assets/");
    invariant(["named_fixture_scenes_only", "source_world_blocks"].includes(renderer.coverage) && isHash(renderer.manifestSha256)
      && manifest.assets.find((asset) => asset.id === manifest.rendererManifest)?.sha256 === renderer.manifestSha256,
    "Invalid pinned renderer coverage/manifest identity.");
    for (const [path, id] of Object.entries(renderer.assetIds)) {
      publicPath(`${renderer.assetBaseUrl}${path}`, "/assets/");
      invariant(manifest.assets.some((asset) => asset.id === id && asset.url === `${renderer.assetBaseUrl}${path}`),
        "Renderer buffer is not bound to its exact declared public asset.");
    }
    for (const fixture of Object.values(renderer.fixtures)) {
      invariant(typeof fixture.sourceInputId === "string" && fixture.sourceInputId.startsWith("original.scenes.")
        && assetsExist(fixture.requiredAssets) && fixture.camera.unitsPerTurn === 16384
        && fixture.camera.near === 50 && Object.values(fixture.camera).every((value) => Number.isFinite(value)),
      "Invalid original renderer fixture/camera binding.");
    }
    if (renderer.coverage === "source_world_blocks") {
      invariant(assetsExist(renderer.commonAssets) && renderer.commonAssets.includes(manifest.rendererManifest!)
        && renderer.regions && typeof renderer.regions === "object" && !Array.isArray(renderer.regions)
        && Object.keys(renderer.regions).length > 0 && Object.keys(renderer.regions).length <= 2048,
      "Invalid source world-block renderer coverage.");
      for (const [id, block] of Object.entries(renderer.regions)) {
        invariant(Number.isSafeInteger(block.square) && block.square >= 0 && block.square <= 65535
          && id === `region.osrs.${block.square}` && assetsExist(block.requiredAssets) && block.requiredAssets.length === 2
          && block.requiredAssets.every((asset) => manifest.assets.find((entry) => entry.id === asset)?.url
            .startsWith(`${renderer.assetBaseUrl}blocks/${block.square}.`)),
        "A renderer region must retain its published square and two original block buffers.");
      }
    }
  }
  return deepFreeze(manifest);
}
