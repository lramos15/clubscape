import assert from "node:assert/strict";
import test from "node:test";
import { bindBankRevision, checkUiIntent, formatUiFixed, formatUiInteger, gameplayUi, gameplayUiProblem, isGameplayUiIntent } from "../gameplay-ui.ts";
import { fixtureUi, fixtureWorld, immutable } from "./component-fixture.ts";
import type { GameplayUiIntent, UiPermission } from "../../shared/contracts.ts";

const allowed: UiPermission = { allowed: true, code: null, reason: null };
function worldWithUi() {
  const world = fixtureWorld();
  return { ...world, ui: fixtureUi(world) };
}
function bankWorld() {
  const world = worldWithUi();
  world.ui.bank = {
    revision: "9007199254740993", capacity: 400, selectedTab: 0, insertMode: false, placeholders: true, amount: 1, noted: false,
    entries: [
      { id: "opaque-entry", slot: 0, tab: 0, item: "item.pickaxe.bronze", value: world.player.inventory[0]!.item!, placeholder: false },
      { id: "opaque-placeholder", slot: 1, tab: 1, item: "item.axe.bronze", value: null, placeholder: true },
    ],
    tabs: [{ tab: 0, firstEntry: "opaque-entry", entries: 1 }, { tab: 1, firstEntry: "opaque-placeholder", entries: 1 }],
    depositEquipment: allowed, unavailableContainers: [],
  };
  return world;
}

function recoveryWorld() {
  const world = worldWithUi();
  const item = { ...world.player.inventory[0]!.item!, quantity: 10 };
  world.recovery = { death: "death-A", storage: "grave", remainingTicks: 40,
    items: [{ id: "recovery-A", item, cost: null }] };
  world.ui.recovery = {
    cofferBalance: "9007199254740993", discard: allowed, cofferOffer: allowed, cofferItems: [],
    management: {
      bankRevision: "9007199254740997",
      panels: [{
        death: "death-A", storage: "grave", fullSelectionFee: "9007199254740995", takeAll: allowed,
        entries: [{ id: "recovery-A", item, unitFee: "9007199254740993", fullStackFee: "9007199254740995",
          inventoryCapacity: 2, bankCapacity: 8, take: allowed, bank: allowed }],
      }],
      bankAll: allowed, bankAllRecords: [{ death: "death-A", items: ["recovery-A"] }],
    },
  };
  return world;
}

test("game.ui.v1 requires the exact complete projection, never a fabricated empty success", () => {
  assert.equal(gameplayUi(fixtureWorld()), null);
  const world = worldWithUi();
  assert.equal(gameplayUi(world), world.ui);
  Reflect.set(world.ui, "version", 2);
  assert.equal(gameplayUiProblem(world)?.code, "ui.capability.game.ui.v1");
  Reflect.set(world.ui, "version", 1);
  Reflect.deleteProperty(world.ui.publicChat, "messages");
  assert.match(gameplayUiProblem(world)!.message, /ui.publicChat.messages/);
});

test("nested malformed fields and duplicate identities are explicit projection failures", () => {
  const paths = [
    (world: ReturnType<typeof worldWithUi>) => Reflect.set(world.ui.equipment, "weightGrams", 9007199254740992),
    (world: ReturnType<typeof worldWithUi>) => Reflect.set(world.ui.equipment, "weightGrams", "1e8"),
    (world: ReturnType<typeof worldWithUi>) => Reflect.set(world.ui.prayers[0]!, "permission", null),
    (world: ReturnType<typeof worldWithUi>) => world.ui.inventoryActions.push(world.ui.inventoryActions[0]!),
    (world: ReturnType<typeof worldWithUi>) => Reflect.set(world.ui.publicChat, "channel", "private"),
  ];
  for (const damage of paths) {
    const world = worldWithUi(); damage(world);
    assert.equal(gameplayUiProblem(world)?.code, "ui.projection.invalid");
    assert.equal(gameplayUi(world), null);
  }
});

test("decimal formatting retains u64 XP/credit and signed gram precision", () => {
  assert.equal(formatUiInteger("18446744073709551615"), "18,446,744,073,709,551,615");
  assert.equal(formatUiFixed("9007199254740993", 1), "900,719,925,474,099.3");
  assert.equal(formatUiFixed("-1001", 3), "-1.001");
  assert.equal(formatUiFixed("10", 0), "10");
  assert.equal(formatUiFixed("0", 3), "0");
  assert.throws(() => formatUiInteger("1.2"), /Invalid/);
  assert.throws(() => formatUiFixed("1", -1), /Invalid/);
});

