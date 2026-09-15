import type { FontAsset, NativeWidget, Rect } from "./assets.ts";
import { UiAssets } from "./assets.ts";

const CP1252: Record<string, number> = {
  "€": 128, "‚": 130, "ƒ": 131, "„": 132, "…": 133, "†": 134, "‡": 135,
  "ˆ": 136, "‰": 137, "Š": 138, "‹": 139, "Œ": 140, "Ž": 142, "‘": 145,
  "’": 146, "“": 147, "”": 148, "•": 149, "–": 150, "—": 151, "˜": 152,
  "™": 153, "š": 154, "›": 155, "œ": 156, "ž": 158, "Ÿ": 159,
};

export function sourceByte(character: string): number {
  const value = character.codePointAt(0) ?? 0;
  if (value === 160) return 32;
  return CP1252[character] ?? ((value > 0 && value < 128 || value >= 160 && value < 256) ? value : 63);
}

export function plainText(text: string): string {
  return text.replace(/<br>/gi, "\n").replace(/<lt>/gi, "\x01").replace(/<gt>/gi, "\x02")
    .replace(/<[^>]*>/g, "").replace(/\x01/g, "<").replace(/\x02/g, ">");
}

export function escapeText(text: string): string {
  return text.replace(/</g, "\x01").replace(/>/g, "<gt>").replace(/\x01/g, "<lt>");
}

export function textAdvance(text: string, font: FontAsset): number {
  let width = 0;
  for (const character of plainText(text)) width += font.advances[sourceByte(character)] ?? 0;
  return width;
}

interface Glyph { char: string; color: number; shadow: number | null; strike: number | null; underline: number | null }

function glyphs(text: string, color: number, shadow: number | null): Glyph[] {
  const output: Glyph[] = [];
  let ink = color, shade = shadow, strike: number | null = null, underline: number | null = null;
  for (const token of text.match(/<[^>]*>|[^<]+|</g) ?? []) {
    if (token.startsWith("<") && token.endsWith(">")) {
      if (token === "<lt>" || token === "<gt>" || token === "<br>") {
        output.push({ char: token === "<lt>" ? "<" : token === "<gt>" ? ">" : "\n", color: ink, shadow: shade, strike, underline });
      } else if (/^<col=[0-9a-f]{1,6}>$/i.test(token)) ink = Number.parseInt(token.slice(5, -1), 16);
      else if (token === "</col>") ink = color;
      else if (token === "<str>") strike = 0x800000;
      else if (/^<str=[0-9a-f]{1,6}>$/i.test(token)) strike = Number.parseInt(token.slice(5, -1), 16);
      else if (token === "</str>") strike = null;
      else if (token === "<u>") underline = 0;
      else if (/^<u=[0-9a-f]{1,6}>$/i.test(token)) underline = Number.parseInt(token.slice(3, -1), 16);
      else if (token === "</u>") underline = null;
      else if (token === "<shad>") shade = 0;
      else if (/^<shad=[0-9a-f]{1,6}>$/i.test(token)) shade = Number.parseInt(token.slice(6, -1), 16);
      else if (token === "</shad>") shade = shadow;
    } else {
      for (const char of token) output.push({ char, color: ink, shadow: shade, strike, underline });
    }
  }
  return output;
}

export function sourceLines(text: string, width: number, font: FontAsset): string[] {
  // Wrap only at native break opportunities. A long indivisible token is clipped, not resized.
  const lines: string[] = [];
  let line = "", advance = 0, breakAt = -1, breakWidth = 0, trimBreak = false;
  for (const token of text.match(/<[^>]*>|[^<]/g) ?? []) {
    if (token === "<br>" || token === "\n") {
      lines.push(line); line = ""; advance = 0; breakAt = -1;
      continue;
    }
    line += token;
    const character = token === "<lt>" ? "<" : token === "<gt>" ? ">" : token.length === 1 ? token : "";
    if (character) {
      advance += font.advances[sourceByte(character)] ?? 0;
      if (character === " " || character === "-") {
        breakAt = line.length; breakWidth = advance; trimBreak = character === " ";
      }
      if (advance > width && breakAt >= 0) {
        lines.push(line.slice(0, breakAt - (trimBreak ? 1 : 0)));
        line = line.slice(breakAt); advance -= breakWidth; breakAt = -1;
      }
    }
  }
  if (line || !lines.length) lines.push(line);
  return lines;
}

