import { AudioFailure, integer, requireAudio } from "./errors.ts";
import { sourceAudioDefaults } from "./native-policy.ts";
import type { SourceAudioChannel } from "./native-policy.ts";
import type { SourceMusicMode, SourceMusicState } from "./native-scene.ts";

export type SourceVolumeChannel = SourceAudioChannel | "master";
export type SourcePercentages = Readonly<Record<SourceVolumeChannel, number>>;
export interface SourceVolumePreferences {
  readonly current: SourcePercentages;
  /** Zero is the original uninitialized memory, not a remembered audible percentage. */
  readonly remembered: SourcePercentages;
}
export type SourcePlaylistSlot = 1 | 2 | 3;
export type SourcePlaylistSelection = 0 | SourcePlaylistSlot;
export type SourceSavedPlaylist = readonly (number | null)[];
export interface SourceSavedPlaylists {
  readonly savedPlaylist1: SourceSavedPlaylist;
  readonly savedPlaylist2: SourceSavedPlaylist;
  readonly savedPlaylist3: SourceSavedPlaylist;
}
export interface SourceMusicPreferences extends SourceSavedPlaylists {
  readonly mode: SourceMusicMode;
  readonly areaMode: "modern" | "classic";
  readonly selectedGroup: number | null;
  readonly currentPlaylist: SourcePlaylistSelection;
  /** Native4137 allows a repeated selection outside Single; Single always repeats. */
  readonly repeatInAreaShuffle: boolean;
  readonly rememberModeOnLogin: boolean;
  readonly keepPlayingOnPlaylistChange: boolean;
}
export interface SourceAudioPreferences {
  readonly version: 1;
  readonly volumes: SourceVolumePreferences;
  readonly music: SourceMusicPreferences;
}
export interface SourceAudioPreferenceBinding {
  readonly playerId: string;
  readonly preferences: SourceAudioPreferences;
  /** Authority, never part of the persisted client-preference record. */
  readonly unlockedGroups: readonly number[];
  readonly musicState: SourceMusicState;
}
export type SourcePlaylistEdit = { readonly kind: "add" | "remove"; readonly group: number };
export interface SourceMusicSkipResult {
  readonly status: "requested" | "pending" | "disabled_mode" | "muted" | "no_alternative";
  readonly previousGroup: number | null;
  readonly nextGroup: number | null;
}

export const SOURCE_VOLUME_CHANNELS = Object.freeze(["master", "music", "effects", "area"] as const);
export const SOURCE_MUTE_FALLBACK_PERCENT = Object.freeze({ master: 100, music: 20, effects: 45, area: 25 });
export const SOURCE_VOLUME_MEMORY_VARBITS = Object.freeze({ master: 14817, music: 12426, effects: 12427, area: 12428 });
export const SOURCE_PLAYLIST_LABELS = Object.freeze(["All music", "Playlist 1", "Playlist 2", "Playlist 3"]);
export const SOURCE_PLAYLIST_CAPACITY = 100;

// Native table44 column5, encoded by9302 as field0*100+field1; NOT row/group+1.
export const SOURCE_STORED_TRACK_IDS: ReadonlyMap<number, number> = new Map([
  [2, 117], [64, 123], [327, 203], [163, 215], [76, 226], [62, 2700], [144, 406], [145, 601],
]);
const storedGroups = new Map([...SOURCE_STORED_TRACK_IDS].map(([group, stored]) => [stored, group]));
const playlistKeys = ["savedPlaylist1", "savedPlaylist2", "savedPlaylist3"] as const;
const musicKeys = ["mode", "areaMode", "selectedGroup", "currentPlaylist", "repeatInAreaShuffle",
  "rememberModeOnLogin", "keepPlayingOnPlaylistChange", ...playlistKeys];

