import { UiAssets, staticModelKey } from "../assets.ts";
import { SourceRaster } from "../raster.ts";
import { paintNativeTree } from "../layout.ts";
import { MinimapPainter } from "../minimap.ts";
import { paintTitleBackground, paintReconnect } from "../entry.ts";
import { TitleFlames } from "../flames.ts";
import { projectAbilityGrid, projectFilterPanel } from "../filters.ts";
import type { AbilityKind, AbilityVisualTruth } from "../filters.ts";
import { projectRecovery, recoveryTemplate } from "../recovery.ts";
import type { RecoveryDisplay } from "../recovery.ts";
import type { ClientAssets } from "../../shared/contracts.ts";

export const testAssets: ClientAssets = {
  baseUrl: "/assets/",
  url: id => `/assets/${id}`,
  async image(id) {
    const image = new Image();
    image.src = this.url(id);
    await image.decode();
    return image;
  },
  async json(id) {
    const response = await fetch(this.url(id));
    if (!response.ok) throw new Error(`Asset ${id}: HTTP ${response.status}`);
    return response.json();
  },
};
let loaded: Promise<UiAssets> | null = null;
const fixtureAssets = () => loaded ??= UiAssets.load(testAssets, error => { throw error; });

export async function sourceFixture(name: string): Promise<void> {
  const canvas = document.querySelector("canvas")!;
  canvas.width = 1920; canvas.height = 1080;
  const assets = await fixtureAssets();
  const widgets = assets.catalogue.templates[name];
  if (!widgets) throw new Error(`Unknown original-runtime fixture ${name}`);
  await assets.preloadItems(widgets.filter(w => w.item >= 0).map(w => w.item));
  await Promise.all(widgets.filter(w => w.type === 6).flatMap(w => {
    const icon = assets.catalogue.staticModels[staticModelKey(w)];
    return icon ? [assets.require(icon.asset)] : [];
  }));
  await Promise.all(Object.values(assets.catalogue.portraits).map(p => assets.require(p.asset)));
  await Promise.all(["ui/minimaps/compass.png", "ui/minimaps/3168-3168-0.png"].map(id => assets.require(id)));
  const raster = new SourceRaster(canvas, assets);
  const minimap = new MinimapPainter(raster);
  // HudCapture.unlockFamilies explicitly supplies native minimap state 2 in this one SOURCE fixture.
  minimap.enabled = name !== "family-guide";
  raster.clear();
  paintNativeTree(raster, widgets, 1920, 1080, w => w.contentType === 1337 ||
    minimap.draw(w, { x: 3222, y: 3218, plane: 0 }));
  document.documentElement.dataset.ready = name;
}

export async function ownerFixture(name: string): Promise<void> {
  const canvas = document.querySelector("canvas")!;
  canvas.width = 1920; canvas.height = 1080;
  const assets = await fixtureAssets();
  const raster = new SourceRaster(canvas, assets), proposal = assets.catalogue.proposals[name]!;
  paintTitleBackground(raster, 1920);
  raster.sprite(499, 779, 171);
  raster.center(proposal.content.heading, 959, 202, 496, 0xffff00);
  proposal.content.lines.forEach((line, index) => raster.center(line, 959, 230 + index * 21, 495));
  proposal.controls.forEach(control => {
    const [x, y, width] = control.rectangle as [number, number, number, number];
    raster.sprite(500, x, y); raster.center(control.label, x + Math.floor(width / 2), y + 25, 496);
  });
  // The frozen source first-title-paint does not yet draw the world-switch widget.
  raster.sprite(811, 1302, 463, { frame: 0 });
  document.documentElement.dataset.ready = `owner-${name}`;
}

export async function flameFixture(cycle: number): Promise<void> {
  const canvas = document.querySelector("canvas")!;
  canvas.width = 765; canvas.height = 280;
  const assets = await fixtureAssets();
  const raster = new SourceRaster(canvas, assets), effect = new TitleFlames(assets.catalogue.flames, 0);
  for (let current = 0; current <= cycle; current++) effect.advance(current);
  raster.fill({ x: 0, y: 0, width: 765, height: 280 }, 0x203040);
  effect.paint(raster, 0);
}

export async function reconnectFixture(): Promise<void> {
  const canvas = document.querySelector("canvas")!;
  canvas.width = 300; canvas.height = 70;
  const raster = new SourceRaster(canvas, await fixtureAssets());
  raster.fill({ x: 0, y: 0, width: 300, height: 70 }, 0x203040);
  paintReconnect(raster);
}