export function countText(quantity: number): { text: string; color: number } {
  if (quantity < 100_000) return { text: String(quantity), color: 0xffff00 };
  if (quantity < 10_000_000) return { text: `${Math.floor(quantity / 1000)}K`, color: 0xffffff };
  return { text: `${Math.floor(quantity / 1_000_000)}M`, color: 0x00ff80 };
}

function css(color: number): string { return `#${(color & 0xffffff).toString(16).padStart(6, "0")}`; }

export class SourceRaster {
  readonly context: CanvasRenderingContext2D;
  readonly canvas: HTMLCanvasElement;
  readonly assets: UiAssets;
  private readonly coloredFonts = new Map<string, HTMLCanvasElement>();
  private readonly scaledSprites = new Map<string, HTMLCanvasElement>();

  constructor(canvas: HTMLCanvasElement, assets: UiAssets) {
    this.canvas = canvas; this.assets = assets;
    const context = canvas.getContext("2d", { alpha: true, willReadFrequently: true });
    if (!context) throw new Error("Canvas2D is required for the original-source interface overlay.");
    this.context = context;
    context.imageSmoothingEnabled = false;
  }

  clear(): void { this.context.clearRect(0, 0, this.canvas.width, this.canvas.height); }
  fill(rect: Rect, color: number): void {
    this.context.fillStyle = css(color);
    this.context.fillRect(Math.floor(rect.x), Math.floor(rect.y), Math.floor(rect.width), Math.floor(rect.height));
  }
  border(rect: Rect, color: number): void {
    this.fill({ ...rect, height: 1 }, color);
    this.fill({ ...rect, y: rect.y + rect.height - 1, height: 1 }, color);
    this.fill({ ...rect, width: 1 }, color);
    this.fill({ ...rect, x: rect.x + rect.width - 1, width: 1 }, color);
  }
  clip(rect: Rect, paint: () => void): void {
    if (rect.width <= 0 || rect.height <= 0) return;
    this.context.save();
    this.context.beginPath(); this.context.rect(rect.x, rect.y, rect.width, rect.height); this.context.clip();
    paint(); this.context.restore();
  }

  image(id: string, x: number, y: number, opacity = 0): boolean {
    const image = this.assets.image(id);
    if (!image) return false;
    this.context.save(); this.context.globalAlpha = (256 - opacity) / 256;
    this.context.drawImage(image, Math.floor(x), Math.floor(y)); this.context.restore();
    return true;
  }

