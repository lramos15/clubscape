import { UiAssets } from "../assets.ts";
import { SourceRaster } from "../raster.ts";
import { paintNativeTree } from "../layout.ts";
import { MinimapPainter } from "../minimap.ts";
import { paintTitleBackground } from "../entry.ts";
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

export async function sourceFixture(name: string): Promise<void> {
  const canvas = document.querySelector("canvas")!;
  canvas.width = 1920; canvas.height = 1080;
  const assets = await UiAssets.load(testAssets, error => { throw error; });
  const widgets = assets.catalogue.templates[name];
  if (!widgets) throw new Error(`Unknown original-runtime fixture ${name}`);
  await assets.preloadItems(widgets.filter(w => w.item >= 0).map(w => w.item));
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
  const assets = await UiAssets.load(testAssets, error => { throw error; });
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
