import type { SourceMusicState } from "../audio/native-scene.ts";
import { SOURCE_MUSIC_MODE_IDS } from "../audio/native-scene.ts";
import type { MusicTrackAsset, NativeWidget, UiCatalogue } from "./assets.ts";
import { projectScrollbar, widgetId } from "./layout.ts";
import { escapeText } from "./raster.ts";
import type { PlayerAudioControls } from "../app/player-audio.ts";
import { sourceSavedPlaylist, SOURCE_PLAYLIST_LABELS, SOURCE_PLAYLIST_CAPACITY } from "../audio/preferences.ts";
import type { SourceAudioPreferenceBinding, SourceMusicPreferences, SourcePlaylistSelection, SourcePlaylistSlot } from "../audio/preferences.ts";

export type UiNativeMusicFlag = "repeatInAreaShuffle" | "rememberModeOnLogin" | "keepPlayingOnPlaylistChange";

export type MusicUiAction =
  | { kind: "mode"; mode: SourceMusicState["mode"] }
  | { kind: "play" | "add" | "remove"; group: number }
  | { kind: "loop"; enabled: boolean }
  | { kind: "area_mode"; mode: SourceMusicState["areaMode"] }
  | { kind: "skip" }
  | { kind: "select_playlist"; selection: SourcePlaylistSelection }
  | { kind: "edit_playlist"; slot: SourcePlaylistSlot; edit: "add" | "remove"; group: number }
  | { kind: "clear_playlist"; slot: SourcePlaylistSlot }
  | { kind: "flag"; flag: UiNativeMusicFlag; enabled: boolean };

export function musicStateProblem(state: SourceMusicState, nativePreferences = false): string | null {
  if (!state || !["area", "single", "shuffle", "playlist"].includes(state.mode) ||
      !["modern", "classic"].includes(state.areaMode) || typeof state.loopEnabled !== "boolean")
    return "The source music mode, area mode or loop preference is invalid.";
  for (const groups of [state.unlockedGroups, state.playlistGroups]) {
    if (!Array.isArray(groups) || groups.length > 100 || new Set(groups).size !== groups.length ||
        groups.some(group => !Number.isInteger(group) || group < 0 || group > 65534))
      return "Source music groups must be unique original IDs within the published 100-track bound.";
  }
  if (state.selectedGroup !== null && !state.unlockedGroups.includes(state.selectedGroup) ||
      state.playlistGroups.some(group => !state.unlockedGroups.includes(group)))
    return "Music selection cannot grant a locked source track.";
  if (state.mode === "single" && state.selectedGroup === null) return "Choose a source-unlocked track for Single Mode.";
  if (!nativePreferences && state.mode === "playlist" && !state.playlistGroups.length) return "The declared current playlist is empty.";
  if (!nativePreferences && state.mode === "shuffle" && !state.unlockedGroups.length) return "There are no declared unlocked tracks to shuffle.";
  return null;
}

export function musicRequest(state: SourceMusicState, action: MusicUiAction, plannedGroup: number | null):
  { state: SourceMusicState; problem: null } | { state: null; problem: string } {
  const before = musicStateProblem(state);
  if (before) return { state: null, problem: before };
  let next: SourceMusicState = { ...state, unlockedGroups: [...state.unlockedGroups], playlistGroups: [...state.playlistGroups] };
  if ("group" in action && !state.unlockedGroups.includes(action.group))
    return { state: null, problem: "That track is no longer source-unlocked. Review the current music state." };
  switch (action.kind) {
    case "mode":
      next = { ...next, mode: action.mode, selectedGroup: action.mode === "single"
        ? state.selectedGroup ?? (plannedGroup !== null && state.unlockedGroups.includes(plannedGroup) ? plannedGroup : null)
        : state.selectedGroup };
      break;
    case "play": next = { ...next, mode: "single", selectedGroup: action.group }; break;
    case "loop": next = { ...next, loopEnabled: action.enabled }; break;
    case "area_mode": next = { ...next, areaMode: action.mode }; break;
    case "add":
      next = { ...next, playlistGroups: state.playlistGroups.includes(action.group) ? state.playlistGroups : [...state.playlistGroups, action.group] };
      break;
    case "remove": next = { ...next, playlistGroups: state.playlistGroups.filter(group => group !== action.group) }; break;
    case "skip": case "select_playlist": case "edit_playlist": case "clear_playlist": case "flag":
      return { state: null, problem: "This control requires the actual player-scoped native preference binding." };
  }
  const problem = musicStateProblem(next);
  return problem ? { state: null, problem } : { state: next, problem: null };
}