  sprite(id: number, x: number, y: number, options: {
    frame?: number; width?: number; height?: number; opacity?: number; flipX?: boolean; flipY?: boolean; tiling?: boolean;
  } = {}): void {
    const sprite = this.assets.catalogue.sprites[id];
    const frame = sprite?.frames[options.frame ?? 0];
    if (!sprite || !frame || !frame.width || !frame.height) return;
    const image = this.assets.image(sprite.asset);
    if (!image) return;
    const width = options.width ?? frame.canvasWidth, height = options.height ?? frame.canvasHeight;
    if (width <= 0 || height <= 0) return;
    const opacity = options.opacity ?? 0;
    const before = opacity ? this.context.getImageData(Math.floor(x), Math.floor(y), width, height) : null;
    const draw = (left: number, top: number, w: number, h: number) => {
      if (w === frame.canvasWidth && h === frame.canvasHeight) {
        this.context.drawImage(image, frame.x, frame.y, frame.width, frame.height,
          left + frame.offsetX, top + frame.offsetY, frame.width, frame.height);
        return;
      }
      const key = `${id}:${options.frame ?? 0}:${w}:${h}`;
      let scaled = this.scaledSprites.get(key);
      if (!scaled) {
        const input = document.createElement("canvas"); input.width = frame.width; input.height = frame.height;
        const sourceContext = input.getContext("2d", { willReadFrequently: true })!;
        sourceContext.drawImage(image, frame.x, frame.y, frame.width, frame.height, 0, 0, frame.width, frame.height);
        const source = sourceContext.getImageData(0, 0, frame.width, frame.height).data;
        scaled = document.createElement("canvas"); scaled.width = w; scaled.height = h;
        const target = scaled.getContext("2d")!, pixels = target.createImageData(w, h);
        const stepX = Math.floor(frame.canvasWidth * 65536 / w), stepY = Math.floor(frame.canvasHeight * 65536 / h);
        for (let dy = 0; dy < h; dy++) for (let dx = 0; dx < w; dx++) {
          const sx = (dx * stepX >> 16) - frame.offsetX, sy = (dy * stepY >> 16) - frame.offsetY;
          if (sx < 0 || sy < 0 || sx >= frame.width || sy >= frame.height) continue;
          const from = (sy * frame.width + sx) * 4, to = (dy * w + dx) * 4;
          pixels.data.set(source.subarray(from, from + 4), to);
        }
        target.putImageData(pixels, 0, 0); this.scaledSprites.set(key, scaled);
      }
      this.context.drawImage(scaled, left, top);
    };
    this.context.save();
    this.context.imageSmoothingEnabled = false;
    this.context.globalAlpha = 1;
    this.context.translate(Math.floor(x) + (options.flipX ? width : 0), Math.floor(y) + (options.flipY ? height : 0));
    this.context.scale(options.flipX ? -1 : 1, options.flipY ? -1 : 1);
    if (options.tiling) {
      this.clip({ x: 0, y: 0, width, height }, () => {
        for (let top = 0; top < height; top += frame.canvasHeight)
          for (let left = 0; left < width; left += frame.canvasWidth) draw(left, top, frame.canvasWidth, frame.canvasHeight);
      });
    } else draw(0, 0, width, height);
    this.context.restore();
    if (before) {
      const after = this.context.getImageData(Math.floor(x), Math.floor(y), width, height);
      const alpha = 256 - opacity;
      for (let i = 0; i < after.data.length; i += 4) {
        if (before.data[i + 3] === 255) {
          for (let channel = 0; channel < 3; channel++)
            after.data[i + channel] = (after.data[i + channel]! * alpha + before.data[i + channel]! * opacity) >> 8;
        } else if (before.data[i + 3] === 0 && after.data[i + 3]) {
          after.data[i + 3] = Math.floor(255 * alpha / 256);
        }
      }
      this.context.putImageData(after, Math.floor(x), Math.floor(y));
    }
  }

  private tintedFont(id: number, color: number): HTMLCanvasElement {
    const key = `${id}:${color}`;
    const existing = this.coloredFonts.get(key);
    if (existing) return existing;
    const sprite = this.assets.catalogue.sprites[id];
    if (!sprite) throw new Error(`Native glyph atlas ${id} is unavailable.`);
    const image = this.assets.images.get(sprite.asset);
    if (!image) throw new Error(`Native glyph atlas ${id} is not loaded.`);
    const canvas = document.createElement("canvas");
    canvas.width = image.naturalWidth; canvas.height = image.naturalHeight;
    const context = canvas.getContext("2d", { willReadFrequently: true })!;
    context.drawImage(image, 0, 0);
    const pixels = context.getImageData(0, 0, canvas.width, canvas.height);
    for (let p = 0; p < pixels.data.length; p += 4) {
      const ink = pixels.data[p + 3] ? 255 : 0;
      pixels.data[p] = color >> 16 & 255; pixels.data[p + 1] = color >> 8 & 255;
      pixels.data[p + 2] = color & 255; pixels.data[p + 3] = ink;
    }
    context.putImageData(pixels, 0, 0);
    this.coloredFonts.set(key, canvas);
    return canvas;
  }

