import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { decodeUiCatalogue } from "../assets.ts";
import { currentSettingValues, defaultSettingsPage, projectAllSettings, settingsMatches } from "../settings.ts";
import { projectBank } from "../bank.ts";
import { projectHud } from "../hud.ts";
import { LEVEL_UP_SOURCE, projectLevelUpChat, projectLevelUpPopup } from "../level-up.ts";
import { TABS, nativeTree, tabWidget, widgetId } from "../layout.ts";
import { fixtureWorld, immutable } from "./component-fixture.ts";
import type { GameplayUiView } from "../../shared/contracts.ts";

const catalogue = decodeUiCatalogue(JSON.parse(readFileSync(resolve(import.meta.dirname, "../../../assets/compiled/ui/manifest.json"), "utf8")));
const row = (id: number) => catalogue.settingsRows[id]!;

test("independent original capture inventory remains bounded at exactly eighteen states", () => {
  const manifest = JSON.parse(readFileSync(resolve(import.meta.dirname, "../../../assets/compiled/ui/manifest.json"), "utf8"));
  assert.equal(manifest.boundedUiInputs.limit, 18);
  assert.equal(manifest.boundedUiInputs.states.length, 18);
  assert.equal(new Set(manifest.boundedUiInputs.states.map((state: { case: string }) => state.case)).size, 18);
  assert.equal(catalogue.tutorialStates.length, 71);
  assert.equal(catalogue.hudSignatures.length, 11);
});

test("All Settings rows preserve native struct IDs and exact source search aliases", () => {
  assert.equal(row(4503).params["1077"], 319);
  assert.equal(row(2734).params["1077"], 14);
  assert.equal(row(2734).label, "Camera zoom distance");
  assert.equal(settingsMatches(row(2734), "ZOOM"), true);
  assert.equal(settingsMatches(row(6389), "loud"), true);
  assert.ok(projectAllSettings(catalogue, { ...defaultSettingsPage(), search: "loud" }).rowIds.includes(4503));
  assert.equal(settingsMatches(row(2734), "no such setting"), false);
  for (const id of [6405, 6406, 6407]) {
    const projection = projectAllSettings(catalogue, { ...defaultSettingsPage(), search: row(id).label });
    const controls = [...projection.controls.values()].filter(control => control.row.id === id);
    assert.equal(controls.length, 1);
    assert.equal(controls[0]!.source.onOp![1], id);
    assert.ok(!row(id).widgets.some(widget => widget.onOp && widget.onOp[1] !== id));
  }
});

test("settings search compacts source row geometry without discarding required unknown controls", () => {
  const state = { ...defaultSettingsPage(), search: "zoom", moreInfo: false, hideLocked: true };
  const projection = projectAllSettings(catalogue, state);
  const zoom = projection.widgets.find(widget => widget.text === "Camera zoom distance")!;
  assert.equal(zoom.y, 372);
  assert.ok(projection.rowIds.includes(2734));
  const values = currentSettingValues(catalogue, fixtureWorld(), null, null, { singleMouse: false, shiftDrop: true, escapeCloses: true });
  const live = projectAllSettings(catalogue, state, values.values);
  assert.ok(live.rowIds.includes(2734));
  assert.ok(live.widgets.some(widget => widget.text === "Camera zoom distance - Unavailable"));
});

test("All Settings source-only slider values never become actual client or game preferences", () => {
  const world = immutable(fixtureWorld());
  const values = currentSettingValues(catalogue, world, null, null, { singleMouse: false, shiftDrop: true, escapeCloses: true });
  assert.equal(values.values.get(4503)!.value, null);
  assert.equal(values.values.get(2734)!.value, null);
  assert.equal(values.values.get(880)!.value, null);
  assert.equal(values.values.get(1104)!.value, true);
  assert.equal(values.values.get(2732)!.value, 1);
  const numeric = projectAllSettings(catalogue, { ...defaultSettingsPage(), search: row(2784).label }, values.values);
  assert.ok(numeric.widgets.some(widget => widget.text === "Unavailable"));
  assert.ok(!numeric.widgets.some(widget => widget.text === "0 coins"));
  const colours = projectAllSettings(catalogue, { ...defaultSettingsPage(), search: "Public chat" }, values.values);
  const swatches = colours.widgets.filter(widget => widget.type === 3 && widget.filled && widget.width === 50 && widget.height === 20);
  assert.ok(swatches.length > 0);
  assert.ok(swatches.every(widget => widget.color === 0x0e0e0c));
  assert.ok(row(2896).widgets.some(widget => widget.type === 3 && widget.filled && widget.color === 255));
});

