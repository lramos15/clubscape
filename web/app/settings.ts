import { AppError, deepFreeze } from "./errors.ts";
import { canonicalJson, sha256 } from "./identity.ts";

export const PREFERENCE_KEY = "clubscape.preferences.v1";
export type AudioChannel = "music" | "effects" | "area";
export interface Preferences {
  schemaVersion: 1;
  profile: "source-resizable-classic-v1";
  audio: Record<AudioChannel, number>;
}
const defaults = (): Preferences => ({
  schemaVersion: 1, profile: "source-resizable-classic-v1",
  audio: { music: 1, effects: 1, area: 1 },
});
export interface PreferenceStore { getItem(key: string): string | null; setItem(key: string, value: string): void }

export class Settings {
  #value: Preferences;
  #store: PreferenceStore | null;
  #explicit = new Set<AudioChannel>();
  constructor(store: PreferenceStore | null) {
    this.#store = store;
    this.#value = defaults();
    try {
      const input = store?.getItem(PREFERENCE_KEY);
      if (input && input.length <= 1024) {
        const value = JSON.parse(input) as Preferences;
        if (value.schemaVersion === 1 && value.profile === "source-resizable-classic-v1") {
          for (const channel of ["music", "effects", "area"] as const) {
            const volume = value.audio?.[channel];
            if (typeof volume === "number" && Number.isFinite(volume) && volume >= 0 && volume <= 1) {
              this.#value.audio[channel] = volume;
              this.#explicit.add(channel);
            }
          }
        }
      }
    } catch { /* Blocked/quota-limited browser storage does not prevent memory-only play. */ }
  }

  read(): Readonly<Preferences> {
    return deepFreeze({ ...this.#value, audio: { ...this.#value.audio } });
  }

  volume(channel: AudioChannel, value: number): number {
    if (!["music", "effects", "area"].includes(channel)) throw new AppError("Unknown audio preference channel.", { kind: "input" });
    if (!Number.isFinite(value)) return this.#value.audio[channel];
    const bounded = Math.round(Math.min(1, Math.max(0, value)) * 1000) / 1000;
    this.#value.audio[channel] = bounded;
    this.#explicit.add(channel);
    try { this.#store?.setItem(PREFERENCE_KEY, JSON.stringify({ ...this.#value, audio: this.audioOverrides() })); } catch { /* Memory-only preferences still work. */ }
    return bounded;
  }

  audioOverrides(): Readonly<Partial<Record<AudioChannel, number>>> {
    return deepFreeze(Object.fromEntries([...this.#explicit].map((channel) => [channel, this.#value.audio[channel]])));
  }

  async hash(visual: unknown): Promise<string> {
    return sha256(new TextEncoder().encode(canonicalJson({
      visual, preferences: { ...this.read(), audio: this.audioOverrides() },
    })));
  }
}