  measure(text: string, font = 495): number {
    const metrics = this.assets.catalogue.fonts[font];
    if (!metrics) throw new Error(`Original font metrics ${font} are unavailable.`);
    return textAdvance(text, metrics);
  }

  text(text: string, x: number, baseline: number, font = 495, color = 0xffffff, shadow: number | null = 0): void {
    const metrics = this.assets.catalogue.fonts[font];
    const atlas = this.assets.catalogue.sprites[font];
    if (!metrics || !atlas) throw new Error(`Original font ${font} is unavailable.`);
    let left = Math.floor(x);
    for (const run of glyphs(text, color, shadow)) {
      if (run.char === "\n") continue;
      const byte = sourceByte(run.char);
      const frame = atlas.frames[byte], advance = metrics.advances[byte] ?? 0;
      if (frame?.width && frame.height) {
        const dx = left + frame.offsetX, dy = Math.floor(baseline) - metrics.ascent + frame.offsetY;
        if (run.shadow !== null) this.context.drawImage(this.tintedFont(font, run.shadow),
          frame.x, frame.y, frame.width, frame.height, dx + 1, dy + 1, frame.width, frame.height);
        this.context.drawImage(this.tintedFont(font, run.color),
          frame.x, frame.y, frame.width, frame.height, dx, dy, frame.width, frame.height);
      }
      if (run.strike !== null) this.fill({ x: left, y: baseline - metrics.ascent + Math.floor(metrics.ascent * 0.7), width: advance, height: 1 }, run.strike);
      if (run.underline !== null) this.fill({ x: left, y: baseline + 1, width: advance, height: 1 }, run.underline);
      left += advance;
    }
  }

  center(text: string, center: number, baseline: number, font = 495, color = 0xffffff, shadow: number | null = 0): void {
    this.text(text, center - Math.floor(this.measure(text, font) / 2), baseline, font, color, shadow);
  }

  textBox(text: string, rect: Rect, options: {
    font?: number; color?: number; shadow?: number | null; lineHeight?: number; xAlign?: number; yAlign?: number;
    clip?: boolean;
  } = {}): void {
    const font = options.font ?? 495, metrics = this.assets.catalogue.fonts[font];
    if (!metrics) throw new Error(`Original font ${font} is missing.`);
    const lineHeight = options.lineHeight || metrics.ascent;
    const frames = this.assets.catalogue.sprites[font]!.frames;
    let minY = Infinity, maxY = -Infinity;
    for (const frame of frames) if (frame.height) {
      minY = Math.min(minY, frame.offsetY); maxY = Math.max(maxY, frame.offsetY + frame.height);
    }
    const maxAscent = metrics.ascent - minY, maxDescent = maxY - metrics.ascent;
    const noWrap = rect.height < maxAscent + maxDescent + lineHeight && rect.height < lineHeight * 2;
    const lines = sourceLines(text, noWrap ? Number.MAX_SAFE_INTEGER : rect.width, metrics);
    let y = rect.y + maxAscent;
    if (options.yAlign === 1) y = rect.y + maxAscent + Math.floor((rect.height - maxAscent - maxDescent - (lines.length - 1) * lineHeight) / 2);
    if (options.yAlign === 2) y = rect.y + rect.height - maxDescent - (lines.length - 1) * lineHeight;
    let spacing = lineHeight;
    if (options.yAlign === 3) {
      const extra = Math.max(0, Math.floor((rect.height - maxAscent - maxDescent - (lines.length - 1) * lineHeight) / (lines.length + 1)));
      y += extra; spacing += extra;
    }
    const draw = () => {
      // Tags carry across line wrapping, as in the source font painter.
      let carry = "";
      for (const line of lines) {
        const width = this.measure(line, font);
        const x = rect.x + (options.xAlign === 1 ? Math.floor((rect.width - width) / 2) : options.xAlign === 2 ? rect.width - width : 0);
        this.text(carry + line, x, y, font, options.color ?? 0xffffff, options.shadow === undefined ? 0 : options.shadow);
        carry += (line.match(/<(?:\/?(?:col|str|u|shad)(?:=[^>]+)?)>/g) ?? []).join("");
        y += spacing;
      }
    };
    if (options.clip === false) draw(); else this.clip(rect, draw);
  }

