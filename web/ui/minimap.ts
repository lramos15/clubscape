import type { NativeWidget } from "./assets.ts";
import type { Tile, WorldView } from "../shared/contracts.ts";
import { SourceRaster } from "./raster.ts";

interface Pixels { width: number; height: number; data: Uint8ClampedArray }

export class MinimapPainter {
  private readonly decoded = new Map<string, Pixels>();
  private readonly masks = new Map<number, Array<[number, number]>>();
  rotation = 0;
  enabled = true;
  private readonly raster: SourceRaster;

  constructor(raster: SourceRaster) { this.raster = raster; }

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
      if (world) {
        const angle = (this.rotation & 16383) / 16384 * Math.PI * 2;
        for (const entity of world.entities) {
          if (!entity.available || entity.tile.plane !== player.plane || entity.instance !== world.player.instance ||
              (entity.kind !== "npc" && entity.kind !== "player")) continue;
          const dx = (entity.tile.x - player.x) * 4, dy = (player.y - entity.tile.y) * 4;
          if (dx * dx + dy * dy > 65 * 65) continue;
          const x = widget.x + (widget.width >> 1) + Math.round(dx * Math.cos(angle) - dy * Math.sin(angle));
          const y = widget.y + (widget.height >> 1) + Math.round(dx * Math.sin(angle) + dy * Math.cos(angle));
          this.raster.image(`ui/minimaps/dot-${entity.kind === "npc" ? 1 : 2}.png`, x - 2, y - 2);
        }
      }
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