function bank(): NonNullable<GameplayUiView["bank"]> {
  const items = fixtureWorld().player.inventory.filter(slot => slot.item).map(slot => slot.item!);
  return { revision: "9007199254740993", capacity: 400, selectedTab: 0, insertMode: true, placeholders: true, amount: 5, noted: true,
    tabs: [{ tab: 0, firstEntry: "main", entries: 1 }, { tab: 1, firstEntry: "tab", entries: 1 }],
    entries: [{ id: "main", slot: 8, tab: 0, item: items[1]!.id, value: items[1]!, placeholder: false },
      { id: "tab", slot: 0, tab: 1, item: items[0]!.id, value: items[0]!, placeholder: false }],
    depositEquipment: { allowed: true, code: null, reason: null }, unavailableContainers: [] };
}

test("bank source sections preserve stable slots and separate main/tab rows instead of flattening them", () => {
  const view = immutable(bank()), projection = projectBank(catalogue, view, "", 0);
  const main = projection.widgets.find(widget => widget.id === widgetId(12, 12) && widget.index === 8)!;
  const tab = projection.widgets.find(widget => widget.id === widgetId(12, 12) && widget.index === 0)!;
  assert.equal(main.y, 369); assert.equal(tab.y, 417);
  assert.equal(projection.widgets.find(widget => widget.id === widgetId(12, 23))!.sprite, 170);
  assert.equal(projection.widgets.find(widget => widget.id === widgetId(12, 24))!.sprite, 2820);
  assert.equal(view.revision, "9007199254740993");
});

test("bank placeholders use original linked IDs, source opacity120 and null ownership", () => {
  const view = bank();
  view.entries[1] = { ...view.entries[1]!, placeholder: true, value: null };
  view.selectedTab = 1;
  const projection = projectBank(catalogue, immutable(view), "", 0);
  const icon = projection.widgets.find(widget => widget.id === widgetId(12, 12) && widget.index === 0)!;
  assert.equal(icon.item, 14760);
  assert.equal(icon.item_quantity, 0);
  assert.equal(icon.opacity, 120);
  assert.equal(projection.entries.get(0)!.value, null);
});

test("HUD highlights use the actual two source borders and remain anchored while resizing", () => {
  const states: GameplayUiView["interfaces"] = TABS.map((tab, index) => ({
    interface: tab.interface, visibility: index === 7 ? "hidden" : "enabled", highlighted: index === 3,
    permission: { allowed: index !== 7, code: null, reason: null },
  }));
  const widgets = projectHud(catalogue.templates["native-settings"]!, catalogue, states, 11);
  assert.ok(!widgets.some(widget => widget.id === tabWidget(7)));
  for (const [width, height] of [[1024,768], [1920,1080], [2560,1440]] as const) {
    const tree = nativeTree(widgets, width, height);
    const frame = tree.find(widget => widget.id === tabWidget(3))!;
    const borders = tree.filter(widget => widget.id === widgetId(161, 98) && widget.type === 3);
    assert.equal(borders.length, 2);
    assert.equal(borders[0]!.x, frame.x - 2); assert.equal(borders[0]!.y, frame.y - 2);
    assert.equal(borders[0]!.width, frame.width + 4);
  }
});

test("level-up source layout exposes original233/162 associations without inventing canonical IDs or outcomes", () => {
  assert.equal(LEVEL_UP_SOURCE.popupGroup, 233);
  assert.equal(LEVEL_UP_SOURCE.parent, widgetId(162, 567));
  assert.equal(LEVEL_UP_SOURCE.font, 497);
  const widgets = projectLevelUpPopup(catalogue, { title: "Actual title", lines: ["Actual detail"], continuation: "Continue" });
  assert.equal(widgets.find(widget => widget.id === LEVEL_UP_SOURCE.title)!.text, "Actual title");
  assert.equal(widgets.find(widget => widget.id === LEVEL_UP_SOURCE.detail)!.text, "Actual detail");
  assert.ok(projectLevelUpChat(catalogue, "Actual committed message").some(widget => widget.text.includes("Actual committed message")));
});
