import type { NativeWidget, Rect } from "./assets.ts";
import { intersect, staticModelKey } from "./assets.ts";
import type { Control, UiAction } from "./input.ts";
import { SourceRaster, countText, escapeText, plainText, sourceLines } from "./raster.ts";
import { cloneTemplate, frameRegions, paintNativeTree, projectScrollbar, tabWidget, TABS, widgetId, widgetKey } from "./layout.ts";
import type { LaidWidget } from "./layout.ts";
import type { GameViewContext } from "./index.ts";
import { isInterfaceUnlocked } from "./index.ts";
import type { ItemView, SkillView } from "../shared/contracts.ts";
import type { AbilityUiView, GameplayUiView } from "../shared/contracts.ts";
import { FILTER_OPTIONS, filterOptionEnabled, projectAbilityGrid, projectFilterPanel } from "./filters.ts";
import type { AbilityVisualTruth } from "./filters.ts";
import { projectRecovery, recoveryControls, recoveryTemplate } from "./recovery.ts";
import type { RecoveryDisplay, RecoveryUiCommand } from "./recovery.ts";
import { bankAmountRequest, bankDefaultAmount, formatUiFixed, formatUiInteger, gameplayUi, gameplayUiProblem, permissionReason } from "./gameplay-ui.ts";
import { productionChoiceLabel, productionSource, projectProduction } from "./production.ts";
import { deathPreviewDetails, projectDeathPreview } from "./death-preview.ts";
import { projectQuestReward, rewardDetails } from "./rewards.ts";
import { audioSourceControl, audioTooltip, projectAudioControls } from "./audio-controls.ts";
import { SOURCE_MUSIC_MODE_IDS } from "../audio/native-scene.ts";
import { projectMusicControls } from "./music-controls.ts";
import { currentSettingValues, projectAllSettings, SETTINGS_CATEGORIES } from "./settings.ts";
import type { SettingRowAsset } from "./assets.ts";
import { sourceAudioDefaults } from "../audio/native-policy.ts";
import { projectBank } from "./bank.ts";
import { projectHud } from "./hud.ts";
import { documentSource, projectDocument } from "./documents.ts";
import { projectLevelUpPopup } from "./level-up.ts";
import { sourceSavedPlaylist } from "../audio/preferences.ts";

const EQUIPMENT = ["head", "cape", "neck", "weapon", "body", "shield", "legs", "hands", "feet", "ring", "ammo"];
const SKILLS = ["attack", "strength", "defence", "ranged", "prayer", "magic", "runecraft", "construction",
  "hitpoints", "agility", "herblore", "thieving", "crafting", "fletching", "slayer", "hunter",
  "mining", "smithing", "fishing", "cooking", "firemaking", "woodcutting", "farming", "sailing"];
const MODALS: Record<string, number> = {
  journal: 119, reward: 153, experience: 929, appearance: 679, "equipment-stats": 84,
  "kept-items": 4, grave: 602, recovery: 669, smithing: 312, production: 270, "all-settings": 134,
  book: 392, "newcomer-map": 615, "level-up": 233,
};

export function paintCharacter(raster: SourceRaster, appearance: Record<string, number>, controls: Control[],
  change: (body: number) => void, confirm: () => void, required: (label: string, field: string) => void,
  declared?: GameplayUiView["appearance"], preview?: (bounds: Rect, model: NativeWidget) => void): Rect | null {
  const source = raster.assets.catalogue.templates["native-appearance"];
  if (!source) throw new Error("The original character-creator frame is missing.");
  const tree = paintNativeTree(raster, source, raster.canvas.width, raster.canvas.height, widget => {
    if (widget.id >> 16 !== 679) return true;
    const child = widget.id & 65535;
    if (child === 72 && widget.type === 4) widget.text = "Unavailable";
    if ([68, 69].includes(child) && widget.index >= 0 && widget.type !== 4) {
      const selected = appearance.body_type === (child === 68 ? 0 : 1);
      const skin = source.find(w => w.id === widgetId(679, selected ? 68 : 69) && w.index === widget.index);
      if (skin) { widget.sprite = skin.sprite; widget.color = skin.color; widget.opacity = skin.opacity; }
    }
    raster.widget(widget);
    return true;
  });
  for (const widget of tree) {
    if (widget.id >> 16 !== 679 || widget.index !== -1 || widget.type !== 0) continue;
    const child = widget.id & 65535;
    if (![68, 69, 72, 74].includes(child) && !widget.actions?.some(Boolean)) continue;
    const row = tree.find(w => w.parent === widget.parent && w.type === 4 && w.text);
    const label = child === 74 ? "Confirm appearance" : child === 68 || child === 69 ? `Body type ${child === 68 ? "A" : "B"}`
      : child === 72 ? "Pronoun options" : `${row && widget.x < row.x ? "Previous" : "Next"} ${plainText(row?.text ?? "appearance option")}`;
    const choice = declared?.choices.body_type?.find(choice => choice.value === (child === 68 ? 0 : 1));
    const unavailable = declared && [68, 69].includes(child) ? permissionReason(choice?.permission, label)
      : declared?.confirmed && child === 74 ? "Appearance is already confirmed." : undefined;
    controls.push({ ...intersect(widget, widget.clip), id: `appearance-${widgetKey(widget)}`, label,
      ...(unavailable ? { disabled: unavailable } : {}),
      ...([68, 69].includes(child) ? { pressed: appearance.body_type === (child === 68 ? 0 : 1) } : {}),
      actions: [{ label, run: () => child === 74 ? confirm() : [68, 69].includes(child) ? change(child === 68 ? 0 : 1)
        : required(label, "human_appearance_controls") }] });
  }
  const model = tree.find(w => w.id >> 16 === 679 && w.type === 6);
  const bounds = model ? tree.find(w => w.id === model.parent && w.index === -1) ?? null : null;
  if (model && bounds) preview?.(bounds, model);
  return bounds;
}

function attachGroup(widgets: NativeWidget[], source: readonly NativeWidget[], group: number): NativeWidget[] {
  const add = new Map<string, NativeWidget>();
  const staticSource = new Map(source.filter(w => w.index === -1).map(w => [w.id, w]));
  for (const widget of source.filter(w => w.id >> 16 === group)) {
    add.set(widgetKey(widget), { ...widget });
    let parent = staticSource.get(widget.parent);
    const seen = new Set<number>();
    while (parent && !seen.has(parent.id)) {
      seen.add(parent.id); add.set(widgetKey(parent), { ...parent });
      parent = staticSource.get(parent.parent);
    }
  }
  return [...widgets.filter(w => w.id >> 16 !== group && !add.has(widgetKey(w))), ...add.values()]
    .sort((a, b) => a.id - b.id || a.index - b.index);
}

export function skillTooltip(skill: SkillView, thresholds: readonly number[] | undefined): string {
  const xp = BigInt(skill.xpTenths);
  const decimal = (value: bigint) => `${(value / 10n).toLocaleString("en-US")}${value % 10n ? "." + value % 10n : ""}`;
  const next = thresholds?.[skill.baseLevel];
  return `${skill.name}: ${skill.currentLevel}/${skill.baseLevel}\nCurrent XP: ${decimal(xp)}` +
    (next === undefined ? "\nMaximum level" : `\nNext level at: ${decimal(BigInt(next))}\nRemaining XP: ${decimal(BigInt(next) > xp ? BigInt(next) - xp : 0n)}`);
}

function normalizedName(widget: NativeWidget): string {
  return plainText(widget.name || widget.text || widget.actions?.find(Boolean) || "").trim();
}

function declaredAbility(ui: GameplayUiView | null, widget: NativeWidget): AbilityUiView | undefined {
  if (!ui) return undefined;
  const group = widget.id >> 16, child = widget.id & 65535;
  const known = group === 541 && child === 9 ? "prayer.thick_skin"
    : group === 218 && child === 6 ? "spell.lumbridge_home_teleport"
      : group === 218 && child === 11 ? "spell.wind_strike" : null;
  const rows = group === 541 ? ui.prayers : ui.spells;
  // Names associate source artwork only; the returned opaque server ID remains the action identity.
  return rows.find(row => known ? row.id === known : row.name === normalizedName(widget));
}