function object(value: unknown, keys: readonly string[], label: string): Record<string, unknown> {
  requireAudio(value !== null && typeof value === "object" && !Array.isArray(value) &&
    [Object.prototype, null].includes(Object.getPrototypeOf(value)),
  "AUDIO_PREFERENCES", `${label} must be a plain serializable record.`);
  const descriptors = Object.getOwnPropertyDescriptors(value);
  requireAudio(Reflect.ownKeys(value).length === keys.length && keys.every((key) =>
    descriptors[key]?.enumerable && Object.hasOwn(descriptors[key]!, "value")),
  "AUDIO_PREFERENCES", `${label} has missing, extra, or accessor fields.`);
  return value as Record<string, unknown>;
}
function choice<T extends string | number>(value: unknown, values: readonly T[], label: string): T {
  const result = values.find((entry) => entry === value);
  requireAudio(result !== undefined, "AUDIO_PREFERENCES", `Invalid ${label}.`);
  return result;
}
function boolean(value: unknown, label: string): boolean {
  requireAudio(typeof value === "boolean", "AUDIO_PREFERENCES", `${label} must be a boolean.`);
  return value;
}
function percentages(value: unknown, label: string): SourcePercentages {
  const record = object(value, SOURCE_VOLUME_CHANNELS, label);
  for (const channel of SOURCE_VOLUME_CHANNELS) {
    requireAudio(integer(record[channel], 0, 100), "AUDIO_PREFERENCES", `${label}.${channel} must be an integer in [0,100].`);
  }
  return Object.freeze({
    master: record.master as number, music: record.music as number,
    effects: record.effects as number, area: record.area as number,
  });
}
export function sourcePreferenceGroup(value: unknown): number {
  requireAudio(integer(value, 0, 65534) && SOURCE_STORED_TRACK_IDS.has(value),
    "AUDIO_PREFERENCE_TRACK", `Source preference track ${String(value)} has no published original music-row binding.`);
  return value;
}
export function sourcePreferenceUnlocks(value: readonly number[]): readonly number[] {
  requireAudio(Array.isArray(value) && value.length <= 100 && new Set(value).size === value.length,
    "AUDIO_PREFERENCE_UNLOCKS", "Supply a bounded, distinct authoritative source unlock list.");
  return Object.freeze(value.map(sourcePreferenceGroup).sort((a, b) => a - b));
}
export function parseSourceSavedPlaylist(value: unknown): SourceSavedPlaylist {
  requireAudio(Array.isArray(value) && value.length === SOURCE_PLAYLIST_CAPACITY &&
    Reflect.ownKeys(value).length === SOURCE_PLAYLIST_CAPACITY + 1 &&
    Array.from({ length: 100 }, (_, i) => Object.getOwnPropertyDescriptor(value, String(i)))
      .every((entry) => entry?.enumerable && Object.hasOwn(entry, "value")),
  "AUDIO_PREFERENCES", "A saved source playlist must contain exactly 100 explicit group/null slots.");
  const slots = Array.from(value, (group: unknown) => group === null ? null : sourcePreferenceGroup(group));
  const groups = slots.filter((group) => group !== null);
  requireAudio(new Set(groups).size === groups.length, "AUDIO_PREFERENCES", "A saved playlist cannot contain duplicate tracks.");
  return Object.freeze(slots);
}
export function parseSourceMusicPreferences(value: unknown): SourceMusicPreferences {
  const record = object(value, musicKeys, "music preferences");
  const mode = choice(record.mode, ["area", "shuffle", "single"], "source music mode");
  const selectedGroup = record.selectedGroup === null ? null : sourcePreferenceGroup(record.selectedGroup);
  requireAudio(mode !== "single" || selectedGroup !== null, "AUDIO_PREFERENCES", "Single Mode needs a selected source track.");
  return Object.freeze({
    mode, areaMode: choice(record.areaMode, ["modern", "classic"], "source area mode"),
    selectedGroup, currentPlaylist: choice(record.currentPlaylist, [0, 1, 2, 3], "numbered playlist"),
    repeatInAreaShuffle: boolean(record.repeatInAreaShuffle, "native4137"),
    rememberModeOnLogin: boolean(record.rememberModeOnLogin, "native19734"),
    keepPlayingOnPlaylistChange: boolean(record.keepPlayingOnPlaylistChange, "native19736"),
    savedPlaylist1: parseSourceSavedPlaylist(record.savedPlaylist1),
    savedPlaylist2: parseSourceSavedPlaylist(record.savedPlaylist2),
    savedPlaylist3: parseSourceSavedPlaylist(record.savedPlaylist3),
  });
}
export function parseSourceVolumePreferences(value: unknown): SourceVolumePreferences {
  const record = object(value, ["current", "remembered"], "volume preferences");
  return Object.freeze({ current: percentages(record.current, "current"), remembered: percentages(record.remembered, "remembered") });
}
export function parseSourceAudioPreferences(value: unknown): SourceAudioPreferences {
  const record = object(value, ["version", "volumes", "music"], "audio preferences");
  requireAudio(record.version === 1, "AUDIO_PREFERENCE_VERSION", "Unsupported audio client-preference version; no guessed migration is applied.");
  return Object.freeze({ version: 1, volumes: parseSourceVolumePreferences(record.volumes),
    music: parseSourceMusicPreferences(record.music) });
}
export function serializeSourceAudioPreferences(value: SourceAudioPreferences): string {
  return JSON.stringify(parseSourceAudioPreferences(value));
}
export function deserializeSourceAudioPreferences(text: string): SourceAudioPreferences {
  requireAudio(typeof text === "string" && text.length <= 16_384, "AUDIO_PREFERENCES", "Stored audio preferences exceed their bounded JSON envelope.");
  let value: unknown;
  try { value = JSON.parse(text); }
  catch (error) {
    throw new AudioFailure("AUDIO_PREFERENCES", `Stored audio preferences are not valid JSON: ${String(error)}`);
  }
  return parseSourceAudioPreferences(value);
}

