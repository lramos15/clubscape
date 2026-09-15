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
}

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
}

export function parseContentManifest(value: unknown): ContentManifest {
  invariant(value !== null && typeof value === "object" && !Array.isArray(value), "Missing public ContentManifest.");
  const manifest = value as ContentManifest;
  invariant(manifest.schemaVersion === 1 && manifest.sourcePackSha256 === SOURCE_PACK_SHA256,
    "The public content manifest does not match the approved source pack.");
  invariant(typeof manifest.contentRevision === "string" && manifest.contentRevision.length > 0
    && manifest.contentRevision.length <= 256 && isHash(manifest.artifactSha256), "Invalid source content identity.");
  invariant(manifest.catalog?.contentRevision === manifest.contentRevision, "Catalog content revision mismatch.");
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
  for (const [id, asset] of Object.entries(manifest.catalog.icons ?? {})) {
    invariant((Object.hasOwn(manifest.catalog.items, id) || Object.hasOwn(manifest.catalog.skills, id))
      && manifest.assets.some((entry) => entry.id === asset && entry.contentType.startsWith("image/")),
    "Source icon bindings must reference known content IDs and declared raster assets.");
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
  return deepFreeze(manifest);
}
