import type { NativeWidget } from "./assets.ts";
import type { Tile, WorldView } from "../shared/contracts.ts";
import { SourceRaster } from "./raster.ts";
import { MinimapSurfaceStore, UiMinimapError } from "./minimap-surface.ts";
import type { UiMinimapSurface, UiMinimapStatus } from "./minimap-surface.ts";
import type { MapIconSprite, MinimapIconPlacements } from "../renderer/src/index.ts";

interface Pixels { width: number; height: number; data: Uint8ClampedArray }
export type UiMinimapProjection = (width: number, height: number, scale: number) => MinimapIconPlacements;

export class MinimapPainter {
  private readonly decoded = new Map<string, Pixels>();
  private readonly masks = new Map<number, Array<[number, number]>>();
  private readonly surfaces = new MinimapSurfaceStore();
  private readonly issues = new Set<string>();
  private iconSprites: ReadonlyMap<number, { sprite: MapIconSprite; canvas: HTMLCanvasElement }> | null = null;
  private iconProjection: UiMinimapProjection | null = null;
  private retainedProjection: { frame: UiMinimapSurface; key: string; value: MinimapIconPlacements } | null = null;
  private suspended = false;
  rotation = 0;
  enabled = true;
  private readonly raster: SourceRaster;

  constructor(raster: SourceRaster) { this.raster = raster; }

  supply(surface: UiMinimapSurface, scope: string): boolean {
    const changed = this.surfaces.set(surface, scope);
    if (changed) this.retainedProjection = null;
    return changed;
  }
  supplySprites(sprites: ReadonlyMap<number, MapIconSprite>): void {
    const copied = new Map<number, { sprite: MapIconSprite; canvas: HTMLCanvasElement }>();
    for (const [element, sprite] of sprites) {
      const sizes = [sprite.width, sprite.height, sprite.maxWidth, sprite.maxHeight];
      const width = Math.max(1, sprite.width), height = Math.max(1, sprite.height);
      if (!Number.isSafeInteger(element) || element < 0 || sprite.element !== element ||
          sizes.some(value => !Number.isSafeInteger(value) || value < 0) ||
          !Number.isSafeInteger(sprite.offsetX) || !Number.isSafeInteger(sprite.offsetY) ||
          sprite.offsetX < 0 || sprite.offsetY < 0 || sprite.offsetX + sprite.width > sprite.maxWidth ||
          sprite.offsetY + sprite.height > sprite.maxHeight ||
          sprite.pixels.width !== width || sprite.pixels.height !== height ||
          sprite.pixels.data.length !== width * height * 4 ||
          (!(sprite.width && sprite.height) && sprite.pixels.data.some(value => value !== 0)))
        throw new UiMinimapError("The renderer supplied an invalid original map-icon sprite.", "ui.minimap.sprite");
      const pixels = new ImageData(width, height);
      pixels.data.set(sprite.pixels.data);
      const canvas = document.createElement("canvas");
      canvas.width = width; canvas.height = height;
      const context = canvas.getContext("2d");
      if (!context) throw new UiMinimapError("A map-icon drawing surface is unavailable.", "ui.minimap.canvas");
      context.putImageData(pixels, 0, 0);
      copied.set(element, { sprite: { ...sprite, pixels }, canvas });
    }
    this.iconSprites = copied;
    this.retainedProjection = null;
  }
  bindProjection(project: UiMinimapProjection): () => void {
    this.iconProjection = project;
    this.retainedProjection = null;
    return () => {
      if (this.iconProjection === project) { this.iconProjection = null; this.retainedProjection = null; }
    };
  }
  suspend(value: boolean): void { this.suspended = value; }
  retainScope(scope: string | null): void {
    this.surfaces.retainScope(scope);
    if (this.surfaces.current() === null) this.retainedProjection = null;
  }
  clear(): void { this.surfaces.clear(); this.issues.clear(); this.retainedProjection = null; }
  dispose(): void { this.clear(); this.decoded.clear(); this.masks.clear(); this.iconSprites = null; this.iconProjection = null; }
  status(): UiMinimapStatus | null {
    const frame = this.surfaces.current();
    const missing = frame?.icons.filter(icon => !this.iconSprites?.has(icon.element)).map(icon => icon.element) ?? [];
    return this.surfaces.status([...new Set(missing)], [...this.issues]);
  }
  information(): string {
    const value = this.status();
    if (!value) return "The renderer has not supplied a dynamic minimap surface.";
    return [
      `Source minimap revision ${value.revision}, plane ${value.plane}.`,
      value.complete ? "The renderer reports complete map data." : "The renderer reports incomplete map data.",
      `Terrain ${value.stats.terrainTiles}; walls ${value.stats.wallMarks}; diagonals ${value.stats.diagonalMarks}; map scenes ${value.stats.mapScenes}.`,
      `Source coverage ${value.sourceMaskPixels} pixels; unresolved ${value.stats.unresolved}.`,
      ...value.notes, ...value.uiIssues,
      ...(value.missingElements.length ? [`Original map-element assets missing: ${value.missingElements.join(", ")}.`] : []),
    ].join("\n");
  }