export function applyNativeMusicControl(controls: PlayerAudioControls,
  binding: SourceAudioPreferenceBinding, action: Exclude<MusicUiAction, { kind: "skip" }>, plannedGroup: number | null): SourceAudioPreferenceBinding {
  const music = binding.preferences.music, player = binding.playerId;
  const update = (patch: Partial<SourceMusicPreferences>) => controls.setMusic(player, { ...music, ...patch });
  switch (action.kind) {
    case "mode": {
      if (action.mode === "playlist" && music.currentPlaylist === 0)
        throw Object.assign(new Error("Choose an actual numbered playlist first."), { errorId: "ui.music.playlist.selection" });
      const mode = action.mode === "playlist" ? "shuffle" : action.mode;
      return update({ mode, selectedGroup: mode === "single" ? music.selectedGroup ?? plannedGroup : music.selectedGroup });
    }
    case "play": return update({ mode: "single", selectedGroup: action.group });
    case "area_mode": return update({ areaMode: action.mode });
    case "flag": return update({ [action.flag]: action.enabled });
    case "select_playlist": return controls.selectPlaylist(player, action.selection);
    case "edit_playlist": return controls.editSavedPlaylist(player, action.slot, { kind: action.edit, group: action.group });
    case "clear_playlist": return controls.setSavedPlaylist(player, action.slot, Array.from({ length: SOURCE_PLAYLIST_CAPACITY }, () => null));
    case "add": case "remove":
      if (music.currentPlaylist === 0)
        throw Object.assign(new Error("All music is not a saved playlist. Choose Playlist 1, 2 or 3."), { errorId: "ui.music.playlist.selection" });
      return controls.editSavedPlaylist(player, music.currentPlaylist, { kind: action.kind, group: action.group });
    case "loop":
      throw Object.assign(new Error("Native preferences use repeatInAreaShuffle; the legacy loop flag is not that setting."), { errorId: "ui.music.native_repeat" });
  }
}

export interface MusicProjection {
  widgets: NativeWidget[];
  tracks: ReadonlyMap<number, MusicTrackAsset>;
  scroll: number;
  extent: number;
  viewportHeight: number;
}

/** Source318 button states, native row geometry and actual supplied unlocks; no song-selection algorithm. */
export function projectMusicControls(catalogue: UiCatalogue, state: SourceMusicState | null,
  playingGroup: number | null, scroll: number, dropdown: boolean, preferences: SourceMusicPreferences | null = null): MusicProjection {
  const source = catalogue.templates[dropdown ? "native-music-filter-open" : "native-music"];
  if (!source || !catalogue.musicTracks) throw new Error("Native music row/control metadata is missing.");
  let widgets = source.map(widget => ({ ...widget }));
  const tracks = new Map(catalogue.musicTracks.map(track => [track.widgetIndex, track]));
  const available = new Set(state?.unlockedGroups ?? []);
  const selectedMode = preferences ? SOURCE_MUSIC_MODE_IDS[preferences.mode]
    : state ? state.mode === "playlist" ? 1 : SOURCE_MUSIC_MODE_IDS[state.mode] : null;
  const modeSource = catalogue.templates[`native-music-mode-${selectedMode ?? 0}`]!;
  for (const widget of widgets) {
    if (widget.id >> 16 !== 239) continue;
    const child = widget.id & 65535;
    if ([14, 15, 16, 17].includes(child) && widget.index >= 0) {
      const key = state ? child : 15;
      const skin = (state ? modeSource : source).find(row => row.id === widgetId(239, key) && row.index === widget.index);
      if (skin && widget.index < 9) {
        widget.sprite = skin.sprite; widget.type = skin.type; widget.color = skin.color; widget.opacity = skin.opacity;
      }
    }
    if (child === 4 && widget.index === -1)
      widget.text = playingGroup === null ? "" : escapeText(catalogue.musicTracks.find(track => track.group === playingGroup)?.name ?? "Source track");
    if (child === 5)
      widget.text = `Unlocked: ${state ? catalogue.musicTracks.filter(track => available.has(track.group)).length : "?"} / ${catalogue.musicTracks.length}`;
  }
  const list = widgets.find(widget => widget.id === widgetId(239, 11) && widget.index === -1)!;
  const viewport = widgets.find(widget => widget.id === widgetId(239, 9) && widget.index === -1)!;
  const rows = widgets.filter(widget => widget.id === list.id && widget.type === 4).sort((a, b) => a.y - b.y);
  const first = Math.min(...rows.map(widget => widget.y)) - list.y;
  const selected = preferences?.currentPlaylist ? sourceSavedPlaylist(preferences, preferences.currentPlaylist)
    : !preferences && state?.mode === "playlist" ? state.playlistGroups : null;
  const visible = selected ? rows.filter(widget => selected.includes(tracks.get(widget.index)!.group)) : rows;
  const visibleIndices = new Set(visible.map(widget => widget.index));
  const extent = visible.length ? first * 2 + visible.length * 15 : 0;
  const position = Math.min(Math.max(0, scroll), Math.max(0, extent - viewport.height));
  widgets = widgets.filter(widget => widget.id !== list.id || widget.type !== 4 || visibleIndices.has(widget.index));
  visible.forEach((widget, index) => {
    widget.y = list.y + first + index * 15 - position;
    widget.originalY = first + index * 15;
    widget.color = state ? available.has(tracks.get(widget.index)!.group) ? 0x0dc10d : 0xff0000 : 0xff981f;
  });
  if (preferences || state?.mode === "playlist") {
    const title = widgets.find(widget => widget.id === widgetId(239, 18) && widget.type === 4)!;
    title.text = preferences ? SOURCE_PLAYLIST_LABELS[preferences.currentPlaylist]! : "Current playlist";
  }
  for (const child of [10, 11]) {
    const content = widgets.find(widget => widget.id === widgetId(239, child) && widget.index === -1)!;
    content.height = content.originalHeight = Math.max(viewport.height, extent);
  }
  projectScrollbar(widgets, widgetId(239, 12), viewport.id, extent, position);
  return { widgets, tracks, scroll: position, extent, viewportHeight: viewport.height };
}

export function musicScrollPosition(y: number, height: number, thumb: number, maximum: number, grab: number): number {
  const travel = Math.max(1, height - 32 - thumb);
  return Math.trunc(Math.max(0, Math.min(travel, y - 16 - grab)) * maximum / travel);
}
