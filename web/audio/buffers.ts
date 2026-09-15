import type { ClientAssets } from "../shared/contracts.ts";
import { AudioFailure, failure, requireAudio } from "./errors.ts";
import { SOURCE_RATE, verifiedBytes } from "./source.ts";
import type { SourceAsset } from "./source.ts";

export interface BufferNotice {
  type: "fetch" | "decoded" | "evicted";
  assetId: string;
  bytes: number;
}

export class BufferCache {
  private readonly assets: ClientAssets;
  private readonly context: AudioContext;
  private readonly notice: (notice: BufferNotice) => void;
  private readonly buffers = new Map<string, AudioBuffer>();
  private readonly pending = new Map<string, { controller: AbortController; promise: Promise<AudioBuffer> }>();
  private readonly waiters: Array<() => void> = [];
  private active = 0;
  private closed = false;
  private bytes = 0;
  private readonly budget = 96 * 1024 * 1024;

  constructor(assets: ClientAssets, context: AudioContext, notice: (notice: BufferNotice) => void) {
    this.assets = assets;
    this.context = context;
    this.notice = notice;
  }

  get state(): Readonly<{ decodedBytes: number; cached: number; pending: number }> {
    return { decodedBytes: this.bytes, cached: this.buffers.size, pending: this.pending.size };
  }

  async load(asset: SourceAsset): Promise<AudioBuffer> {
    requireAudio(!this.closed, "AUDIO_DISPOSED", "Audio has been disposed.");
    requireAudio(asset.playable, "AUDIO_SOURCE_SILENCE", "The verified source silence has no file to load.");
    const cached = this.buffers.get(asset.id);
    if (cached) {
      this.buffers.delete(asset.id);
      this.buffers.set(asset.id, cached);
      return cached;
    }
    const pending = this.pending.get(asset.id);
    if (pending) return pending.promise;
    const controller = new AbortController();
    const promise = Promise.resolve().then(() => this.decode(asset, controller)).finally(() => {
      if (this.pending.get(asset.id)?.controller === controller) this.pending.delete(asset.id);
    });
    this.pending.set(asset.id, { controller, promise });
    return promise;
  }

  cancelUnused(wanted: ReadonlySet<string>): void {
    for (const [id, pending] of this.pending) {
      if (!wanted.has(id)) {
        pending.controller.abort(new AudioFailure("AUDIO_CANCELLED", "Superseded audio load."));
        this.pending.delete(id);
      }
    }
  }

  dispose(): void {
    this.closed = true;
    this.cancelUnused(new Set());
    this.buffers.clear();
    this.bytes = 0;
  }

  private async decode(asset: SourceAsset, controller: AbortController): Promise<AudioBuffer> {
    requireAudio(asset.playable, "AUDIO_SOURCE_SILENCE", "The verified source silence is not a decoder fallback.");
    if (this.active >= 4) await new Promise<void>((resolve) => this.waiters.push(resolve));
    this.active++;
    const timer = setTimeout(() => controller.abort(new AudioFailure(
      "AUDIO_LOAD_TIMEOUT", `Timed out loading ${asset.id}.`,
    )), 15_000);
    try {
      controller.signal.throwIfAborted();
      this.notice({ type: "fetch", assetId: asset.id, bytes: asset.bytes });
      const data = await verifiedBytes(this.assets, asset.id, asset.bytes, asset.sha256, controller.signal);
      controller.signal.throwIfAborted();
      let buffer: AudioBuffer;
      try {
        buffer = await this.context.decodeAudioData(data);
      } catch (error) {
        throw failure(error, "AUDIO_DECODE", `Cannot decode original audio ${asset.id}`);
      }
      controller.signal.throwIfAborted();
      requireAudio(buffer.sampleRate === SOURCE_RATE && buffer.length === asset.frames &&
        buffer.numberOfChannels === asset.channels,
      "AUDIO_DECODE_FORMAT", `Decoded source dimensions changed for ${asset.id}.`);
      let peak = 0;
      for (let channel = 0; channel < buffer.numberOfChannels; channel++) {
        for (const sample of buffer.getChannelData(channel)) {
          requireAudio(Number.isFinite(sample), "AUDIO_DECODE_SIGNAL", `Nonfinite PCM in ${asset.id}.`);
          peak = Math.max(peak, Math.abs(sample));
        }
      }
      requireAudio(peak > 0 && peak <= asset.peak + 0.000023,
        "AUDIO_DECODE_SIGNAL", `Decoded original signal exceeds its pinned float bound: ${asset.id}.`);
      const bytes = buffer.length * buffer.numberOfChannels * 4;
      while (this.bytes + bytes > this.budget && this.buffers.size) {
        const [id, old] = this.buffers.entries().next().value!;
        this.buffers.delete(id);
        const oldBytes = old.length * old.numberOfChannels * 4;
        this.bytes -= oldBytes;
        this.notice({ type: "evicted", assetId: id, bytes: oldBytes });
      }
      this.buffers.set(asset.id, buffer);
      this.bytes += bytes;
      this.notice({ type: "decoded", assetId: asset.id, bytes });
      return buffer;
    } catch (error) {
      if (controller.signal.aborted) throw controller.signal.reason;
      throw failure(error, "AUDIO_NETWORK", `Cannot load original audio ${asset.id}`);
    } finally {
      clearTimeout(timer);
      this.active--;
      this.waiters.shift()?.();
    }
  }
}

export function repeatedEffect(
  context: BaseAudioContext, original: AudioBuffer, asset: SourceAsset, repeats: number,
): AudioBuffer {
  const span = asset.loopEnd - asset.loopStart;
  // Native RawPcmStream disables repetition when its sample loop has no extent.
  if (repeats <= 1 || span <= 0) return original;
  const length = original.length + (repeats - 1) * span;
  requireAudio(length <= 16_777_216, "AUDIO_REPEAT_LIMIT", "The source repeat request exceeds the audio memory bound.");
  const output = context.createBuffer(original.numberOfChannels, length, original.sampleRate);
  for (let channel = 0; channel < original.numberOfChannels; channel++) {
    const input = original.getChannelData(channel);
    const samples = output.getChannelData(channel);
    samples.set(input.subarray(0, asset.loopEnd));
    for (let repeat = 1; repeat < repeats; repeat++) {
      samples.set(input.subarray(asset.loopStart, asset.loopEnd), asset.loopEnd + (repeat - 1) * span);
    }
    samples.set(input.subarray(asset.loopEnd), asset.loopEnd + (repeats - 1) * span);
  }
  return output;
}