  private pixels(id: string): Pixels | null {
    const previous = this.decoded.get(id);
    if (previous) return previous;
    const image = this.raster.assets.image(id);
    if (!image) return null;
    const canvas = document.createElement("canvas");
    canvas.width = image.naturalWidth; canvas.height = image.naturalHeight;
    const context = canvas.getContext("2d", { willReadFrequently: true })!;
    context.drawImage(image, 0, 0);
    const result = { width: canvas.width, height: canvas.height, data: context.getImageData(0, 0, canvas.width, canvas.height).data };
    this.decoded.set(id, result);
    return result;
  }

  private mask(spriteId: number): Array<[number, number]> {
    const previous = this.masks.get(spriteId);
    if (previous) return previous;
    const sprite = this.raster.assets.catalogue.sprites[spriteId]!, frame = sprite.frames[0]!;
    const image = this.pixels(sprite.asset)!;
    const rows: Array<[number, number]> = [];
    for (let y = 0; y < frame.height; y++) {
      let start = 0, end = frame.width;
      for (let x = 0; x < frame.width; x++) {
        if (!image.data[((frame.y + y) * image.width + frame.x + x) * 4 + 3]) { start = x; break; }
      }
      for (let x = frame.width - 1; x >= start; x--) {
        if (!image.data[((frame.y + y) * image.width + frame.x + x) * 4 + 3]) { end = x + 1; break; }
      }
      rows.push([start, end]);
    }
    this.masks.set(spriteId, rows); return rows;
  }

  private project(widget: NativeWidget, image: Pixels, centerX: number, centerY: number): void {
    const mask = this.mask(widget.sprite);
    const surface = document.createElement("canvas"); surface.width = widget.width; surface.height = widget.height;
    const context = surface.getContext("2d")!, output = context.createImageData(widget.width, widget.height);
    const angle = (this.rotation & 16383) / 16384 * Math.PI * 2;
    const sin = Math.trunc(Math.sin(angle) * 65536), cos = Math.trunc(Math.cos(angle) * 65536);
    for (let y = 0; y < widget.height; y++) {
      const row = mask[y];
      if (!row) continue;
      for (let x = row[0]; x < row[1]; x++) {
        const dx = x - (widget.width >> 1), dy = y - (widget.height >> 1);
        const sx = centerX + (dy * sin + dx * cos >> 16), sy = centerY + (dy * cos - dx * sin >> 16);
        const to = (y * widget.width + x) * 4;
        output.data[to + 3] = 255;
        if (sx < 0 || sy < 0 || sx >= image.width || sy >= image.height) continue;
        const from = (sy * image.width + sx) * 4;
        output.data.set(image.data.subarray(from, from + 4), to);
      }
    }
    context.putImageData(output, 0, 0);
    this.raster.context.drawImage(surface, widget.x, widget.y);
  }

