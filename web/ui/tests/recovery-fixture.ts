import type { ItemView, RecoveryContextView, UiPermission } from "../../shared/contracts.ts";
import { fixtureUi, fixtureWorld } from "./component-fixture.ts";

export function recoveryContextFixture(empty = false) {
  const base = fixtureWorld(), world = { ...base, ui: fixtureUi(base) };
  const allowed: UiPermission = { allowed: true, code: null, reason: null };
  const denied: UiPermission = { allowed: false, code: "InvalidTarget", reason: "There are no items in this source recovery context." };
  const arrow: ItemView = {
    id: "item.arrow.bronze", sourceId: 882, name: "Bronze arrow", quantity: 7,
    actions: [], iconAsset: null, instanceId: null, charges: null,
  };
  const first = { id: "entry.one", item: { ...arrow }, unitFee: "42", fullStackFee: "294",
    inventoryCapacity: 7, bankCapacity: 7, take: { ...allowed }, bank: { ...allowed } };
  const second = { id: "entry.two", item: { ...arrow }, unitFee: "44", fullStackFee: "308",
    inventoryCapacity: 7, bankCapacity: 7, take: { ...allowed }, bank: { ...allowed } };
  const context: RecoveryContextView = {
    version: 1, identity: { kind: "death_office", interface: "interface.death_retrieval", instance: null },
    counts: { entries: empty ? 0 : 2, nativeItemTypes: empty ? 0 : 1, capacity: 120,
      capacityUnit: "item_types_or_instances", stored: empty ? 0 : 1, offered: empty ? 0 : 1 },
    slots: empty ? [] : [
      { slot: 0, death: "death.one", currentStorage: "death_office", entry: first,
        selectedTypeCaption: { kind: "source", sourceId: 882, quantity: "14", unitFee: "42", totalFee: "588" } },
      { slot: 1, death: "death.two", currentStorage: "grave", entry: second,
        selectedTypeCaption: { kind: "source", sourceId: 882, quantity: "14", unitFee: "44", totalFee: "616" } },
    ],
    takeAll: {
      permission: { ...(empty ? denied : allowed) },
      selection: {
        context: { kind: "death_office", interface: "interface.death_retrieval", instance: null },
        records: empty ? [] : [
          { death: "death.one", entries: [{ id: "entry.one", quantity: 7, current_storage: "death_office" }] },
          { death: "death.two", entries: [{ id: "entry.two", quantity: 7, current_storage: "grave" }] },
        ],
      },
      plan: empty ? null : { totalFee: "602", partial: false, transfers: [
        { death: "death.one", id: "entry.one", quantity: 7, fee: "294" },
        { death: "death.two", id: "entry.two", quantity: 7, fee: "308" },
      ] },
    },
  };
  world.ui.activeInterface = context.identity.interface;
  world.recovery = empty ? null : { death: "death.one", storage: "death_office", remainingTicks: null,
    items: [{ id: first.id, item: first.item, cost: null }] };
  world.ui.recovery = {
    cofferBalance: "9007199254740993", discard: { ...allowed }, cofferOffer: { ...allowed }, cofferItems: [],
    management: {
      context, bankRevision: "9007199254740997",
      panels: empty ? [] : [
        { death: "death.one", storage: "death_office", fullSelectionFee: "294", takeAll: { ...allowed }, entries: [first] },
        { death: "death.two", storage: "death_office", fullSelectionFee: "308", takeAll: { ...allowed }, entries: [second] },
      ],
      bankAll: { ...(empty ? denied : allowed) },
      bankAllRecords: empty ? [] : [{ death: "death.one", items: ["entry.one"] }, { death: "death.two", items: ["entry.two"] }],
    },
  };
  return { world, context };
}