export async function filterProjection(kind: AbilityKind, mask: number, open: boolean): Promise<void> {
  const canvas = document.querySelector("canvas")!;
  canvas.width = 1920; canvas.height = 1080;
  const assets = await fixtureAssets();
  await assets.preloadItems([1265, 1351, 590, 303, 317, 315, 1511, 1925, 1931, 995]);
  await Promise.all(["ui/minimaps/compass.png", "ui/minimaps/3168-3168-0.png"].map(id => assets.require(id)));
  let widgets = assets.catalogue.templates[`native-${kind}-mask-0${open ? "-filters" : ""}`]!.map(w => ({ ...w }));
  if (open) projectFilterPanel(widgets, kind, mask);
  else {
    const group = kind === "prayer" ? 541 : 218;
    const observed = (bit: number) => new Set(assets.catalogue.templates[`native-${kind}-mask-${bit}`]!
      .filter(w => w.id >> 16 === group && w.index === -1 && w.name).map(w => w.id));
    const level = observed(8), requirements = observed(kind === "prayer" ? 16 : 32);
    const resources = kind === "magic" ? observed(16) : new Set<number>(), lower = kind === "prayer" ? observed(1) : new Set<number>();
    const facts: Record<string, AbilityVisualTruth> = {};
    for (const widget of widgets.filter(w => w.id >> 16 === group && w.index === -1 && w.name))
      facts[widget.id] = { level: level.has(widget.id), requirements: requirements.has(widget.id),
        resources: kind === "magic" ? resources.has(widget.id) : null, lowerTier: kind === "prayer" ? !lower.has(widget.id) : null };
    widgets = projectAbilityGrid(widgets, assets.catalogue, kind, mask, facts);
  }

  const raster = new SourceRaster(canvas, assets), minimap = new MinimapPainter(raster);
  paintNativeTree(raster, widgets, 1920, 1080, w => w.contentType === 1337 || minimap.draw(w, { x: 3222, y: 3218, plane: 0 }));
}

export async function recoveryProjection(name: string): Promise<void> {
  const canvas = document.querySelector("canvas")!;
  canvas.width = 1920; canvas.height = 1080;
  const assets = await fixtureAssets(), source = assets.catalogue.templates[name]!;
  const scrolling = /native-retrieval-(scroll|long)-(602|669)/.exec(name);
  const office = scrolling ? scrolling[2] === "669" : name.startsWith("native-retrieval-669"), group = office ? 669 : 602;
  const sourceRows = source.filter(w => w.id === group * 65536 + 3 && w.index >= 0 && w.item >= 0);
  const selected = sourceRows.find(w => w.border === 2);
  const [, , , cofferText, , feeText] = name.split("-");
  const display: RecoveryDisplay = {
    storage: office ? "death_office" : "grave",
    items: sourceRows.map(row => ({ id: `source-fixture-slot-${row.index}`, slot: row.index, allowed: true, reason: null,
      item: { id: `source-fixture-item-${row.item}`, sourceId: row.item, name: assets.catalogue.items[row.item]!.name,
        quantity: row.item_quantity, actions: [], iconAsset: null, instanceId: null, charges: null } })),
    selectedId: selected ? `source-fixture-slot-${selected.index}` : null,
    coffer: office ? scrolling ? "12345" : cofferText! : null, unitFee: office ? scrolling ? 42 : Number(feeText) : null,
    capacity: 120, bankAll: name === "native-retrieval-602-35-0-1", discardAll: false, scroll: scrolling?.[1] === "scroll" ? 60 : 0,
  };
  if (name.includes("--1-")) { display.selectedId = null; display.unitFee = 0; }
  const widgets = projectRecovery(recoveryTemplate(assets.catalogue, display), display);
  await assets.preloadItems([...widgets.filter(w => w.item >= 0).map(w => w.item)]);
  await Promise.all(["ui/minimaps/compass.png", "ui/minimaps/3168-3168-0.png"].map(id => assets.require(id)));
  const raster = new SourceRaster(canvas, assets), minimap = new MinimapPainter(raster);
  paintNativeTree(raster, widgets, 1920, 1080, w => w.contentType === 1337 || minimap.draw(w, { x: 3222, y: 3218, plane: 0 }));
}
