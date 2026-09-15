import { UiAssets } from "../assets.ts";
import { SourceRaster } from "../raster.ts";
import { paintNativeTree } from "../layout.ts";
import { MinimapPainter } from "../minimap.ts";
import { projectAllSettings } from "../settings.ts";
import type { SettingsPageState } from "../settings.ts";
import { testAssets } from "./source-fixture.ts";
import { projectBank } from "../bank.ts";
import { projectHud } from "../hud.ts";
import { TABS } from "../layout.ts";
import { projectLevelUpChat, projectLevelUpPopup } from "../level-up.ts";
import type { GameplayUiView, ItemView } from "../../shared/contracts.ts";

let loaded: Promise<UiAssets> | null = null;

export async function settingsProjectionFixture(state: SettingsPageState): Promise<void> {
  const canvas = document.querySelector("canvas")!;
  canvas.width = 1920; canvas.height = 1080;
  const assets = await (loaded ??= UiAssets.load(testAssets, error => { throw error; }));
  const widgets = projectAllSettings(assets.catalogue, state).widgets;
  await assets.preloadItems(widgets.filter(widget => widget.item >= 0).map(widget => widget.item));
  await Promise.all(["ui/minimaps/compass.png", "ui/minimaps/3168-3168-0.png"].map(id => assets.require(id)));
  const raster = new SourceRaster(canvas, assets), minimap = new MinimapPainter(raster);
  paintNativeTree(raster, widgets, 1920, 1080, widget => widget.contentType === 1337 ||
    minimap.draw(widget, { x: 3222, y: 3218, plane: 0 }));
}

export async function independentProjectionFixture(name: string): Promise<void> {
  const canvas = document.querySelector("canvas")!;
  canvas.width = 1920; canvas.height = 1080;
  const assets = await (loaded ??= UiAssets.load(testAssets, error => { throw error; }));
  const catalogue = assets.catalogue, source = catalogue.templates[name]!;
  let widgets = source;
  if (name.startsWith("bounded-bank-")) {
    const values = catalogue.templates["bounded-bank-tabs"]!.filter(widget => widget.id === 12 * 65536 + 12 && widget.item >= 0 && widget.item !== 6512);
    const canonical = (id: number) => Object.entries(catalogue.presentation!.sourceItems).find(([, source]) => source === id)![0];
    const item = (sourceId: number, quantity: number): ItemView => ({
      id: canonical(sourceId), sourceId, name: catalogue.items[sourceId]!.name, quantity,
      iconAsset: null, instanceId: null, charges: null, actions: [],
    });
    const selected = name === "bounded-bank-selected-tab" ? 2 : name === "bounded-bank-placeholder-selected" ? 1 : 0;
    const entries = values.map(widget => ({
      id: `source-entry-${widget.index}`, slot: widget.index, tab: widget.index < 4 ? 1 : widget.index < 8 ? 2 : 0,
      item: canonical(widget.item), value: item(widget.item, widget.item_quantity), placeholder: false,
    }));
    const bank: NonNullable<GameplayUiView["bank"]> = {
      revision: "1", capacity: 1000, selectedTab: selected, insertMode: selected === 1, placeholders: selected === 1,
      amount: selected === 1 ? 5 : 1, noted: selected === 1,
      tabs: [0, 1, 2].map(tab => ({ tab, firstEntry: `source-entry-${tab === 0 ? 8 : tab === 1 ? 0 : 4}`, entries: 4 })),
      entries, depositEquipment: { allowed: true, code: null, reason: null }, unavailableContainers: [],
    };
    if (selected === 1) bank.entries = bank.entries.map(entry => entry.slot === 0
      ? { ...entry, item: canonical(1265), value: null, placeholder: true } : entry);
    widgets = projectBank(catalogue, bank, "", 0).widgets;
  } else if (name.startsWith("bounded-hud-")) {
    const introduced = name === "bounded-hud-hidden" ? [10,11] : [0,1,2,3,4,5,6,10,11];
    const active = name === "bounded-hud-locked" ? 6 : 11;
    const base = [...source.filter(widget => widget.id >> 16 !== 161),
      ...catalogue.templates[TABS[active]!.template]!.filter(widget => widget.id >> 16 === 161)];
    widgets = projectHud(base, catalogue, TABS.map((tab, index) => ({
      interface: tab.interface, visibility: introduced.includes(index) ? "enabled" : "hidden",
      permission: { allowed: introduced.includes(index), code: null, reason: null },
      highlighted: name === "bounded-hud-highlight" && index === 3,
    })), active);
  } else if (name === "bounded-levelup-popup") {
    widgets = projectLevelUpPopup(catalogue, { title: "Source-only level-up title.", lines: ["Source-only level-up detail."], continuation: "Click here to continue" });
  } else if (name === "bounded-levelup-chat") widgets = projectLevelUpChat(catalogue, "Source-only level-up chat notice.");
  await assets.preloadItems(widgets.filter(widget => widget.item >= 0).map(widget => widget.item));
  await Promise.all(["ui/minimaps/compass.png", "ui/minimaps/3168-3168-0.png"].map(id => assets.require(id)));
  const raster = new SourceRaster(canvas, assets), minimap = new MinimapPainter(raster);
  paintNativeTree(raster, widgets, 1920, 1080, widget => widget.contentType === 1337 ||
    minimap.draw(widget, { x: 3222, y: 3218, plane: 0 }));
}