  private point(widget: NativeWidget, dx: number, dy: number, width: number, height: number,
    paint: (x: number, y: number) => void): void {
    const distance = dx * dx + dy * dy;
    if (distance > 6400) return;
    const angle = (this.rotation & 16383) / 16384 * Math.PI * 2;
    const sin = Math.trunc(Math.sin(angle) * 65536), cos = Math.trunc(Math.cos(angle) * 65536);
    const x = widget.x + (widget.width >> 1) + ((dx * cos + dy * sin) >> 16) - Math.trunc(width / 2);
    const y = widget.y + (widget.height >> 1) - ((dy * cos - dx * sin) >> 16) - Math.trunc(height / 2);
    if (distance <= 2500) { paint(x, y); return; }
    const mask = this.mask(widget.sprite);
    for (let row = Math.max(0, y - widget.y); row < Math.min(widget.height, y + height - widget.y); row++) {
      const span = mask[row];
      if (span) this.raster.clip({ x: widget.x + span[0], y: widget.y + row, width: span[1] - span[0], height: 1 }, () => paint(x, y));
    }
  }

  private dot(widget: NativeWidget, player: Tile, tile: Tile, kind: number): void {
    if (tile.plane !== player.plane) return;
    const id = `ui/minimaps/dot-${kind}.png`, image = this.raster.assets.image(id);
    if (image) this.point(widget, (tile.x - player.x) * 4, (tile.y - player.y) * 4,
      image.naturalWidth, image.naturalHeight, (x, y) => { this.raster.image(id, x, y); });
  }

  private unavailable(widget: NativeWidget): void {
    this.mask(widget.sprite).forEach(([start, end], y) =>
      this.raster.fill({ x: widget.x + start, y: widget.y + y, width: end - start, height: 1 }, 0));
    this.raster.textBox("Map data unavailable", widget, { font: 494, xAlign: 1, yAlign: 1, color: 0xffff00 });
  }

  private live(widget: NativeWidget, player: Tile, world: WorldView): void {
    this.issues.clear();
    const frame = this.surfaces.current();
    if (!frame || frame.plane !== player.plane) {
      this.unavailable(widget);
      return;
    }
    const projectionKey = [widget.width, widget.height, player.x, player.y, this.rotation & 16383].join("/");
    let placement: MinimapIconPlacements | null = null;
    if (frame.icons.length) {
      if (this.suspended) {
        this.issues.add("Live map-icon projection is paused until the world reconnects.");
        const previous = this.retainedProjection;
        if (previous?.frame !== frame || previous.key !== projectionKey) {
          this.issues.add("No matching previously rendered native projection is available.");
          this.unavailable(widget);
          return;
        }
        placement = previous.value;
      } else {
        if (!this.iconProjection || !this.iconSprites)
          throw new UiMinimapError("Bind the renderer's original map-icon sprites and placement projection.", "ui.minimap.binding");
        this.retainedProjection = null;
        placement = this.iconProjection(widget.width, widget.height, frame.scale / 128);
      }
      if (placement.minimapAngle !== (this.rotation & 16383) || placement.scale !== frame.scale / 128 ||
          !Number.isSafeInteger(placement.missingSprites) || placement.missingSprites < 0 || !Array.isArray(placement.icons))
        throw new UiMinimapError("The renderer map-icon projection has a stale camera or invalid source scale.", "ui.minimap.projection");
    }
    this.project(widget, frame.pixels, frame.marginX + (player.x - frame.baseX) * frame.scale + 2,
      frame.height - frame.marginY - (player.y - frame.baseY) * frame.scale - 2);
    if (placement) {
      if (placement.missingSprites) this.issues.add(`${placement.missingSprites} original map-icon sprites are unavailable.`);
      const source = new Set(frame.icons.filter(icon => icon.plane === player.plane)
        .map(icon => JSON.stringify([icon.element, icon.x, icon.y])));
      const seen = new Set<string>();
      for (const icon of placement.icons) {
        const identity = JSON.stringify([icon.element, icon.tileX, icon.tileY]);
        if (!source.has(identity) || seen.has(identity) || typeof icon.clipped !== "boolean" ||
            ![icon.x, icon.y, icon.drawX, icon.drawY, icon.dx, icon.dy].every(Number.isSafeInteger))
          throw new UiMinimapError("The renderer map-icon placement does not match the supplied surface.", "ui.minimap.identity");
        seen.add(identity);
        const image = this.iconSprites?.get(icon.element);
        if (!image) throw new UiMinimapError(`Original map-icon sprite ${icon.element} is unavailable.`, "ui.minimap.sprite");
        if (!(image.sprite.width && image.sprite.height)) continue;
        const x = widget.x + icon.drawX, y = widget.y + icon.drawY;
        const paint = () => this.raster.context.drawImage(image.canvas, x, y);
        if (!icon.clipped) paint();
        else {
          const mask = this.mask(widget.sprite);
          for (let row = Math.max(0, icon.drawY); row < Math.min(widget.height, icon.drawY + image.sprite.height); row++) {
            const span = mask[row];
            if (span) this.raster.clip({ x: widget.x + span[0], y: widget.y + row, width: span[1] - span[0], height: 1 }, paint);
          }
        }
        if (!this.suspended) this.retainedProjection = { frame, key: projectionKey, value: structuredClone(placement) };
      }
    }
    const piles = new Set<string>();
    for (const ground of world.groundItems) {
      const key = JSON.stringify(ground.tile);
      if (piles.has(key)) continue;
      piles.add(key); this.dot(widget, player, ground.tile, 0);
    }
    for (const entity of world.entities) {
      if (entity.tile.plane !== player.plane || entity.instance !== world.player.instance ||
          (entity.kind !== "npc" && entity.kind !== "player")) continue;
      if (entity.kind === "npc") {
        const definition = entity.sourceId === null ? null : this.raster.assets.catalogue.npcs[entity.sourceId];
        if (!definition) { this.issues.add(`Original NPC map flags are unavailable for ${entity.id}.`); continue; }
        if (!definition.mapVisible || !definition.interactable) continue;
      }
      this.dot(widget, player, entity.tile, entity.kind === "npc" ? 1 : 2);
    }
    this.raster.fill({ x: widget.x + (widget.width >> 1) - 1, y: widget.y + (widget.height >> 1) - 1, width: 3, height: 3 }, 0xffffff);
  }

