import type { GameplayUiIntent, GameplayUiView } from "../shared/contracts.ts";
import type { Control } from "./input.ts";
import { SourceRaster, escapeText } from "./raster.ts";
import { formatUiInteger } from "./gameplay-ui.ts";
import { entryErrorLines } from "./entry.ts";

/** Source chat-modal language for the opaque, server-owned confirmation presentation. */
export function paintConfirmation(raster: SourceRaster, view: NonNullable<GameplayUiView["confirmation"]>,
  page: number, controls: Control[], choose: (intent: GameplayUiIntent) => void, advance: () => void): void {
  const y = raster.canvas.height - 165;
  raster.sprite(1017, 0, y);
  raster.center(escapeText(view.title), 259, y + 24, 496, 0x800000, null);
  const details = [
    ...view.lines,
    ...view.items.map(item => `${item.quantity.toLocaleString("en-US")} x ${item.name}`),
    ...(view.credit === null ? [] : [`Credit: ${formatUiInteger(view.credit)}`]),
  ];
  const lines = details.flatMap(line => entryErrorLines(line, 472, raster.assets.catalogue.fonts[495]!));
  const pageSize = 4;
  page = Math.min(page, Math.max(0, Math.ceil(lines.length / pageSize) - 1));
  const more = (page + 1) * pageSize < lines.length;
  raster.textBox(lines.slice(page * pageSize, (page + 1) * pageSize).join("<br>"),
    { x: 20, y: y + 30, width: 472, height: 66 }, { font: 495, color: 0, shadow: null, lineHeight: 16, xAlign: 1, yAlign: 1 });
  const yes = more ? "Continue" : "Confirm";
  raster.center(yes, 139, y + 121, 495, 0x0000ff, null);
  raster.center("Cancel", 379, y + 121, 495, 0x0000ff, null);
  controls.push({ x: 18, y: y + 102, width: 240, height: 27, id: "ui-confirm-accept", label: yes,
    actions: [{ label: yes, run: more ? advance : () => choose({ kind: "ui_confirm", confirmation_id: view.id, accept: true }) }] },
  { x: 259, y: y + 102, width: 240, height: 27, id: "ui-confirm-cancel", label: "Cancel confirmation",
    actions: [{ label: "Cancel", run: () => choose({ kind: "ui_confirm", confirmation_id: view.id, accept: false }) }] });
}
