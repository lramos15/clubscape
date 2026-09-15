import type { SourceMusicState } from "../audio/native-scene.ts";
import { SOURCE_MUSIC_MODE_IDS } from "../audio/native-scene.ts";
import type { MusicTrackAsset, NativeWidget, UiCatalogue } from "./assets.ts";
import { projectScrollbar, widgetId } from "./layout.ts";
import { escapeText } from "./raster.ts";

export type MusicUiAction =
  | { kind: "mode"; mode: SourceMusicState["mode"] }
  | { kind: "play" | "add" | "remove"; group: number }
  | { kind: "loop"; enabled: boolean }
  | { kind: "area_mode"; mode: SourceMusicState["areaMode"] };

export function musicStateProblem(state: SourceMusicState): string | null {
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
  if (state.mode === "playlist" && !state.playlistGroups.length) return "The declared current playlist is empty.";
  if (state.mode === "shuffle" && !state.unlockedGroups.length) return "There are no declared unlocked tracks to shuffle.";
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
  }
  const problem = musicStateProblem(next);
  return problem ? { state: null, problem } : { state: next, problem: null };
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
  playingGroup: number | null, scroll: number, dropdown: boolean): MusicProjection {
  const source = catalogue.templates[dropdown ? "native-music-filter-open" : "native-music"];
  if (!source || !catalogue.musicTracks) throw new Error("Native music row/control metadata is missing.");
  let widgets = source.map(widget => ({ ...widget }));
  const tracks = new Map(catalogue.musicTracks.map(track => [track.widgetIndex, track]));
  const available = new Set(state?.unlockedGroups ?? []);
  const selectedMode = state ? state.mode === "playlist" ? 1 : SOURCE_MUSIC_MODE_IDS[state.mode] : null;
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
  const visible = state?.mode === "playlist" ? rows.filter(widget => state.playlistGroups.includes(tracks.get(widget.index)!.group)) : rows;
  const visibleIndices = new Set(visible.map(widget => widget.index));
  const extent = visible.length ? first * 2 + visible.length * 15 : 0;
  const position = Math.min(Math.max(0, scroll), Math.max(0, extent - viewport.height));
  widgets = widgets.filter(widget => widget.id !== list.id || widget.type !== 4 || visibleIndices.has(widget.index));
  visible.forEach((widget, index) => {
    widget.y = list.y + first + index * 15 - position;
    widget.originalY = first + index * 15;
    widget.color = state ? available.has(tracks.get(widget.index)!.group) ? 0x0dc10d : 0xff0000 : 0xff981f;
  });
  if (state?.mode === "playlist") {
    const title = widgets.find(widget => widget.id === widgetId(239, 18) && widget.type === 4)!;
    title.text = "Current playlist";
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