  item(sourceId: number, quantity: number, x: number, y: number, quantityMode = 2, selected = false, opacity = 0): boolean {
    const path = this.assets.itemAsset(sourceId, quantity, selected);
    if (!path || !this.image(path, x, y, opacity)) return false;
    if (quantityMode === 1 || (quantityMode === 2 && (this.assets.catalogue.items[sourceId]?.stackable === 1 || quantity !== 1))) {
      const count = countText(quantity);
      // Native item sprites reserve RGB zero for transparency; their black glyph shadow is RGB 1.
      this.text(count.text, x, y + 9, 494, count.color, 1);
    }
    return true;
  }

  widget(widget: NativeWidget): void {
    if (widget.type === 3) {
      const before = widget.opacity ? this.context.getImageData(widget.x, widget.y, widget.width, widget.height) : null;
      if (widget.filled) this.fill(widget, widget.color); else this.border(widget, widget.color);
      if (before) {
        const after = this.context.getImageData(widget.x, widget.y, widget.width, widget.height), alpha = 256 - widget.opacity;
        for (let i = 0; i < after.data.length; i += 4) {
          if (before.data[i + 3] === 255) for (let channel = 0; channel < 3; channel++) {
            after.data[i + channel] = (after.data[i + channel]! * alpha + before.data[i + channel]! * widget.opacity) >> 8;
          }
        }
        this.context.putImageData(after, widget.x, widget.y);
      }
    } else if (widget.type === 4 && widget.text && widget.font >= 0) {
      this.textBox(widget.text, widget, { font: widget.font, color: widget.color, shadow: widget.shadow ? 0 : null,
        lineHeight: widget.lineHeight, xAlign: widget.xText, yAlign: widget.yText, clip: false });
    } else if (widget.type === 5) {
      if (widget.item >= 0) this.item(widget.item, widget.item_quantity, widget.x, widget.y, widget.quantityMode, widget.border === 2, widget.opacity);
      else if (widget.sprite >= 0) this.sprite(widget.sprite, widget.x, widget.y, {
        width: widget.width, height: widget.height, tiling: widget.tiling, opacity: widget.opacity, flipX: widget.flipX, flipY: widget.flipY,
      });
    } else if (widget.type === 6 && widget.modelType === 2) {
      const portrait = this.assets.catalogue.portraits[`npc-${widget.model}`];
      if (portrait) this.image(portrait.asset, widget.x + portrait.offsetX, widget.y + portrait.offsetY);
    } else if (widget.type === 9) {
      let x = widget.x, y = widget.lineDirection ? widget.y + widget.height : widget.y;
      const endX = widget.x + widget.width, endY = widget.lineDirection ? widget.y : widget.y + widget.height;
      const dx = Math.abs(endX - x), dy = -Math.abs(endY - y);
      const sx = x < endX ? 1 : -1, sy = y < endY ? 1 : -1;
      let error = dx + dy;
      for (;;) {
        this.fill({ x, y, width: widget.lineWidth || 1, height: widget.lineWidth || 1 }, widget.color);
        if (x === endX && y === endY) break;
        const twice = error * 2;
        if (twice >= dy) { error += dy; x += sx; }
        if (twice <= dx) { error += dx; y += sy; }
      }
    }
  }
}