test("opaque inventory actions recheck item, instance and actual permission without mutating inventory", () => {
  const world = worldWithUi();
  const request: GameplayUiIntent = {
    kind: "item_action", inventory_slot: 0, expected_item: "item.pickaxe.bronze", expected_instance: null, action: "action.component.0.0",
  };
  assert.equal(checkUiIntent(world, request), null);
  assert.equal(checkUiIntent(world, { ...request, expected_item: "item.axe.bronze" })?.code, "ui.identity.stale");
  assert.equal(checkUiIntent(world, { ...request, expected_instance: "different-instance" })?.code, "ui.identity.stale");
  assert.match(checkUiIntent(world, { ...request, action: "Wield" })!.message, /permission is unavailable/);
  world.ui.inventoryActions[0]!.actions[0]!.permission = { allowed: false, code: "Requirements", reason: "Actual server requirement." };
  immutable(world);
  assert.equal(checkUiIntent(world, request)?.code, "Requirements");
  assert.match(checkUiIntent(world, request)!.message, /Actual server requirement/);
  assert.equal(world.player.inventory[0]!.item!.quantity, 1);
});

test("production retains menu/recipe identities and distinct single/make-X permissions", () => {
  const world = worldWithUi();
  world.ui.production = { id: "opaque-menu", interface: "interface.smithing", target: { kind: "spawn", spawn: "source-anvil" },
    recipes: [{ recipe: "opaque-recipe", name: "Bronze dagger", outputs: [], single: allowed,
      makeX: { allowed: false, code: "Tutorial", reason: "Only a single item is permitted here." } }] };
  const request: GameplayUiIntent = { kind: "production_select", menu_id: "opaque-menu", recipe: "opaque-recipe", quantity: 1, mode: "single" };
  assert.equal(checkUiIntent(world, request), null);
  assert.equal(checkUiIntent(world, { ...request, menu_id: "expired-menu" })?.code, "ui.identity.stale");
  assert.equal(checkUiIntent(world, { ...request, mode: "make_x" })?.code, "Tutorial");
  assert.equal(checkUiIntent(world, { ...request, quantity: 4294967296 })?.code, "ui.request.invalid");
});

test("inventory-only production accepts explicit null, not a missing or malformed target field", () => {
  const world = worldWithUi();
  world.ui.production = { id: "inventory-menu", interface: "interface.cooking", target: null,
    recipes: [{ recipe: "inventory-recipe", name: "Bread dough", outputs: [], single: allowed,
      makeX: { allowed: false, code: "Tutorial", reason: "Only single production is permitted." } }] };
  const request: GameplayUiIntent = { kind: "production_select", menu_id: "inventory-menu", recipe: "inventory-recipe", quantity: 1, mode: "single" };
  assert.equal(gameplayUiProblem(world), null);
  assert.equal(checkUiIntent(world, request), null);
  assert.equal(checkUiIntent(world, { ...request, mode: "make_x" })?.code, "Tutorial");
  assert.equal(checkUiIntent(world, { ...request, menu_id: "closed-menu" })?.code, "ui.identity.stale");
  Reflect.deleteProperty(world.ui.production, "target");
  assert.match(gameplayUiProblem(world)!.message, /ui.production.target/);
  Reflect.set(world.ui.production, "target", { kind: "spawn", spawn: "" });
  assert.equal(gameplayUiProblem(world)?.code, "ui.projection.invalid");
  world.ui.production.target = { kind: "temporary_object", object: "actual-facility" };
  assert.equal(checkUiIntent(world, request), null);
  world.ui.production.target = null;
  immutable(world);
  assert.equal(checkUiIntent(world, request), null);
  assert.equal(world.ui.production.target, null);
});
test("bank requests retain stable entries, placeholders and tab identities", () => {
  const world = bankWorld();
  immutable(world);
  assert.equal(gameplayUiProblem(world), null);
  const check = (intent: GameplayUiIntent) => checkUiIntent(world, bindBankRevision(intent, world.ui.bank!.revision));
  assert.equal(check({ kind: "bank_withdraw_entry", entry_id: "opaque-entry", quantity: 1, noted: true }), null);
  assert.equal(check({ kind: "bank_release_placeholder", entry_id: "opaque-placeholder" }), null);
  assert.equal(check({ kind: "bank_withdraw_entry", entry_id: "opaque-placeholder", quantity: 1, noted: false })?.code, "ui.identity.stale");
  assert.equal(check({ kind: "bank_release_placeholder", entry_id: "opaque-entry" })?.code, "ui.identity.stale");
  assert.equal(check({ kind: "bank_move", entry_id: "opaque-entry", before_entry_id: "opaque-placeholder", tab: 0 })?.code, "ui.identity.stale");
  assert.equal(check({ kind: "bank_move", entry_id: "opaque-entry", before_entry_id: "opaque-placeholder", tab: 1 }), null);
  assert.equal(check({ kind: "bank_move", entry_id: "opaque-entry", before_entry_id: null, tab: 9 })?.code, "ui.identity.stale");
  assert.equal(check({ kind: "bank_set_options", amount: 0, noted: false })?.code, "ui.bank.amount.unsupported");
});