export function sourceAudioPreferenceDefaults(): SourceAudioPreferences {
  return parseSourceAudioPreferences({
    version: 1,
    volumes: { current: sourceAudioDefaults().sliders, remembered: { master: 0, music: 0, effects: 0, area: 0 } },
    music: {
      mode: "area", areaMode: "modern", selectedGroup: null, currentPlaylist: 0,
      repeatInAreaShuffle: false, rememberModeOnLogin: false, keepPlayingOnPlaylistChange: false,
      savedPlaylist1: Array(100).fill(null), savedPlaylist2: Array(100).fill(null), savedPlaylist3: Array(100).fill(null),
    },
  });
}
export function sourceToggleVolumeMute(value: SourceVolumePreferences, channel: SourceVolumeChannel): SourceVolumePreferences {
  const volume = parseSourceVolumePreferences(value);
  choice(channel, SOURCE_VOLUME_CHANNELS, "source volume channel");
  const current = { ...volume.current }, remembered = { ...volume.remembered };
  if (current[channel] > 0) {
    remembered[channel] = current[channel];
    current[channel] = 0;
  } else {
    if (remembered[channel] === 0) remembered[channel] = SOURCE_MUTE_FALLBACK_PERCENT[channel];
    current[channel] = remembered[channel];
  }
  return parseSourceVolumePreferences({ current, remembered });
}
export function sourceSavedPlaylist(value: SourceSavedPlaylists, slot: SourcePlaylistSlot): SourceSavedPlaylist {
  choice(slot, [1, 2, 3], "saved playlist slot");
  return value[playlistKeys[slot - 1]!];
}
export function sourcePlaylistGroups(value: SourceMusicPreferences, unlocked: readonly number[]): readonly number[] {
  return value.currentPlaylist === 0 ? unlocked
    : sourceSavedPlaylist(value, value.currentPlaylist).filter((group) => group !== null);
}
export function sourceEditPlaylist(value: SourceSavedPlaylist, edit: SourcePlaylistEdit): SourceSavedPlaylist {
  const slots = [...parseSourceSavedPlaylist(value)];
  requireAudio(edit && (edit.kind === "add" || edit.kind === "remove"),
    "AUDIO_PREFERENCES", "Invalid source playlist edit.");
  const group = sourcePreferenceGroup(edit.group), at = slots.indexOf(group);
  if (edit.kind === "add" && at === -1) {
    const empty = slots.indexOf(null);
    requireAudio(empty !== -1, "AUDIO_PLAYLIST_FULL", "The original saved playlist has 100 occupied slots.");
    slots[empty] = group;
  } else if (edit.kind === "remove" && at !== -1) slots[at] = null;
  return Object.freeze(slots);
}
export function sourceSelectPlaylist(
  value: SourceMusicPreferences, slot: SourcePlaylistSelection, playingGroup: number | null,
): SourceMusicPreferences {
  const music = parseSourceMusicPreferences(value);
  choice(slot, [0, 1, 2, 3], "playlist selection");
  let mode = music.mode;
  if (!music.keepPlayingOnPlaylistChange) {
    if (slot === 0) mode = "area";
    else if (mode === "area" || (mode === "single" && playingGroup !== null &&
      !sourceSavedPlaylist(music, slot).includes(playingGroup))) mode = "shuffle";
  }
  return parseSourceMusicPreferences({ ...music, mode, currentPlaylist: slot });
}
export function validateSourcePreferenceUnlocks(value: SourceAudioPreferences, unlocked: readonly number[]): void {
  const available = new Set(sourcePreferenceUnlocks(unlocked));
  for (const group of [value.music.selectedGroup, ...playlistKeys.flatMap((key) => value.music[key])]) {
    requireAudio(group === null || available.has(group), "AUDIO_PREFERENCE_UNLOCKS",
      `Stored source track ${group} is not in this character's authoritative unlock set; preferences cannot grant it.`);
  }
}