  draw(widget: NativeWidget, player: Tile, world: WorldView | null = null): boolean {
    if (widget.contentType === 1339) {
      const compass = this.pixels("ui/minimaps/compass.png");
      if (compass) this.project(widget, compass, 25, 25);
      return true;
    }
    if (widget.contentType !== 1338) return false;
    if (!this.enabled) {
      this.mask(widget.sprite).forEach(([start, end], y) =>
        this.raster.fill({ x: widget.x + start, y: widget.y + y, width: end - start, height: 1 }, 0));
      return true;
    }
    if (world) { this.live(widget, player, world); return true; }
    // Original controlled source replays have no WorldView; production never uses these reference rasters.
    const candidates = this.raster.assets.catalogue.minimaps.filter(m => m.plane === player.plane &&
      player.x >= m.baseX && player.x < m.baseX + 104 && player.y >= m.baseY && player.y < m.baseY + 104);
    const selected = candidates.sort((a, b) =>
      Math.max(Math.abs(player.x - a.baseX - 52), Math.abs(player.y - a.baseY - 52)) -
      Math.max(Math.abs(player.x - b.baseX - 52), Math.abs(player.y - b.baseY - 52)))[0];
    if (!selected) {
      this.raster.textBox("Map data unavailable", widget, { font: 494, xAlign: 1, yAlign: 1, color: 0xffff00 });
      return true;
    }
    const image = this.pixels(selected.asset);
    if (image) {
      this.project(widget, image, (player.x - selected.baseX) * 4 + 50, 462 - (player.y - selected.baseY) * 4);
      this.raster.fill({ x: widget.x + (widget.width >> 1) - 1, y: widget.y + (widget.height >> 1) - 1, width: 3, height: 3 }, 0xffffff);
    }
    return true;
  }

  destination(widget: NativeWidget, player: Tile, x: number, y: number): Tile | null {
    const dx = x - widget.x, dy = y - widget.y, row = this.mask(widget.sprite)[Math.floor(dy)];
    if (!row || dx < row[0] || dx >= row[1]) return null;
    const mx = dx - (widget.width >> 1), my = dy - (widget.height >> 1);
    const angle = (this.rotation & 16383) / 16384 * Math.PI * 2;
    return { x: player.x + Math.round((my * Math.sin(angle) + mx * Math.cos(angle)) / 4),
      y: player.y - Math.round((my * Math.cos(angle) - mx * Math.sin(angle)) / 4), plane: player.plane };
  }
}
