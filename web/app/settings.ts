import { AppError, deepFreeze } from "./errors.ts";
import { canonicalJson, sha256 } from "./identity.ts";
import { sourceAudioDefaults } from "../audio/index.ts";

export const PREFERENCE_KEY = "clubscape.preferences.v2";
export const LEGACY_PREFERENCE_KEY = "clubscape.preferences.v1";
export type AudioChannel = "music" | "effects" | "area";
export interface Preferences {
  schemaVersion: 2;
  profile: "source-resizable-classic-v1";
  audioSemantics: "native-source-slider-v1";
  audio: Record<AudioChannel, number>;
}
const defaults = (): Preferences => {
  const source = sourceAudioDefaults().sliders;
  return {
    schemaVersion: 2, profile: "source-resizable-classic-v1", audioSemantics: "native-source-slider-v1",
    audio: { music: source.music / 100, effects: source.effects / 100, area: source.area / 100 },
  };
};
export interface PreferenceStore { getItem(key: string): string | null; setItem(key: string, value: string): void }

export function sourceSliderPosition(value: number): number {
  if (!Number.isFinite(value)) throw new AppError("A source audio slider requires a finite normalized position.", { kind: "input" });
  return Math.round(Math.min(1, Math.max(0, value)) * 100) / 100;
}

export class Settings {
  #value: Preferences;
  #store: PreferenceStore | null;
  #explicit = new Set<AudioChannel>();
  #legacyAudio = false;
  constructor(store: PreferenceStore | null) {
    this.#store = store;
    this.#value = defaults();
    try {
      const input = store?.getItem(PREFERENCE_KEY);
      this.#legacyAudio = !input && store?.getItem(LEGACY_PREFERENCE_KEY) != null;
      if (input && input.length <= 1024) {
        const value = JSON.parse(input) as Preferences;
        if (value.schemaVersion === 2 && value.profile === "source-resizable-classic-v1"
          && value.audioSemantics === "native-source-slider-v1") {
          for (const channel of ["music", "effects", "area"] as const) {
            const volume = value.audio?.[channel];
            if (typeof volume === "number" && Number.isFinite(volume) && volume >= 0 && volume <= 1) {
              this.#value.audio[channel] = Math.round(volume * 100) / 100;
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
    const bounded = sourceSliderPosition(value);
    this.#value.audio[channel] = bounded;
    this.#explicit.add(channel);
    try { this.#store?.setItem(PREFERENCE_KEY, JSON.stringify({ ...this.#value, audio: this.audioOverrides() })); } catch { /* Memory-only preferences still work. */ }
    return bounded;
  }

  audioOverrides(): Readonly<Partial<Record<AudioChannel, number>>> {
    return deepFreeze(Object.fromEntries([...this.#explicit].map((channel) => [channel, this.#value.audio[channel]])));
  }

  migrationNotice(): AppError | null {
    return this.#legacyAudio ? new AppError(
      "Old provisional linear audio preferences were not reinterpreted as native source sliders. Native defaults are active; set source slider preferences explicitly.",
      { kind: "audio_preferences" },
    ) : null;
  }

  async hash(visual: unknown): Promise<string> {
    return sha256(new TextEncoder().encode(canonicalJson({
      visual, preferences: { ...this.read(), audio: this.audioOverrides() },
    })));
  }
}
