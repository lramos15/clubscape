import assert from "node:assert/strict";
import { test } from "node:test";
import { GAMEPLAY_UI_CAPABILITY } from "../../shared/contracts.ts";
import type { GameplayUiView, WorldView } from "../../shared/contracts.ts";
import { gameplayUiSupport, validateGameplayUi } from "../gameplay-ui.ts";
import { deepFreeze } from "../errors.ts";

// Version/projection fixtures only. They are never attached to the real browser/server world.
function view(): GameplayUiView {
  return {
    version: 1, activeInterface: null, production: {
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

test("placeholders and pending production target correction never acquire fake values or targets", () => {
  const malformed = view();
  malformed.bank!.entries[0]!.placeholder = false;
  assert.throws(() => validateGameplayUi(malformed), /placeholder/);
  const pending = view();
  Reflect.set(pending.production!, "target", null);
  assert.throws(() => validateGameplayUi(pending), /no dummy target/);
  assert.equal(pending.production!.target, null);
});
