import {
  parseSourceAudioPreferences, sourceAudioPreferenceDefaults, sourceEditPlaylist,
  sourceSelectPlaylist, sourceToggleVolumeMute,
} from "../../audio/index.ts";
import type { SourceAudioPreferenceBinding, SourceAudioPreferences, SourceAudioScene, SourceMusicSkipResult } from "../../audio/index.ts";
import type { AudioEvent, WorldView } from "../../shared/contracts.ts";
import { deepFreeze } from "../errors.ts";
import type { NativeAudioPreferences, PlayerAudioRuntime } from "../player-audio.ts";

// Controlled composition fixtures only. They do not execute protocol or grant game progress.
export function audioFixtureWorld(playerId = "actor.audio_fixture"): WorldView {
  return deepFreeze({
    revision: "9007199254740993", tick: "9007199254740994",
    player: {
      id: playerId, displayName: "Audio contract fixture", appearance: {}, region: "region.audio_fixture",
      tile: { x: 3094, y: 3107, plane: 0 }, instance: null, inventory: [], equipment: [], skills: [],
      hitpoints: 10, prayerPoints: 1, runEnergy: 100, questPoints: 0,
      tutorialStage: "stage.fixture", tutorialInstruction: "", quests: [], unlockedInterfaces: [],
      activePrayers: [], activity: "idle", animation: "", settings: [],
    },
    entities: [], groundItems: [], dialogue: null, bank: null, shop: null, recovery: null, messages: [],
  });
}

export function audioFixtureRecord(percent: number): SourceAudioPreferences {
  const base = sourceAudioPreferenceDefaults();
  const savedPlaylist1 = [...base.music.savedPlaylist1];
  savedPlaylist1[1] = 62; savedPlaylist1[99] = 76;
  return parseSourceAudioPreferences({
    ...base, volumes: {
      current: { master: 37, music: percent, effects: 66, area: 83 },
      remembered: { master: 37, music: 21, effects: 66, area: 83 },
    },
    music: { ...base.music, savedPlaylist1, rememberModeOnLogin: true },
  });
}

export class FixturePreferenceRuntime implements PlayerAudioRuntime {
  calls: Array<{ kind: string; args: readonly unknown[] }> = [];
  binding: SourceAudioPreferenceBinding | null = null;
  applying: ((record: SourceAudioPreferences) => SourceAudioPreferences) | null = null;
  updating: (() => void) | null = null;
  skipResult: Promise<SourceMusicSkipResult> = Promise.resolve({ status: "disabled_mode", previousGroup: null, nextGroup: null });
  observe: (binding: SourceAudioPreferenceBinding | null) => void = () => {};

  update(world: WorldView | null, events: readonly AudioEvent[], scene?: SourceAudioScene | null): void {
    this.calls.push({ kind: "world", args: [world, events, scene] });
    this.updating?.();
    if (world === null) this.binding = null;
  }
  disconnected(): void { this.calls.push({ kind: "disconnect", args: [] }); }

  #replace(record: SourceAudioPreferences): SourceAudioPreferenceBinding {
    if (!this.binding) throw new Error("Fixture must be explicitly bound.");
    this.binding = deepFreeze({ ...this.binding, preferences: parseSourceAudioPreferences(record) });
    this.observe(this.binding);
    return this.binding;
  }
  preferences: NativeAudioPreferences = {
    read: () => this.binding,
    apply: (playerId, record, unlocked) => {
      this.calls.push({ kind: "apply", args: [playerId, record, unlocked] });
      const preferences = this.applying?.(record) ?? record;
      this.binding = deepFreeze({
        playerId, preferences, unlockedGroups: [...unlocked],
        musicState: { mode: "area", areaMode: "modern", unlockedGroups: [...unlocked],
          selectedGroup: null, playlistGroups: [], loopEnabled: false },
      });
      this.observe(this.binding);
      return this.binding;
    },
    music: (player, value) => {
      this.calls.push({ kind: "music", args: [player, value] });
      return this.#replace({ ...this.binding!.preferences, music: value });
    },
    playlist: (player, slot) => {
      this.calls.push({ kind: "playlist", args: [player, slot] });
      return this.#replace({ ...this.binding!.preferences,
        music: sourceSelectPlaylist(this.binding!.preferences.music, slot, null) });
    },
    replace: (player, slot, entries) => {
      this.calls.push({ kind: "replace", args: [player, slot, entries] });
      const key = slot === 1 ? "savedPlaylist1" : slot === 2 ? "savedPlaylist2" : "savedPlaylist3";
      return this.#replace({ ...this.binding!.preferences, music: { ...this.binding!.preferences.music, [key]: entries } });
    },
    edit: (player, slot, edit) => {
      this.calls.push({ kind: "edit", args: [player, slot, edit] });
      const key = slot === 1 ? "savedPlaylist1" : slot === 2 ? "savedPlaylist2" : "savedPlaylist3";
      const music = this.binding!.preferences.music;
      return this.#replace({ ...this.binding!.preferences, music: { ...music, [key]: sourceEditPlaylist(music[key], edit) } });
    },
    toggle: (player, channel) => {
      this.calls.push({ kind: "toggle", args: [player, channel] });
      return this.#replace({ ...this.binding!.preferences, volumes: sourceToggleVolumeMute(this.binding!.preferences.volumes, channel) });
    },
    percent: (player, channel, value) => {
      this.calls.push({ kind: "percent", args: [player, channel, value] });
      const record = this.binding!.preferences;
      return this.#replace({ ...record, volumes: { ...record.volumes, current: { ...record.volumes.current, [channel]: value } } });
    },
    skip: (player) => {
      this.calls.push({ kind: "skip", args: [player] });
      return this.skipResult;
    },
  };
}
