import type { NativeWidget, SettingRowAsset, UiCatalogue } from "./assets.ts";
import { projectScrollbar, widgetId, widgetKey } from "./layout.ts";
import { escapeText, plainText } from "./raster.ts";
import type { WorldView } from "../shared/contracts.ts";
import type { UiAudioView } from "./audio-controls.ts";
import type { SourceMusicState } from "../audio/native-scene.ts";

export const SETTINGS_CATEGORIES = ["Activities", "Audio", "Chat", "Controls", "Display", "Gameplay", "Interfaces", "Warnings"] as const;
export interface SettingsPageState {
  category: number; search: string | null; moreInfo: boolean; hideLocked: boolean; scroll: number;
  choice: { setting: number; x: number; y: number; width: number; height: number; scroll: number } | null;
}
export interface SettingValue {
  value: number | string | boolean | null;
  reason?: string;
  locked?: boolean;
}
export interface ClientInputSettings { singleMouse: boolean; shiftDrop: boolean; escapeCloses: boolean }
export interface SettingsValues {
  values: Map<number, SettingValue>;
  available: Set<number>;
}

export function currentSettingValues(catalogue: UiCatalogue, world: WorldView, audio: UiAudioView | null,
  music: SourceMusicState | null, input: ClientInputSettings): SettingsValues {
  const values = new Map<number, SettingValue>(), available = new Set<number>();
  for (const row of Object.values(catalogue.settingsRows)) values.set(row.id, {
    value: null, reason: "This setting is not supplied by the current client/service projection.",
    ...(row.label.startsWith("Modern Layout") ? { locked: true } : {}),
  });
  const known = (id: number, value: SettingValue["value"], enabled = true) => {
    values.set(id, { value }); if (enabled) available.add(id);
  };
  if (audio && !audio.disposed) {
    known(4503, audio.percentages.master); known(2753, audio.percentages.music);
    known(2754, audio.percentages.effects); known(2755, audio.percentages.area);
    known(6335, null);
  }
  if (music) {
    known(6393, music.mode === "area" ? 0 : music.mode === "single" ? 2 : 1);
    known(2974, music.areaMode === "modern" ? 0 : 1);
  }
  known(2769, input.singleMouse); known(1104, input.shiftDrop); known(2775, input.escapeCloses);
  known(2732, 1); known(2852, 0); known(2853, null);
  const supplies = world.player.settings.find(setting => setting.setting === "death_supply_piles");
  const equip = world.player.settings.find(setting => setting.setting === "death_auto_equip");
  if (supplies) known(2861, supplies.enabled);
  if (equip) known(880, equip.enabled);
  return { values, available };
}
export interface SettingsProjection {
  widgets: NativeWidget[];
  controls: Map<string, { row: SettingRowAsset; source: NativeWidget }>;
  nodes: Map<string, SettingRowAsset>;
  rowIds: number[];
  extent: number;
  scroll: number;
}

const BODY = [19, 20, 22];
const categoryName = (index: number) => SETTINGS_CATEGORIES[index]!.toLowerCase();
const settingKind = (row: SettingRowAsset) => Number(row.params["1078"] ?? 0);

export function defaultSettingsPage(): SettingsPageState {
  return { category: 0, search: null, moreInfo: true, hideLocked: false, scroll: 0, choice: null };
}

export function settingsMatches(row: SettingRowAsset, query: string): boolean {
  const term = query.toLowerCase();
  return [row.params["1088"], row.params["1086"], row.params["1096"], SETTINGS_CATEGORIES[row.category]]
    .some(value => typeof value === "string" && value.toLowerCase().includes(term));
}

