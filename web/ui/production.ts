import type { GameplayUiView } from "../shared/contracts.ts";
import type { NativeWidget, UiCatalogue } from "./assets.ts";
import { widgetId, projectScrollbar } from "./layout.ts";
import { escapeText } from "./raster.ts";

export type ProductionView = NonNullable<GameplayUiView["production"]>;
export type ProductionAmount = number | "x";
export interface ProductionProjection {
  widgets: NativeWidget[];
  choices: Map<number, ProductionView["recipes"][number]>;
  problems: string[];
}

export function productionSource(catalogue: UiCatalogue, view: ProductionView): 270 | 312 | null {
  const sources = catalogue.presentation?.interfaces[view.interface]?.sourceIds ?? [];
  return sources.includes(312) ? 312 : sources.includes(270) ? 270 : null;
}

export function projectProduction(catalogue: UiCatalogue, view: ProductionView, amount: ProductionAmount,
  scroll: number, hovered: string | null): ProductionProjection {
  const group = productionSource(catalogue, view), problems: string[] = [];
  const choices = new Map<number, ProductionView["recipes"][number]>();
  const name = group === 312 ? "native-smithing-bronze" : `native-production-choice-${Math.max(1, view.recipes.length)}`;
  const source = catalogue.templates[name];
  if (!group || !source) return { widgets: [], choices, problems: [`No calibrated original production layout for ${view.interface} with ${view.recipes.length} choices.`] };
  let widgets = source.map(widget => ({ ...widget }));
  if (group === 312) {
    for (const recipe of view.recipes) {
      const item = recipe.outputs[0];
      const icon = item && widgets.find(widget => widget.id >> 16 === group && widget.type === 5 && widget.item === item.sourceId);
      if (!icon) {
        problems.push(`The source smithing row for ${recipe.name} is not in the calibrated bronze table.`);
        continue;
      }
      choices.set(icon.id, recipe);
      icon.item_quantity = item.quantity;
      const label = widgets.find(widget => widget.id === icon.id && widget.type === 4 && widget.index === 1);
      if (label) { label.color = recipe.single.allowed || recipe.makeX.allowed ? 0xffffff : 0; label.shadow = label.color !== 0; }
    }
    const quantity = widgets.find(widget => widget.id === widgetId(312, 7) && widget.type === 4);
    if (quantity) quantity.text = `<col=ffffff>${amount === "x" ? "X" : amount}</col>`;
    return { widgets, choices, problems };
  }
  for (let index = 0; index < view.recipes.length; index++) {
    const recipe = view.recipes[index]!, id = widgetId(group, 15 + index), output = recipe.outputs[0];
    choices.set(id, recipe);
    const parent = widgets.find(widget => widget.id === id && widget.index === -1)!;
    parent.name = `<col=ff9040>${escapeText(recipe.name)}</col>`;
    const icon = widgets.find(widget => widget.id === id && widget.type === 6);
    if (icon && output) {
      const model = Object.values(catalogue.staticModels).find(model =>
        model.widget.id === id && model.widget.item === output.sourceId && model.widget.width === icon.width && model.widget.height === icon.height);
      if (model) {
        const { modelType, model: id, item, item_quantity, modelZoom, modelRotation } = model.widget;
        Object.assign(icon, { modelType, model: id, item, item_quantity, modelZoom, modelRotation });
      }
      else problems.push(`The native ${icon.width}x${icon.height} production icon for ${output.name} is unavailable.`);
      icon.x = parent.x + Math.trunc((parent.width - icon.width) / 2);
      icon.y = parent.y + Math.trunc((parent.height - icon.height) / 2);
    } else problems.push(`The projection has no source-bound primary output icon for ${recipe.name}.`);
    if (hovered === recipe.recipe) {
      const hover = catalogue.templates["native-production-hover"]!;
      const first = hover.find(widget => widget.id === widgetId(group, 15) && widget.index === -1)!;
      const replacement = hover.filter(widget => widget.id === first.id && [3, 5].includes(widget.type)).map(widget => ({
        ...widget, id, parent: id, x: parent.x + widget.x - first.x, y: parent.y + widget.y - first.y,
      }));
      if (parent.width === first.width && parent.height === first.height) {
        widgets = widgets.filter(widget => !(widget.id === id && [3, 5].includes(widget.type)));
        widgets.push(...replacement);
      }
    }
  }
  if (view.recipes.length === 0) {
    widgets = widgets.filter(widget => !(widget.id === widgetId(group, 14) && widget.index >= 0) && !(widget.id === widgetId(group, 15)));
    const heading = widgets.find(widget => widget.id === widgetId(group, 4))!;
    heading.text = "The server has supplied no production choices.";
  }
  const chosen = amount === "x" ? 11 : amount === 1 ? 7 : amount === 5 ? 8 : amount === 10 ? 9 : 11;
  for (const widget of widgets) {
    const child = widget.id & 65535;
    if (widget.id >> 16 !== group || ![7, 8, 9, 11, 12].includes(child) || widget.index < 0) continue;
    const skinChild = child === chosen ? 7 : 8;
    const prototype = source.find(row => row.id === widgetId(group, skinChild) && row.index === widget.index);
    if (prototype && [3, 5].includes(widget.type)) {
      widget.type = prototype.type; widget.sprite = prototype.sprite; widget.color = prototype.color;
      widget.filled = prototype.filled; widget.opacity = prototype.opacity;
    }
    if (widget.type === 4) {
      const label = child === 7 ? "1" : child === 8 ? "5" : child === 9 ? "10" : child === 12 ? "All" : "X";
      widget.text = child === chosen ? `<col=ffffff>${label}</col>` : label;
    }
  }
  const viewport = widgets.find(widget => widget.id === widgetId(group, 13) && widget.index === -1)!;
  const content = widgets.find(widget => widget.id === widgetId(group, 14) && widget.index === -1)!;
  const position = Math.min(Math.max(0, scroll), Math.max(0, content.height - viewport.height));
  const descendants = new Set([content.id, ...choices.keys()]);
  for (const widget of widgets) if (descendants.has(widget.id)) widget.y -= position;
  projectScrollbar(widgets, widgetId(group, 33), viewport.id, content.height, position);
  return { widgets: widgets.sort((a, b) => a.id - b.id || a.index - b.index), choices, problems };
}

export function productionChoiceLabel(widgets: readonly NativeWidget[], id: number): string {
  const widget = widgets.find(widget => widget.id === id && widget.index === -1);
  return widget?.actions?.find(Boolean) ?? "Make";
}
