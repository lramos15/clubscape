import type { NativeWidget, UiCatalogue } from "./assets.ts";
import { widgetId } from "./layout.ts";

export type AbilityKind = "prayer" | "magic";
export interface AbilityMetadata {
  widget: number; sourceItem: number; kind: AbilityKind; level: number; category: number;
  rapid: boolean; higherTier: number; tier: number; sprites: number[];
  runes: Array<{ sourceId: number; quantity: number }>;
}
/** Presentation facts only. Unknown requirements remain visible rather than being invented. */
export interface AbilityVisualTruth {
  level?: boolean | null; resources?: boolean | null; requirements?: boolean | null;
  lowerTier?: boolean | null; supersededByMultiSkill?: boolean | null;
}
export const FILTER_OPTIONS = {
  prayer: [
    "Show lower tiers of tiered prayers",
    "Show tiered prayers even if multi-skill prayers are available",
    "Show Rapid Healing prayers",
    "Show prayers you lack the Prayer level to activate",
    "Show prayers you lack the requirements to activate",
  ],
  magic: [
    "Show Combat spells", "Show Teleport spells", "Show Utility spells",
    "Show spells you lack the Magic level to cast", "Show spells you lack the runes to cast",
    "Show spells you lack the requirements to cast", "Enable icon resizing",
  ],
} as const;

export function filterOptionEnabled(kind: AbilityKind, option: number, mask: number): boolean {
  return !(kind === "prayer" && option === 1 && !(mask & 1));
}

export function abilityVisible(kind: AbilityKind, mask: number, metadata: AbilityMetadata | undefined,
  truth: AbilityVisualTruth = {}): boolean {
  if (kind === "magic") {
    const categoryBit = metadata?.category === 0 ? 0 : metadata?.category === 2 ? 1 : metadata?.category === 1 ? 2 : -1;
    if (categoryBit >= 0 && mask & 1 << categoryBit) return false;
    if (mask & 8 && truth.level === false) return false;
    if (mask & 16 && truth.resources === false) return false;
    if (mask & 32 && truth.requirements === false) return false;
  } else {
    if (mask & 1 && truth.lowerTier === true) return false;
    if (mask & 1 && mask & 2 && truth.supersededByMultiSkill === true) return false;
    if (mask & 4 && metadata?.rapid) return false;
    if (mask & 8 && truth.level === false) return false;
    if (mask & 16 && truth.requirements === false) return false;
  }
  return true;
}

/** Source script2611, Classic branch: integer cell sizing, gaps and centered container. */
export function spellGrid(count: number, width: number, height: number, resize: boolean): {
  size: number; columns: number; rows: number; gapX: number; gapY: number; width: number; height: number;
} {
  const size = resize && count <= 20 ? 40 : 24;
  const columns = resize && count <= 15 ? 3 : count <= (resize ? 20 : 28) ? 4
    : Math.max(4, Math.min(7, Math.trunc((count + 8) / 9)));
  const gapX = Math.max(0, Math.min(resize ? Math.trunc(size * 5 / 7) : size,
    Math.trunc((width - size * columns) / (columns - 1))));
  const rows = Math.max(1, Math.ceil(count / columns));
  const gapY = rows < 2 ? 0 : Math.max(0, Math.min(gapX, Math.trunc((height - size * rows) / (rows - 1))));
  const resultHeight = rows * size + (rows - 1) * gapY;
  return { size, columns, rows, gapX, gapY, width: columns * size + (columns - 1) * gapX,
    height: resize ? resultHeight : Math.max(resultHeight, height - 30) };
}

export function projectFilterPanel(widgets: NativeWidget[], kind: AbilityKind, mask: number): void {
  const group = kind === "prayer" ? 541 : 218, container = widgetId(group, kind === "prayer" ? 42 : 206);
  const firstCheckbox = kind === "prayer" ? 5 : 7;
  widgets.forEach(widget => {
    if (widget.id !== container || widget.index < firstCheckbox) return;
    const option = Math.floor((widget.index - firstCheckbox) / 2);
    if (option >= FILTER_OPTIONS[kind].length) return;
    const enabled = filterOptionEnabled(kind, option, mask);
    if ((widget.index - firstCheckbox) % 2 === 0)
      widget.sprite = enabled ? mask & 1 << option ? 8383 : 8384 : 8380;
    else widget.color = 0xff981f;
  });
}

export function projectAbilityGrid(widgets: NativeWidget[], catalogue: UiCatalogue, kind: AbilityKind, mask: number,
  truths: Readonly<Record<string, AbilityVisualTruth>>): NativeWidget[] {
  const group = kind === "prayer" ? 541 : 218;
  const entries = widgets.filter(w => w.id >> 16 === group && w.index === -1 && w.name)
    .sort((a, b) => a.y - b.y || a.x - b.x);
  const visible = entries.filter(entry => abilityVisible(kind, mask, catalogue.abilities[entry.id], truths[entry.id]));
  const visibleIds = new Set(visible.map(entry => entry.id)), allIds = new Set(entries.map(entry => entry.id));
  const result = widgets.filter(w => !allIds.has(w.id) || visibleIds.has(w.id));
  if (!mask) return result;
  if (kind === "prayer") {
    const root = result.find(w => w.id === widgetId(541, 3) && w.index === -1)!;
    visible.forEach((entry, index) => {
      const x = root.x + index % 5 * 37, y = root.y + Math.floor(index / 5) * 37;
      const dx = x - entry.x, dy = y - entry.y;
      result.filter(w => w.id === entry.id).forEach(w => { w.x += dx; w.y += dy; });
    });
  } else {
    const parent = result.find(w => w.id === widgetId(218, 1) && w.index === -1)!;
    const root = result.find(w => w.id === widgetId(218, 3) && w.index === -1)!;
    if (!visible.length) {
      const native = catalogue.templates["native-magic-mask-127"]?.find(w =>
        w.id === root.id && w.index >= 0 && w.text === "No spells match your selected filters.");
      if (native) result.push({ ...native });
      return result;
    }
    const grid = spellGrid(visible.length, parent.width, parent.height, !(mask & 64));
    root.x = parent.x + Math.trunc((parent.width - grid.width) / 2);
    root.y = parent.y + Math.trunc((parent.height - grid.height) / 2);
    root.width = root.originalWidth = grid.width; root.height = root.originalHeight = grid.height;
    root.originalX = root.originalY = 0; root.xMode = root.yMode = 1;
    visible.forEach((entry, index) => {
      entry.x = root.x + index % grid.columns * (grid.size + grid.gapX);
      entry.y = root.y + Math.floor(index / grid.columns) * (grid.size + grid.gapY);
      entry.width = entry.originalWidth = grid.size; entry.height = entry.originalHeight = grid.size;
      const meta = catalogue.abilities[entry.id], truth = truths[entry.id];
      if (meta) {
        const disabled = truth?.level === false || truth?.resources === false || truth?.requirements === false ||
          entry.sprite === meta.sprites[1] || entry.sprite === meta.sprites[3];
        const sprite = meta.sprites[(grid.size >= 40 ? 2 : 0) + (disabled ? 1 : 0)];
        if (sprite !== undefined && sprite >= 0) entry.sprite = sprite;
      }
    });
  }
  return result;
}
