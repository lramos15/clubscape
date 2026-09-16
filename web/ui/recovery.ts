import type { ItemView } from "../shared/contracts.ts";
import type { NativeWidget, UiCatalogue } from "./assets.ts";
import type { Control } from "./input.ts";
import { intersect } from "./assets.ts";
import { nativeTree, projectScrollbar, widgetId, widgetKey } from "./layout.ts";
import { escapeText } from "./raster.ts";

export interface RecoveryDisplayItem {
  id: string; item: ItemView; slot: number; allowed: boolean | null; reason: string | null;
  unitFee?: string; fullStackFee?: string; inventoryCapacity?: number; bankCapacity?: number;
}
/** Explicit display inputs: null is unknown, never a fabricated zero balance or fee. */
export interface RecoveryDisplay {
  storage: "grave" | "death_office";
  items: readonly RecoveryDisplayItem[];
  selectedId: string | null;
  coffer: string | null;
  unitFee: number | null;
  capacity: number | null;
  bankAll: boolean;
  bankAllReason?: string;
  takeAll?: boolean;
  takeAllReason?: string;
  fullSelectionFee?: string;
  discardAll: boolean;
  discardReason?: string;
  scroll: number;
}
export type RecoveryUiCommand =
  | { kind: "select"; id: string }
  | { kind: "retrieve"; id: string; amount: 1 | 5 | "x" | "all" }
  | { kind: "take_all" | "bank_all" | "discard_all" | "close" }
  | { kind: "examine"; id: string };

export function recoveryTemplate(catalogue: UiCatalogue, view: RecoveryDisplay): NativeWidget[] {
  const key = view.storage === "grave" ? view.bankAll ? "native-retrieval-602-35-0-1" : "native-retrieval-602-34-0-0"
    : view.selectedId === null ? "native-retrieval-669-12345--1-0" : "native-retrieval-669-12345-7-42";
  const source = catalogue.templates[key];
  if (!source) throw new Error(`Original populated retrieval layout is missing: ${key}`);
  return source.map(widget => ({ ...widget }));
}

export function recoveryFeeText(view: RecoveryDisplay): string {
  const selected = view.items.find(row => row.id === view.selectedId);
  const coffer = view.coffer === null ? "Unavailable" : BigInt(view.coffer).toLocaleString("en-US");
  if (!selected) return `Select an item to retrieve.<br>Death's Coffer: <col=ffffff>${coffer}</col>`;
  // Source3492 uses INV_TOTAL for the selected item type, while the outline remains on its selected slot.
  const quantity = selected.unitFee === undefined ? view.items.filter(row => row.item.id === selected.item.id)
    .reduce((total, row) => total + row.item.quantity, 0) : selected.item.quantity;
  const name = escapeText(selected.item.name);
  const fee = selected.unitFee !== undefined ? BigInt(selected.unitFee) : view.unitFee === null ? null : BigInt(view.unitFee);
  let feeText = fee === null ? "Fee: unavailable" : `Fee: <col=ffffff>${fee.toLocaleString("en-US")} ${fee === 1n ? "coin" : "coins"}</col>`;
  if (quantity > 1 && fee !== null) {
    feeText += " each";
    if (fee !== 1n && (selected.fullStackFee !== undefined || fee === 0n || BigInt(quantity) <= 2147483647n / fee)) {
      const total = selected.fullStackFee !== undefined ? BigInt(selected.fullStackFee) : BigInt(quantity) * fee;
      feeText += ` (<col=ffffff>${total.toLocaleString("en-US")}</col>)`;
    }
  }
  return `${quantity > 1 ? quantity.toLocaleString("en-US") + " x " : ""}${name}:<br>${feeText}<br>Death's Coffer: <col=ffffff>${coffer}${view.coffer === null ? "" : " coins"}</col>`;
}

