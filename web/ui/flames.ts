import type { SourceRaster } from "./raster.ts";

export interface FlameAssets {
  palettes: number[][];
  runes: Array<{ width: number; height: number; offsetX: number; offsetY: number; mask: number[] }>;
}

/** The original Math.random visual fixture uses java.util.Random's 48-bit sequence. */
class JavaVisualRandom {
  private state: bigint;
  constructor(seed: number) { this.state = (BigInt(seed) ^ 0x5deece66dn) & ((1n << 48n) - 1n); }
  private bits(count: number): number {
    this.state = (this.state * 0x5deece66dn + 11n) & ((1n << 48n) - 1n);
    return Number(this.state >> BigInt(48 - count));
  }
  next(): number { return (this.bits(26) * 134217728 + this.bits(27)) / 9007199254740992; }
}

export class TitleFlames {
  private noise = new Int32Array(32768);
  private noiseScratch = new Int32Array(32768);
  private intensity = new Int32Array(32768);
  private blur = new Int32Array(32768);
  private offsets = new Int32Array(256);
  private noiseOffset = 0;
  private phase = 0;
  private sparks = 0;
  private fadeGreen = 0;
  private fadeBlue = 0;
  private lastCycle = 0;
  private readonly random: JavaVisualRandom;
  private readonly source: FlameAssets;

  constructor(source: FlameAssets, seed: number) {
    if (source.palettes.length !== 3 || source.palettes.some(palette => palette.length !== 256) || source.runes.length < 12)
      throw new Error("Original title-effect palettes or rune masks are missing.");
    this.source = source; this.random = new JavaVisualRandom(seed);
    this.seedNoise(null);
  }

  private seedNoise(rune: FlameAssets["runes"][number] | null): void {
    this.noise.fill(0);
    for (let i = 0; i < 5000; i++) this.noise[Math.floor(this.random.next() * 32768)] = Math.floor(this.random.next() * 256);
    for (let pass = 0; pass < 20; pass++) {
      for (let y = 1; y < 255; y++) for (let x = 1; x < 127; x++) {
        const i = x + y * 128;
        this.noiseScratch[i] = (this.noise[i - 1]! + this.noise[i + 1]! + this.noise[i - 128]! + this.noise[i + 128]!) >> 2;
      }
      [this.noise, this.noiseScratch] = [this.noiseScratch, this.noise];
    }
    if (rune) for (let y = 0; y < rune.height; y++) for (let x = 0; x < rune.width; x++) {
      if (rune.mask[x + y * rune.width]) this.noise[x + 16 + rune.offsetX + (y + 16 + rune.offsetY) * 128] = 0;
    }
  }

