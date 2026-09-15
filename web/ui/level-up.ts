import type { NativeWidget, UiCatalogue } from "./assets.ts";
import { escapeText, sourceLines } from "./raster.ts";
import { widgetId } from "./layout.ts";

export const LEVEL_UP_SOURCE = Object.freeze({
  sourceName: "levelup_display", popupGroup: 233, parent: widgetId(162, 567),
  title: widgetId(233, 1), detail: widgetId(233, 2), continuation: widgetId(233, 3),
  font: 497, chatGroup: 162,
});

/** Source styling only. The backend still owns canonical interface IDs, text and level outcomes. */
export function projectLevelUpPopup(catalogue: UiCatalogue, fields: { title: string; lines: readonly string[]; continuation: string },
  page = 0): NativeWidget[] {
  const source = catalogue.templates["bounded-levelup-popup"];
  if (!source) throw new Error("Original current level-up popup widgets are missing.");
  const lines = fields.lines.flatMap(line => sourceLines(escapeText(line), 390, catalogue.fonts[497]!));
  return source.map(widget => {
    const value = { ...widget };
    if (widget.id === LEVEL_UP_SOURCE.title) value.text = escapeText(fields.title);
    if (widget.id === LEVEL_UP_SOURCE.detail) value.text = lines.slice(page * 2, page * 2 + 2).join("<br>");
    if (widget.id === LEVEL_UP_SOURCE.continuation) value.text = escapeText(fields.continuation);
    return value;
  });
}

export function projectLevelUpChat(catalogue: UiCatalogue, text: string): NativeWidget[] {
  const source = catalogue.templates["bounded-levelup-chat"];
  if (!source) throw new Error("Original level-up chat styling is missing.");
  return source.map(widget => ({ ...widget, text: widget.id >> 16 === 162
    ? widget.text.replace("Source-only level-up chat notice.", escapeText(text)) : widget.text }));
}
