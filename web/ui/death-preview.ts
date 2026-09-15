import type { GameplayUiView, ItemView } from "../shared/contracts.ts";
import type { NativeWidget, UiCatalogue } from "./assets.ts";
import { projectScrollbar, widgetId, widgetKey } from "./layout.ts";
import { formatUiInteger } from "./gameplay-ui.ts";
import { textAdvance } from "./raster.ts";

export type DeathPreview = NonNullable<GameplayUiView["keptOnDeath"]>;
export interface DeathPreviewProjection {
  widgets: NativeWidget[];
  items: Map<string, { item: ItemView; kept: boolean }>;
}

export function deathPreviewDetails(view: DeathPreview): string {
  return `Normal unsafe non-PvP death\nFull gravestone fee: ${formatUiInteger(view.fullGraveFee)} coins` +
    `\nFull Death's Office fee: ${formatUiInteger(view.fullOfficeFee)} coins\nValue revision: ${view.valueRevision}`;
}

export function projectDeathPreview(catalogue: UiCatalogue, view: DeathPreview, scroll: number): DeathPreviewProjection {
  const source = catalogue.templates["native-death-preview-populated"];
  if (!source) throw new Error("The native populated death-preview layout is missing.");
  const original = source.map(widget => ({ ...widget }));
  const content = original.find(widget => widget.id === widgetId(4, 5) && widget.index === -1)!;
  const columns = Math.max(1, Math.trunc(content.width / (36 + 5)));
  const gap = columns > 1 ? Math.trunc((content.width - columns * 36) / (columns - 1)) : 0;
  const start = Math.trunc((content.width - (columns * 36 + (columns - 1) * gap)) / 2);
  const nodes = original.filter(widget => !(widget.id >> 16 === 4 &&
    ([6, 7, 8, 9, 10].includes(widget.id & 65535) || widget.id === content.id && widget.index >= 0)));
  const items = new Map<string, { item: ItemView; kept: boolean }>();
  let top = 0, separators = 0;
  for (const [child, rows, kept] of [[6, view.kept, true], [7, view.lost, false]] as const) {
    if (rows.length === 0) continue;
    if (top > 0) {
      for (let index = 0; index < 2; index++) {
        const prototype = original.find(widget => widget.id === content.id && widget.index === index)!;
        nodes.push({ ...prototype, index: separators++, y: content.y + top + 8 + index });
      }
      top += 16;
    }
    const id = widgetId(4, child), prototype = original.find(widget => widget.id === id && widget.index === -1)!;
    const height = 17 + Math.floor((rows.length - 1) / columns) * (32 + gap) + 32;
    nodes.push({ ...prototype, y: content.y + top, originalY: top, height, originalHeight: height, heightMode: 0 });
    const icon = original.find(widget => widget.id === id && widget.item >= 0)!;
    rows.forEach((item, index) => {
      const x = start + index % columns * (36 + gap), y = 17 + Math.floor(index / columns) * (32 + gap);
      const widget = { ...icon, index, x: content.x + x, y: content.y + top + y,
        originalX: x, originalY: y, item: item.sourceId ?? -1, item_quantity: item.quantity, name: item.name };
      nodes.push(widget); items.set(widgetKey(widget), { item, kept });
    });
    const header = original.find(widget => widget.id === id && widget.type === 4)!;
    const fee = view.fullGraveFee === "0" ? "None" : view.fullGraveFee === "1" ? "1 coin" : `${formatUiInteger(view.fullGraveFee)} coins`;
    const text = kept ? header.text : `Items that go to your <col=ffffff>GRAVESTONE</col>: <col=ffffff>(Fee: ${fee})</col>`;
    nodes.push({ ...header, index: rows.length, y: content.y + top + 3,
      text: textAdvance(text, catalogue.fonts[header.font]!) <= header.width ? text
        : "Items that go to your <col=ffffff>GRAVESTONE</col>: <col=ffffff>(View fee)</col>" });
    top += height;
  }
  const position = Math.min(Math.max(0, scroll), Math.max(0, top - content.height));
  for (const widget of nodes) if (widget.parent === content.id && widget.index >= 0 || [6, 7].some(child => widget.id === widgetId(4, child)))
    widget.y -= position;
  const footer = nodes.find(widget => widget.id === widgetId(4, 18))!;
  footer.text = "View retrieval fees";
  projectScrollbar(nodes, widgetId(4, 11), content.id, top, position);
  return { widgets: nodes.sort((a, b) => a.id - b.id || a.index - b.index), items };
}