  advance(cycle: number): void {
    cycle = Math.max(0, Math.floor(cycle));
    if (this.lastCycle === 0) this.lastCycle = cycle;
    let delta = cycle - this.lastCycle;
    if (delta >= 256 || delta < 0) delta = 0;
    this.lastCycle = cycle;
    if (!delta) return;
    this.noiseOffset += delta * 128;
    if (this.noiseOffset > this.noise.length) {
      this.noiseOffset -= this.noise.length;
      this.seedNoise(this.source.runes[Math.floor(this.random.next() * 12)]!);
    }
    for (let i = 0; i < (256 - delta) * 128; i++)
      this.intensity[i] = Math.max(0, this.intensity[i + delta * 128]! -
        Math.trunc(delta * this.noise[(i + this.noiseOffset) & 32767]! / 6));
    for (let y = 256 - delta; y < 256; y++) for (let x = 0; x < 128; x++) {
      const random = Math.floor(this.random.next() * 100);
      this.intensity[y * 128 + x] = random < 50 && x > 10 && x < 118 ? 255 : 0;
    }
    if (this.fadeGreen > 0) this.fadeGreen -= delta * 4;
    if (this.fadeBlue > 0) this.fadeBlue -= delta * 4;
    if (this.fadeGreen === 0 && this.fadeBlue === 0) {
      const switchPalette = Math.floor(this.random.next() * Math.trunc(2000 / delta));
      if (switchPalette === 0) this.fadeGreen = 1024;
      if (switchPalette === 1) this.fadeBlue = 1024;
    }
    this.offsets.copyWithin(0, delta);
    for (let y = 256 - delta; y < 256; y++)
      this.offsets[y] = Math.trunc(Math.sin(this.phase / 14) * 16 + Math.sin(this.phase / 15) * 14 + Math.sin(this.phase++ / 16) * 12);
    this.sparks += delta;
    const radius = Math.trunc(((cycle & 1) + delta) / 2);
    if (!radius) return;
    for (let i = 0; i < this.sparks * 100; i++) {
      const x = Math.floor(this.random.next() * 124) + 2, y = Math.floor(this.random.next() * 128) + 128;
      this.intensity[x + y * 128] = 192;
    }
    this.sparks = 0;
    const divisor = radius * 2 + 1;
    for (let y = 0; y < 256; y++) {
      let sum = 0;
      for (let x = -radius; x < 128; x++) {
        if (x + radius < 128) sum += this.intensity[y * 128 + x + radius]!;
        if (x - radius - 1 >= 0) sum -= this.intensity[y * 128 + x - radius - 1]!;
        if (x >= 0) this.blur[y * 128 + x] = Math.trunc(sum / divisor);
      }
    }
    for (let x = 0; x < 128; x++) {
      let sum = 0;
      for (let y = -radius; y < 256; y++) {
        if (y + radius < 256) sum += this.blur[x + (y + radius) * 128]!;
        if (y - radius - 1 >= 0) sum -= this.blur[x + (y - radius - 1) * 128]!;
        if (y >= 0) this.intensity[x + y * 128] = Math.trunc(sum / divisor);
      }
    }
  }

  private mix(first: number, second: number, alpha: number): number {
    const inverse = 256 - alpha;
    return (((first & 0xff00ff) * inverse + (second & 0xff00ff) * alpha & 0xff00ff00) +
      ((first & 0xff00) * inverse + (second & 0xff00) * alpha & 0xff0000)) >>> 8;
  }

  paint(raster: SourceRaster, padding: number): void {
    const base = this.source.palettes[0]!, alternate = this.source.palettes[this.fadeGreen > 0 ? 1 : 2]!;
    const fade = this.fadeGreen > 0 ? this.fadeGreen : this.fadeBlue;
    const palette = fade > 0 ? base.map((color, index) => fade > 768 ? this.mix(color, alternate[index]!, 1024 - fade)
      : fade > 256 ? alternate[index]! : this.mix(alternate[index]!, color, 256 - fade)) : base;
    const width = raster.canvas.width, height = Math.min(raster.canvas.height, 264);
    const pixels = raster.context.getImageData(0, 0, width, height);
    for (const origin of [padding - 22, padding + 659]) {
      for (let row = 1; row < 255; row++) {
        const y = row + 8, offset = Math.trunc(this.offsets[row]! * (256 - row) / 256);
        if (y >= height) continue;
        for (let column = 0; column < 128; column++) {
          const x = origin + offset + column;
          if (x < Math.max(0, padding) || x >= Math.min(width, padding + 765)) continue;
          const alpha = this.intensity[(row - 1) * 128 + column]!;
          if (!alpha) continue;
          const color = palette[alpha]!, p = (x + y * width) * 4, inverse = 256 - alpha;
          pixels.data[p] = ((color >> 16 & 255) * alpha + pixels.data[p]! * inverse) >> 8;
          pixels.data[p + 1] = ((color >> 8 & 255) * alpha + pixels.data[p + 1]! * inverse) >> 8;
          pixels.data[p + 2] = ((color & 255) * alpha + pixels.data[p + 2]! * inverse) >> 8;
          pixels.data[p + 3] = 255;
        }
      }
    }
    raster.context.putImageData(pixels, 0, 0);
  }

  dispose(): void { this.intensity.fill(0); this.blur.fill(0); this.noise.fill(0); this.noiseScratch.fill(0); this.offsets.fill(0); }
}
