import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import type { UiCatalogue } from "../../../web/ui/assets.ts";
import { decodeUiCatalogue } from "../../../web/ui/assets.ts";
import { uiRuntimeImages } from "../ui-assets.ts";

test("actual UI delivery includes hash-published runtime minimap primitives omitted by the catalogue's terrain list", async () => {
  const root = new URL("../../../assets/compiled/ui/", import.meta.url);
  const raw = JSON.parse(await readFile(new URL("manifest.json", root), "utf8")) as UiCatalogue;
  const catalogue = decodeUiCatalogue(raw);
  const provenance = JSON.parse(await readFile(new URL("provenance.json", root), "utf8")) as {
    assets: Array<{ path: string; sha256: string; bytes: number }>;
  };
  const pins = new Map(provenance.assets.map((asset) => [asset.path, asset]));
  const images = uiRuntimeImages(catalogue, pins);
  assert(images.includes("ui/minimaps/compass.png"));
  for (let index = 0; index < 7; index++) assert(images.includes(`ui/minimaps/dot-${index}.png`));
  assert(images.every((path) => pins.has(path)));
  assert(!images.some((path) => /(?:reference|panel|capture)/.test(path)));
  assert(Object.values(catalogue.staticModels).every((model) => images.includes(model.asset)));
  assert(images.some((path) => /^ui\/models\/[a-f0-9]{64}\.png$/.test(path)));
  assert(catalogue.templates["native-production-choice-1"]!.some((widget) => typeof widget.id === "number"));
  assert.notEqual(catalogue, raw, "Widget interning is decoded for inspection, not rewritten in the published bytes.");
  pins.delete("ui/minimaps/dot-1.png");
  assert.throws(() => uiRuntimeImages(catalogue, pins), /unpublished source minimap primitive/);
});
