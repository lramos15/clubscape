import { UiAssets, staticModelKey } from "../assets.ts";
import { SourceRaster, plainText } from "../raster.ts";
import { paintNativeTree, widgetId } from "../layout.ts";
import { MinimapPainter } from "../minimap.ts";
import { projectProduction } from "../production.ts";
import { projectDeathPreview } from "../death-preview.ts";
import { projectQuestReward } from "../rewards.ts";
import { testAssets } from "./source-fixture.ts";
import type { ItemView } from "../../shared/contracts.ts";

let loaded: Promise<UiAssets> | null = null;
const load = () => loaded ??= UiAssets.load(testAssets, error => { throw error; });

export async function presentationProjection(name: string): Promise<void> {
  const canvas = document.querySelector("canvas")!;
  canvas.width = 1920; canvas.height = 1080;
  const assets = await load(), catalogue = assets.catalogue, source = catalogue.templates[name]!;
  const item = (sourceId: number, quantity: number): ItemView => ({
    id: `source-only-${sourceId}`, sourceId, name: catalogue.items[sourceId]!.name, quantity,
    instanceId: null, charges: null, iconAsset: null, actions: [],
  });
  let widgets = source;
  if (name.startsWith("native-production")) {
    const choices = source.filter(widget => widget.id >> 16 === 270 && widget.type === 6 && widget.item >= 0)
      .sort((a, b) => a.id - b.id);
    const yes = { allowed: true, code: null, reason: null };
    widgets = projectProduction(catalogue, {
      id: "source-only-menu", interface: "interface.cooking", target: { kind: "spawn", spawn: "source-only-facility" },
      recipes: choices.map((widget, index) => ({
        recipe: `source-only-${index}`, name: plainText(source.find(parent => parent.id === widget.id && parent.index === -1)!.name),
        outputs: [item(widget.item, 1)], single: yes, makeX: yes,
      })),
    }, name === "native-production-amount-5" ? 5 : 1, 0, name === "native-production-hover" ? "source-only-0" : null).widgets;
  } else if (name === "native-death-preview-populated") {
    const rows = (child: number) => source.filter(widget => widget.id === widgetId(4, child) && widget.item >= 0)
      .map(widget => item(widget.item, widget.item_quantity));
    widgets = projectDeathPreview(catalogue, {
      scope: "normal_unsafe_non_pvp", kept: rows(6), lost: rows(7),
      fullGraveFee: "0", fullOfficeFee: "0", valueRevision: "1",
    }, 0).widgets;
  } else if (name === "native-reward-fields") {
    widgets = projectQuestReward(catalogue.templates["native-reward"]!, catalogue, {
      id: "source-only-reward", kind: "quest", interface: "interface.quest_reward", title: "Source fixture reward",
      lines: ["Source-only award line"], items: [item(315, 2)], xp: [{ skill: "skill.cooking", amountTenths: "12345" }],
      questPoints: 1, quest: null, skill: null, level: null, continuation: { kind: "ui_dismiss", presentation_id: "source-only-reward" },
    }, [{ id: "skill.cooking", name: "Cooking", xpTenths: "0", baseLevel: 1, currentLevel: 1, iconAsset: null }], 7, 0);
    const group = widgets.filter(widget => widget.id >> 16 === 153);
    widgets = [...source.filter(widget => widget.id >> 16 !== 153), ...group];
  }
  await assets.preloadItems(widgets.filter(widget => widget.item >= 0).map(widget => widget.item));
  await Promise.all(widgets.filter(widget => widget.type === 6).flatMap(widget => {
    const icon = catalogue.staticModels[staticModelKey(widget)];
    return icon ? [assets.require(icon.asset)] : [];
  }));
  await Promise.all(["ui/minimaps/compass.png", "ui/minimaps/3168-3168-0.png"].map(id => assets.require(id)));
  const raster = new SourceRaster(canvas, assets), minimap = new MinimapPainter(raster);
  paintNativeTree(raster, widgets, 1920, 1080, widget => widget.contentType === 1337 ||
    minimap.draw(widget, { x: 3222, y: 3218, plane: 0 }));
}
