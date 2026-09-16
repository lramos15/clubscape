import assert from "node:assert/strict";
import { test } from "node:test";
import { GAMEPLAY_UI_CAPABILITY, UI_AMOUNTS_CAPABILITY, UI_RECOVERY_CAPABILITY } from "../../shared/contracts.ts";
import type { GameplayUiView, WorldView } from "../../shared/contracts.ts";
import { captureUiBankRevision, gameplayUiSupport, validateActorObservers, validateGameplayUi, validateUiAmount } from "../gameplay-ui.ts";
import { deepFreeze } from "../errors.ts";

// Version/projection fixtures only. They are never attached to the real browser/server world.
function view(): GameplayUiView {
  return {
    version: 1, activeTab: "interface.inventory", activeInterface: null, document: null, production: {
      id: "menu.original", interface: "interface.production", target: { kind: "spawn", spawn: "spawn.original" },
      recipes: [],
    }, reward: {
      id: "presentation.original", kind: "quest", interface: "interface.reward", title: "Fixture", lines: [], items: [],
      xp: [{ skill: "skill.fixture", amountTenths: "18446744073709551615" }], questPoints: 1, quest: "quest.fixture",
      skill: null, level: null, continuation: { kind: "ui_dismiss", presentation_id: "presentation.original" },
    }, confirmation: {
      id: "confirmation.original", kind: "coffer", title: "Fixture", lines: [], items: [], credit: "9007199254740993",
    }, interfaces: [], combatStyle: null, combatStyles: [], prayers: [], spells: [],
    equipment: {
      bonuses: { attack: {}, defence: {}, meleeStrength: 0, rangedStrength: 0, magicDamagePercent: 0, prayer: 0 },
      weightGrams: "18446744073709551615", slots: [],
    }, inventoryActions: [], bank: {
      revision: "9007199254740993", capacity: 1, selectedTab: 0, insertMode: false, placeholders: true,
      amount: 1, noted: false, tabs: [{ tab: 0, firstEntry: "entry.original", entries: 1 }],
      entries: [{ id: "entry.original", slot: 0, tab: 0, item: "item.fixture", value: null, placeholder: true }],
      depositEquipment: { allowed: false, code: "unavailable", reason: "Fixture only" }, unavailableContainers: [],
    }, keptOnDeath: {
      scope: "normal_unsafe_non_pvp", kept: [], lost: [], fullGraveFee: "9007199254740993",
      fullOfficeFee: "18446744073709551615", valueRevision: "9007199254740994",
    }, recovery: {
      cofferBalance: "18446744073709551615",
      discard: { allowed: false, code: "unavailable", reason: "Fixture only" },
      cofferOffer: { allowed: false, code: "unavailable", reason: "Fixture only" }, cofferItems: [],
    }, appearance: { choices: {}, base: null, confirmed: false },
    publicChat: { permission: { allowed: false, code: "unavailable", reason: "Fixture only" },
      maximumBytes: 120, channel: "public", messages: [] },
  };
}

test("legacy world absence is unsupported, never a fabricated empty version-1 UI", () => {
  const legacy = {} as WorldView;
  assert.deepEqual(gameplayUiSupport(["game.v1"], false, legacy).reason, "not_advertised");
  assert.equal(gameplayUiSupport([GAMEPLAY_UI_CAPABILITY], false, legacy).reason, "wire_unavailable");
  assert.equal(gameplayUiSupport([GAMEPLAY_UI_CAPABILITY], true, legacy).reason, "view_missing");
  assert.equal(Object.hasOwn(legacy, "ui"), false);
  assert(!gameplayUiSupport(["game.v1"], false, legacy).available);
});

