import type { NativeWidget, Rect } from "./assets.ts";
import { intersect } from "./assets.ts";
import { SourceRaster } from "./raster.ts";

export const TABS = [
  { name: "Combat Options", interface: "interface.combat", group: 593, template: "native-combat", key: "F1" },
  { name: "Skills", interface: "interface.skills", group: 320, template: "native-skills", key: "F2" },
  { name: "Quest List", interface: "interface.quests", group: 399, template: "native-quest-list", key: "F3" },
  { name: "Inventory", interface: "interface.inventory", group: 149, template: "native-inventory", key: "Escape" },
  { name: "Worn Equipment", interface: "interface.equipment", group: 387, template: "native-equipment", key: "F4" },
  { name: "Prayer", interface: "interface.prayer", group: 541, template: "native-prayer", key: "F5" },
  { name: "Magic", interface: "interface.magic", group: 218, template: "native-magic", key: "F6" },
  { name: "Grouping", interface: "interface.grouping", group: 707, template: "native-grouping", key: "" },
  { name: "Friends List", interface: "interface.account_links", group: 109, template: "native-friends", key: "" },
  { name: "Account", interface: "interface.account", group: 429, template: "native-account", key: "" },
  { name: "Logout", interface: "interface.logout", group: 182, template: "native-logout", key: "" },
  { name: "Settings", interface: "interface.settings", group: 116, template: "native-settings", key: "" },
  { name: "Emotes", interface: "interface.emotes", group: 216, template: "native-emotes", key: "" },
  { name: "Music Player", interface: "interface.music", group: 239, template: "native-music", key: "" },
] as const;

export function widgetId(group: number, child: number): number { return group * 65536 + child; }
export function tabWidget(slot: number): number { return widgetId(161, slot < 7 ? 59 + slot : 43 + slot - 7); }
export function widgetKey(w: Pick<NativeWidget, "id" | "index">): string { return `${w.id}:${w.index}`; }

export function frameRegions(width: number, height: number): Record<"minimap" | "chat" | "sidebar" | "sidecontent", Rect> {
  return {
    minimap: { x: width - 211, y: 0, width: 211, height: 207 },
    chat: { x: 0, y: height - 165, width: 519, height: 165 },
    sidebar: { x: width - 241, y: height - 335, width: 241, height: 335 },
    sidecontent: { x: width - 216, y: height - 298, width: 190, height: 261 },
  };
}

function size(value: number, mode: number, available: number): number {
  if (mode === 1) return available - value;
  if (mode === 2) return value * available >> 14;
  return value;
}
function position(value: number, mode: number, available: number, extent: number): number {
  if (mode === 1) return value + (available - extent >> 1);
  if (mode === 2) return available - extent - value;
  if (mode === 3) return value * available >> 14;
  if (mode === 4) return (available - extent >> 1) + (value * available >> 14);
  if (mode === 5) return available - extent - (value * available >> 14);
  return value;
}

export interface LaidWidget extends NativeWidget { clip: Rect }

export function nativeTree(widgets: readonly NativeWidget[], width: number, height: number): LaidWidget[] {
  const children = new Map<number, NativeWidget[]>();
  for (const widget of widgets) {
    const list = children.get(widget.parent) ?? [];
    list.push(widget); children.set(widget.parent, list);
  }
  // Native gp paints static children, dynamic children, then attached interface groups.
  for (const [parent, nodes] of children) {
    const order = (w: NativeWidget) => w.id >> 16 !== parent >> 16 ? 2 : w.index < 0 ? 0 : 1;
    nodes.sort((a, b) => order(a) - order(b));
  }
  const output: LaidWidget[] = [], visited = new Set<string>();
  const viewport = { x: 0, y: 0, width, height };
  const walk = (widget: NativeWidget, parent: Rect, sourceParent: Rect, clip: Rect) => {
    const key = widgetKey(widget);
    if (visited.has(key)) return;
    visited.add(key);
    let w = widget.width, h = widget.height, x = widget.x, y = widget.y;
    if (width !== 1920 || height !== 1080) {
      // Preserve the script-resolved offset, changing only the native parent alignment.
      const calculatedWidth = size(widget.originalWidth, widget.widthMode, sourceParent.width);
      const calculatedHeight = size(widget.originalHeight, widget.heightMode, sourceParent.height);
      w += size(widget.originalWidth, widget.widthMode, parent.width) - calculatedWidth;
      h += size(widget.originalHeight, widget.heightMode, parent.height) - calculatedHeight;
      x += parent.x - sourceParent.x
        + position(widget.originalX, widget.xMode, parent.width, w)
        - position(widget.originalX, widget.xMode, sourceParent.width, widget.width);
      y += parent.y - sourceParent.y
        + position(widget.originalY, widget.yMode, parent.height, h)
        - position(widget.originalY, widget.yMode, sourceParent.height, widget.height);
    }
    const rect = { x, y, width: w, height: h };
    output.push({ ...widget, ...rect, clip });
    const nextClip = widget.type === 0 ? intersect(clip, rect) : clip;
    const descendants = children.get(widget.id) ?? [];
    for (const child of descendants) {
      if (child === widget || (widget.index >= 0 && child.id === widget.id)) continue;
      walk(child, rect, widget, nextClip);
    }
  };
  for (const root of widgets) if (root.parent === -1 && root.id >> 16 === 161) {
    walk(root, viewport, { x: 0, y: 0, width: 1920, height: 1080 }, viewport);
  }
  return output;
}

export function paintNativeTree(raster: SourceRaster, widgets: readonly NativeWidget[], width: number, height: number,
  override?: (widget: LaidWidget) => boolean): LaidWidget[] {
  const tree = nativeTree(widgets, width, height);
  for (const widget of tree) {
    raster.clip(widget.clip, () => {
      if (!override?.(widget)) raster.widget(widget);
    });
  }
  return tree;
}

export function cloneTemplate(widgets: readonly NativeWidget[]): NativeWidget[] {
  return widgets.map(w => ({ ...w }));
}

/** Source CS2 231/740: scrollbar sizing and thumb position, independent of gameplay. */
export function projectScrollbar(widgets: NativeWidget[], barId: number, contentId: number, total: number, position: number): void {
  const bar = widgets.find(w => w.id === barId && w.index === -1);
  const content = widgets.find(w => w.id === contentId && w.index === -1);
  if (!bar || !content) return;
  const visible = Math.max(1, content.height), extent = Math.max(visible, total);
  const track = Math.max(10, bar.height - 32);
  const thumb = Math.max(10, Math.min(track, Math.trunc(visible * track / extent)));
  const offset = 16 + Math.trunc((bar.height - 32 - thumb) * position / Math.max(1, total - visible));
  content.scrollHeight = total > visible ? total : 0; content.scrollY = position;
  for (const widget of widgets) if (widget.id === barId && widget.index >= 1 && widget.index <= 3) {
    widget.y = bar.y + offset + (widget.index === 3 ? thumb - 5 : 0);
    widget.originalY = offset + (widget.index === 3 ? thumb - 5 : 0);
    widget.height = widget.originalHeight = widget.index === 1 ? thumb : 5;
    widget.heightMode = 0; widget.yMode = 0;
  }
}