test("source semantic All remains distinct from literal bank and production quantities", () => {
  const world = bankWorld();
  world.ui.bank!.amountSelection = { kind: "all" };
  const bank: GameplayUiIntent = { kind: "bank_set_amount", amount: { kind: "all" }, noted: true };
  assert(isGameplayUiIntent(bank));
  assert.equal(checkUiIntent(world, bindBankRevision(bank, world.ui.bank!.revision)), null);
  assert.equal(checkUiIntent(world, bank)?.code, "ui.bank.revision.required");
  const invalid = { kind: "bank_set_amount", amount: { kind: "quantity", quantity: 0 }, noted: false } as const;
  assert.equal(checkUiIntent(world, bindBankRevision(invalid, world.ui.bank!.revision))?.code, "ui.request.invalid");
  Reflect.set(world.ui.bank!, "amountSelection", { kind: "all", quantity: 1 });
  assert.equal(gameplayUiProblem(world)?.code, "ui.projection.invalid");
  world.ui.bank!.amountSelection = { kind: "all" };
  world.ui.production = { id: "menu-A", interface: "interface.cooking", target: null,
    recipes: [{ recipe: "recipe-A", name: "Dough", outputs: [], single: allowed, makeX: allowed, all: allowed }] };
  const production: GameplayUiIntent = { kind: "production_select_all", menu_id: "menu-A", recipe: "recipe-A" };
  assert(isGameplayUiIntent(production));
  assert.equal(checkUiIntent(world, production), null);
  world.ui.production.recipes[0]!.all = { allowed: false, code: "SourceSingle", reason: "The source currently permits only one." };
  assert.equal(checkUiIntent(world, production)?.code, "SourceSingle");
});

test("current recovery retains exact fees and forwards selected partial quantities without a pricing formula", () => {
  const world = recoveryWorld();
  const before = JSON.stringify(world);
  const request: GameplayUiIntent = { kind: "recovery_take", death: "death-A", storage: "grave",
    items: [{ id: "recovery-A", amount: { kind: "quantity", quantity: 5 } }] };
  assert(isGameplayUiIntent(request));
  assert.equal(gameplayUiProblem(world), null);
  assert.equal(checkUiIntent(world, request), null);
  assert.equal(checkUiIntent(world, { ...request, storage: "death_office" })?.code, "ui.identity.stale");
  assert.equal(checkUiIntent(world, { ...request, items: [] })?.code, "ui.identity.stale");
  assert.equal(checkUiIntent(world, { ...request, items: [request.items[0]!, request.items[0]!] })?.code, "ui.identity.stale");
  assert.equal(JSON.stringify(world), before);
  world.ui.recovery!.management!.panels[0]!.entries[0]!.take =
    { allowed: false, code: "SourceFee", reason: "The source fee is not affordable." };
  assert.equal(checkUiIntent(world, request)?.code, "SourceFee");
  assert.equal(checkUiIntent(world, { ...request, items: [{ id: "recovery-A", amount: { kind: "all" } }] }), null,
    "The exact whole-panel request uses its separately supplied source Take-All permission.");
});

test("recovery Bank-All binds its own bank revision and never creates a source permission", () => {
  const world = recoveryWorld();
  assert.equal(world.ui.bank, null);
  const management = world.ui.recovery!.management!;
  const request: GameplayUiIntent = { kind: "recovery_bank_all", records: management.bankAllRecords };
  assert(isGameplayUiIntent(request));
  assert.equal(checkUiIntent(world, bindBankRevision(request, management.bankRevision)), null);
  assert.equal(checkUiIntent(world, request)?.code, "ui.bank.revision.required");
  assert.equal(checkUiIntent(world, bindBankRevision(request, "1"))?.code, "ui.bank.revision.stale");
  assert.equal(checkUiIntent(world, bindBankRevision({ ...request,
    records: [{ death: "different-death", items: ["recovery-A"] }] }, management.bankRevision))?.code, "ui.identity.stale");
  management.bankAll = { allowed: false, code: "source_permission_unverified", reason: "Ordinary-grave Bank-All is not source-verified." };
  const problem = checkUiIntent(world, bindBankRevision(request, management.bankRevision));
  assert.equal(problem?.code, "source_permission_unverified");
  assert.match(problem!.message, /not source-verified/);
});

