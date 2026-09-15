import { requireAudio } from "./errors.ts";
import { SOURCE_CYCLE_SECONDS, SOURCE_RATE } from "./source.ts";

export const AUDIO_RENDER_QUANTUM_FRAMES = 128;

export function sourceSchedulingLookahead(baseLatency: number): number {
  requireAudio(Number.isFinite(baseLatency) && baseLatency >= 0, "AUDIO_DEVICE_CLOCK",
    "The audio device did not provide a valid scheduling latency.");
  return Math.min(SOURCE_CYCLE_SECONDS, Math.max(SOURCE_CYCLE_SECONDS / 2,
    baseLatency + AUDIO_RENDER_QUANTUM_FRAMES / SOURCE_RATE));
}
