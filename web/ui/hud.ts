import type { GameplayUiView } from "../shared/contracts.ts";
import type { NativeWidget, UiCatalogue } from "./assets.ts";
import { TABS, tabWidget, widgetId } from "./layout.ts";

export function projectHud(widgets: readonly NativeWidget[], catalogue: UiCatalogue,
  states: readonly GameplayUiView["interfaces"][number][], activeTab: number): NativeWidget[] {
  const source = widgets.map(widget => ({ ...widget }));
  const hidden = new Set<number>();
  for (let index = 0; index < TABS.length; index++) {
    const state = states.find(state => state.interface === TABS[index]!.interface);
    const frame = source.find(widget => widget.id === tabWidget(index) && widget.index === -1);
    const icon = widgetId(161, index < 7 ? 66 + index : 50 + index - 7);
    if (!state || state.visibility !== "enabled") {
      hidden.add(icon);
      if (state?.visibility === "hidden") hidden.add(tabWidget(index));
    }
    if (frame) {
      const selected = catalogue.templates[TABS[index]!.template]?.find(widget => widget.id === frame.id && widget.index === -1);
      frame.sprite = index === activeTab && state?.visibility === "enabled" ? selected?.sprite ?? frame.sprite : -1;
    }
  }
  const output = source.filter(widget => !hidden.has(widget.id) && !(widget.id === widgetId(161, 98) && widget.index >= 0));
  const highlight = catalogue.templates["bounded-hud-highlight"]!.filter(widget => widget.id === widgetId(161, 98) && widget.type === 3);
  for (let index = 0; index < TABS.length; index++) {
    const state = states.find(state => state.interface === TABS[index]!.interface);
    const frame = source.find(widget => widget.id === tabWidget(index) && widget.index === -1);
    if (!state?.highlighted || state.visibility === "hidden" || !frame) continue;
    highlight.forEach((prototype, border) => {
      const inset = border ? -1 : -2, x = frame.x + inset, y = frame.y + inset;
      const width = frame.width - inset * 2, height = frame.height - inset * 2;
      output.push({ ...prototype, index: 2000 + index * 2 + border, x, y, width, height,
        originalX: 1920 - x - width, originalY: 1080 - y - height, originalWidth: width, originalHeight: height,
        xMode: 2, yMode: 2, widthMode: 0, heightMode: 0 });
    });
  }
  return output;
}