test("malformed new authority fields do not become ignored successful projections", () => {
  for (const change of [
    (world: ReturnType<typeof recoveryWorld>) => Reflect.set(world.ui.recovery!.management!, "bankRevision", 1),
    (world: ReturnType<typeof recoveryWorld>) => Reflect.set(world.ui.recovery!.management!.panels[0]!.entries[0]!, "unitFee", "18446744073709551616"),
    (world: ReturnType<typeof recoveryWorld>) => Reflect.set(world.ui.recovery!.management!.panels[0]!.entries[0]!, "inventoryCapacity", -1),
    (world: ReturnType<typeof recoveryWorld>) => world.ui.recovery!.management!.panels.push(world.ui.recovery!.management!.panels[0]!),
  ]) {
    const world = recoveryWorld(); change(world);
    assert.equal(gameplayUiProblem(world)?.code, "ui.projection.invalid");
  }
});

test("placeholder zero-quantity objects, mismatched item IDs and duplicate source slots are not valid bank views", () => {
  const world = bankWorld(), bank = world.ui.bank!;
  bank.entries[1]!.value = { ...world.player.inventory[1]!.item!, quantity: 0 };
  assert.equal(gameplayUiProblem(world)?.code, "ui.projection.invalid");
  bank.entries[1]!.value = null;
  bank.entries[0]!.item = "a-different-item";
  assert.equal(gameplayUiProblem(world)?.code, "ui.projection.invalid");
  bank.entries[0]!.item = bank.entries[0]!.value!.id;
  bank.entries[1]!.slot = 0;
  assert.equal(gameplayUiProblem(world)?.code, "ui.projection.invalid");
});

test("coffer/discard requests depend on declared permissions and preserve recovery/instance identity", () => {
  const world = worldWithUi();
  world.recovery = { death: "death-A", storage: "grave", remainingTicks: 40,
    items: [{ id: "recovery-A", item: world.player.inventory[0]!.item!, cost: null }] };
  world.ui.recovery = { cofferBalance: "9007199254740993", discard: allowed, cofferOffer: allowed,
    cofferItems: [world.ui.inventoryActions[0]!] };
  immutable(world);
  const offer: GameplayUiIntent = { kind: "coffer_offer", inventory_slot: 0, expected_item: "item.pickaxe.bronze", expected_instance: null, quantity: 1 };
  assert.equal(checkUiIntent(world, offer), null);
  assert.equal(checkUiIntent(world, { ...offer, expected_instance: "replaced" })?.code, "ui.identity.stale");
  const discard: GameplayUiIntent = { kind: "request_recovery_discard", death: "death-A", storage: "grave", items: ["recovery-A"] };
  assert.equal(checkUiIntent(world, discard), null);
  assert.equal(checkUiIntent(world, { ...discard, storage: "death_office" })?.code, "ui.identity.stale");
  assert.equal(checkUiIntent(world, { ...discard, items: [] })?.code, "ui.identity.stale");
  assert.equal(checkUiIntent(world, { ...discard, items: ["recovery-A", "recovery-A"] })?.code, "ui.identity.stale");
  assert.equal(world.ui.recovery.cofferBalance, "9007199254740993");
});

test("public chat checks declared channel, UTF-8 bytes and server permission, not JS string length", () => {
  const world = worldWithUi();
  world.ui.publicChat.maximumBytes = 3;
  assert.equal(checkUiIntent(world, { kind: "public_chat", channel: "public", text: "é" }), null);
  assert.equal(checkUiIntent(world, { kind: "public_chat", channel: "public", text: "éé" })?.code, "ui.chat.invalid");
  assert.equal(checkUiIntent(world, { kind: "public_chat", channel: "public", text: "   " })?.code, "ui.chat.invalid");
  world.ui.publicChat.permission = { allowed: false, code: "Muted", reason: "Actual server chat rejection." };
  assert.equal(checkUiIntent(world, { kind: "public_chat", channel: "public", text: "a" })?.code, "Muted");
});

test("presentation requests are exhaustive and require the current opaque identity", () => {
  const world = worldWithUi();
  world.ui.confirmation = { id: "opaque-confirmation", kind: "coffer_offer", title: "Confirm", lines: [], items: [], credit: "9007199254740993" };
  immutable(world);
  assert.equal(checkUiIntent(world, { kind: "ui_confirm", confirmation_id: "opaque-confirmation", accept: false }), null);
  assert.equal(checkUiIntent(world, { kind: "ui_confirm", confirmation_id: "old-confirmation", accept: true })?.code, "ui.identity.stale");
  assert.equal(isGameplayUiIntent({ kind: "bank_deposit_equipment" }), true);
  assert.equal(isGameplayUiIntent({ kind: "shop_buy", shop: "shop-A", item_index: 0, quantity: 1, expected_item: "item.coins" }), false);
});
