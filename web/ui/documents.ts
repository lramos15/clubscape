import type { GameplayUiView, Tile } from "../shared/contracts.ts";
import type { NativeWidget, UiCatalogue } from "./assets.ts";
import { widgetId, widgetKey } from "./layout.ts";
import { escapeText, sourceLines } from "./raster.ts";

export type UiDocument = NonNullable<GameplayUiView["document"]>;
export interface DocumentProjection {
  widgets: NativeWidget[]; group: 392 | 615; part: number; parts: number; previous: boolean; next: boolean;
}
const BOOK_LINES = [...Array.from({ length: 15 }, (_, index) => 44 + index), ...Array.from({ length: 15 }, (_, index) => 60 + index)];

function bookLines(text: string, width: number, catalogue: UiCatalogue, font: number): string[] {
  const active = new Map<string, string>();
  return sourceLines(text, width, catalogue.fonts[font]!).map(line => {
    const prefix = [...active.values()].join("");
    for (const tag of line.match(/<\/?(?:col|shad|str|u)(?:=[0-9a-f]{1,6})?>/gi) ?? []) {
      const key = tag.replace(/[</>]/g, "").split("=")[0]!.toLowerCase();
      if (tag.startsWith("</")) active.delete(key); else active.set(key, tag);
    }
    return prefix + line;
  });
}

export function documentSource(catalogue: UiCatalogue, document: UiDocument): 392 | 615 | null {
  const ids = catalogue.presentation?.interfaces[document.interface]?.sourceIds;
  return document.nativeMap ? ids?.includes(615) ? 615 : null : ids?.includes(392) ? 392 : null;
}

/** Original source2043, including its underground coordinate adjustment and inclusive map bounds. */
export function newcomerMapMarker(tile: Tile): { x: number; y: number } | null {
  const sourceY = tile.x >= 1024 && tile.x <= 3583 && tile.y >= 8960 && tile.y <= 10367 ? tile.y - 6400 : tile.y;
  const x = tile.x - 2912, y = sourceY - 3136;
  if (x < 0 || x > 384 || y < 0 || y > 384) return null;
  const left = 75 + Math.trunc(y * 85 / 384), top = 235 - Math.trunc(y * 188 / 384);
  return {
    x: left + Math.trunc(x * (295 - Math.trunc(y * 53 / 384)) / 384),
    y: top + Math.trunc(x * (70 - Math.trunc(y * 20 / 384)) / 384),
  };
}

export function projectDocument(catalogue: UiCatalogue, document: UiDocument, requestedPart: number,
  tutors: boolean, player: Tile | null): DocumentProjection {
  const group = documentSource(catalogue, document);
  if (group === null) throw new Error(`No original document layout is bound to ${document.interface}.`);
  if (group === 615) {
    const source = catalogue.templates[tutors ? "ui4-map-tutors-shown" : "ui4-map-tutors-hidden"];
    if (!source || !catalogue.documentMarker) throw new Error("Original newcomer-map artwork or marker is missing.");
    const widgets = source.map(widget => ({ ...widget }));
    const location = player && newcomerMapMarker(player);
    if (location) {
      const root = widgets.find(widget => widget.id === widgetId(615, 0) && widget.index === -1)!;
      const marker = catalogue.documentMarker.widget;
      const x = location.x - Math.trunc(marker.width / 2), y = location.y - Math.trunc(marker.height / 2);
      widgets.push({ ...marker, x: root.x + x, y: root.y + y, originalX: x, originalY: y, xMode: 0, yMode: 0 });
    }
    return { widgets, group, part: 0, parts: 1, previous: false, next: false };
  }
  const first = catalogue.templates["ui4-book-first"], last = catalogue.templates["ui4-book-last"];
  if (!first || !last) throw new Error("Original facing-page book artwork is missing.");
  const union = new Map(first.map(widget => [widgetKey(widget), { ...widget }]));
  for (const widget of last) if (widget.id === widgetId(392, 75) || widget.id === widgetId(392, 76))
    union.set(widgetKey(widget), { ...widget });
  const line = first.find(widget => widget.id === widgetId(392, 44))!;
  const pages = document.pages.map(page => bookLines(page, line.width, catalogue, line.font));
  const current = pages[document.page];
  if (!current) throw new Error("The authoritative book page is outside its supplied page list.");
  const parts = Math.max(1, Math.ceil(current.length / BOOK_LINES.length));
  const part = Math.max(0, Math.min(requestedPart, parts - 1));
  const before = pages.slice(0, document.page).reduce((count, page) => count + Math.max(1, Math.ceil(page.length / BOOK_LINES.length)), 0);
  const previous = document.page > 0 || part > 0, next = document.page + 1 < document.pages.length || part + 1 < parts;
  const widgets = [...union.values()].filter(widget =>
    previous || ![widgetId(392, 75), widgetId(392, 76)].includes(widget.id)).filter(widget =>
    next || ![widgetId(392, 77), widgetId(392, 78)].includes(widget.id));
  for (const widget of widgets) {
    if (widget.id >> 16 !== 392) continue;
    const child = widget.id & 65535;
    if (child === 6) widget.text = escapeText(document.title);
    if (child === 9 || child === 10) widget.text = String((before + part) * 2 + (child === 9 ? 1 : 2));
    const index = BOOK_LINES.indexOf(child);
    if (index >= 0) widget.text = current[part * BOOK_LINES.length + index] ?? "";
  }
  return { widgets, group, part, parts, previous, next };
}