export function projectAllSettings(catalogue: UiCatalogue, state: SettingsPageState,
  values?: ReadonlyMap<number, SettingValue>): SettingsProjection {
  const search = state.search !== null;
  const source = catalogue.templates[search ? "bounded-settings-search-zoom" : `bounded-settings-${categoryName(state.category)}`];
  if (!source) throw new Error("Original All Settings window geometry is missing.");
  const originals = source.map(widget => ({ ...widget }));
  const root = originals.find(widget => widget.id === widgetId(134, 19) && widget.index === -1)!;
  const viewport = originals.find(widget => widget.id === widgetId(134, 14) && widget.index === -1)!;
  const rows = Object.values(catalogue.settingsRows).filter(row => search || row.category === state.category)
    .sort((a, b) => a.category - b.category || a.top - b.top || a.labelIndex - b.labelIndex);
  let sectionMatches = false, currentCategory = -1;
  let selected = rows.filter(row => {
    if (row.category !== currentCategory) { currentCategory = row.category; sectionMatches = false; }
    if (search && settingKind(row) === 5) sectionMatches = settingsMatches(row, state.search!);
    return (!search || sectionMatches || settingsMatches(row, state.search!)) &&
      (!state.hideLocked || (values ? values.get(row.id)?.locked !== true : !row.sourceLocked));
  });
  const widgets = originals.filter(widget => !(widget.id >> 16 === 134 && BODY.includes(widget.id & 65535) && widget.index >= 0));
  const controls = new Map<string, { row: SettingRowAsset; source: NativeWidget }>();
  const nodes = new Map<string, SettingRowAsset>();
  const indices = new Map<number, number>();
  let top = search ? 5 : Math.max(0, (selected[0]?.top ?? root.y) - root.y);
  let stripe = search ? 200 : selected[0]?.widgets.find(widget => widget.type === 3 && widget.color === 0 && [200, 220].includes(widget.opacity))?.opacity ?? 220;
  const tooMany = search && selected.filter(row => ![5, 6, 8].includes(settingKind(row))).length >= 80;
  if (search && (!state.search!.length || !selected.length || tooMany)) {
    const sourceMessage = catalogue.templates["bounded-settings-search-empty"]!
      .find(widget => widget.id === root.id && widget.type === 4 && widget.index >= 0)!;
    const message = !state.search!.length ? "<br><br>Search for a setting using the search bar above."
      : tooMany ? `<br><br>Your search for '${escapeText(state.search!)}' returned too many results.<br><br>Please try refining your search to narrow down the results.`
        : sourceMessage.text;
    widgets.push({ ...sourceMessage, text: message });
    selected = [];
    top = viewport.height;
  }
  for (const row of selected) {
    const kind = settingKind(row);
    const descriptions = row.widgets.filter(widget => widget.id === root.id && row.descriptionIndices.includes(widget.index));
    const removedHeight = state.moreInfo ? 0 : descriptions.reduce((height, widget) => height + widget.height, 0);
    const backdrop = row.widgets.find(widget => widget.id === root.id && widget.type === 3 && widget.color === 0 &&
      widget.width >= root.width - 10 && [200, 220].includes(widget.opacity));
    const fullHeight = backdrop?.height ?? row.height;
    const height = Math.max(1, fullHeight - removedHeight);
    const delta = root.y + top - row.top;
    const value = values?.get(row.id);
    const colourControl = kind === 9 ? row.widgets.find(widget => widget.onOp && widget.type === 3) : undefined;
    for (const original of row.widgets) {
      if (!state.moreInfo && original.id === root.id && row.descriptionIndices.includes(original.index)) continue;
      const widget = { ...original };
      const next = indices.get(widget.id) ?? 0;
      indices.set(widget.id, next + 1);
      widget.index = next;
      nodes.set(widgetKey(widget), row);
      widget.y += delta;
      widget.originalY += delta;
      if (original === backdrop) {
        widget.height = height;
        if (search || !state.moreInfo) widget.opacity = stripe;
      }
      if (removedHeight && original !== backdrop && !(original.id === root.id && original.index === row.labelIndex)) {
        if (kind === 1) {
          if (!row.descriptionIndices.includes(original.index)) widget.y -= removedHeight;
        } else if (kind !== 5) {
          if (original.height === fullHeight) widget.height = height;
          else widget.y -= Math.trunc(removedHeight / 2);
        }
      }
      if (value && original.id === root.id && original.index === row.labelIndex && kind === 1)
        widget.text = value.value === null ? `${row.label} - Unavailable` : typeof value.value === "number" && [30, 31, 32, 319].includes(Number(row.params["1077"]))
          ? `${row.label} - ${value.value}%` : row.label;
      if (original.actions?.some(Boolean) || original.onOp || original.id !== root.id && original.type === 5 && kind === 1)
        controls.set(widgetKey(widget), { row, source: original });
      if (value && kind === 0 && original.id === root.id && original.type === 5 && original.width === 18) {
        if (typeof value.value === "boolean") widget.sprite = value.locked ? value.value ? 2848 : 2850 : value.value ? 2847 : 2849;
        else widget.sprite = 2850;
      }
      if (value && kind === 2 && original.id === root.id && original.type === 4 && original.index !== row.labelIndex &&
          !row.descriptionIndices.includes(original.index)) {
        const choices = catalogue.settingsDefinitions.choices[String(row.params["1091"])];
        const option = choices?.keys.indexOf(Number(value.value)) ?? -1;
        widget.text = value.value === null ? "Unavailable" : option >= 0 ? escapeText(choices!.stringVals?.[option] ?? String(value.value)) : escapeText(String(value.value));
      }
      if (value && kind === 4 && original.id === root.id && original.type === 4 && original.index !== row.labelIndex &&
          !row.descriptionIndices.includes(original.index))
        widget.text = value.value === null ? "Unavailable" : escapeText(String(value.value));
      if (value?.value === null && colourControl && original.type === 3 && original.filled &&
          original.x === colourControl.x && original.y === colourControl.y &&
          original.width === colourControl.width && original.height === colourControl.height) {
        widget.color = 0x0e0e0c; widget.opacity = 0;
      }
      widgets.push(widget);
    }
    top += search || !state.moreInfo ? height + Number(row.params["1079"] ?? 0) : row.height;
    if (backdrop) stripe = stripe === 200 ? 220 : 200;
  }
  const scroll = Math.min(Math.max(0, state.scroll), Math.max(0, top - viewport.height));
  for (const widget of widgets) {
    if (widget.id >> 16 !== 134) continue;
    const child = widget.id & 65535;
    if (BODY.includes(child) && widget.index >= 0) widget.y -= scroll;
    if (BODY.includes(child) && widget.index === -1) widget.height = widget.originalHeight = Math.max(top, viewport.height);
    if (child === 12) widget.text = search ? `${escapeText(state.search!)}<col=f4f4f4>*</col>` : "*";
    if (child === 6 && widget.type === 4) widget.text = state.moreInfo ? "<col=ffffff>Less <col=ff981f>info" : "<col=ffffff>More <col=ff981f>info";
    if (child === 31 && widget.type === 4) widget.text = state.hideLocked ? "<col=ffffff>Show<col=ff981f> locked" : "<col=ffffff>Hide<col=ff981f> locked";
  }
  projectScrollbar(widgets, widgetId(134, 21), viewport.id, top, scroll);
  if (values) {
    for (const [key, { row, source: original }] of controls) {
      if (settingKind(row) !== 1 || original.type !== 5 || original.width !== 16) continue;
      const widget = widgets.find(widget => widgetKey(widget) === key)!;
      const value = values.get(row.id)?.value;
      const track = [...controls].find(([other, item]) => item.row.id === row.id && item.source.type === 3 &&
        item.source.height === 16 && item.source.actions?.includes("Select"));
      const bounds = track && widgets.find(widget => widgetKey(widget) === track[0]);
      if (typeof value === "number" && bounds) widget.x = bounds.x + Math.trunc((bounds.width - 16) * value / 100);
      else widget.sprite = -1;
    }
  }
  return { widgets, controls, nodes, rowIds: selected.map(row => row.id), extent: top, scroll };
}

export function settingsControlLabel(row: SettingRowAsset, widget: NativeWidget): string {
  return `${plainText(widget.actions?.find(Boolean) ?? "Select")} ${row.label}`;
}
