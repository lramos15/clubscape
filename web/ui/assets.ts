import { SOURCE_PACK_SHA256 } from "../shared/contracts.ts";
import type { ClientAssets } from "../shared/contracts.ts";

export interface Rect { x: number; y: number; width: number; height: number }
export interface SpriteFrame extends Rect {
  offsetX: number; offsetY: number; canvasWidth: number; canvasHeight: number;
}
export interface SpriteAsset { sourceId: number; asset: string; frames: SpriteFrame[] }
export interface FontAsset { sourceId: number; ascent: number; advances: number[] }
export interface NativeWidget extends Rect {
  id: number; index: number; parent: number; type: number; contentType: number;
  text: string; sprite: number; item: number; item_quantity: number;
  font: number; color: number; shadow: boolean; lineHeight: number; xText: number; yText: number;
  opacity: number; filled: boolean; tiling: boolean; flipX: boolean; flipY: boolean; border: number;
  spriteShadow: number;
  scrollX: number; scrollY: number; scrollWidth: number; scrollHeight: number;
  originalX: number; originalY: number; originalWidth: number; originalHeight: number;
  xMode: number; yMode: number; widthMode: number; heightMode: number;
  actions: Array<string | null> | null; name: string; targetVerb: string;
  quantityMode: number; dragTime: number; dragZone: number;
  modelType: number; model: number; modelZoom: number; modelRotation: number[];
  onOp: Array<unknown> | null; noClickThrough: boolean; if3: boolean;
  lineWidth: number; lineDirection: boolean;
}
export interface ItemAsset {
  name: string; examine: string; stackable: number;
  interfaceOptions: Array<string | null>; shiftClickDropIndex: number;
  notedID: number; notedTemplate: number;
  icons: Array<{ minimum: number; asset: string; selectedAsset: string; zeroShadowAsset: string }>;
}
export interface TutorialBinding {
  state_id: string; case_id: string; hud_signature_id: string; declared_controls: string[];
  visual_variant_ids: string[]; source_text_record_ids: string[];
}
export interface UiCatalogue {
  version: number; sourcePackSha256: string; sourceCache: number; nativeCanvas: number[];
  sprites: Record<string, SpriteAsset>; fonts: Record<string, FontAsset>; items: Record<string, ItemAsset>;
  titleBackground: string; templates: Record<string, NativeWidget[]>;
  namedSprites: Record<string, number>;
  minimaps: Array<{ baseX: number; baseY: number; plane: number; asset: string }>;
  combatCategories: Array<{ id: number; columnValues: unknown[][] }>;
  questTable: { available: number; maximum_points: number };
  definitions: Record<string, Record<string, unknown>>;
  portraits: Record<string, { asset: string; offsetX: number; offsetY: number }>;
  npcs: Record<string, { name: string; examine: string | null }>;
  proposals: Record<string, {
    content: { heading: string; lines: string[]; buttons: string[] };
    frame: number[]; controls: Array<{ label: string; rectangle: number[] }>;
  }>;
  tutorialStates: TutorialBinding[];
  hudSignatures: Array<{ id: string; expected_introduced_tabs: string[]; state_ids: string[] }>;
  presentation?: {
    interfaces: Record<string, { name: string; sourceIds: number[] }>;
    skills: Record<string, { name: string; sourceId: number; thresholds: number[] }>;
    styleIds: string[];
    experiences: Array<{ id: string; name: string }>;
    appearance: Record<string, number[]>;
    sourceItems: Record<string, number>;
    weapons: Record<string, string[]>;
    weaponCategories: Record<string, number>;
    equipment: Record<string, { attack: Record<string, number>; defence: Record<string, number>;
      melee_strength: number; ranged_strength: number; magic_damage_percent: number; prayer: number }>;
    runEnergyScale: number;
  };
}

export function contains(rect: Rect, x: number, y: number): boolean {
  return x >= rect.x && y >= rect.y && x < rect.x + rect.width && y < rect.y + rect.height;
}

