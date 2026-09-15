import { UiAssets, staticModelKey } from "../assets.ts";
import { SourceRaster } from "../raster.ts";
import { paintNativeTree } from "../layout.ts";
import { MinimapPainter } from "../minimap.ts";
import { projectDocument } from "../documents.ts";
import { testAssets } from "./source-fixture.ts";

let loaded: Promise<UiAssets> | null = null;
export async function documentProjectionFixture(name: string): Promise<void> {
  const assets = await (loaded ??= UiAssets.load(testAssets, error => { throw error; }));
  const canvas = document.querySelector("canvas")!;
  canvas.width = 1920; canvas.height = 1080;
  const map = name.startsWith("ui4-map"), page = name === "ui4-book-last" ? 1 : 0;
  const fields = {
    id: "source-only-document", interface: map ? "interface.newcomer_map" : "interface.read_book",
    title: "Source-only book title", page, nativeMap: map, mapAsset: null,
    pages: map ? [] : [0, 1].map(page => Array.from({ length: 30 }, (_, line) => `Source-only line ${page * 30 + line + 1}`).join("<br>")),
  };
  const widgets = projectDocument(assets.catalogue, fields, 0, name === "ui4-map-tutors-shown", null).widgets;
  await assets.preloadItems(widgets.filter(widget => widget.item >= 0).map(widget => widget.item));
  await Promise.all(widgets.filter(widget => widget.type === 6).flatMap(widget => {
    const image = assets.catalogue.staticModels[staticModelKey(widget)];
    return image ? [assets.require(image.asset)] : [];
  }));
  await Promise.all(["ui/minimaps/compass.png", "ui/minimaps/3168-3168-0.png"].map(id => assets.require(id)));
  const raster = new SourceRaster(canvas, assets), minimap = new MinimapPainter(raster);
  paintNativeTree(raster, widgets, 1920, 1080, widget => widget.contentType === 1337 ||
    minimap.draw(widget, { x: 3222, y: 3218, plane: 0 }));
}
