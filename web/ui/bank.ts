import type { GameplayUiView } from "../shared/contracts.ts";
import type { NativeWidget, UiCatalogue } from "./assets.ts";
import { projectScrollbar, widgetId } from "./layout.ts";
import { bankDefaultAmount } from "./gameplay-ui.ts";

export type BankUiView = NonNullable<GameplayUiView["bank"]>;
export interface BankProjection {
  widgets: NativeWidget[];
  entries: Map<number, BankUiView["entries"][number]>;
  tabs: Map<number, BankUiView["tabs"][number]>;
  dropTabs: Map<number, number>;
  scroll: number;
}

/** Original Classic bank tab/section geometry. Entry ownership and amounts remain authoritative. */
export function projectBank(catalogue: UiCatalogue, view: BankUiView, search: string, requestedScroll: number): BankProjection {
  const source = catalogue.templates["bounded-bank-tabs"];
  if (!source) throw new Error("Native multi-tab bank geometry is missing.");
  const entries = new Map<number, BankUiView["entries"][number]>(), tabs = new Map<number, BankUiView["tabs"][number]>(), dropTabs = new Map<number, number>();
  const item = source.find(widget => widget.id === widgetId(12, 12) && widget.index === 8)!;
  const line = source.find(widget => widget.id === item.id && widget.sprite === 897)!;
  const drop = source.find(widget => widget.id === item.id && widget.type === 3)!;
  const content = source.find(widget => widget.id === item.id && widget.index === -1)!;
  const tabBackground = source.find(widget => widget.id === widgetId(12, 10) && widget.index === 0)!;
  const allIcon = source.find(widget => widget.id === widgetId(12, 10) && widget.index === 10)!;
  const itemIcon = source.find(widget => widget.id === widgetId(12, 10) && widget.index === 11)!;
  const newIcon = source.find(widget => widget.id === widgetId(12, 10) && widget.sprite === 1082)!;
  const widgets = source.filter(widget => !((widget.id === item.id || widget.id === tabBackground.id) && widget.index >= 0))
    .map(widget => ({ ...widget }));
  const orderedTabs = [...view.tabs].sort((a, b) => a.tab - b.tab);
  orderedTabs.forEach((tab, index) => {
    widgets.push({ ...tabBackground, index: 2000 + tab.tab, x: tabBackground.x + index * 40,
      sprite: tab.tab === view.selectedTab ? 1079 : 1077 });
    const first = view.entries.find(entry => entry.id === tab.firstEntry);
    const sourceId = first?.value?.sourceId ?? (first ? catalogue.presentation?.sourceItems[first.item] : undefined) ?? -1;
    const itemId = first?.placeholder ? catalogue.items[sourceId]?.placeholderId ?? -1 : sourceId;
    const icon = tab.tab === 0 ? allIcon : itemIcon;
    widgets.push({ ...icon, index: tab.tab, x: tabBackground.x + index * 40 + (tab.tab === 0 ? 2 : 3),
      item: tab.tab === 0 ? -1 : itemId, item_quantity: first?.value?.quantity ?? 0, quantityMode: 0,
      opacity: tab.tab === 0 ? allIcon.opacity : first?.placeholder ? 120 : 0 });
    tabs.set(tab.tab, tab);
  });
  if (orderedTabs.length < 10) {
    widgets.push({ ...tabBackground, index: 3000, x: tabBackground.x + orderedTabs.length * 40, sprite: 1080 });
    widgets.push({ ...newIcon, index: 1000, x: tabBackground.x + orderedTabs.length * 40 + 2 });
  }
  const groups = view.selectedTab === 0 && !search ? orderedTabs.map(tab => tab.tab) : [view.selectedTab];
  let y = content.y, count = 0, decoration = 10000;
  for (const tab of groups) {
    const rows = [...view.entries].filter(entry => (view.selectedTab === 0 && search || entry.tab === tab) &&
      (entry.value?.name ?? catalogue.items[catalogue.presentation?.sourceItems[entry.item] ?? -1]?.name ?? entry.item)
        .toLocaleLowerCase().includes(search.toLocaleLowerCase())).sort((a, b) => a.slot - b.slot);
    if (!rows.length) continue;
    if (count++) {
      y += 12;
      widgets.push({ ...line, index: decoration++, y: y - 7 });
    }
    rows.forEach((entry, index) => {
      const sourceId = entry.value?.sourceId ?? catalogue.presentation?.sourceItems[entry.item] ?? -1;
      const graphic = entry.placeholder ? catalogue.items[sourceId]?.placeholderId ?? -1 : sourceId;
      widgets.push({ ...item, index: entry.slot, x: item.x + index % 8 * 48, y: y + Math.floor(index / 8) * 36,
        item: graphic, item_quantity: entry.value?.quantity ?? 0, name: entry.value?.name ?? entry.item,
        opacity: entry.placeholder ? 120 : 0,
        actions: entry.placeholder ? ["Release", "Examine"] : item.actions });
      entries.set(entry.slot, entry);
    });
    if (view.selectedTab === 0 && !search && rows.length % 8) {
      const index = decoration++;
      widgets.push({ ...drop, index, x: item.x + rows.length % 8 * 48,
        y: y + Math.floor(rows.length / 8) * 36 + 4, width: (8 - rows.length % 8) * 48 - 12 });
      dropTabs.set(index, tab);
    }
    y += Math.ceil(rows.length / 8) * 36;
  }
  const extent = Math.max(0, y - content.y);
  const scroll = Math.min(Math.max(0, requestedScroll), Math.max(0, extent - content.height));
  for (const widget of widgets) {
    if (widget.id === item.id && widget.index >= 0) widget.y -= scroll;
    if (widget.id >> 16 !== 12) continue;
    const child = widget.id & 65535;
    if (child === 3) widget.text = view.selectedTab === 0 ? "The Bank of Gielinor" : `Tab ${view.selectedTab}`;
    if (child === 5) widget.text = String(view.entries.length);
    if (child === 8) widget.text = "";
    if (child === 23) widget.sprite = 170;
    if (child === 24) widget.sprite = view.insertMode ? 2820 : 2821;
    if (child === 25) widget.sprite = view.noted ? 179 : 170;
    if (child === 40) widget.sprite = view.placeholders ? 179 : 170;
    if ([29, 31, 33, 35, 37].includes(child)) {
      const amount = bankDefaultAmount(view);
      const selected = child === 29 ? amount === 1 : child === 31 ? amount === 5 : child === 33 ? amount === 10
        : child === 37 ? amount === "all" : typeof amount === "number" && ![1, 5, 10].includes(amount);
      widget.sprite = selected ? 179 : 170;
    }
  }
  projectScrollbar(widgets, widgetId(12, 13), item.id, extent, scroll);
  return { widgets, entries, tabs, dropTabs, scroll };
}
