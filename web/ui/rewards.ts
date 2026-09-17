import type { GameplayUiView, SkillView } from "../shared/contracts.ts";
import type { NativeWidget, UiCatalogue } from "./assets.ts";
import { formatUiFixed } from "./gameplay-ui.ts";
import { sourceLines, escapeText } from "./raster.ts";

export type RewardView = NonNullable<GameplayUiView["reward"]>;

export function rewardDetails(view: RewardView, skills: readonly SkillView[]): string[] {
  return [...view.lines,
    ...view.items.map(item => `${item.quantity.toLocaleString("en-US")} x ${item.name}`),
    ...view.xp.map(award => `${formatUiFixed(award.amountTenths, 1)} ${skills.find(skill => skill.id === award.skill)?.name ?? award.skill} XP`),
    ...(view.questPoints ? [`${view.questPoints} Quest ${view.questPoints === 1 ? "point" : "points"}`] : []),
  ];
}

export function projectQuestReward(widgets: NativeWidget[], catalogue: UiCatalogue, view: RewardView,
  skills: readonly SkillView[], totalQuestPoints: number, scroll: number): NativeWidget[] {
  const rows = rewardDetails(view, skills).flatMap(line => sourceLines(escapeText(line), 180, catalogue.fonts[495]!));
  const start = Math.min(Math.floor(Math.max(0, scroll) / 15), Math.max(0, rows.length - 7));
  return widgets.map(widget => {
    if (widget.id >> 16 !== 153 || widget.type !== 4) return { ...widget };
    const child = widget.id & 65535;
    const text = child === 3 ? "Congratulations!" : child === 4 ? escapeText(view.title)
      : child === 6 ? `Total Quest Points: ${totalQuestPoints}` : child === 8 ? "You are awarded:"
        : child >= 9 && child <= 15 ? rows[start + child - 9] ?? "" : widget.text;
    return { ...widget, text };
  });
}