export function projectRecovery(widgets: NativeWidget[], view: RecoveryDisplay): NativeWidget[] {
  const group = view.storage === "grave" ? 602 : 669;
  const grid = widgets.find(widget => widget.id === widgetId(group, 3) && widget.index === -1)!;
  const prototype = widgets.find(widget => widget.id === grid.id && widget.index >= 0 && widget.item >= 0)!;
  if (!grid || !prototype) throw new Error("Native populated retrieval item geometry is missing.");
  const columns = view.storage === "grave" ? 8 : 9, pitchX = view.storage === "grave" ? 46 : 50;
  const pitchY = view.storage === "grave" ? 40 : 42;
  const firstX = view.storage === "grave" ? 1 : 2, firstY = view.storage === "grave" ? 3 : 0;
  const extent = view.items.length ? firstY + Math.floor(Math.max(...view.items.map(row => row.slot)) / columns) * pitchY + prototype.height : 0;
  const scroll = Math.min(Math.max(0, Math.floor(view.scroll)), Math.max(0, extent - grid.height));
  const output = widgets.filter(widget => !(widget.id === grid.id && widget.index >= 0));
  for (const row of view.items) {
    output.push({ ...prototype, index: row.slot, x: grid.x + firstX + row.slot % columns * pitchX,
      y: grid.y + firstY + Math.floor(row.slot / columns) * pitchY - scroll,
      item: row.item.sourceId ?? -1, item_quantity: row.item.quantity, name: `<col=ff9040>${escapeText(row.item.name)}</col>`,
      border: row.id === view.selectedId && view.storage === "death_office" ? 2 : 1 });
  }
  for (const widget of output) {
    if (widget.id >> 16 !== group) continue;
    const child = widget.id & 65535;
    if (child === 1 && widget.index === 1) widget.text = view.storage === "grave" ? "Gravestone"
      : `Death's Office Item Retrieval <col=ffb83f>(${view.items.length}/${view.capacity === null ? "?" : view.capacity})</col>`;
    if (group === 669 && child === 11) widget.text = recoveryFeeText(view);
  }
  projectScrollbar(output, widgetId(group, 4), grid.id, extent, scroll);
  return output;
}

export function recoveryControls(widgets: readonly NativeWidget[], width: number, height: number,
  view: RecoveryDisplay, dispatch: (command: RecoveryUiCommand) => void): Control[] {
  const group = view.storage === "grave" ? 602 : 669, controls: Control[] = [];
  const selected = view.items.find(row => row.id === view.selectedId);
  for (const widget of nativeTree(widgets, width, height)) {
    if (widget.id >> 16 !== group) continue;
    const rect = intersect(widget, widget.clip);
    if (rect.width <= 0 || rect.height <= 0) continue;
    const child = widget.id & 65535;
    if (child === 3 && widget.index >= 0) {
      const row = view.items.find(row => row.slot === widget.index);
      if (!row) continue;
      const select = view.storage === "death_office";
      const label = `${select ? "Select" : "Take-All"} ${row.item.name}`;
      controls.push({ ...rect, id: `recovery-item-${row.id}`, label, pressed: row.id === view.selectedId,
        ...(!select && row.allowed === false ? { disabled: row.reason ?? "This item is currently unavailable." } : {}),
        ...(select && row.allowed === false ? { tooltip: row.reason ?? "This item is currently unavailable." } : {}),
        ...(row.allowed === null ? { tooltip: "Retrieval permission and affordability are not projected; the server validates this request." } : {}),
        actions: [{ label, run: () => dispatch(select ? { kind: "select", id: row.id } : { kind: "retrieve", id: row.id, amount: "all" }) },
          { label: `Examine ${row.item.name}`, run: () => dispatch({ kind: "examine", id: row.id }) }] });
      continue;
    }
    const operation = widget.actions?.find(Boolean);
    if (!operation) continue;
    let command: RecoveryUiCommand | null = null;
    if (operation === "Close") command = { kind: "close" };
    else if (operation === "Take-All") command = { kind: "take_all" };
    else if (operation === "Bank-All") command = { kind: "bank_all" };
    else if (operation === "Discard-All") command = { kind: "discard_all" };
    else if (group === 669 && [6, 7, 8, 9].includes(child) && selected)
      command = { kind: "retrieve", id: selected.id, amount: child === 6 ? 1 : child === 7 ? 5 : child === 8 ? "x" : "all" };
    if (!command) continue;
    const disabled = command.kind === "discard_all" && !view.discardAll ? view.discardReason ?? "Discard permission has not been supplied."
      : command.kind === "bank_all" && !view.bankAll ? view.bankAllReason ?? "Bank-All permission has not been supplied."
      : command.kind === "take_all" && view.takeAll === false ? view.takeAllReason ?? "Take-All is not currently permitted."
      : command.kind === "retrieve" && selected?.allowed === false ? selected.reason ?? "This item is currently unavailable."
      : command.kind !== "close" && view.items.length === 0 ? "There are no items to retrieve." : undefined;
    controls.push({ ...rect, id: `recovery-option-${widgetKey(widget)}`, label: command.kind === "retrieve" ? `Retrieve ${operation}` : operation,
      ...(disabled ? { disabled } : {}), actions: [{ label: operation, run: () => dispatch(command!) }] });
  }
  return controls;
}