function varp(values: ReadonlyMap<number, number>, id: number): number {
  const value = values.get(id);
  requireAudio(integer(value, -2147483648, 2147483647), "AUDIO_PREFERENCE_VARP",
    `Supply genuine original preference varp ${id}; missing or corrupt fields are not defaults.`);
  return value;
}
export function sourceVolumePreferencesFromVarps(values: ReadonlyMap<number, number>): SourceVolumePreferences {
  const override = varp(values, 5588);
  requireAudio(override === 0 || override === 1, "AUDIO_PREFERENCE_VARP", "Invalid original area-volume override flag.");
  const memory = varp(values, 3109), masterMemory = varp(values, 3797);
  return parseSourceVolumePreferences({
    current: { master: varp(values, 3796), music: varp(values, 168), effects: varp(values, 169),
      area: varp(values, override === 1 ? 5589 : 872) },
    remembered: { master: masterMemory & 255, music: memory & 255,
      effects: (memory >>> 8) & 127, area: (memory >>> 15) & 127 },
  });
}
export function sourceSavedPlaylistsFromVarps(values: ReadonlyMap<number, number>): SourceSavedPlaylists {
  const slots = (slot: number): SourceSavedPlaylist => {
    const entries: (number | null)[] = [];
    for (let i = 0; i < 50; i++) {
      const packed = varp(values, 5239 + (slot - 1) * 50 + i);
      for (const code of [packed & 65535, packed >>> 16]) {
        if (code === 0) entries.push(null);
        else {
          const group = storedGroups.get(code);
          requireAudio(group !== undefined, "AUDIO_PREFERENCE_TRACK",
            `Stored native track identity ${code} needs its original published row; it is not an audio group ID.`);
          entries.push(group);
        }
      }
    }
    return parseSourceSavedPlaylist(entries);
  };
  return Object.freeze({ savedPlaylist1: slots(1), savedPlaylist2: slots(2), savedPlaylist3: slots(3) });
}
export function sourceSavedPlaylistsToVarps(value: SourceSavedPlaylists): ReadonlyMap<number, number> {
  const values = new Map<number, number>();
  for (const slot of [1, 2, 3] as const) {
    const entries = parseSourceSavedPlaylist(sourceSavedPlaylist(value, slot));
    const code = (group: number | null) => group === null ? 0 : SOURCE_STORED_TRACK_IDS.get(group)!;
    for (let i = 0; i < 50; i++) values.set(5239 + (slot - 1) * 50 + i,
      code(entries[i * 2]!) | (code(entries[i * 2 + 1]!) << 16));
  }
  return values;
}
export function sourceAudioPreferencesFromVarps(values: ReadonlyMap<number, number>): SourceAudioPreferences {
  const flags = varp(values, 19), currentRow = varp(values, 3883);
  const rows = new Map([[2549, 2], [2583, 64], [2674, 327], [2721, 163], [2777, 76], [2938, 62], [3012, 144], [3237, 145]]);
  const selectedGroup = currentRow === -1 ? null : rows.get(currentRow);
  requireAudio(selectedGroup !== undefined, "AUDIO_PREFERENCE_TRACK", `Original current music row ${currentRow} is not a published menu track.`);
  return parseSourceAudioPreferences({
    version: 1, volumes: sourceVolumePreferencesFromVarps(values),
    music: {
      ...sourceSavedPlaylistsFromVarps(values),
      mode: choice(varp(values, 18), [0, 1, 2], "musicplay") === 0 ? "area" : varp(values, 18) === 1 ? "shuffle" : "single",
      areaMode: choice((flags >>> 10) & 3, [0, 1], "native12233") === 0 ? "modern" : "classic",
      selectedGroup, currentPlaylist: (flags >>> 14) & 15,
      repeatInAreaShuffle: (flags & 1) !== 0, rememberModeOnLogin: (flags & (1 << 21)) !== 0,
      keepPlayingOnPlaylistChange: (flags & (1 << 23)) !== 0,
    },
  });
}