test("only negotiated, wire-supported, complete version-1 projections are admitted", () => {
  const world = { ui: view() } as WorldView;
  assert.throws(() => gameplayUiSupport([], true, world), /without negotiated/);
  assert.throws(() => gameplayUiSupport([GAMEPLAY_UI_CAPABILITY], false, world), /generated wire decoder/);
  assert.equal(gameplayUiSupport([GAMEPLAY_UI_CAPABILITY], true, world).available, true);
  Reflect.set(world.ui!, "version", 2);
  assert.throws(() => gameplayUiSupport([GAMEPLAY_UI_CAPABILITY], true, world), /version-1/);
  Reflect.set(world, "ui", { version: 1 });
  assert.throws(() => gameplayUiSupport([GAMEPLAY_UI_CAPABILITY], true, world), /complete version-1/);
});

test("exact UI fees/XP/revisions/weights and identities survive validation and recursive immutability", () => {
  const original = view();
  const before = JSON.stringify(original);
  validateGameplayUi(original);
  assert.equal(JSON.stringify(original), before);
  const frozen = deepFreeze(original);
  assert.equal(frozen.reward!.xp[0]!.amountTenths, "18446744073709551615");
  assert.equal(frozen.equipment.weightGrams, "18446744073709551615");
  assert.equal(frozen.bank!.entries[0]!.id, "entry.original");
  assert.equal(frozen.bank!.entries[0]!.value, null);
  assert.deepEqual(frozen.reward!.continuation, { kind: "ui_dismiss", presentation_id: "presentation.original" });
  assert(Object.isFrozen(frozen.bank!.entries[0]));
  const malformed = view();
  Reflect.set(malformed.recovery!, "cofferBalance", 9007199254740992);
  assert.throws(() => validateGameplayUi(malformed), /decimal strings/);
  const overflow = view();
  overflow.reward!.xp[0]!.amountTenths = "18446744073709551616";
  assert.throws(() => validateGameplayUi(overflow), /decimal strings/);
});

test("placeholders never acquire fake spendable values", () => {
  const malformed = view();
  malformed.bank!.entries[0]!.placeholder = false;
  assert.throws(() => validateGameplayUi(malformed), /placeholder/);
});

test("inventory-only production accepts explicit null while retaining complete version-1 negotiation", () => {
  const source = view();
  source.production!.target = null;
  const before = JSON.stringify(source);
  validateGameplayUi(source);
  const world = { ui: source } as WorldView;
  assert.equal(gameplayUiSupport([GAMEPLAY_UI_CAPABILITY], true, world).available, true);
  assert.throws(() => gameplayUiSupport([], true, world), /without negotiated/);
  assert.throws(() => gameplayUiSupport([GAMEPLAY_UI_CAPABILITY], false, world), /generated wire decoder/);
  assert.equal(JSON.stringify(source), before);
  assert.equal(source.production!.id, "menu.original");
  assert.equal(source.production!.target, null, "No dummy facility is installed.");
  source.production!.target = { kind: "temporary_object", object: "dynamic_object.original" };
  validateGameplayUi(source);
});

test("a missing or malformed projected target remains a protocol error, not an inventory-only menu", () => {
  for (const target of [undefined, {}, [], 1, { kind: "spawn", spawn: "" }, { kind: "inventory" }]) {
    const malformed = view();
    Reflect.set(malformed.production!, "target", target);
    assert.throws(() => validateGameplayUi(malformed), (error: unknown) =>
      error instanceof Error && "kind" in error && error.kind === "protocol");
  }
  const absent = view();
  Reflect.deleteProperty(absent.production!, "target");
  assert.throws(() => validateGameplayUi(absent), /target field is missing/);
});

test("the exact bank revision is retained independently of advancing world revisions", () => {
  const source = view();
  for (const revision of ["9007199254741993", "9007199254741994"]) {
    const world = { revision, tick: revision, ui: source } as WorldView;
    assert.equal(gameplayUiSupport([GAMEPLAY_UI_CAPABILITY], true, world).available, true);
    assert.equal(world.ui!.bank!.revision, "9007199254740993");
    assert.notEqual(world.ui!.bank!.revision, world.revision);
  }
});

