import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import type { UiCatalogue } from "../../../web/ui/assets.ts";
import { uiRuntimeImages } from "../ui-assets.ts";

test("actual UI delivery includes hash-published runtime minimap primitives omitted by the catalogue's terrain list", async () => {
  const root = new URL("../../../assets/compiled/ui/", import.meta.url);
  const catalogue = JSON.parse(await readFile(new URL("manifest.json", root), "utf8")) as UiCatalogue;
  const provenance = JSON.parse(await readFile(new URL("provenance.json", root), "utf8")) as {
    assets: Array<{ path: string; sha256: string; bytes: number }>;
  };
  const pins = new Map(provenance.assets.map((asset) => [asset.path, asset]));
  const images = uiRuntimeImages(catalogue, pins);
  assert(images.includes("ui/minimaps/compass.png"));
  for (let index = 0; index < 7; index++) assert(images.includes(`ui/minimaps/dot-${index}.png`));
  assert(images.every((path) => pins.has(path)));
  assert(!images.some((path) => /(?:reference|panel|capture)/.test(path)));
  pins.delete("ui/minimaps/dot-1.png");
  assert.throws(() => uiRuntimeImages(catalogue, pins), /unpublished source minimap primitive/);
});
