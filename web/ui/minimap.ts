import type { NativeWidget } from "./assets.ts";
import type { Tile, WorldView } from "../shared/contracts.ts";
import { SourceRaster } from "./raster.ts";
import { MinimapSurfaceStore } from "./minimap-surface.ts";
import type { UiMinimapSurface, UiMinimapStatus } from "./minimap-surface.ts";

interface Pixels { width: number; height: number; data: Uint8ClampedArray }

export class MinimapPainter {
  private readonly decoded = new Map<string, Pixels>();
  private readonly masks = new Map<number, Array<[number, number]>>();
  private readonly surfaces = new MinimapSurfaceStore();
  private readonly issues = new Set<string>();
  rotation = 0;
  enabled = true;
  private readonly raster: SourceRaster;

  constructor(raster: SourceRaster) { this.raster = raster; }

  supply(surface: UiMinimapSurface, scope: string): boolean { return this.surfaces.set(surface, scope); }
  retainScope(scope: string | null): void { this.surfaces.retainScope(scope); }
  clear(): void { this.surfaces.clear(); this.issues.clear(); }
  dispose(): void { this.clear(); this.decoded.clear(); this.masks.clear(); }
  status(): UiMinimapStatus | null {
    const frame = this.surfaces.current();
    const missing = frame?.icons.filter(icon => !this.raster.assets.catalogue.mapElements[icon.element]).map(icon => icon.element) ?? [];
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

  private live(widget: NativeWidget, player: Tile, world: WorldView): void {
    this.issues.clear();
    const frame = this.surfaces.current();
    if (!frame || frame.plane !== player.plane) {
      this.mask(widget.sprite).forEach(([start, end], y) =>
        this.raster.fill({ x: widget.x + start, y: widget.y + y, width: end - start, height: 1 }, 0));
      this.raster.textBox("Map data unavailable", widget, { font: 494, xAlign: 1, yAlign: 1, color: 0xffff00 });
      return;
    }
    this.project(widget, frame.pixels, frame.marginX + (player.x - frame.baseX) * frame.scale + 2,
      frame.height - frame.marginY - (player.y - frame.baseY) * frame.scale - 2);
    for (const icon of frame.icons) {
      if (icon.plane !== player.plane) continue;
      const element = this.raster.assets.catalogue.mapElements[icon.element];
      if (!element || element.sprite < 0) continue;
      const sprite = this.raster.assets.catalogue.sprites[element.sprite];
      const shape = sprite?.frames[0];
      if (!shape) { this.issues.add(`Original map-element sprite ${element.sprite} is unavailable.`); continue; }
      this.point(widget, (icon.x - player.x) * frame.scale, (icon.y - player.y) * frame.scale,
        shape.canvasWidth, shape.canvasHeight, (x, y) => {
          this.raster.sprite(element.sprite, x, y);
        });
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