export function paintGame(raster: SourceRaster, ui: GameViewContext): void {
  const { world, local, controls, inputs } = ui, catalogue = raster.assets.catalogue;
  const authoritative = gameplayUi(world);
  const preferenceBinding = ui.audio?.preferences;
  const nativeMusic = preferenceBinding?.playerId === world.player.id ? preferenceBinding.preferences.music : null;
  const bankView = authoritative?.bank ?? null;
  const production = authoritative?.production ?? null;
  const reward = authoritative?.reward ?? null;
  const questReward = reward?.kind === "quest" &&
    catalogue.presentation?.interfaces[reward.interface]?.sourceIds.includes(153) ? reward : null;
  const levelReward = reward?.kind === "level_up" &&
    catalogue.presentation?.interfaces[reward.interface]?.sourceIds.includes(233) ? reward : null;
  const document = authoritative?.document ?? null;
  const documentGroup = document && !reward && !production ? documentSource(catalogue, document) : null;
  const documentProjection = document && documentGroup !== null
    ? projectDocument(catalogue, document, local.documentPart, local.documentTutors, world.player.tile) : null;
  if (document && !reward && !production && documentGroup === null)
    ui.required(document.title, `document_source:${document.interface}`);
  if (document?.mapAsset && documentGroup === 392)
    ui.required("This document's map image", "book_map_image_layout");
  if (documentProjection) local.documentPart = documentProjection.part;
  if (authoritative?.activeTab !== null && authoritative && local.tab < 0)
    ui.required("The selected source side tab", `active_tab_source:${authoritative.activeTab}`);
  const deathPreview = authoritative?.keptOnDeath ?? null;
  const deathProjection = deathPreview ? projectDeathPreview(catalogue, deathPreview, local.scroll) : null;
  const productionGroup = production ? productionSource(catalogue, production) : null;
  const productionProjection = production ? projectProduction(catalogue, production, local.productionAmount, local.scroll, ui.hoveredProduction) : null;
  if (productionProjection?.problems.length) ui.notice(productionProjection.problems.join("\n"), "error", "ui.source.production");
  const banking = authoritative ? bankView !== null : world.bank !== null;
  const width = raster.canvas.width, height = raster.canvas.height;
  const regions = frameRegions(width, height);
  const tab = TABS[local.tab] ?? TABS[3];
  let templateName: string = banking ? "native-bank" : world.shop ? "native-shop" : tab.template;
  if (!banking && !world.shop && local.tab === 11 && local.settingsPage === "audio") templateName = "native-audio-default";
  const thickSkinSelected = authoritative ? authoritative.prayers.find(row => row.id === "prayer.thick_skin")?.selected === true
    : world.player.activePrayers.includes("prayer.thick_skin");
  if (!banking && !world.shop && local.tab === 5 && thickSkinSelected &&
      catalogue.templates["native-prayer-active"]) templateName = "native-prayer-active";
  if (!banking && !world.shop && local.tab === 6 && !["item.rune.air", "item.rune.mind"].every(id =>
    world.player.inventory.some(slot => slot.item?.id === id && slot.item.quantity > 0)) && catalogue.templates["native-magic-missing-runes"])
    templateName = "native-magic-missing-runes";
  const filterKind = local.tab === 5 ? "prayer" : local.tab === 6 ? "magic" : null;
  if (!banking && !world.shop && filterKind && local.filterPanel === filterKind)
    templateName = `native-${filterKind}-mask-0-filters`;
  if (!catalogue.templates[templateName]) templateName = "native-inventory";
  let widgets = cloneTemplate(catalogue.templates[templateName]!);
  if (authoritative && local.tab < 0 && !banking && !world.shop)
    widgets = widgets.filter(widget => !TABS.some(tab => tab.group === widget.id >> 16));
  const bankProjection = bankView ? projectBank(catalogue, bankView, local.bankSearch, local.scroll) : null;
  if (bankProjection) { widgets = bankProjection.widgets; local.scroll = bankProjection.scroll; }
  if (templateName === "native-audio-default") widgets = projectAudioControls(catalogue, ui.audio);
  const musicProjection = !banking && !world.shop && local.tab === 13
    ? projectMusicControls(catalogue, ui.music, ui.audio?.playingGroup ?? null, local.scroll, local.musicDropdown, nativeMusic) : null;
  if (musicProjection) { widgets = musicProjection.widgets; local.scroll = musicProjection.scroll; }
  if (!banking && !world.shop && filterKind) {
    const mask = filterKind === "prayer" ? local.prayerFilters : local.magicFilters;
    if (local.filterPanel === filterKind) projectFilterPanel(widgets, filterKind, mask);
    else {
      const truths: Record<string, AbilityVisualTruth> = {};
      const skill = world.player.skills.find(s => s.id === `skill.${filterKind === "prayer" ? "prayer" : "magic"}`);
      for (const [id, metadata] of Object.entries(catalogue.abilities)) {
        if (metadata.kind !== filterKind) continue;
        truths[id] = {
          level: skill ? Math.max(skill.baseLevel, skill.currentLevel) >= metadata.level : null,
          ...(filterKind === "magic" ? { resources: metadata.runes.every(rune =>
            world.player.inventory.reduce((sum, slot) => sum + (slot.item?.sourceId === rune.sourceId ? slot.item.quantity : 0), 0) >= rune.quantity) } : {}),
          ...ui.abilityVisuals[id],
        };
      }
      widgets = projectAbilityGrid(widgets, catalogue, filterKind, mask, truths);
    }
  }
  let modal = local.journal ? "journal" : local.modal;
  if (authoritative) {
    modal = production ? productionGroup === 312 ? "smithing" : "production"
      : questReward ? "reward" : levelReward ? "level-up"
        : documentProjection ? documentProjection.group === 615 ? "newcomer-map" : "book"
        : authoritative.activeInterface === "interface.equipment_stats" ? "equipment-stats"
          : authoritative.activeInterface === "interface.items_kept_on_death" && authoritative.keptOnDeath ? "kept-items"
            : authoritative.activeInterface === "interface.quests" && local.journal ? "journal"
              : authoritative.activeInterface === "interface.appearance" ? "appearance"
                : authoritative.activeInterface === "interface.experience" ? "experience" : null;
  } else {
    if (ui.state.phase === "character" || world.player.tutorialStage === "stage.tutorial.appearance") modal = "appearance";
    if (world.player.tutorialStage === "stage.tutorial.experience") modal = "experience";
  }
  if (local.modal === "all-settings" && !production && !reward && !document && !authoritative?.confirmation) modal = "all-settings";
  const settingsValues = modal === "all-settings" ? currentSettingValues(catalogue, world, ui.audio, ui.music, local.clientInput) : null;
  const settingsProjection = settingsValues ? projectAllSettings(catalogue, local.settings, settingsValues.values) : null;
  if (settingsProjection) local.settings.scroll = settingsProjection.scroll;
  const recoveryOpen = world.recovery !== null && (!authoritative || ["interface.grave", "interface.death_retrieval"].includes(authoritative.activeInterface ?? ""));
  if (recoveryOpen) modal = world.recovery!.storage === "grave" ? "grave" : "recovery";
  const management = authoritative?.recovery?.management;
  const recoveryPanel = management?.panels.find(panel =>
    panel.death === world.recovery?.death && panel.storage === world.recovery?.storage);
  const discardReason = permissionReason(authoritative?.recovery?.discard, "Discard recovery items");
  const bankAllReason = permissionReason(management?.bankAll, "Bank-All");
  const takeAllReason = permissionReason(recoveryPanel?.takeAll, "Take-All");
  const recovery: RecoveryDisplay | null = recoveryOpen ? {
    storage: world.recovery!.storage,
    items: world.recovery!.items.map((row, slot) => {
      const entry = recoveryPanel?.entries.find(entry => entry.id === row.id);
      return entry ? {
        id: entry.id, slot, item: entry.item, allowed: entry.take.allowed,
        reason: permissionReason(entry.take, "Retrieve item") ?? null,
        unitFee: entry.unitFee, fullStackFee: entry.fullStackFee,
        inventoryCapacity: entry.inventoryCapacity, bankCapacity: entry.bankCapacity,
      } : { id: row.id, slot, item: row.item, allowed: null, reason: null };
    }),
    selectedId: world.recovery!.items.some(row => row.id === local.recoverySelected) ? local.recoverySelected : null,
    coffer: authoritative?.recovery?.cofferBalance ?? null, unitFee: null, capacity: null,
    bankAll: management?.bankAll.allowed ?? false,
    ...(bankAllReason ? { bankAllReason } : {}),
    ...(recoveryPanel ? {
      takeAll: recoveryPanel.takeAll.allowed, ...(takeAllReason ? { takeAllReason } : {}),
      fullSelectionFee: recoveryPanel.fullSelectionFee,
    } : {}),
    discardAll: authoritative?.recovery?.discard.allowed ?? false,
    ...(discardReason ? { discardReason } : {}),
    scroll: local.scroll,
  } : null;
  if (recovery) {
    widgets = attachGroup(widgets, recoveryTemplate(catalogue, recovery), recovery.storage === "grave" ? 602 : 669);
  } else if (productionProjection && productionGroup && productionProjection.widgets.length) {
    widgets = attachGroup(widgets, productionProjection.widgets, productionGroup);
  } else if (modal === "kept-items" && deathProjection) {
    widgets = attachGroup(widgets, deathProjection.widgets, 4);
  } else if (questReward) {
    widgets = attachGroup(widgets, projectQuestReward(catalogue.templates["native-reward"]!, catalogue,
      questReward, world.player.skills, world.player.questPoints, local.scroll), 153);
  } else if (levelReward) {
    widgets = attachGroup(widgets, projectLevelUpPopup(catalogue,
      { title: levelReward.title, lines: levelReward.lines, continuation: "Click here to continue" }, local.dialoguePage), 233);
  } else if (documentProjection) {
    widgets = attachGroup(widgets, documentProjection.widgets, documentProjection.group);
  } else if (settingsProjection) {
    widgets = attachGroup(widgets, settingsProjection.widgets, 134);
  } else if (modal && catalogue.templates[`native-${modal}`]) {
    widgets = attachGroup(widgets, catalogue.templates[`native-${modal}`]!, MODALS[modal]!);
  }
  if (world.dialogue && !levelReward && catalogue.templates["native-guide-dialogue"]) {
    widgets = widgets.filter(w => w.id >> 16 !== 162);
    widgets = attachGroup(widgets, catalogue.templates["native-guide-dialogue"]!, 162);
    widgets = attachGroup(widgets, catalogue.templates["native-guide-dialogue"]!, 231);
  }

  const liveItems = new Map<string, ItemView>();
  const inventoryGroup = banking ? 15 : world.shop ? 301 : 149;
  const itemPrototype = widgets.find(w => w.id >> 16 === inventoryGroup && w.item >= 0) ??
    catalogue.templates["native-inventory"]!.find(w => w.id >> 16 === 149 && w.item >= 0)!;
  if (widgets.some(w => w.id >> 16 === inventoryGroup)) {
    widgets = widgets.filter(w => !(w.id >> 16 === inventoryGroup && w.index >= 0 && w.type === 5));
    for (const slot of world.player.inventory) if (slot.item) {
      const widget = { ...itemPrototype, index: slot.index, x: itemPrototype.x + slot.index % 4 * 42,
        y: itemPrototype.y + Math.floor(slot.index / 4) * 36, item: slot.item.sourceId ?? -1,
        item_quantity: slot.item.quantity, border: local.selectedItem?.slot === slot.index ? 2 : 1,
        quantityMode: 2, text: "", name: slot.item.name, actions: slot.item.actions };
      widgets.push(widget); liveItems.set(widgetKey(widget), slot.item);
    }
  }
  const displayedBankEntries = new Map<number, NonNullable<GameplayUiView["bank"]>["entries"][number]>();
  const displayedBankTabs = new Map<number, NonNullable<GameplayUiView["bank"]>["tabs"][number]>();
  if (bankProjection) {
    for (const [slot, entry] of bankProjection.entries) {
      displayedBankEntries.set(slot, entry);
      if (entry.value) liveItems.set(`${widgetId(12, 12)}:${slot}`, entry.value);
    }
    for (const [tab, value] of bankProjection.tabs) displayedBankTabs.set(tab, value);
  } else if (banking) {
    const prototype = widgets.find(w => w.id === widgetId(12, 12) && w.index === 0)!;
    widgets = widgets.filter(w => !(w.id === widgetId(12, 12) && w.index >= 0));
    const slots = world.bank!.slots.filter(s => s.item && s.item.name.toLocaleLowerCase().includes(local.bankSearch.toLocaleLowerCase()));
    const content = widgets.find(widget => widget.id === widgetId(12, 12) && widget.index === -1)!;
    const extent = slots.length ? Math.floor((slots.length - 1) / 8) * 36 + prototype.height : 0;
    local.scroll = Math.min(local.scroll, Math.max(0, extent - content.height));
    projectScrollbar(widgets, widgetId(12, 13), content.id, extent, local.scroll);
    slots.forEach((slot, index) => {
      const item = slot.item;
      const widget = { ...prototype, index: slot.index, x: prototype.x + index % 8 * 48,
        y: prototype.y + Math.floor(index / 8) * 36 - local.scroll, item: item?.sourceId ?? -1,
        item_quantity: item?.quantity ?? 0, name: item?.name ?? "" };
      widgets.push(widget);
      if (item) liveItems.set(widgetKey(widget), item);
    });
  }
  if (world.shop) {
    const prototype = widgets.find(w => w.id === widgetId(300, 16) && w.index === 1)!;
    widgets = widgets.filter(w => !(w.id === widgetId(300, 16) && w.index >= 0));
    local.scroll = Math.min(local.scroll, Math.max(0, Math.ceil(world.shop.rows.length / 8) * 47 - 212));
    world.shop.rows.forEach((row, index) => {
      const widget = { ...prototype, index: row.index + 1, x: prototype.x + index % 8 * 47,
        y: prototype.y + Math.floor(index / 8) * 47 - local.scroll, item: row.item.sourceId ?? -1,
        item_quantity: row.stock, name: row.item.name };
      widgets.push(widget); liveItems.set(widgetKey(widget), row.item);
    });
  }

  for (const equipmentGroup of [387, 84]) for (let slotIndex = 0; slotIndex < EQUIPMENT.length; slotIndex++) {
    const start = equipmentGroup === 387 ? 15 : 10;
    const id = widgetId(equipmentGroup, start + slotIndex);
    const parent = widgets.find(w => w.id === id && w.index === -1);
    if (!parent) continue;
    const item = world.player.equipment.find(e => e.slot === `slot.${EQUIPMENT[slotIndex]}`)?.item;
    widgets = widgets.filter(w => !(w.id === id && w.index > 0));
    if (item) {
      const prototype = catalogue.templates[equipmentGroup === 387 ? "native-equipment" : "native-equipment-stats"]!
        .find(w => w.id === widgetId(equipmentGroup, start + 3) && w.index === 1)!;
      const widget = { ...prototype, id, parent: id, index: 1, x: parent.x + 2, y: parent.y + 2,
        item: item.sourceId ?? -1, item_quantity: item.quantity };
      widgets.push(widget); liveItems.set(widgetKey(widget), item);
    } else {
      const source = catalogue.templates[equipmentGroup === 387 ? "native-equipment" : "native-equipment-stats"]!.find(w => w.id === id && w.index === 2);
      if (source) widgets.push({ ...source });
      else {
        // Empty equipped slots use the same native placeholders as the other stock slots.
        const placeholder = [156, 157, 158, 159, 161, 162, 163, 164, 165, 160, 166][slotIndex]!;
        const proto = catalogue.templates["native-equipment"]!.find(w => w.id === widgetId(387, 15) && w.index === 2)!;
        widgets.push({ ...proto, id, parent: id, x: parent.x + 2, y: parent.y + 2, sprite: placeholder });
      }
    }
  }
  if (!banking && !world.shop && local.tab === 2 && !modal) {
    const scroller = widgets.find(w => w.id >> 16 === 399 && w.type === 0 && w.scrollHeight > w.height);
    if (scroller) {
      local.scroll = Math.min(local.scroll, scroller.scrollHeight - scroller.height);
      const parents = new Map(widgets.filter(w => w.index < 0).map(w => [w.id, w.parent]));
      for (const widget of widgets) {
        if (widget === scroller) continue;
        let parent = widget.parent;
        const visited = new Set<number>();
        while (parent !== -1 && !visited.has(parent)) {
          if (parent === scroller.id) { widget.y -= local.scroll; break; }
          visited.add(parent); parent = parents.get(parent) ?? -1;
        }
      }
    }
  }
  if (recovery) widgets = projectRecovery(widgets, recovery);
  if (authoritative) widgets = projectHud(widgets, catalogue, authoritative.interfaces, local.tab);

  const sourceRectangles = new Map<string, LaidWidget>();
  const sourceTabRects = new Map<number, Rect>();
  const register = (widget: LaidWidget, id: string, label: string, actions: UiAction[], properties: Partial<Control> = {}) => {
    const rect = intersect(widget, widget.clip);
    if (rect.width <= 0 || rect.height <= 0) return;
    const menu = [...actions];
    if (widget.item < 0 && ![149, 15, 301, 387, 593].includes(widget.id >> 16) &&
        !(authoritative && [4, 12, 153, 270, 312].includes(widget.id >> 16)) && ![116, 134, 239].includes(widget.id >> 16)) {
      for (const operation of widget.actions ?? []) if (operation &&
        !menu.some(action => plainText(action.label).startsWith(plainText(operation)))) {
        menu.push({ label: operation, run: () => ui.unavailable(plainText(operation)) });
      }
    }
    controls.push({ ...rect, id, label: plainText(label), actions: menu, ...properties });
  };
  const sourceClose = (widget: LaidWidget) => register(widget, `close-${widgetKey(widget)}`, "Close interface", [{
    label: "Close", run: () => {
      if (widget.id >> 16 === 134) { ui.change(() => { local.modal = null; local.settings.choice = null; }); return; }
      if (authoritative?.reward && widget.id >> 16 === 153) {
        ui.continueReward(authoritative.reward.id, authoritative.reward.continuation);
        return;
      }
      ui.change(() => { local.modal = null; local.journal = null; });
      ui.send({ kind: "close_interface" });
    },
  }]);
  const settingsScope = (row: SettingRowAsset) => {
    const outside = /clan|group ironman|sailing|world map|modern layout|old school store|mobile|combat achievement|leagues|last man standing|clue scroll|stash|birdhouse|phantom muspah|dwarf multicannon|quetzal|barracuda|cargo|boat|helm|boss health|hunter|slayer|beta world/i.test(row.label);
    if (outside) ui.unavailable(row.label);
    else ui.required(row.label, `source_setting:${row.id}`);
  };
  const openAllSettings = (category?: number) => ui.change(() => {
    local.modal = "all-settings"; local.settings.choice = null;
    if (category !== undefined) {
      local.settings.category = category; local.settings.search = null; local.settings.scroll = 0;
    }
  });
  const applySetting = (id: number, value?: number) => {
    const state = settingsValues?.values.get(id), row = catalogue.settingsRows[id]!;
    if (!settingsValues?.available.has(id)) { settingsScope(row); return; }
    if ([4503, 2753, 2754, 2755].includes(id)) {
      const channel = id === 4503 ? "master" : id === 2753 ? "music" : id === 2754 ? "effects" : "area";
      if (value !== undefined) ui.audioPercent(channel, value);
    } else if (id === 6335) {
      const defaults = sourceAudioDefaults();
      for (const channel of ["master", "music", "effects", "area"] as const) ui.audioPercent(channel, defaults.sliders[channel]);
    } else if (id === 6393 && value !== undefined) ui.musicAction({ kind: "mode", mode: value === 0 ? "area" : value === 1 ? "shuffle" : "single" });
    else if (id === 2974 && value !== undefined) ui.musicAction({ kind: "area_mode", mode: value === 0 ? "modern" : "classic" });
    else if (id === 6394 && (value === 0 || value === 1 || value === 2 || value === 3)) ui.musicAction({ kind: "select_playlist", selection: value });
    else if (id === 6395 && nativeMusic) ui.musicAction({ kind: "flag", flag: "rememberModeOnLogin", enabled: !nativeMusic.rememberModeOnLogin });
    else if (id === 6397 && nativeMusic) ui.musicAction({ kind: "flag", flag: "keepPlayingOnPlaylistChange", enabled: !nativeMusic.keepPlayingOnPlaylistChange });
    else if ([6405, 6406, 6407].includes(id)) ui.musicAction({ kind: "clear_playlist", slot: id === 6405 ? 1 : id === 6406 ? 2 : 3 });
    else if (id === 2769) ui.change(() => { local.clientInput.singleMouse = !local.clientInput.singleMouse; });
    else if (id === 1104) ui.change(() => { local.clientInput.shiftDrop = !local.clientInput.shiftDrop; });
    else if (id === 2775) ui.change(() => { local.clientInput.escapeCloses = !local.clientInput.escapeCloses; });
    else if (id === 2861 || id === 880) ui.send({ kind: "set_setting", setting: {
      setting: id === 2861 ? "death_supply_piles" : "death_auto_equip", enabled: state?.value !== true,
    } });
    else if (id === 2732) {
      if (value !== 1) ui.unavailable(value === 0 ? "Fixed layout" : "Resizable Modern layout");
    } else if (id === 2852 || id === 2853) {
      if (id === 2852 && value !== 0) ui.unavailable("Interface scaling beyond the approved source UI scale 1");
    } else settingsScope(row);
  };
  const primaryAction = (widget: LaidWidget) => {
    const label = normalizedName(widget);
    const op = plainText(widget.actions?.find(Boolean) ?? "");
    const group = widget.id >> 16, child = widget.id & 65535;
    if (recovery && (group === 602 || group === 669)) return;
    if (document && documentProjection && group === documentProjection.group) {
      if (widget.index === -1 && (group === 392 && child === 7 || group === 615 && child === 20))
        register(widget, "document-close", `Close ${document.title}`, [{ label: "Close",
          run: () => ui.sendUi({ kind: "ui_dismiss", presentation_id: document.id }) }], { tooltip: document.title });
      else if (group === 392 && widget.index === -1 && [75, 77].includes(child)) {
        const previous = child === 75;
        register(widget, previous ? "document-previous" : "document-next", previous ? "Previous page" : "Next page", [{
          label: previous ? "Previous page" : "Next page", run: () => {
            if (!ui.presentationCurrent("document", document.id, document.page)) return;
            if (previous && local.documentPart > 0) ui.change(() => { local.documentPart--; });
            else if (!previous && local.documentPart + 1 < documentProjection.parts) ui.change(() => { local.documentPart++; });
            else ui.sendUi({ kind: "ui_document_page", document_id: document.id, page: document.page + (previous ? -1 : 1) });
          },
        }]);
      } else if (group === 615 && child === 18 && widget.index === -1)
        register(widget, "document-tutors", local.documentTutors ? "Hide Tutors" : "Show Tutors", [{
          label: local.documentTutors ? "Hide" : "Show",
          run: () => { if (ui.presentationCurrent("document", document.id)) ui.change(() => { local.documentTutors = !local.documentTutors; }); },
        }], { pressed: local.documentTutors });
      return;
    }
    if (group === 233 && levelReward && widget.index === -1) {
      if (child === 3) {
        const lines = levelReward.lines.flatMap(line => sourceLines(escapeText(line), 390, catalogue.fonts[497]!));
        const more = (local.dialoguePage + 1) * 2 < lines.length;
        register(widget, "level-up-continue", more ? "Continue level-up text" : "Continue level-up", [{
          label: "Continue", run: () => {
            if (!ui.presentationCurrent("reward", levelReward.id)) return;
            if (more) ui.change(() => { local.dialoguePage++; });
            else ui.continueReward(levelReward.id, levelReward.continuation);
          },
        }]);
      } else if (child === 1 || child === 2) register(widget, `level-up-details-${child}`, "Level-up details", [{
        label: "View details", run: () => ui.notice(rewardDetails(levelReward, world.player.skills).join("\n")),
      }], { tooltip: rewardDetails(levelReward, world.player.skills).join("\n") });
      return;
    }
    if (op === "Close" && group !== 161 && group !== 239) { sourceClose(widget); return; }
    if (group === 134 && settingsProjection && settingsValues) {
      if (widget.index === -1 && child === 5)
        register(widget, "settings-info", local.settings.moreInfo ? "Less info" : "More info", [{ label: "Toggle",
          run: () => ui.change(() => { local.settings.moreInfo = !local.settings.moreInfo; local.settings.choice = null; }) }]);
      else if (widget.index === -1 && child === 7)
        register(widget, "settings-locked", local.settings.hideLocked ? "Show locked" : "Hide locked", [{ label: "Toggle",
          run: () => ui.change(() => { local.settings.hideLocked = !local.settings.hideLocked; local.settings.scroll = 0; }) }]);
      else if (widget.index === -1 && child === 11 && local.settings.search === null)
        register(widget, "settings-search-start", "Search settings", [{ label: "Search",
          run: () => {
            ui.change(() => { local.settings.search = ""; local.settings.scroll = 0; });
            ui.focusInput("settings-search");
          } }]);
      else if (child === 24 && widget.index >= 0) {
        const category = widget.index;
        if (SETTINGS_CATEGORIES[category]) register(widget, `settings-category-${category}`, SETTINGS_CATEGORIES[category]!, [{ label: "Select",
          run: () => openAllSettings(category) }]);
      } else if (widget.index === -1 && child === 21) {
        const thumb = widgets.find(row => row.id === widget.id && row.index === 1)!;
        register(widget, "settings-scroll", "Settings scroll", [], { scrollbar: {
          value: settingsProjection.scroll, maximum: Math.max(0, settingsProjection.extent - 256), thumb: thumb.height, page: 256,
          current: () => local.settings.scroll, change: value => ui.change(() => { local.settings.scroll = value; }),
        } });
      } else {
        const descriptor = settingsProjection.controls.get(widgetKey(widget));
        if (descriptor) {
          const { row, source } = descriptor, kind = Number(row.params["1078"] ?? 0);
          const known = settingsValues.values.get(row.id)!;
          if (kind === 1 && source.type === 5) return;
          const slider = kind === 1 && source.type === 3 && source.height === 16 && source.actions?.includes("Select") &&
            [30, 31, 32, 319].includes(Number(row.params["1077"]));
          const label = row.label, id = `setting-${row.id}-${widget.index}`;
          register(widget, id, label, [{ label: source.actions?.find(Boolean) ?? "Select", run: () => {
            if (kind === 2) ui.change(() => { local.settings.choice = { setting: row.id, x: widget.x, y: widget.y,
              width: widget.width, height: widget.height, scroll: 0 }; });
            else if (!slider) applySetting(row.id);
          } }], { tooltip: label + (known.value === null && !settingsValues.available.has(row.id) ? "\nRequired setting value is unavailable." : ""),
            ...(slider ? { slider: { value: typeof known.value === "number" ? known.value : null,
              current: () => ui.audioValue(row.id === 4503 ? "master" : row.id === 2753 ? "music" : row.id === 2754 ? "effects" : "area"),
              change: value => applySetting(row.id, value) } } : {}) });
        }
      }
      return;
    }
    if (group === 4 && deathPreview && deathProjection) {
      const entry = deathProjection.items.get(widgetKey(widget));
      if (entry) register(widget, `death-preview-${child}-${widget.index}`, `${entry.kept ? "Kept" : "Lost"}: ${entry.item.name}`, [{
        label: `Check ${escapeText(entry.item.name)}`, run: () => ui.notice(
          `${entry.kept ? "Kept" : "Lost"}: ${entry.item.quantity.toLocaleString("en-US")} x ${entry.item.name}\n${deathPreviewDetails(deathPreview)}`),
      }], { tooltip: `${entry.kept ? "Kept" : "Lost"}: ${entry.item.quantity.toLocaleString("en-US")} x ${entry.item.name}` });
      else if (widget.index === -1 && ([14, 15, 16, 17].includes(child) || op === "Toggle"))
        register(widget, `death-preview-mode-${child}`, "Death preview mode", [{
          label: "Death preview mode", run: () => ui.required("Alternate death preview scenarios", "death_preview_scope_options"),
        }], { tooltip: "The server supplies only normal unsafe non-PvP death information." });
      else if (widget.index === -1 && child === 18 || child === 7 && widget.type === 4)
        register(widget, `death-preview-values-${child}`, "View retrieval fees", [{
          label: "View retrieval fees", run: () => ui.notice(deathPreviewDetails(deathPreview)),
        }], { tooltip: deathPreviewDetails(deathPreview) });
      return;
    }
    if (group === 153 && questReward && child === 8) register(widget, "reward-details", "View reward details", [{
      label: "View reward details", run: () => ui.notice(rewardDetails(questReward, world.player.skills).join("\n")),
    }], { tooltip: rewardDetails(questReward, world.player.skills).join("\n") });
    if (production && productionProjection && group === productionGroup) {
      const recipe = productionProjection.choices.get(widget.id);
      if (recipe && widget.index === -1) {
        const verb = productionChoiceLabel(productionProjection.widgets, widget.id);
        const singleReason = permissionReason(recipe.single, recipe.name), manyReason = permissionReason(recipe.makeX, recipe.name);
        const allReason = permissionReason(recipe.all, "Make-All");
        const make = (quantity: number, mode: "single" | "make_x") =>
          ui.sendUi({ kind: "production_select", menu_id: production.id, recipe: recipe.recipe, quantity, mode });
        const multiple = (quantity: number) => make(quantity, "make_x");
        const all = () => ui.sendUi({ kind: "production_select_all", menu_id: production.id, recipe: recipe.recipe });
        const output = recipe.outputs.map(item => `${item.quantity.toLocaleString("en-US")} x ${item.name}`).join("\n");
        const defaultReason = local.productionAmount === "all" ? allReason : local.productionAmount !== 1 ? manyReason : singleReason;
        const actions: UiAction[] = [{ label: `${verb} ${escapeText(recipe.name)}`,
          run: () => local.productionAmount === "all" ? all() : local.productionAmount === "x" ? ui.prompt(`${verb} how many?`, multiple)
            : make(local.productionAmount, local.productionAmount === 1 ? "single" : "make_x"),
          ...(defaultReason ? { disabled: defaultReason } : {}) },
        { label: `${verb}-1 ${escapeText(recipe.name)}`, run: () => make(1, "single"), ...(singleReason ? { disabled: singleReason } : {}) },
        ...[5, 10].map(quantity => ({ label: `${verb}-${quantity} ${escapeText(recipe.name)}`, run: () => multiple(quantity),
          ...(manyReason ? { disabled: manyReason } : {}) })),
        { label: `${verb}-X ${escapeText(recipe.name)}`, run: () => ui.prompt(`${verb} how many?`, multiple),
          ...(manyReason ? { disabled: manyReason } : {}) },
        { label: `${verb}-All ${escapeText(recipe.name)}`, run: all, ...(allReason ? { disabled: allReason } : {}) },
        { label: "View outputs", run: () => ui.notice(output || "The projection supplies no item outputs.") }];
        const order = [...productionProjection.choices.keys()].indexOf(widget.id);
        const shortcut = group === 270 ? ["space", "2", "3", "4", "5", "6", "7", "8", "9", "0", "a", "b", "c", "d", "e", "f", "g", "h"][order] : undefined;
        register(widget, `production-${recipe.recipe}`, recipe.name, actions,
          { productionRecipe: recipe.recipe, tooltip: `${recipe.name}${output ? "\n" + output : ""}${singleReason ? "\n" + singleReason : ""}`,
            ...(shortcut ? { shortcut } : {}) });
      } else if (widget.index === -1 && group === 270 && [7, 8, 9, 11, 12].includes(child)) {
        const amount = child === 12 ? "all" : child === 7 ? 1 : child === 8 ? 5 : child === 9 ? 10 : "x";
        const label = child === 12 ? "All" : String(amount).toUpperCase();
        register(widget, `production-amount-${label}`, `Production quantity ${label}`, [{
          label: `Quantity: ${label}`, run: () => ui.change(() => { local.productionAmount = amount; }),
        }], { pressed: local.productionAmount === amount });
      } else if (widget.index === -1 && group === 312 && child === 7) {
        register(widget, "production-quantity", "Production quantity", [
          ...[1, 5, 10].map(amount => ({ label: `Quantity: ${amount}`, run: () => ui.change(() => { local.productionAmount = amount; }) })),
          { label: "Quantity: X", run: () => ui.change(() => { local.productionAmount = "x"; }) },
          { label: "Quantity: All", run: () => ui.change(() => { local.productionAmount = "all"; }) },
        ]);
      } else if (widget.index === -1 && group === 312 && op) {
        register(widget, `production-source-${child}`, label, [{ label: `${op} ${escapeText(label)}`,
          disabled: "The authoritative production menu does not offer this source recipe.",
          run: () => ui.notice("The authoritative production menu does not offer this source recipe.", "error") }]);
      }
      return;
    }
    if (group === 161) {
      const index = TABS.findIndex((_, i) => tabWidget(i) === widget.id);
      if (index >= 0) {
        const tab = TABS[index]!, unlocked = isInterfaceUnlocked(world, tab.interface);
        sourceTabRects.set(index, widget);
        const declared = authoritative?.interfaces.find(row => row.interface === tab.interface);
        if (declared?.visibility === "hidden") return;
        register(widget, `tab-${index}`, tab.name, [{ label: tab.name, run: () => ui.openTab(index) },
          ...(index === 11 ? [3, 1, 4].map(category => ({ label: SETTINGS_CATEGORIES[category]!, run: () => openAllSettings(category) })) : [])],
          { pressed: local.tab === index, ...(unlocked ? {} : { disabled: authoritative
            ? permissionReason(declared?.permission, tab.name) ?? "This interface is locked."
            : "This tab has not been unlocked in the tutorial." }) });
      }
      if (widget.contentType === 1338) {
        const status = ui.minimap.status();
        register(widget, "minimap", "Minimap", [{ label: "Walk here", run: () => ui.minimapClick(widget) }],
          { tooltip: ui.minimap.information(), ...(!ui.minimap.enabled || !status
            ? { disabled: ui.minimap.enabled ? "The renderer has not supplied a dynamic minimap." : "The minimap is unavailable in the current source state." } : {}) });
      }
      if (widget.contentType === 1339) register(widget, "compass", "Compass", [{ label: "Face North", run: ui.faceNorth }]);
    }
    if (group === 160) {
      if (child === 26) register(widget, "run", "Toggle run", [{
        label: "Toggle Run", run: () => ui.send({ kind: "set_setting", setting: { setting: "run",
          enabled: !world.player.settings.find(s => s.setting === "run")?.enabled } }),
      }], { pressed: world.player.settings.find(s => s.setting === "run")?.enabled ?? false,
        tooltip: `Run energy: ${Math.floor(world.player.runEnergy / (catalogue.presentation?.runEnergyScale ?? 100))}%` });
      if (child === 18) register(widget, "quick-prayer", "Quick prayers", [
        { label: "Toggle Quick-prayers", run: () => ui.send({ kind: "set_prayer", prayer: "prayer.thick_skin",
          enabled: !thickSkinSelected }),
          ...(authoritative ? permissionReason(authoritative.prayers.find(row => row.id === "prayer.thick_skin")?.permission, "Quick prayers")
            ? { disabled: permissionReason(authoritative.prayers.find(row => row.id === "prayer.thick_skin")?.permission, "Quick prayers")! } : {}
            : world.player.prayerPoints <= 0 ? { disabled: "You have no Prayer points left." } : {}) },
        { label: "Setup Quick-prayers", run: () => ui.change(() => { local.tab = 5; local.quickPrayer = !local.quickPrayer; }) },
      ], { tooltip: `Prayer points: ${world.player.prayerPoints}` });
      if (child === 7) register(widget, "hitpoints", "Hitpoints", [{ label: "Hitpoints", run: () => ui.notice(`Hitpoints: ${world.player.hitpoints}`) }],
        { tooltip: `Hitpoints: ${world.player.hitpoints}` });
      if (child === 34) register(widget, "special-attack", "Special attack", [{ label: "Special attack", run: () => ui.unavailable("Special attacks") }]);
      if (child === 49) register(widget, "world-map", "World map", [{ label: "World map", run: () => ui.unavailable("World map") }]);
      if (child === 52) register(widget, "wiki", "Wiki", [{ label: "Wiki", run: () => ui.unavailable("In-client wiki lookup") }]);
      if (child === 6) register(widget, "xp-drops", "XP drops", [{ label: "XP drops", run: () => ui.unavailable("XP-drop configuration") }]);
    }
    if (group === inventoryGroup && liveItems.has(widgetKey(widget)) && widget.index >= 0) {
      const actions = ui.inventoryActions(widget.index);
      const definition = catalogue.items[widget.item];
      const shifted = definition?.shiftClickDropIndex === -2 ? "Drop"
        : definition && definition.shiftClickDropIndex >= 0 ? definition.interfaceOptions[definition.shiftClickDropIndex] : null;
      const shiftAction = shifted && local.clientInput.shiftDrop ? actions.find(action => plainText(action.label).startsWith(shifted + " ")) : undefined;
      register(widget, `inventory-${widget.index}`, plainText(actions[0]?.label ?? label), actions,
        { draggableSlot: widget.index, ...(shiftAction ? { shiftAction } : {}) });
    }
    if ((group === 387 && child >= 15 && child <= 25 || group === 84 && child >= 10 && child <= 20) && widget.index === -1) {
      const slotIndex = child - (group === 387 ? 15 : 10);
      const slot = `slot.${EQUIPMENT[slotIndex]}`, actions = ui.equipmentActions(slot);
      if (actions.length) register(widget, `equipment-${slot}`, plainText(actions[0]!.label), actions);
      else register(widget, `equipment-${slot}`, `Empty ${EQUIPMENT[slotIndex]} slot`, [], { disabled: "Nothing is equipped in this slot." });
    }
    if (group === 387 && [1, 3, 5, 7].includes(child) && widget.index === -1) {
      register(widget, `equipment-control-${child}`, op, [{ label: op, run: () => {
        if (child === 1 || child === 5) {
          if (authoritative && child === 5) ui.sendUi({ kind: "open_death_preview" });
          else {
            if (!authoritative) ui.change(() => { local.modal = child === 1 ? "equipment-stats" : "kept-items"; });
            ui.send({ kind: "open_interface", interface: child === 1 ? "interface.equipment_stats" : "interface.items_kept_on_death" });
          }
        } else ui.unavailable(op);
      } }]);
    }
    if (group === 320 && child >= 1 && child <= 24 && widget.index === -1) {
      const skill = world.player.skills.find(s => s.id === `skill.${SKILLS[child - 1]}`);
      if (skill) register(widget, `skill-${skill.id}`, skill.name, [{ label: `View ${skill.name} guide`,
        run: () => ui.notice(skillTooltip(skill, catalogue.presentation?.skills[skill.id]?.thresholds)) }],
      { tooltip: skillTooltip(skill, catalogue.presentation?.skills[skill.id]?.thresholds) });
    }
    if (group === 12) {
      if (bankView && child === 10 && widget.index >= 0) {
        const tab = displayedBankTabs.get(widget.index);
        if (tab) {
          const label = tab.tab === 0 ? "All bank items" : `Bank tab ${tab.tab}`;
          register(widget, `bank-tab-${tab.tab}`, label, [
            { label, run: () => ui.sendUi({ kind: "bank_select_tab", tab: tab.tab }) },
            ...(tab.tab === 0 ? [] : [{ label: "Collapse tab", run: () => ui.sendUi({ kind: "bank_collapse_tab", tab: tab.tab }) }]),
          ], { bankTab: tab.tab, pressed: bankView.selectedTab === tab.tab });
        } else if (widget.index === 1000) register(widget, "bank-new-tab", "Create bank tab", [{
          label: "Create bank tab", run: () => ui.notice("Drag a bank entry here, or choose Create tab in its menu."),
        }], { bankCreate: true });
        return;
      }
      if (child === 12 && widget.index >= 0) {
        const dropTab = bankProjection?.dropTabs.get(widget.index);
        if (dropTab !== undefined) {
          register(widget, `bank-section-${dropTab}`, `Bank tab ${dropTab} drop area`, [], { bankTab: dropTab, focusable: false });
          return;
        }
        const entry = displayedBankEntries.get(widget.index);
        if (bankView && !entry) return;
        const actions = entry ? ui.bankEntryActions(entry.id) : ui.bankActions(widget.index);
        register(widget, entry ? `bank-entry-${entry.id}` : `bank-${widget.index}`, plainText(actions[0]?.label ?? label), actions,
          entry ? { bankEntryId: entry.id } : {});
      } else if (op) {
        if (bankView) {
          if (child === 10) return;
          const reason = child === 49 ? permissionReason(bankView.depositEquipment, "Deposit worn items") : undefined;
          register(widget, `bank-control-${child}-${widget.index}`, op, [{ label: op, run: () => {
            if (child === 25) ui.sendUi(bankAmountRequest(bankView, bankView.amountSelection ?? { kind: "quantity", quantity: bankView.amount }, !bankView.noted));
            else if ([29, 31, 33].includes(child)) ui.sendUi(bankAmountRequest(bankView, { kind: "quantity", quantity: child === 29 ? 1 : child === 31 ? 5 : 10 }));
            else if (child === 35) ui.prompt("Set custom quantity:", quantity => ui.sendUi(bankAmountRequest(bankView, { kind: "quantity", quantity })));
            else if (child === 37) ui.sendUi(bankAmountRequest(bankView, { kind: "all" }));
            else if (child === 42) ui.change(() => { local.bankSearchOpen = !local.bankSearchOpen; });
            else if (child === 47) ui.depositAll();
            else if (child === 49) ui.sendUi({ kind: "bank_deposit_equipment" });
            else if (child === 40) ui.sendUi({ kind: "bank_set_placeholders", enabled: !bankView.placeholders });
            else if (child === 23) ui.sendUi({ kind: "bank_set_insert", enabled: !bankView.insertMode });
            else if (child === 45) ui.notice(bankView.unavailableContainers.map(row => permissionReason(row.permission, row.label) ?? row.label).join("\n") ||
              "The projection declares no supported empty-container operation.", "information");
            else if (child === 107) ui.openTab(4);
            else ui.unavailable(op);
          } }], reason ? { disabled: reason } : {});
          return;
        }
        register(widget, `bank-control-${child}-${widget.index}`, op, [{ label: op, run: () => {
          if (child === 25) ui.change(() => { if (world.bank!.allowNotes) local.bankNotes = !local.bankNotes; });
          else if ([29, 31, 33, 37].includes(child)) ui.change(() => { local.bankAmount = child === 29 ? 1 : child === 31 ? 5 : child === 33 ? 10 : "all"; });
          else if (child === 35) ui.prompt("Set custom quantity:", quantity => ui.change(() => { local.bankAmount = quantity; }));
          else if (child === 42) ui.change(() => { local.bankSearchOpen = !local.bankSearchOpen; });
          else if (child === 47) ui.depositAll();
          else if (child === 49) ui.required("Deposit worn items", "bank_deposit_equipment_intent");
          else if (child === 40 || child === 10) ui.required("Bank tabs/placeholders", "bank_tab_and_placeholder_state");
          else if (child === 23) ui.required("Bank insert/swap", "move_bank_intent");
          else if (child === 45) ui.required("Empty containers", "bank_empty_containers_intent");
          else if (child === 107) ui.change(() => { local.tab = 4; });
          else ui.unavailable(op);
        } }], child === 25 && !world.bank!.allowNotes ? { disabled: "This bank does not permit notes." } : {});
      }
    }
    if (group === 300) {
      if (child === 16 && widget.index > 0) {
        const actions = ui.shopActions(widget.index - 1);
        register(widget, `shop-${widget.index - 1}`, plainText(actions[0]?.label ?? label), actions);
      } else if ([5, 8, 10, 12, 14].includes(child) && widget.index === -1) {
        register(widget, `shop-mode-${child}`, op || "Value", [{ label: op || "Value", run: () => ui.change(() => {
          local.shopValue = child === 5;
          if (child !== 5) local.shopAmount = child === 8 ? 1 : child === 10 ? 5 : child === 12 ? 10 : 50;
        }) }], { pressed: child === 5 ? local.shopValue : !local.shopValue && local.shopAmount === (child === 8 ? 1 : child === 10 ? 5 : child === 12 ? 10 : 50) });
      }
    }
    if (group === 541 && widget.index === -1 && widget.name) {
      if (authoritative) {
        const ability = declaredAbility(authoritative, widget);
        if (ability?.visible === false) return;
        const reason = permissionReason(ability?.permission, ability?.name ?? label);
        register(widget, `prayer-${child}`, ability?.name ?? label, [{
          label: `${ability?.selected ? "Deactivate" : "Activate"} ${escapeText(ability?.name ?? label)}`,
          run: () => { if (ability) ui.send({ kind: "set_prayer", prayer: ability.id, enabled: !ability.selected }); },
        }], { pressed: ability?.selected ?? false, ...(reason ? { disabled: reason } : {}), tooltip: reason ?? ability?.name ?? label });
        return;
      }
      const thickSkin = label === "Thick Skin";
      register(widget, `prayer-${child}`, label, [{ label: `${world.player.activePrayers.includes("prayer.thick_skin") && thickSkin ? "Deactivate" : "Activate"} ${label}`,
        run: () => thickSkin ? ui.send({ kind: "set_prayer", prayer: "prayer.thick_skin", enabled: !world.player.activePrayers.includes("prayer.thick_skin") })
          : ui.unavailable(label),
        ...(thickSkin && world.player.prayerPoints <= 0 ? { disabled: "You have no Prayer points left." } : {}),
      }], { pressed: thickSkin && world.player.activePrayers.includes("prayer.thick_skin"),
        tooltip: thickSkin ? `Level 1: Thick Skin\nPrayer points: ${world.player.prayerPoints}` : `${label}\nNot available in this slice.` });
    }
    if (group === 218 && widget.name && (widget.targetVerb || op)) {
      if (authoritative) {
        const ability = declaredAbility(authoritative, widget);
        if (ability?.visible === false) return;
        const reason = permissionReason(ability?.permission, ability?.name ?? label);
        register(widget, `spell-${child}`, ability?.name ?? label, [{
          label: `Cast ${escapeText(ability?.name ?? label)}`,
          run: () => {
            if (!ability) return;
            if (ability.id === "spell.lumbridge_home_teleport") ui.send({ kind: "cast", spell: ability.id, target: null });
            else ui.change(() => { local.selectedItem = null; local.selectedSpell = ability.id; });
          },
        }], { pressed: ability?.selected || local.selectedSpell === ability?.id, ...(reason ? { disabled: reason } : {}), tooltip: reason ?? ability?.name ?? label });
        return;
      }
      const spell = label === "Wind Strike" ? "spell.wind_strike" : label === "Lumbridge Home Teleport" ? "spell.lumbridge_home_teleport" : null;
      const runeCount = (id: string) => world.player.inventory.reduce((total, slot) => total + (slot.item?.id === id ? slot.item.quantity : 0), 0);
      const missing = spell === "spell.wind_strike" && (!runeCount("item.rune.air") || !runeCount("item.rune.mind"));
      register(widget, `spell-${child}`, label, [{ label: `Cast ${label}`, run: () => {
        if (!spell) ui.unavailable(label);
        else if (spell === "spell.lumbridge_home_teleport") ui.send({ kind: "cast", spell, target: null });
        else ui.change(() => { local.selectedItem = null; local.selectedSpell = spell; });
      }, ...(missing ? { disabled: "Wind Strike requires an Air rune and a Mind rune." } : {}) }],
      { pressed: spell !== null && local.selectedSpell === spell,
        tooltip: spell === "spell.wind_strike" ? `Level 1: Wind Strike\nAir rune: ${runeCount("item.rune.air")}/1\nMind rune: ${runeCount("item.rune.mind")}/1` : label });
    }
    if ((group === 541 || group === 218) && widget.text === "Filters") {
      const kind = group === 541 ? "prayer" : "magic";
      register(widget, `filters-${group}`, "Filters", [{ label: "Filters", run: () =>
        ui.change(() => { local.filterPanel = local.filterPanel === kind ? null : kind; }) }]);
    }
    if ((group === 541 && child === 42 || group === 218 && child === 206) && widget.type === 3 && widget.index >= 0) {
      const kind = group === 541 ? "prayer" : "magic", option = widget.index;
      const label = FILTER_OPTIONS[kind][option];
      if (label) {
        const mask = kind === "prayer" ? local.prayerFilters : local.magicFilters;
        register(widget, `filter-${kind}-${option}`, label, [{ label: "Change", run: () => ui.change(() => {
          if (kind === "prayer") local.prayerFilters ^= 1 << option;
          else local.magicFilters ^= 1 << option;
        }) }], { pressed: !(mask & 1 << option),
          ...(!filterOptionEnabled(kind, option, mask) ? { disabled: "Enable tier filtering before changing this dependent option." } : {}) });
      }
    }
    if (group === 182 && (op || widget.text.toLowerCase().includes("logout"))) {
      const logout = /logout/i.test(op || widget.text);
      register(widget, logout ? "logout" : `logout-scope-${widgetKey(widget)}`, logout ? "Logout" : op,
        [{ label: logout ? "Logout" : op, run: logout ? ui.logout : () => ui.unavailable(op) }]);
    }
    if (group === 116 && widget.index === -1 && [58, 66, 67].includes(child)) {
      const page = child === 58 ? "controls" : child === 66 ? "audio" : null;
      const name = page === "audio" ? "Audio settings" : page === "controls" ? "Controls settings" : "Display settings";
      register(widget, `settings-page-${child}`, name, [{ label: name, run: () => page
        ? ui.change(() => { local.settingsPage = page; })
        : ui.required("Display settings", "renderer_display_settings") }], { pressed: page === local.settingsPage });
      return;
    }
    const audioControl = local.settingsPage === "audio" ? audioSourceControl(widget) : undefined;
    if (audioControl) {
      const unavailable = !ui.audio || ui.audio.disposed ? "Bind the actual source audio observer to enable these controls." : undefined;
      const track = child === audioControl.track;
      const label = track ? audioControl.label : `${ui.audio?.percentages[audioControl.channel] === 0 ? "Unmute" : "Mute"} ${audioControl.label}`;
      const tooltip = ui.audio ? track ? audioTooltip(ui.audio, audioControl.channel) : label : unavailable!;
      register(widget, `audio-${track ? "slider" : "mute"}-${audioControl.channel}`, label,
        track ? [] : [{ label, run: () => ui.audioMute(audioControl.channel) },
          ...(audioControl.channel === "master" ? [{ label: ui.audio?.enabled ? "Mute sound" : "Enable sound", run: ui.audioToggle }] : [])],
        { tooltip, ...(unavailable ? { disabled: unavailable } : {}),
          ...(track ? { slider: { value: ui.audio?.percentages[audioControl.channel] ?? null,
            current: () => ui.audioValue(audioControl.channel),
            change: value => ui.audioPercent(audioControl.channel, value) } } : {}) });
      return;
    }
    if (group === 116 && op) {
      register(widget, `settings-${child}-${widget.index}`, op, [{ label: op, run: () => {
        if (/all settings/i.test(op)) {
          openAllSettings();
        } else if (/run/i.test(op)) ui.send({ kind: "set_setting", setting: { setting: "run",
          enabled: !world.player.settings.find(s => s.setting === "run")?.enabled } });
        else if (/music|sound|area/i.test(op)) ui.required("Additional audio preferences", "source_audio_preference_projection");
        else ui.unavailable(op);
      } }]);
    }
    if (group === 239 && widget.index === -1 && [14, 15, 16].includes(child)) {
      const mode = child === 14 ? "area" : child === 15 ? "shuffle" : "single";
      const label = `${mode[0]!.toUpperCase()}${mode.slice(1)} Mode`;
      register(widget, `music-mode-${SOURCE_MUSIC_MODE_IDS[mode]}`, label, [{
        label, run: () => ui.musicAction({ kind: "mode", mode }),
      }, ...(nativeMusic ? [{
        label: nativeMusic.repeatInAreaShuffle ? "Prevent repeated selections outside Single" : "Allow repeated selections outside Single",
        run: () => ui.musicAction({ kind: "flag", flag: "repeatInAreaShuffle", enabled: !nativeMusic.repeatInAreaShuffle }),
      }] : ui.music ? [{ label: ui.music.loopEnabled ? "Disable looping" : "Enable looping",
        run: () => ui.musicAction({ kind: "loop", enabled: !ui.music!.loopEnabled }) },
      ...(mode === "shuffle" ? [{ label: "Play current playlist", run: () => ui.musicAction({ kind: "mode", mode: "playlist" }) }] : [])] : [])],
      { pressed: ui.music?.mode === mode || mode === "shuffle" && ui.music?.mode === "playlist" });
    } else if (group === 239 && child === 17 && widget.index === -1) {
      const available = ui.music?.mode === "shuffle" || ui.music?.mode === "playlist";
      register(widget, "music-skip", "Skip Track", [{ label: "Skip Track", run: () => ui.musicAction({ kind: "skip" }) }],
        available ? {} : { disabled: ui.music ? "Skip Track is available only in Shuffle Mode." : "Supply source music state before using Skip Track." });
    } else if (group === 239 && child === 18 && widget.index === -1) {
      register(widget, "music-list-filter", "Music list", [{ label: "Expand", run: () => ui.change(() => { local.musicDropdown = !local.musicDropdown; }) }]);
    } else if (group === 239 && child === 12 && widget.index === -1 && musicProjection) {
      const thumb = widgets.find(row => row.id === widget.id && row.index === 1)!;
      register(widget, "music-scrollbar", "Music list scroll", [], { scrollbar: {
        value: musicProjection.scroll, maximum: Math.max(0, musicProjection.extent - musicProjection.viewportHeight),
        thumb: thumb.height, page: musicProjection.viewportHeight, current: () => local.scroll,
        change: value => ui.change(() => { local.scroll = value; }),
      } });
    } else if (group === 239 && child === 20 && widget.actions?.includes("Close")) {
      register(widget, "music-filter-close", "Close music list menu", [{ label: "Close", run: () => ui.change(() => { local.musicDropdown = false; }) }]);
    } else if (group === 239 && child === 21 && widget.type === 3 && widget.index > 0) {
      const index = Math.floor((widget.index - 1) / 2);
      const label = index === 0 ? "All music" : `Playlist ${index}`;
      register(widget, `music-filter-${index}`, label, [{ label, run: () => {
        if (nativeMusic && (index === 0 || index === 1 || index === 2 || index === 3)) {
          ui.musicAction({ kind: "select_playlist", selection: index });
          ui.change(() => { local.musicDropdown = false; local.scroll = 0; });
        } else if (index !== 0) ui.notice("Load the actual player-scoped audio record before using numbered playlists.", "error", "ui.audio.preference.binding");
        else {
          ui.change(() => { local.musicDropdown = false; local.scroll = 0; });
          if (ui.music?.mode === "playlist") ui.musicAction({ kind: "mode", mode: "shuffle" });
        }
      } }]);
    } else if (group === 239 && child === 11 && widget.type === 4 && widget.index >= 0) {
      const track = musicProjection?.tracks.get(widget.index);
      if (track) {
        const unlocked = ui.music?.unlockedGroups.includes(track.group) ?? false;
        const unavailable = !ui.music ? "The current source music state has not been supplied." : !unlocked ? "This source track is locked." : undefined;
        register(widget, `music-track-${track.group}`, track.name, [
          { label: `Play ${escapeText(track.name)}`, run: () => ui.musicAction({ kind: "play", group: track.group }),
            ...(unavailable ? { disabled: unavailable } : {}) },
          { label: `Unlock hint ${escapeText(track.name)}`, run: () => ui.notice(`${track.name}\n${track.hint || "No original unlock hint is recorded."}`) },
          ...(!nativeMusic ? [{ label: `${ui.music?.playlistGroups.includes(track.group) ? "Remove from" : "Add to"} current playlist`,
            run: () => ui.musicAction({ kind: ui.music?.playlistGroups.includes(track.group) ? "remove" : "add", group: track.group }),
            ...(unavailable ? { disabled: unavailable } : {}) }] : []),
          ...([1, 2, 3] as const).map(slot => {
            const included = nativeMusic ? sourceSavedPlaylist(nativeMusic, slot).includes(track.group) : false;
            return { label: `${included ? "Remove from" : "Add to"} playlist ${slot}`, run: () => nativeMusic
              ? ui.musicAction({ kind: "edit_playlist", slot, edit: included ? "remove" : "add", group: track.group })
              : ui.notice("Load the actual player-scoped audio record before using numbered playlists.", "error", "ui.audio.preference.binding"),
              ...(unavailable ? { disabled: unavailable } : {}) };
          }),
        ], { pressed: ui.music?.selectedGroup === track.group, tooltip: track.name });
      }
    } else if (group === 239 && op) register(widget, `music-${widgetKey(widget)}`, label || op, [{
      label: label || op, run: () => ui.required(label || "Music control", "source_music_control"),
    }]);
    if ([707, 109, 429, 712, 216].includes(group) && op) register(widget, `scope-${widgetKey(widget)}`, op,
      [{ label: op, run: () => ui.unavailable(op) }]);
    if (group === 399 && widget.type === 4 && widget.text && widget.index >= 0) {
      const name = plainText(widget.text);
      const quest = world.player.quests.find(q => q.name === name);
      register(widget, `quest-${widgetKey(widget)}`, name, [{ label: name, run: () => {
        if (!quest) ui.unavailable(name);
        else {
          ui.change(() => { local.journal = quest.id; local.scroll = 0; });
          ui.send({ kind: "open_interface", interface: "interface.quests" });
        }
      } }]);
    }
  };

  const skills = new Map(world.player.skills.map(s => [s.id, s]));
  const dialogue = world.dialogue;
  const weapon = world.player.equipment.find(e => e.slot === "slot.weapon")?.item;
  const styleIds = weapon?.sourceId == null ? [] : catalogue.presentation?.weapons[weapon.sourceId] ?? [];
  const category = weapon?.sourceId == null ? 0 : catalogue.presentation?.weaponCategories[weapon.sourceId] ?? 0;
  const styleValues = catalogue.combatCategories.find(c => c.columnValues[0]?.[0] === category)?.columnValues[1] ?? [];
  const styleEntries: Array<{ position: number; label: string; tooltip: string; sprite: number }> = [];
  for (let index = 0; index < styleValues.length; index += 4) styleEntries.push({
    position: Number(styleValues[index]), label: String(styleValues[index + 1]),
    tooltip: String(styleValues[index + 2]), sprite: Number(styleValues[index + 3]),
  });
  const styleNames = (id: string) => styleEntries[styleIds.indexOf(id)]?.label ?? "";
  const combatRows = [0, 1, 2, 3].map(index => {
    const sourceId = styleIds[index], sourceName = styleEntries.find(entry => entry.position === index)?.label;
    return authoritative?.combatStyles.find(row => sourceId ? row.id === sourceId : row.name === sourceName);
  });

  const tree = paintNativeTree(raster, widgets, width, height, widget => {
    sourceRectangles.set(widgetKey(widget), widget);
    primaryAction(widget);
    const group = widget.id >> 16, child = widget.id & 65535;
    if (document && documentProjection && group === documentProjection.group) {
      if (group === 392 && child === 76 && ui.hoveredControl === "document-previous") widget.sprite = 2385;
      if (group === 392 && child === 78 && ui.hoveredControl === "document-next") widget.sprite = 2389;
      if ((group === 392 && child === 8 || group === 615 && child === 20) && ui.hoveredControl === "document-close") widget.sprite = 540;
      if (document.mapAsset && group === 615 && child === 1 && widget.type === 6) {
        const native = catalogue.staticModels[staticModelKey(widget)];
        const reference = native && raster.assets.image(native.asset), supplied = raster.assets.image(document.mapAsset);
        if (native && reference && supplied) {
          if (supplied.naturalWidth !== reference.naturalWidth || supplied.naturalHeight !== reference.naturalHeight)
            ui.notice("The document map asset does not match its original model-only dimensions.", "error", "ui.document.asset.size");
          else raster.image(document.mapAsset, widget.x + native.offsetX, widget.y + native.offsetY);
        }
        return true;
      }
    }
    if (group === 134 && settingsProjection && settingsValues) {
      const row = settingsProjection.nodes.get(widgetKey(widget));
      if (row && Number(row.params["1078"] ?? 0) === 0 && widget.type === 5 && widget.width === 18 &&
          settingsValues.values.get(row.id)?.value === null) {
        raster.widget(widget); raster.center("?", widget.x + 9, widget.y + 13, 494, 0xff981f);
        return true;
      }
      if (row && Number(row.params["1078"]) === 9 && widget.onOp && settingsValues.values.get(row.id)?.value === null) {
        raster.widget(widget);
        raster.textBox("?", widget, { font: 494, color: 0xff981f, xAlign: 1, yAlign: 1 });
        return true;
      }
    }
    if (group === 84 && widget.type === 6) {
      const parent = sourceRectangles.get(`${widget.parent}:-1`);
      if (parent) ui.preview(parent, widget);
      return true;
    }
    if (authoritative && (group === 541 || group === 218)) {
      const owner = widget.index >= 0 ? widgets.find(row => row.id === widget.id && row.index === -1) : widget;
      const ability = owner?.name ? declaredAbility(authoritative, owner) : undefined;
      if (ability?.visible === false) return true;
      if (ability && widget.index === -1 && group === 218 && widget.type === 5) {
        const metadata = catalogue.abilities[widget.id];
        if (metadata) {
          const large = widget.width > 24;
          widget.sprite = metadata.sprites[(large ? 2 : 0) + (ability.permission.allowed ? 0 : 1)] ?? widget.sprite;
        }
      }
    }
    if (widget.contentType === 1337) return true;
    if (ui.minimap.draw(widget, world.player.tile, world)) return true;
    if (group === 161 && widget.type === 5) {
      const tabIndex = TABS.findIndex((_, index) => widget.id === tabWidget(index));
      if (authoritative && tabIndex >= 0) {
        const declared = authoritative.interfaces.find(row => row.interface === TABS[tabIndex]!.interface);
        if (declared?.visibility === "hidden") return true;
      }
      for (const [slot, rect] of sourceTabRects) if (!isInterfaceUnlocked(world, TABS[slot]!.interface) &&
        widget.id !== tabWidget(slot) && widget.x >= rect.x && widget.y >= rect.y &&
        widget.x + widget.width <= rect.x + rect.width && widget.y + widget.height <= rect.y + rect.height) return true;
    }
    const liveItem = liveItems.get(widgetKey(widget));
    if (liveItem?.iconAsset) {
      raster.image(liveItem.iconAsset, widget.x, widget.y);
      if (liveItem.quantity > 1) {
        const quantity = countText(world.shop && group === 300 ? widget.item_quantity : liveItem.quantity);
        raster.text(quantity.text, widget.x, widget.y + 9, 494, quantity.color, 1);
      }
      return true;
    }
    if (group === 160) {
      if (child === 10) widget.text = String(world.player.hitpoints);
      if (child === 21) widget.text = String(world.player.prayerPoints);
      if (child === 29) widget.text = String(Math.floor(world.player.runEnergy / (catalogue.presentation?.runEnergyScale ?? 100)));
      if (child === 37) widget.text = "";
      if (child === 32 && world.player.runEnergy > 0) return true;
      if (child === 24 && world.player.prayerPoints > 0) return true;
    }
    if (group === 320) {
      const skill = skills.get(`skill.${SKILLS[child - 1]}`);
      if (widget.type === 4 && widget.index === 4) widget.text = skill ? String(skill.currentLevel) : "?";
      if (widget.type === 4 && widget.index === 5) widget.text = skill ? String(skill.baseLevel) : "?";
      if (child === 32) widget.text = `Total level: ${world.player.skills.reduce((n, s) => n + s.baseLevel, 0)}`;
    }
    if (group === 593) {
      if (child === 3) widget.text = weapon ? escapeText(weapon.name) : "Unarmed";
      if ([6, 10, 14, 18].includes(child) && widget.index === -1) {
        const index = [6, 10, 14, 18].indexOf(child), style = styleIds[index];
        if (authoritative) {
          const ability = combatRows[index];
          if (ability?.visible === false) return true;
          const reason = permissionReason(ability?.permission, ability?.name ?? "Combat style");
          register(widget, `combat-style-${index}`, ability?.name ?? "Combat style unavailable", [{
            label: ability?.name ?? "Combat style unavailable", run: () => { if (ability) ui.send({ kind: "set_combat_style", style: ability.id }); },
          }], { pressed: ability?.selected ?? false, ...(reason ? { disabled: reason } : {}) });
          const skin = widgets.find(row => row.id === widgetId(593, ability?.selected ? 6 : 10) && row.index === -1);
          if (skin) widget.sprite = skin.sprite;
          raster.widget(widget); return true;
        }
        if (style) register(widget, `combat-style-${index}`, styleNames(style), [{ label: styleNames(style), run: () => ui.send({ kind: "set_combat_style", style }) }],
          { tooltip: `${styleNames(style)}\nThe server selects and validates the combat style.` });
        else if (weapon && styleIds.length && index >= styleIds.length) return true;
        else register(widget, `combat-style-${index}`, ["Punch", "Kick", "Block", "Block"][index]!,
          [{ label: "Combat style", run: () => ui.required("Combat style", "unarmed_combat_styles") }]);
        const neutral = widgets.find(w => w.id === widgetId(593, 10) && w.index === -1);
        if (neutral && widget.type === 5) widget.sprite = neutral.sprite;
      }
      if ([7, 11, 15, 19].includes(child) && widget.type === 5) {
        const entry = styleEntries.find(style => style.position === [7, 11, 15, 19].indexOf(child));
        if (!entry) return true;
        widget.sprite = entry.sprite;
      }
      if ([9, 13, 17, 21].includes(child)) {
        const index = [9, 13, 17, 21].indexOf(child);
        widget.text = authoritative ? combatRows[index]?.visible ? escapeText(combatRows[index]!.name) : ""
          : styleIds[index] ? styleNames(styleIds[index]!) : weapon && styleIds.length ? "" : ["Punch", "Kick", "Block", ""][index]!;
      }
      if (child === 32) register(widget, "auto-retaliate", "Auto retaliate", [{ label: "Auto retaliate", run: () => ui.send({
        kind: "set_setting", setting: { setting: "auto_retaliate", enabled: !world.player.settings.find(s => s.setting === "auto_retaliate")?.enabled },
      }) }], { pressed: world.player.settings.find(s => s.setting === "auto_retaliate")?.enabled ?? false });
      if (child === 36) widget.text = `Auto Retaliate<br>(${world.player.settings.find(s => s.setting === "auto_retaliate")?.enabled ? "On" : "Off"})`;
    }
    if (group === 541 && widget.type === 4 && /^\d+ \/ \d+$/.test(widget.text)) {
      widget.text = `${world.player.prayerPoints} / ${skills.get("skill.prayer")?.baseLevel ?? "?"}`;
    }
    if (group === 399 && widget.type === 4) {
      const quest = world.player.quests.find(q => q.name === plainText(widget.text));
      if (quest) widget.color = quest.completed ? 0x00ff00 : /not_started|unstarted/i.test(quest.stage) ? 0xff0000 : 0xffff00;
      if (/Quest Points:/.test(widget.text)) widget.text = `Quest Points: ${world.player.questPoints}/${catalogue.questTable.maximum_points}`;
      if (/Completed:/.test(widget.text)) widget.text = `Completed: ${world.player.quests.filter(q => q.completed).length}/${catalogue.questTable.available}`;
    }
    if (group === 12) {
      if (child === 3) widget.text = bankView && bankView.selectedTab !== 0 ? `Tab ${bankView.selectedTab}` : "The Bank of Gielinor";
      if (child === 5) widget.text = String(bankView ? bankView.entries.length : world.bank!.slots.filter(slot => slot.item).length);
      if (child === 8) widget.text = "";
      if (child === 5) register(widget, "bank-space", "Bank space", [{
        label: "Bank space", run: () => ui.notice(`${bankView ? bankView.entries.length : world.bank!.slots.filter(slot => slot.item).length} / ${bankView?.capacity ?? world.bank!.capacity}`),
      }], { tooltip: `Bank capacity: ${bankView?.capacity ?? world.bank!.capacity}` });
      if (child === 25) widget.sprite = (bankView ? bankView.noted : local.bankNotes) ? 179 : 170;
      if (bankView && child === 23) widget.sprite = 170;
      if (bankView && child === 24) widget.sprite = bankView.insertMode ? 2820 : 2821;
      if (bankView && child === 40) widget.sprite = bankView.placeholders ? 179 : 170;
      if ([29, 31, 33, 35, 37].includes(child)) {
        const amount = bankView ? bankDefaultAmount(bankView) : local.bankAmount;
        const selected = child === 29 ? amount === 1 : child === 31 ? amount === 5 : child === 33 ? amount === 10
          : child === 37 ? amount === "all" : typeof amount === "number" && ![1, 5, 10].includes(amount);
        widget.sprite = selected ? 179 : 170;
      }
    }
    if (group === 300 && child === 1 && widget.type === 4) widget.text = escapeText(world.shop!.name);
    if (group === 116 && widget.type === 4 && /^\d+%$/.test(widget.text))
      widget.text = `${Math.floor(world.player.runEnergy / (catalogue.presentation?.runEnergyScale ?? 100))}%`;
    if (group === 162 && widget.type === 4 && widget.text.includes("Reference")) widget.text = escapeText(world.player.displayName) + ":";
    if (group === 231 && dialogue) {
      if (child === 4) widget.text = escapeText(dialogue.speakerName);
      if (child === 6) {
        const lines = sourceLines(dialogue.text, widget.width, catalogue.fonts[widget.font]!);
        const pageSize = Math.max(1, Math.floor(widget.height / (widget.lineHeight || catalogue.fonts[widget.font]!.ascent)));
        widget.text = lines.slice(local.dialoguePage * pageSize, (local.dialoguePage + 1) * pageSize).join("<br>");
      }
      if (child === 5) {
        const body = widgets.find(w => w.id === widgetId(231, 6))!;
        const lines = sourceLines(dialogue.text, body.width, catalogue.fonts[body.font]!);
        const more = (local.dialoguePage + 1) * Math.floor(body.height / (body.lineHeight || catalogue.fonts[body.font]!.ascent)) < lines.length;
        const single = dialogue.choices.length === 1 ? dialogue.choices[0] : null;
        widget.text = more || single ? "Click here to continue" : dialogue.choices.length ? "" : "Waiting for server...";
        if (more || single) register(widget, "dialogue-continue", "Continue dialogue", [{ label: "Continue", run: () => {
          if (more) ui.change(() => { local.dialoguePage++; });
          else if (single) ui.send({ kind: "select_dialogue", speaker: dialogue.speaker, choice: single.id });
        } }]);
      }
      if (widget.type === 6) {
        const entity = world.entities.find(e => e.id === dialogue.speaker);
        const portrait = entity?.sourceId !== null && entity?.sourceId !== undefined ? catalogue.portraits[`npc-${entity.sourceId}`] : null;
        if (dialogue.portraitAsset) raster.image(dialogue.portraitAsset, widget.x + (portrait?.offsetX ?? 0), widget.y + (portrait?.offsetY ?? 0));
        else if (portrait) raster.image(portrait.asset, widget.x + portrait.offsetX, widget.y + portrait.offsetY);
        return true;
      }
    }
    if (group === 119 && widget.type === 4) {
      const quest = world.player.quests.find(q => q.id === local.journal);
      if (quest) {
        if (widget.height < 35) widget.text = escapeText(quest.name);
        else widget.text = quest.journal;
      } else widget.text = "";
    }
    if (group === 153 && widget.type === 4 && !questReward) widget.text = "";
    if (group === 84 && authoritative && widget.type === 4) {
      const bonuses = authoritative.equipment.bonuses;
      const names = ["Stab", "Slash", "Crush", "Magic", "Ranged"];
      const key = names[(child >= 30 ? child - 30 : child - 24)]?.toLowerCase();
      const signed = (value: number) => value >= 0 ? "+" + value : String(value);
      if (child >= 24 && child <= 28 && key) widget.text = `${names[child - 24]}: ${bonuses.attack[key] === undefined ? "Unavailable" : signed(bonuses.attack[key])}`;
      else if (child >= 30 && child <= 34 && key) widget.text = `${names[child - 30]}: ${bonuses.defence[key] === undefined ? "Unavailable" : signed(bonuses.defence[key])}`;
      else if (child === 36) widget.text = `Melee strength: ${signed(bonuses.meleeStrength)}`;
      else if (child === 37) widget.text = `Ranged strength: ${signed(bonuses.rangedStrength)}`;
      else if (child === 38) widget.text = `Magic damage: ${signed(bonuses.magicDamagePercent)}%`;
      else if (child === 39) widget.text = `Prayer: ${signed(bonuses.prayer)}`;
      else if (child === 51) {
        const full = `${formatUiFixed(authoritative.equipment.weightGrams, 3)} kg`;
        widget.text = raster.measure(full, widget.font) > widget.width ? "View weight" : full;
        register(widget, "equipment-weight", "Equipment weight", [{
          label: "View weight", run: () => ui.notice(`${full}\n${formatUiInteger(authoritative.equipment.weightGrams)} grams`),
        }], { tooltip: `${formatUiInteger(authoritative.equipment.weightGrams)} grams` });
      }
    } else if ([84, 4].includes(group) && widget.type === 4 && /\d/.test(widget.text)) widget.text = "";
    raster.widget(widget);
    return true;
  });
  const modalGroup = banking ? 12 : world.shop ? 300 : modal ? MODALS[modal] : null;
  if (modalGroup) {
    const frame = tree.find(widget => widget.id === widgetId(modalGroup, 0) && widget.index === -1);
    if (frame) ui.capture(intersect(frame, frame.clip));
  }
  if (local.tab === 11 && local.settingsPage === "audio" && !ui.audio)
    for (const control of controls.filter(control => control.id.startsWith("audio-slider-")))
      raster.textBox("Unavailable", control, { font: 494, color: 0xffff00, xAlign: 1, yAlign: 1 });
  if (settingsProjection) {
    if (local.settings.search !== null) {
      const box = tree.find(widget => widget.id === widgetId(134, 11) && widget.index === -1)!;
      inputs.length = 0;
      inputs.push({ ...box, id: "settings-search", label: "Search settings", type: "text", autocomplete: "off",
        value: local.settings.search, maximum: 80, change: value => ui.change(() => { local.settings.search = value; local.settings.scroll = 0; }),
        submit: () => ui.focusInput("settings-search") });
    }
    const choice = local.settings.choice;
    if (choice) {
      const row = catalogue.settingsRows[choice.setting]!, values = catalogue.settingsDefinitions.choices[String(row.params["1091"])];
      if (values?.stringVals) {
        const rowHeight = values.keys.length === 2 ? 24 : 20, visible = Math.min(120, values.keys.length * rowHeight);
        const bottom = tree.find(widget => widget.id === widgetId(134, 14) && widget.index === -1)!;
        const box = { x: choice.x, y: choice.y + choice.height + visible + 4 > bottom.y + bottom.height ? choice.y - visible - 4 : choice.y + choice.height,
          width: choice.width, height: visible + 4 };
        raster.fill(box, 0x0e0e0c); raster.border({ x: box.x + 1, y: box.y + 1, width: box.width - 2, height: box.height - 2 }, 0x474745);
        controls.length = 0; inputs.length = 0;
        const maximum = Math.max(0, values.keys.length * rowHeight - visible);
        choice.scroll = Math.min(choice.scroll, maximum);
        const first = Math.floor(choice.scroll / rowHeight);
        values.keys.slice(first, first + Math.floor(visible / rowHeight)).forEach((key, offset) => {
          const index = first + offset;
          const bounds = { x: box.x + 2, y: box.y + 2 + offset * rowHeight, width: box.width - 4 - (maximum ? 16 : 0), height: rowHeight };
          raster.textBox(escapeText(values.stringVals![index]!), bounds, { font: 495, color: 0xff981f, xAlign: 1, yAlign: 1 });
          controls.push({ ...bounds, id: `settings-option-${key}`, label: values.stringVals![index]!,
            actions: [{ label: "Select", run: () => { local.settings.choice = null; applySetting(row.id, key); ui.change(() => {}); } }] });
        });
        if (maximum) {
          const bar = { x: box.x + box.width - 18, y: box.y + 2, width: 16, height: visible };
          const track = Math.max(10, visible - 32), thumb = Math.max(10, Math.trunc(visible * track / (values.keys.length * rowHeight)));
          const offset = 16 + Math.trunc((visible - 32 - thumb) * choice.scroll / maximum);
          raster.sprite(792, bar.x, bar.y + 16, { width: 16, height: track, tiling: true });
          raster.sprite(790, bar.x, bar.y + offset, { width: 16, height: thumb, tiling: true });
          raster.sprite(789, bar.x, bar.y + offset); raster.sprite(791, bar.x, bar.y + offset + thumb - 5);
          raster.sprite(773, bar.x, bar.y); raster.sprite(788, bar.x, bar.y + visible - 16);
          controls.push({ ...bar, id: "settings-option-scroll", label: "Setting choices scroll", actions: [],
            scrollbar: { value: choice.scroll, maximum, thumb, page: visible, current: () => choice.scroll,
              change: value => ui.change(() => { choice.scroll = value; }) } });
        }
      } else ui.required(row.label, "source_setting_choices");
    }
  }

  // Empty inventory cells remain real drop targets; they are not fabricated item widgets.
  const inventoryRoot = tree.find(w => w.id >> 16 === inventoryGroup && w.index === -1);
  if (inventoryRoot) {
    const first = tree.find(w => w.id >> 16 === inventoryGroup && w.index >= 0 && w.type === 5);
    const offsetX = first ? first.x - inventoryRoot.x - first.index % 4 * 42 : 16;
    const offsetY = first ? first.y - inventoryRoot.y - Math.floor(first.index / 4) * 36 : 8;
    for (let index = 0; index < 28; index++) {
      if (controls.some(c => c.id === `inventory-${index}`)) continue;
      const rect = { x: inventoryRoot.x + offsetX + index % 4 * 42, y: inventoryRoot.y + offsetY + Math.floor(index / 4) * 36, width: 36, height: 32 };
      controls.push({ ...rect, id: `inventory-${index}`, label: `Empty inventory slot ${index + 1}`, actions: [],
        draggableSlot: index, focusable: false });
    }
  }
  if (!dialogue && !levelReward && productionGroup !== 270) {
    const publicLines = authoritative?.publicChat.messages.map(message => `${message.sender}: ${message.text}`) ?? [];
    const lines = [...world.messages.map(message => message.text), ...publicLines].slice(-7).map(escapeText);
    lines.forEach((line, index) => raster.textBox(line, { x: 7, y: height - 164 + index * 14, width: 485, height: 14 },
      { font: 495, color: 0, shadow: null, lineHeight: 14 }));
    if (world.player.tutorialInstruction && world.player.tutorialStage !== "stage.tutorial.mainland") {
      raster.sprite(1017, 0, height - 165);
      raster.textBox(world.player.tutorialInstruction, { x: 14, y: height - 154, width: 481, height: 112 },
        { font: 495, color: 0, shadow: null, lineHeight: 16, xAlign: 1, yAlign: 1 });
    }
    if (authoritative && !local.bankSearchOpen && !local.amount && !authoritative.reward && !authoritative.confirmation) {
      const prefix = `${world.player.displayName}: `;
      const x = 7 + raster.measure(escapeText(prefix), 495);
      const rect = { x, y: height - 45, width: Math.max(1, 487 - x + 7), height: 19 };
      const reason = permissionReason(authoritative.publicChat.permission, "Public chat");
      raster.clip(rect, () => raster.text(escapeText(local.chatDraft) + "*", x, height - 30, 495, reason ? 0x777777 : 0x0000ff, null));
      inputs.push({ ...rect, id: "public-chat", label: "Public chat", type: "text", autocomplete: "off",
        value: local.chatDraft, maximum: authoritative.publicChat.maximumBytes, disabled: Boolean(reason),
        change: value => ui.change(() => { local.chatDraft = value; }), submit: ui.sendChat });
      if (reason) controls.push({ ...rect, id: "chat-permission", label: "Public chat unavailable",
        actions: [{ label: "Public chat unavailable", run: () => ui.notice(reason, "error", authoritative.publicChat.permission.code ?? undefined) }] });
    } else if (!authoritative) {
      raster.text("game.ui.v1 unavailable - legacy interface", 7, height - 30, 494, 0x800000, null);
      controls.push({ x: 7, y: height - 45, width: 487, height: 19, id: "chat-input", label: "Unsupported game.ui.v1",
        actions: [{ label: "Unsupported game.ui.v1", run: () => ui.notice(gameplayUiProblem(world)!.message, "error", "ui.capability.game.ui.v1") }] });
    }
  }
  if (dialogue && !levelReward && dialogue.choices.length > 1) {
    raster.sprite(1017, 0, height - 165);
    raster.center("Select an Option", 259, height - 143, 496, 0, null);
    const lineHeight = Math.min(25, Math.floor(100 / dialogue.choices.length));
    const start = height - 130;
    dialogue.choices.forEach((choice, index) => {
      const rect = { x: 16, y: start + index * lineHeight, width: 485, height: lineHeight };
      raster.textBox(choice.text, rect, { font: 496, color: 0x0000ff, shadow: null, xAlign: 1, yAlign: 1 });
      controls.push({ ...rect, id: `dialogue-choice-${index}`, label: plainText(choice.text),
        actions: [{ label: choice.text, run: () => ui.send({ kind: "select_dialogue", speaker: dialogue.speaker, choice: choice.id }) }] });
    });
  }
  if (local.bankSearchOpen && banking) {
    raster.sprite(1017, 0, height - 165);
    raster.center("Show items whose names contain:", 259, height - 125, 496, 0, null);
    raster.center(escapeText(local.bankSearch) + "<col=0000ff>*</col>", 259, height - 93, 496, 0, null);
    inputs.push({ x: 100, y: height - 114, width: 320, height: 29, id: "bank-search", label: "Search bank",
      type: "text", autocomplete: "off", value: local.bankSearch, maximum: 120,
      change: value => ui.change(() => { local.bankSearch = value; local.scroll = 0; }),
      submit: () => ui.change(() => { local.bankSearchOpen = false; }) });
    controls.push({ x: 16, y: height - 60, width: 486, height: 25, id: "bank-search-close", label: "Close bank search",
      actions: [{ label: "Close search", run: () => ui.change(() => { local.bankSearchOpen = false; }) }] });
    raster.center("Click here to continue", 259, height - 42, 495, 0x0000ff, null);
  }
  if (modal === "journal") {
    const quest = world.player.quests.find(q => q.id === local.journal);
    const content = tree.filter(w => w.id >> 16 === 119 && w.type === 0 && w.height > 100).sort((a, b) => a.width * a.height - b.width * b.height)[0];
    if (quest && content) {
      const rect = { x: content.x + 16, y: content.y + 42, width: content.width - 42, height: content.height - 58 };
      const lines = sourceLines(quest.journal, rect.width, catalogue.fonts[495]!);
      const first = Math.min(Math.floor(local.scroll / 16), Math.max(0, lines.length - Math.floor(rect.height / 16)));
      raster.textBox(lines.slice(first).join("<br>"), rect, { font: 495, color: 0, shadow: null, lineHeight: 16 });
    }
  }
  if (modal === "appearance" || modal === "experience") {
    const group = MODALS[modal]!, frame = tree.find(w => w.id === widgetId(group, 0));
    if (frame) {
      const candidates = tree.filter(w => w.id >> 16 === group &&
        (w.actions?.some(Boolean) || modal === "appearance" && w.index === -1 && [68, 69].includes(w.id & 65535)));
      for (const widget of candidates) {
        const child = widget.id & 65535;
        const rowLabel = tree.find(w => w.parent === widget.parent && w.type === 4 && w.text)?.text;
        const name = modal === "appearance" && [68, 69].includes(child) ? `Body type ${child === 68 ? "A" : "B"}`
          : modal === "appearance" && rowLabel && widget.type === 0 && child !== 74
            ? `${widget.x < (tree.find(w => w.parent === widget.parent && w.type === 4)?.x ?? widget.x) ? "Previous" : "Next"} ${plainText(rowLabel)}`
            : normalizedName(widget);
        if (modal === "appearance") {
          register(widget, `appearance-${widgetKey(widget)}`, name || "Appearance control", [{ label: name || "Appearance control", run: () => {
            if (child === 74) ui.confirmAppearance();
            else if (child === 68 || child === 69) ui.change(() => { local.appearance.body_type = child === 68 ? 0 : 1; });
            else ui.required(name || "Appearance options", "appearance_options");
          } }], [68, 69].includes(child) ? { pressed: local.appearance.body_type === (child === 68 ? 0 : 1) } : {});
        }
      }
      if (modal === "experience") {
        const choices = catalogue.presentation?.experiences ?? [];
        candidates.forEach(candidate => {
          const label = tree.find(w => w.id === candidate.id && w.type === 4 && w.text)?.text;
          const choice = choices.find(c => c.name === label);
          if (!choice) return;
          register(candidate, `experience-${choice.id}`, choice.name, [{ label: choice.name,
            run: () => ui.send({ kind: "select_experience", experience: choice.id }) }]);
        });
      }
    }
  }
  if (recovery && world.recovery) {
    const snapshot = world.recovery;
    const dispatch = (command: RecoveryUiCommand) => {
      if (command.kind === "select") ui.change(() => { local.recoverySelected = command.id; });
      else if (command.kind === "close") ui.send({ kind: "close_interface" });
      else if (command.kind === "examine") {
        const item = snapshot.items.find(row => row.id === command.id)?.item;
        if (item) ui.notice(item.sourceId === null ? item.name : catalogue.items[item.sourceId]?.examine || item.name);
      } else if (command.kind === "take_all") {
        if (recoveryPanel) ui.sendUi({ kind: "recovery_take", death: recoveryPanel.death, storage: recoveryPanel.storage,
          items: recoveryPanel.entries.map(row => ({ id: row.id, amount: { kind: "all" } })) });
        else ui.send({ kind: "reclaim", death: snapshot.death, storage: snapshot.storage, items: snapshot.items.map(row => row.id) });
      }
      else if (command.kind === "discard_all") ui.sendUi({ kind: "request_recovery_discard",
        death: snapshot.death, storage: snapshot.storage, items: snapshot.items.map(row => row.id) });
      else if (command.kind === "retrieve") {
        const item = snapshot.items.find(row => row.id === command.id);
        if (recoveryPanel) {
          const take = (amount: import("../shared/contracts.ts").UiAmount) => ui.sendUi({
            kind: "recovery_take", death: recoveryPanel.death, storage: recoveryPanel.storage, items: [{ id: command.id, amount }],
          });
          if (command.amount === "x") ui.prompt("Retrieve how many?", quantity => take({ kind: "quantity", quantity }));
          else take(command.amount === "all" ? { kind: "all" } : { kind: "quantity", quantity: command.amount });
        } else if (item && (command.amount === "all" || typeof command.amount === "number" && command.amount >= item.item.quantity))
          ui.send({ kind: "reclaim", death: snapshot.death, storage: snapshot.storage, items: [command.id] });
        else ui.required("Partial-quantity retrieval", "reclaim_quantity");
      } else if (management) ui.sendUi({ kind: "recovery_bank_all", records: structuredClone(management.bankAllRecords) });
      else ui.required("Bank-All", "recovery_bank_all");
    };
    controls.push(...recoveryControls(widgets, width, height, recovery, dispatch));
  }
  if (!authoritative && (modal === "equipment-stats" || modal === "kept-items" || modal === "reward")) {
    const frame = tree.find(w => w.id === widgetId(MODALS[modal]!, 0));
    if (frame) {
      const message = modal === "equipment-stats" ? "Equipment bonuses have not been supplied by the server."
        : modal === "kept-items" ? "Items kept on death have not been supplied by the server."
          : "The server has not supplied a reward breakdown.";
      raster.textBox(message, { x: frame.x + 30, y: frame.y + 45, width: frame.width - 60, height: 70 },
        { font: 495, color: 0xffff00, xAlign: 1, yAlign: 1, lineHeight: 16 });
    }
  }
  // Native chat channel controls keep their original positions, including out-of-scope channels.
  const channels = ["All", "Game", "Public", "Private", "Channel", "Clan", "Trade", "Report"];
  for (const widget of tree.filter(w => w.id >> 16 === 162 && w.type === 4 && channels.includes(plainText(w.text)))) {
    const name = plainText(widget.text);
    register(widget, `chat-${name}`, name, [{ label: name, run: () => ui.unavailable(`${name} chat controls`) }]);
  }
  if (local.selectedItem || local.selectedSpell) {
    const label = local.selectedItem ? `Use ${escapeText(local.selectedItem.name)} ->` : "Cast Wind Strike ->";
    raster.text(label, 4, 15, 496, 0xffffff);
  }
  if (settingsProjection) {
    for (let index = inputs.length - 1; index >= 0; index--)
      if (inputs[index]!.id !== "settings-search" || local.settings.choice) inputs.splice(index, 1);
    if (local.settings.choice) for (let index = controls.length - 1; index >= 0; index--)
      if (!controls[index]!.id.startsWith("settings-option-")) controls.splice(index, 1);
  }
}