test("bank preconditions capture the displayed bank revision and never replace an explicit stale retry", () => {
  const world = { revision: "9007199254741999", ui: view() } as WorldView;
  const input = { kind: "bank_placeholder" as const, entry_id: "9007199254740995" };
  const captured = captureUiBankRevision(input, world);
  assert.deepEqual(captured, { ...input, expected_bank_revision: "9007199254740993" });
  world.ui!.bank!.revision = "9007199254740994";
  assert.deepEqual(captureUiBankRevision(captured, world), { ...input, expected_bank_revision: "9007199254740993" });
  assert.throws(() => captureUiBankRevision(input, null), /bank revision is unavailable/);
  assert.deepEqual(input, { kind: "bank_placeholder", entry_id: "9007199254740995" });
});

test("complete UI4 requires active-tab/document fields and preserves native-map metadata", () => {
  const current = view();
  current.document = { id: "document.original", interface: "interface.map", title: "Source map",
    pages: ["Source page"], page: 0, mapAsset: "asset.source.map", nativeMap: true };
  validateGameplayUi(current);
  assert.equal(current.document.nativeMap, true);
  Reflect.deleteProperty(current, "activeTab");
  assert.throws(() => validateGameplayUi(current), /complete version-1/);
});

test("actual final-step running and explicit nullable observer state are not inferred from settings or activity", () => {
  const world = { player: { running: true, movementTick: "9007199254740993", action: null,
    activity: "idle", settings: [{ setting: "run", enabled: false }] }, entities: [] } as unknown as WorldView;
  validateActorObservers(["game.observer.v1"], world);
  assert.equal(world.player.running, true);
  assert.equal(world.player.action, null);
  Reflect.deleteProperty(world.player, "running");
  assert.throws(() => validateActorObservers(["game.observer.v1"], world), /actual movement/);
  validateActorObservers([], world);
});

test("semantic All and recovery views are negotiated separately and never become sentinel quantities or additive fees", () => {
  const current = view();
  const denied = { allowed: false, code: "unavailable", reason: "Source permission is not verified." };
  current.bank!.amountSelection = { kind: "all" };
  current.recovery!.management = {
    bankRevision: "9007199254743333",
    panels: [{ death: "death.actual", storage: "grave", entries: [{
      id: "recovery_item.actual", item: { id: "item.fixture", name: "Fixture", quantity: 7,
        sourceId: 1, iconAsset: null, instanceId: null, charges: null, actions: [] },
      unitFee: "3", fullStackFee: "17", inventoryCapacity: 3, bankCapacity: 2, take: denied, bank: denied,
    }], fullSelectionFee: "18446744073709551615", takeAll: denied }],
    bankAll: denied, bankAllRecords: [{ death: "death.actual", items: ["recovery_item.actual"] }],
  };
  const world = { ui: current } as WorldView;
  const capabilities = [GAMEPLAY_UI_CAPABILITY, UI_AMOUNTS_CAPABILITY, UI_RECOVERY_CAPABILITY];
  assert.equal(gameplayUiSupport(capabilities, true, world).complete, true);
  assert.throws(() => gameplayUiSupport([GAMEPLAY_UI_CAPABILITY], true, world), /amount negotiation/);
  const before = JSON.stringify(current);
  validateGameplayUi(current);
  assert.equal(JSON.stringify(current), before);
  const request = captureUiBankRevision({ kind: "recovery_bank_all", records: current.recovery!.management.bankAllRecords }, world);
  assert.equal(request.kind, "recovery_bank_all");
  if (request.kind !== "recovery_bank_all") throw new Error("Wrong captured request kind.");
  assert.equal(request.expected_bank_revision, "9007199254743333");
  assert.notEqual(request.expected_bank_revision, current.bank!.revision);
  current.recovery!.management.bankRevision = "9007199254743334";
  const retry = captureUiBankRevision(request, world);
  if (retry.kind !== "recovery_bank_all") throw new Error("Wrong retry request kind.");
  assert.equal(retry.expected_bank_revision, "9007199254743333");
  assert.equal(current.recovery!.management.bankAll.allowed, false);
  validateUiAmount({ kind: "all" });
  assert.throws(() => validateUiAmount({ kind: "quantity", quantity: 0 }), /positive quantity or All/);
  assert.equal(current.bank!.amount, 1);
});