export function intersect(a: Rect, b: Rect): Rect {
  const x = Math.max(a.x, b.x), y = Math.max(a.y, b.y);
  return { x, y, width: Math.max(0, Math.min(a.x + a.width, b.x + b.width) - x),
    height: Math.max(0, Math.min(a.y + a.height, b.y + b.height) - y) };
}

export class UiAssets {
  readonly images = new Map<string, HTMLImageElement>();
  private readonly loading = new Map<string, Promise<HTMLImageElement>>();
  private readonly failed = new Set<string>();
  private onLoad: () => void = () => {};
  private disposed = false;
  readonly catalogue: UiCatalogue;
  private readonly client: ClientAssets;
  private readonly onError: (error: Error, id: string) => void;

  private constructor(
    catalogue: UiCatalogue, client: ClientAssets, onError: (error: Error, id: string) => void,
  ) { this.catalogue = catalogue; this.client = client; this.onError = onError; }

  static async load(client: ClientAssets, onError: (error: Error, id: string) => void): Promise<UiAssets> {
    const value = await client.json("ui/manifest.json") as UiCatalogue;
    if (value.version !== 1 || value.sourcePackSha256 !== SOURCE_PACK_SHA256 || value.sourceCache !== 2695) {
      throw new Error("UI assets do not match the owner-approved source pack.");
    }
    for (const id of [494, 495, 496, 497]) {
      if (value.fonts[id]?.advances.length !== 256 || value.sprites[id]?.frames.length !== 256) {
        throw new Error(`Original CP1252 font ${id} is missing or corrupt.`);
      }
    }
    const result = new UiAssets(value, client, onError);
    // Small UI frames are a single startup dependency, not a copy of the world or a reference screenshot.
    await Promise.all([value.titleBackground, ...Object.values(value.sprites).map(s => s.asset)]
      .map(id => result.require(id)));
    return result;
  }

  changed(listener: () => void): void { this.onLoad = listener; }
  dispose(): void {
    this.disposed = true; this.onLoad = () => {};
    this.images.clear(); this.loading.clear(); this.failed.clear();
  }

  require(id: string): Promise<HTMLImageElement> {
    const old = this.loading.get(id);
    if (old) return old;
    const request = this.client.image(id).then(image => {
      if (!image.naturalWidth || !image.naturalHeight) throw new Error(`UI image has no pixels: ${id}`);
      if (!this.disposed) { this.images.set(id, image); this.onLoad(); }
      return image;
    }).catch(error => {
      if (!this.disposed) this.failed.add(id);
      throw error;
    });
    this.loading.set(id, request);
    return request;
  }

  image(id: string): HTMLImageElement | null {
    const image = this.images.get(id);
    if (image) return image;
    if (!this.loading.has(id) && !this.failed.has(id)) {
      void this.require(id).catch(error => {
        if (!this.disposed) this.onError(error instanceof Error ? error : new Error(String(error)), `ui.asset.${id}`);
      });
    }
    return null;
  }

  async retryFailed(): Promise<void> {
    if (this.disposed) return;
    const ids = [...this.failed];
    for (const id of ids) { this.failed.delete(id); this.loading.delete(id); }
    await Promise.all(ids.map(id => this.require(id)));
  }

  async preloadItems(ids: readonly number[]): Promise<void> {
    await Promise.all(ids.flatMap(id => this.catalogue.items[id]?.icons.flatMap(icon =>
      [this.require(icon.asset), this.require(icon.selectedAsset), this.require(icon.zeroShadowAsset)]) ?? []));
  }

  itemAsset(sourceId: number, quantity: number, selected = false, shadow = 0x333333): string | null {
    const definitions = this.catalogue.items[sourceId]?.icons;
    if (!definitions) return null;
    let chosen = definitions[0];
    for (const icon of definitions) if (quantity >= icon.minimum) chosen = icon;
    return chosen ? (selected ? chosen.selectedAsset : shadow === 0 ? chosen.zeroShadowAsset : chosen.asset) : null;
  }
}
