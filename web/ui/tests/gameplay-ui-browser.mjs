import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { browserHost, launchBrowser, results } from "./browser-host.mjs";

const host = await browserHost(), browser = await launchBrowser(), cases = [], errors = [];
const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
page.on("pageerror", error => errors.push(error.message));
const frame = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
const click = async id => { await page.locator(`[data-ui-control="${id}"]`).click(); await frame(); };
const intents = () => page.evaluate(() => window.component.services.intents);
const last = async () => (await intents()).at(-1);
const reset = () => page.evaluate(() => { window.component.services.intents.length = 0; });
const mount = async (versioned = true) => {
  await page.evaluate(async versioned => {
    const fixture = await import("/web/ui/tests/component-fixture.ts");
    await fixture.mount("world");
    if (versioned) window.component.services.enableUi();
  }, versioned);
  await frame();
};
const patch = async (code, data) => { await page.evaluate(code, data); await frame(); };
const capture = name => page.screenshot({ path: resolve(results, `versioned/${name}.png`) });
async function check(name, run) {
  try { await run(); cases.push({ name, passed: true }); }
  catch (error) { cases.push({ name, passed: false, error: error.message }); throw error; }
}

try {
  await mkdir(resolve(results, "versioned"), { recursive: true });
  await page.goto(host.url);
  await check("legacy/unknown version is explicit unsupported capability, not empty v1 success", async () => {
    await mount(false);
    assert.match(await page.getByRole("status").innerText(), /does not provide game\.ui\.v1/);
    await click("chat-input");
    assert.match(await page.getByRole("status").innerText(), /game\.ui\.v1/);
    await mount();
    await patch(() => {
      const s = window.component.services, world = structuredClone(s.state().world);
      world.ui.version = 2; s.patchWorld(world);
    });
    assert.match(await page.getByRole("status").innerText(), /game\.ui\.v1/);
  });
  await check("opaque inventory action IDs and item/instance identities are retained", async () => {
    await mount(); await reset();
    await click("inventory-0");
    assert.deepEqual(await last(), { kind: "item_action", inventory_slot: 0, expected_item: "item.pickaxe.bronze",
      expected_instance: null, action: "action.component.0.0" });
    await patch(() => {
      const s = window.component.services, world = structuredClone(s.state().world);
      world.player.inventory[0].item.instanceId = "instance.original";
      world.ui.inventoryActions[0].instance = "instance.original";
      s.patchWorld(world);
    });
    await page.locator('[data-ui-control="inventory-0"]').click({ button: "right" }); await frame();
    await patch(() => {
      const s = window.component.services, world = structuredClone(s.state().world);
      world.player.inventory[0].item.instanceId = "instance.replaced";
      world.ui.inventoryActions[0].instance = "instance.replaced"; s.patchWorld(world);
    });
    await reset();
    await page.getByRole("button", { name: "Wield Bronze pickaxe", exact: true }).click(); await frame();
    assert.equal((await intents()).length, 0);
    assert.match(await page.getByRole("status").innerText(), /instance changed/);
    await click("notice-close");
    await patch(() => {
      const s = window.component.services, world = structuredClone(s.state().world);
      world.ui.inventoryActions[0].actions[0].permission = { allowed: false, code: "Requirements", reason: "The authoritative requirement is not met." };
      s.patchWorld(world);
    });
    assert.equal(await page.locator('[data-ui-control="inventory-0"]').isDisabled(), true);
  });
  await check("source interface and ability permissions replace legacy inferred availability", async () => {
    await mount();
    await patch(() => {
      const s = window.component.services, world = structuredClone(s.state().world);
      world.ui.interfaces.find(row => row.interface === "interface.skills").visibility = "hidden";
      world.ui.interfaces.find(row => row.interface === "interface.combat").permission = { allowed: false, code: "Locked", reason: "Source combat interface locked." };
      s.patchWorld(world);
    });
    assert.equal(await page.locator('[data-ui-control="tab-1"]').count(), 0);
    assert.equal(await page.locator('[data-ui-control="tab-0"]').isDisabled(), true);
    await click("tab-5"); await reset();
    await patch(() => {
      const s = window.component.services, world = structuredClone(s.state().world);
      world.ui.prayers[0].permission = { allowed: false, code: "Depleted", reason: "Source prayer points depleted." };
      s.patchWorld(world);
    });
    assert.equal(await page.getByRole("button", { name: "Thick Skin", exact: true }).isDisabled(), true);
    assert.equal((await intents()).length, 0);
  });
  await check("stable bank entries, placeholders, options, tabs and equipment deposits dispatch exact v1 requests", async () => {
    await mount();
    await patch(() => {
      const s = window.component.services, world = structuredClone(s.state().world), yes = { allowed: true, code: null, reason: null };
      world.bank = { banker: "spawn.banker", capacity: 400, allowNotes: true, slots: [] };
      world.ui.activeInterface = "interface.bank";
      world.ui.bank = { revision: "9007199254740993", capacity: 400, selectedTab: 0, insertMode: false, placeholders: false,
        amount: 5, noted: true, tabs: [{ tab: 0, firstEntry: "entry-A", entries: 2 }, { tab: 1, firstEntry: null, entries: 0 }],
        entries: [
          { id: "entry-A", slot: 12, tab: 0, item: world.player.inventory[0].item.id, value: { ...world.player.inventory[0].item, quantity: 12 }, placeholder: false },
          { id: "entry-placeholder", slot: 20, tab: 0, item: "item.axe.bronze", value: null, placeholder: true },
        ], depositEquipment: yes, unavailableContainers: [{ id: "empty.unavailable", label: "Empty containers", permission: { allowed: false, code: "Unavailable", reason: "No supported container action." } }] };
      s.patchWorld(world);
    });
    await reset(); await click("bank-entry-entry-A");
    const bankRevision = await page.evaluate(() => window.component.services.state().world.ui.bank.revision);
    assert.deepEqual(await last(), { kind: "bank_withdraw_entry", entry_id: "entry-A", quantity: 5, noted: true, expected_bank_revision: bankRevision });
    await page.locator('[data-ui-control="bank-entry-entry-placeholder"]').click({ button: "right" }); await frame();
    assert.equal(await page.getByRole("button", { name: /Withdraw/ }).count(), 0);
    await page.getByRole("button", { name: /Release placeholder/ }).click(); await frame();
    assert.deepEqual(await last(), { kind: "bank_release_placeholder", entry_id: "entry-placeholder", expected_bank_revision: bankRevision });
    await click("bank-control-23--1");
    assert.deepEqual(await last(), { kind: "bank_set_insert", enabled: true, expected_bank_revision: bankRevision });
    await click("bank-control-40--1");
    assert.deepEqual(await last(), { kind: "bank_set_placeholders", enabled: true, expected_bank_revision: bankRevision });
    await click("bank-control-49--1");
    assert.deepEqual(await last(), { kind: "bank_deposit_equipment", expected_bank_revision: bankRevision });
    await click("bank-control-31--1");
    assert.deepEqual(await last(), { kind: "bank_set_options", amount: 5, noted: true, expected_bank_revision: bankRevision });
    await click("bank-tab-1");
    assert.deepEqual(await last(), { kind: "bank_select_tab", tab: 1, expected_bank_revision: bankRevision });
    assert.equal(await page.evaluate(() => window.component.services.state().world.ui.bank.selectedTab), 0);
    await patch(() => {
      const s = window.component.services, w = structuredClone(s.state().world);
      w.ui.bank.amountSelection = { kind: "all" }; s.patchWorld(w);
    });
    await click("bank-entry-entry-A");
    assert.deepEqual(await last(), { kind: "bank_withdraw_entry", entry_id: "entry-A", quantity: 12, noted: true, expected_bank_revision: bankRevision });
    await click("bank-control-25--1");
    assert.deepEqual(await last(), { kind: "bank_set_amount", amount: { kind: "all" }, noted: false, expected_bank_revision: bankRevision });
    await click("bank-control-31--1");
    assert.deepEqual(await last(), { kind: "bank_set_amount", amount: { kind: "quantity", quantity: 5 }, noted: true, expected_bank_revision: bankRevision });
    await capture("bank-v1-stable-entries");
  });
  await check("bank drag reorders by entry identity and cancels cleanly on blur or outside release", async () => {
    await mount();
    await patch(() => {
      const s = window.component.services, w = structuredClone(s.state().world), yes = { allowed: true, code: null, reason: null };
      w.ui.activeInterface = "interface.bank";
      w.ui.bank = { revision: "12", capacity: 400, selectedTab: 0, insertMode: false, placeholders: false, amount: 1, noted: false,
        tabs: [{ tab: 0, firstEntry: "drag-A", entries: 2 }, { tab: 1, firstEntry: null, entries: 0 }],
        entries: [0, 1].map((slot, index) => ({ id: index ? "drag-B" : "drag-A", slot, tab: 0,
          item: w.player.inventory[slot].item.id, value: w.player.inventory[slot].item, placeholder: false })),
        depositEquipment: yes, unavailableContainers: [] }; s.patchWorld(w);
    });
    const box = async id => {
      const value = await page.locator(`[data-ui-control="${id}"]`).boundingBox();
      assert.ok(value); return { x: value.x + value.width / 2, y: value.y + value.height / 2 };
    };
    const a = await box("bank-entry-drag-A"), b = await box("bank-entry-drag-B");
    await reset();
    await page.mouse.move(a.x, a.y); await page.mouse.down(); await page.waitForTimeout(130);
    await page.mouse.move(b.x, b.y, { steps: 4 }); await page.mouse.up(); await frame();
    const bankRevision = await page.evaluate(() => window.component.services.state().world.ui.bank.revision);
    assert.deepEqual(await last(), { kind: "bank_move", entry_id: "drag-A", before_entry_id: "drag-B", tab: 0, expected_bank_revision: bankRevision });
    assert.deepEqual(await page.evaluate(() => window.component.services.state().world.ui.bank.entries.map(entry => entry.id)), ["drag-A", "drag-B"]);
    await reset();
    await page.mouse.move(a.x, a.y); await page.mouse.down(); await page.waitForTimeout(130);
    await page.mouse.move(b.x, b.y);
    await page.evaluate(() => window.dispatchEvent(new Event("blur")));
    await page.mouse.up(); await frame(); assert.equal((await intents()).length, 0);
    await page.mouse.move(a.x, a.y); await page.mouse.down(); await page.waitForTimeout(130);
    await page.mouse.move(1200, 500);
    await page.evaluate(() => window.dispatchEvent(new PointerEvent("pointerup", { clientX: 1200, clientY: 500, button: 0 })));
    await page.mouse.up(); await frame(); assert.equal((await intents()).length, 0);
    await page.mouse.move(a.x, a.y); await page.mouse.down(); await page.waitForTimeout(130);
    const tab = await box("bank-tab-1");
    await page.mouse.move(tab.x, tab.y); await page.mouse.up(); await frame();
    assert.deepEqual(await last(), { kind: "bank_move", entry_id: "drag-A", before_entry_id: null, tab: 1, expected_bank_revision: bankRevision });
    await reset();
    await page.locator('[data-ui-control="bank-entry-drag-A"]').click({ button: "right" }); await frame();
    await patch(() => {
      const s = window.component.services, w = structuredClone(s.state().world);
      w.ui.bank.entries[0].item = w.player.inventory[1].item.id; w.ui.bank.entries[0].value = w.player.inventory[1].item; s.patchWorld(w);
    });
    await page.getByRole("button", { name: "Withdraw-1 Bronze pickaxe", exact: true }).click(); await frame();
    assert.equal((await intents()).length, 0);
    assert.match(await page.getByRole("status").innerText(), /identity changed/);
  });
  await check("public chat checks UTF-8 bytes and never optimistically echoes or drops a rejected draft", async () => {
    await mount(); await reset();
    await page.getByLabel("Public chat", { exact: true }).fill("Hello from the UI");
    await page.getByLabel("Public chat", { exact: true }).press("Enter"); await frame();
    assert.deepEqual(await last(), { kind: "public_chat", channel: "public", text: "Hello from the UI" });
    assert.equal(await page.getByLabel("Public chat", { exact: true }).inputValue(), "Hello from the UI");
    assert.equal(await page.evaluate(() => window.component.services.state().world.ui.publicChat.messages.length), 0);
    await patch(() => {
      const s = window.component.services, world = structuredClone(s.state().world);
      world.ui.publicChat.messages.push({ id: "message.opaque.1", actor: world.player.id, sender: world.player.displayName,
        channel: "public", text: "Hello from the UI", colour: 0, effect: 0 });
      s.patchWorld(world);
    });
    assert.equal(await page.getByLabel("Public chat", { exact: true }).inputValue(), "");
    await patch(() => { const s = window.component.services, w = structuredClone(s.state().world); w.ui.publicChat.maximumBytes = 3; s.patchWorld(w); });
    await page.getByLabel("Public chat", { exact: true }).fill("éé"); await reset();
    await page.getByLabel("Public chat", { exact: true }).press("Enter"); await frame();
    assert.equal((await intents()).length, 0);
    assert.match(await page.getByRole("status").innerText(), /3 UTF-8 bytes/);
    await click("notice-close");
    await patch(() => {
      const s = window.component.services, w = structuredClone(s.state().world);
      w.ui.publicChat.maximumBytes = 80; s.patchWorld(w);
      s.rejection = { message: "The server rejected this chat message.", errorId: "chat.authoritative.rejection" };
    });
    await page.getByLabel("Public chat", { exact: true }).fill("Rejected draft");
    await page.getByLabel("Public chat", { exact: true }).press("Enter"); await frame();
    assert.match(await page.getByRole("status").innerText(), /chat.authoritative.rejection/);
    await click("notice-close");
    assert.equal(await page.getByLabel("Public chat", { exact: true }).inputValue(), "Rejected draft");
    assert.equal(await page.evaluate(() => window.component.services.state().world.ui.publicChat.messages.length), 1);
  });
  await check("production single/make-X modes keep menu IDs, reject locked controls and retain failed amounts", async () => {
    await mount();
    await patch(() => {
      const s = window.component.services, w = structuredClone(s.state().world);
      const yes = { allowed: true, code: null, reason: null };
      w.ui.activeInterface = "interface.cooking";
      w.ui.production = { id: "menu.opaque.cooking", interface: "interface.cooking", target: { kind: "spawn", spawn: "spawn.range" },
        recipes: [{ recipe: "recipe.opaque.shrimps", name: "Cook shrimps", outputs: [{ ...w.player.inventory[2].item, quantity: 1 }], single: yes, makeX: yes },
          { recipe: "recipe.opaque.bread", name: "Cook bread", outputs: [{ ...w.player.inventory[2].item, id: "item.bread", name: "Bread", sourceId: 2309, quantity: 1 }],
            single: { allowed: false, code: "Requirements", reason: "Source ingredients unavailable." }, makeX: { allowed: false, code: "Requirements", reason: "Source ingredients unavailable." } }] };
      s.patchWorld(w);
    });
    await reset(); await click("production-recipe.opaque.shrimps");
    assert.deepEqual(await last(), { kind: "production_select", menu_id: "menu.opaque.cooking", recipe: "recipe.opaque.shrimps", quantity: 1, mode: "single" });
    assert.equal(await page.locator('[data-ui-control="production-recipe.opaque.bread"]').isDisabled(), true);
    await click("production-amount-5"); await click("production-recipe.opaque.shrimps");
    assert.deepEqual(await last(), { kind: "production_select", menu_id: "menu.opaque.cooking", recipe: "recipe.opaque.shrimps", quantity: 5, mode: "make_x" });
    await click("production-amount-X"); await click("production-recipe.opaque.shrimps");
    await page.getByLabel("Make how many?", { exact: true }).fill("7");
    await patch(() => { window.component.services.rejection = { message: "Only one item can be made here.", errorId: "server.production.mode" }; });
    await page.getByLabel("Make how many?", { exact: true }).press("Enter"); await frame();
    assert.equal((await last()).quantity, 7);
    assert.match(await page.getByRole("status").innerText(), /server.production.mode/);
    await click("notice-close");
    assert.equal(await page.getByLabel("Make how many?", { exact: true }).inputValue(), "7");
    await page.getByLabel("Make how many?", { exact: true }).press("Escape"); await frame();
    await click("production-amount-1");
    await page.locator('[data-ui-control="production-recipe.opaque.shrimps"]').click({ button: "right" }); await frame();
    await patch(() => { const s = window.component.services, w = structuredClone(s.state().world); w.ui.production.id = "menu.replaced"; s.patchWorld(w); });
    await reset(); await page.getByRole("button", { name: "Make-1 Cook shrimps", exact: true }).click(); await frame();
    assert.equal((await intents()).length, 0);
    assert.match(await page.getByRole("status").innerText(), /changed or closed/);
    await click("notice-close");
    await click("production-amount-All");
    assert.equal((await intents()).length, 0);
    assert.equal(await page.locator('[data-ui-control="production-recipe.opaque.shrimps"]').isDisabled(), true);
    await patch(() => {
      const s = window.component.services, w = structuredClone(s.state().world);
      w.ui.production.recipes[0].all = { allowed: true, code: null, reason: null }; s.patchWorld(w);
    });
    await click("production-recipe.opaque.shrimps");
    assert.deepEqual(await last(), { kind: "production_select_all", menu_id: "menu.replaced", recipe: "recipe.opaque.shrimps" });
    await page.mouse.move(600, 600); await frame();
    await capture("production-v1-original-choice-geometry");
  });
  await check("native smithing preserves unavailable source rows and sends only declared recipe identities", async () => {
    await mount();
    await patch(() => {
      const s = window.component.services, w = structuredClone(s.state().world), yes = { allowed: true, code: null, reason: null };
      w.ui.activeInterface = "interface.smithing";
      w.ui.production = { id: "smithing.menu", interface: "interface.smithing", target: { kind: "spawn", spawn: "anvil.opaque" },
        recipes: [{ recipe: "bronze.dagger.opaque", name: "Bronze dagger", outputs: [{ ...w.player.inventory[0].item,
          id: "item.dagger.bronze", name: "Bronze dagger", sourceId: 1205, quantity: 1 }], single: yes,
          makeX: { allowed: false, code: "Tutorial", reason: "Only single production is enabled." } }] };
      s.patchWorld(w);
    });
    await reset(); await click("production-bronze.dagger.opaque");
    assert.deepEqual(await last(), { kind: "production_select", menu_id: "smithing.menu", recipe: "bronze.dagger.opaque", quantity: 1, mode: "single" });
    assert.equal(await page.locator('[data-ui-control="production-source-10"]').isDisabled(), true);
    await page.locator('[data-ui-control="production-bronze.dagger.opaque"]').click({ button: "right" }); await frame();
    assert.equal(await page.getByRole("button", { name: "Smith-X Bronze dagger", exact: true }).isDisabled(), true);
    await page.getByRole("button", { name: "Cancel", exact: true }).click(); await frame();
    assert.equal(await page.evaluate(() => window.component.ui.capturesPointer(850, 550)), true);
    await capture("smithing-v1-native-rows");
  });
  await check("inventory-only null target renders native controls and preserves authority on reject/cancel", async () => {
    await mount();
    await patch(() => {
      const s = window.component.services, w = structuredClone(s.state().world), yes = { allowed: true, code: null, reason: null };
      w.ui.activeInterface = "interface.cooking";
      w.ui.production = { id: "inventory.menu.opaque", interface: "interface.cooking", target: null,
        recipes: [{ recipe: "inventory.dough.opaque", name: "Bread dough",
          outputs: [{ ...w.player.inventory[2].item, id: "item.bread.dough", name: "Bread dough", sourceId: 2307, quantity: 1 }],
          single: yes, makeX: { allowed: false, code: "Tutorial", reason: "Only single production is permitted." } }] };
      s.patchWorld(w);
    });
    await reset();
    await click("production-inventory.dough.opaque");
    assert.deepEqual(await last(), { kind: "production_select", menu_id: "inventory.menu.opaque",
      recipe: "inventory.dough.opaque", quantity: 1, mode: "single" });
    assert.equal(await page.evaluate(() => window.component.services.state().world.ui.production.target), null);
    assert.equal(await page.evaluate(() => window.component.services.state().world.player.inventory.some(row => row.item?.id === "item.bread.dough")), false);
    await page.locator('[data-ui-control="production-inventory.dough.opaque"]').click({ button: "right" }); await frame();
    assert.equal(await page.getByRole("button", { name: "Make-X Bread dough", exact: true }).isDisabled(), true);
    await page.getByRole("button", { name: "Cancel", exact: true }).click(); await frame();
    await patch(() => { window.component.services.rejection = { message: "Authoritative ingredients are unavailable.", errorId: "source.ingredients" }; });
    await click("production-inventory.dough.opaque");
    assert.match(await page.getByRole("status").innerText(), /source.ingredients/);
    await click("notice-close");
    assert.equal(await page.locator('[data-ui-control="production-inventory.dough.opaque"]').count(), 1);
    await capture("inventory-only-null-production");
    await reset(); await page.locator("canvas").press("Escape"); await frame();
    assert.deepEqual(await last(), { kind: "close_interface" });
    assert.equal(await page.evaluate(() => window.component.services.state().world.ui.production.id), "inventory.menu.opaque");
  });
  await check("death preview uses actual kept/lost rows and exact monetary strings, not fixture classifications", async () => {
    await mount();
    await patch(() => {
      const s = window.component.services, w = structuredClone(s.state().world);
      w.ui.activeInterface = "interface.items_kept_on_death";
      w.ui.keptOnDeath = { scope: "normal_unsafe_non_pvp", kept: [w.player.inventory[1].item],
        lost: [{ ...w.player.inventory[2].item, quantity: 37 }], fullGraveFee: "9007199254740993",
        fullOfficeFee: "18446744073709551615", valueRevision: "9007199254740995" };
      s.patchWorld(w);
    });
    assert.equal(await page.locator('[data-ui-control^="death-preview-6-"]').count(), 1);
    assert.equal(await page.locator('[data-ui-control^="death-preview-7-"]').count(), 1);
    await click("death-preview-values-18");
    assert.match(await page.getByRole("status").innerText(), /9,007,199,254,740,993/);
    assert.match(await page.getByRole("status").innerText(), /18,446,744,073,709,551,615/);
    await click("notice-close"); await reset();
    await click("death-preview-mode-16");
    assert.equal((await intents()).length, 0);
    assert.match(await page.getByRole("status").innerText(), /death_preview_scope_options/);
    await click("notice-close"); await capture("death-preview-v1-authoritative-rows");
  });
  await check("live recovery projects only real rows and sends discard requests through versioned confirmation flow", async () => {
    await mount();
    await patch(() => {
      const s = window.component.services, w = structuredClone(s.state().world), yes = { allowed: true, code: null, reason: null };
      w.recovery = { death: "death.opaque", storage: "grave", items: [{ id: "owned.recovery", item: w.player.inventory[1].item, cost: null }], remainingTicks: 20 };
      w.ui.activeInterface = "interface.grave";
      w.ui.recovery = { cofferBalance: "9007199254740993", discard: yes, cofferOffer: yes, cofferItems: [w.ui.inventoryActions[0]] };
      s.patchWorld(w);
    });
    assert.equal(await page.locator('[data-ui-control^="recovery-item-"]').count(), 1);
    await reset(); await page.getByRole("button", { name: "Discard-All", exact: true }).click(); await frame();
    assert.deepEqual(await last(), { kind: "request_recovery_discard", death: "death.opaque", storage: "grave", items: ["owned.recovery"] });
    assert.equal(await page.evaluate(() => window.component.services.state().world.recovery.items.length), 1);
    await patch(() => {
      const s = window.component.services, w = structuredClone(s.state().world);
      w.ui.recovery.discard = { allowed: false, code: "Ownership", reason: "Actual discard permission was revoked." };
      s.patchWorld(w);
    });
    assert.equal(await page.getByRole("button", { name: "Discard-All", exact: true }).isDisabled(), true);
    await page.locator('[data-ui-control="inventory-0"]').click({ button: "right" }); await frame();
    await page.getByRole("button", { name: "Offer to Death's Coffer Bronze pickaxe", exact: true }).click(); await frame();
    await page.getByLabel("Offer how many?", { exact: true }).fill("1");
    await page.getByLabel("Offer how many?", { exact: true }).press("Enter"); await frame();
    assert.deepEqual(await last(), { kind: "coffer_offer", inventory_slot: 0, expected_item: "item.pickaxe.bronze", expected_instance: null, quantity: 1 });
    await patch(() => {
      const s = window.component.services, w = structuredClone(s.state().world);
      w.ui.confirmation = { id: "coffer.confirmation", kind: "coffer_offer", title: "Confirm offering", lines: ["Authoritative confirmation"],
        items: [w.player.inventory[0].item], credit: "4321" }; s.patchWorld(w);
    });
    await click("ui-confirm-cancel");
    assert.deepEqual(await last(), { kind: "ui_confirm", confirmation_id: "coffer.confirmation", accept: false });
    assert.equal(await page.evaluate(() => window.component.services.state().world.ui.recovery.cofferBalance), "9007199254740993");
    await capture("recovery-v1-discard-coffer");
  });
  await check("current recovery sends partial and All quantities with exact source records, fees and independent bank revision", async () => {
    await mount();
    await patch(() => {
      const s = window.component.services, w = structuredClone(s.state().world), yes = { allowed: true, code: null, reason: null };
      const item = { ...w.player.inventory[0].item, quantity: 7 };
      w.recovery = { death: "death.current", storage: "death_office", remainingTicks: null,
        items: [{ id: "entry.current", item, cost: null }] };
      w.ui.activeInterface = "interface.death_retrieval";
      w.ui.recovery = { cofferBalance: "9007199254740993", discard: yes, cofferOffer: yes, cofferItems: [],
        management: { bankRevision: "9007199254740997", panels: [{
          death: "death.current", storage: "death_office", fullSelectionFee: "17", takeAll: yes,
          entries: [{ id: "entry.current", item, unitFee: "3", fullStackFee: "17",
            inventoryCapacity: 2, bankCapacity: 7, take: yes, bank: yes }],
        }], bankAll: yes, bankAllRecords: [{ death: "death.current", items: ["entry.current"] }] } };
      s.patchWorld(w);
    });
    await click("recovery-item-entry.current");
    await reset();
    await page.getByRole("button", { name: "Retrieve 5", exact: true }).click(); await frame();
    assert.deepEqual(await last(), { kind: "recovery_take", death: "death.current", storage: "death_office",
      items: [{ id: "entry.current", amount: { kind: "quantity", quantity: 5 } }] });
    await page.getByRole("button", { name: "Retrieve X", exact: true }).click(); await frame();
    await page.getByRole("textbox", { name: "Retrieve how many?", exact: true }).fill("3");
    await page.getByRole("textbox", { name: "Retrieve how many?", exact: true }).press("Enter"); await frame();
    assert.deepEqual(await last(), { kind: "recovery_take", death: "death.current", storage: "death_office",
      items: [{ id: "entry.current", amount: { kind: "quantity", quantity: 3 } }] });
    await page.keyboard.press("Escape"); await frame();
    await page.getByRole("button", { name: "Take-All", exact: true }).click(); await frame();
    assert.deepEqual(await last(), { kind: "recovery_take", death: "death.current", storage: "death_office",
      items: [{ id: "entry.current", amount: { kind: "all" } }] });
    await patch(() => {
      const s = window.component.services, w = structuredClone(s.state().world);
      w.recovery.storage = "grave"; w.ui.recovery.management.panels[0].storage = "grave";
      w.ui.activeInterface = "interface.grave"; s.patchWorld(w);
    });
    await page.getByRole("button", { name: "Bank-All", exact: true }).click(); await frame();
    assert.deepEqual(await last(), { kind: "recovery_bank_all", records: [{ death: "death.current", items: ["entry.current"] }],
      expected_bank_revision: "9007199254740997" });
    assert.equal(await page.evaluate(() => window.component.services.state().world.recovery.items[0].item.quantity), 7);
  });
  await check("malformed nested projections surface errors and cannot dispatch through legacy fallbacks", async () => {
    await mount(); await reset();
    await patch(() => {
      const s = window.component.services, w = structuredClone(s.state().world);
      w.ui.equipment.weightGrams = "1.5";
      s.patchWorld(w);
    });
    assert.match(await page.getByRole("status").innerText(), /ui.equipment.weightGrams/);
    assert.match(await page.getByRole("status").innerText(), /ui.projection.invalid/);
    assert.equal((await intents()).length, 0);
  });
  await check("selected combat style remains identity-bound when projection order changes", async () => {
    await mount(); await click("tab-0");
    await patch(() => {
      const s = window.component.services, w = structuredClone(s.state().world);
      w.ui.combatStyles = w.ui.combatStyles.toReversed();
      for (const style of w.ui.combatStyles) style.selected = style.name === "Lunge";
      w.ui.combatStyle = w.ui.combatStyles.find(style => style.selected).id;
      s.patchWorld(w);
    });
    assert.equal(await page.locator('[data-ui-control="combat-style-1"]').getAttribute("aria-pressed"), "true");
    await reset(); await click("combat-style-1");
    assert.deepEqual(await last(), { kind: "set_combat_style", style: "style.sword.bronze.stab.aggressive" });
    await click("tab-4"); await click("equipment-control-1");
    await patch(() => {
      const s = window.component.services, w = structuredClone(s.state().world);
      w.ui.activeInterface = "interface.equipment_stats"; w.ui.equipment.weightGrams = "9007199254740993"; s.patchWorld(w);
    });
    assert.match(await page.locator('[data-ui-control="equipment-weight"]').getAttribute("aria-description"), /9,007,199,254,740,993 grams/);
    const preview = await page.evaluate(() => window.getUiPreviewRequest(window.component.ui));
    assert.equal(preview.purpose, "equipment");
    assert.deepEqual(preview.bounds, { x: 585, y: 295, width: 500, height: 324 });
    assert.equal(preview.equipment[0].item.id, "item.sword.bronze");
    await capture("equipment-v1-bonuses-weight");
  });
  await check("declared body-type permissions are respected in character phase without extra cosmetics", async () => {
    await mount();
    await patch(() => {
      const s = window.component.services, w = structuredClone(s.state().world);
      w.ui.activeInterface = "interface.appearance"; w.ui.appearance.confirmed = false;
      w.ui.appearance.choices.body_type[1].permission = { allowed: false, code: "Unavailable", reason: "That declared body type is temporarily unavailable." };
      s.publish({ ...s.state(), world: w, phase: "character" });
    });
    assert.equal(await page.getByRole("button", { name: "Body type B", exact: true }).isDisabled(), true);
    await reset(); await page.getByRole("button", { name: "Confirm appearance", exact: true }).click(); await frame();
    assert.deepEqual(await last(), { kind: "confirm_appearance", appearance: { body_type: 0 } });
    assert.equal(await page.evaluate(() => window.component.services.state().world.ui.appearance.confirmed), false);
    assert.equal(await page.evaluate(() => window.component.services.calls.some(call => call.method === "createCharacter")), false);
    const preview = await page.evaluate(() => window.getUiPreviewRequest(window.component.ui));
    assert.equal(preview.purpose, "appearance");
    assert.equal(preview.appearance.body_type, 0);
    assert.deepEqual(preview.bounds, { x: 595, y: 299, width: 480, height: 315 });
    await capture("appearance-v1-declared-body-types");
  });
  await check("reward continuation and confirmation IDs stay authoritative with no local reward/credit grants", async () => {
    await mount(); await reset();
    await patch(() => {
      const s = window.component.services, world = structuredClone(s.state().world);
      world.ui.reward = { id: "reward.opaque", kind: "quest", interface: "interface.quest_reward", title: "Cook's Assistant",
        lines: ["Authoritative award line"], items: [], xp: [{ skill: "skill.cooking", amountTenths: "9007199254740993" }],
        questPoints: 1, quest: "quest.cooks_assistant", skill: null, level: null,
        continuation: { kind: "ui_dismiss", presentation_id: "reward.opaque" } };
      world.ui.activeInterface = "interface.quest_reward"; s.patchWorld(world);
    });
    await page.locator("canvas").press(" "); await frame();
    assert.deepEqual(await last(), { kind: "ui_dismiss", presentation_id: "reward.opaque" });
    assert.equal(await page.evaluate(() => window.component.services.state().world.ui.reward.id), "reward.opaque");
    await patch(() => {
      const s = window.component.services, world = structuredClone(s.state().world);
      world.ui.reward = null; world.ui.activeInterface = null;
      world.ui.confirmation = { id: "confirmation.opaque", kind: "coffer_offer", title: "Confirm offering",
        lines: ["Review the authoritative offer."], items: [], credit: "9007199254740993" };
      s.patchWorld(world);
    });
    await click("ui-confirm-accept");
    assert.deepEqual(await last(), { kind: "ui_confirm", confirmation_id: "confirmation.opaque", accept: true });
    assert.equal(await page.evaluate(() => window.component.services.state().world.ui.confirmation.credit), "9007199254740993");
    await click("ui-confirm-cancel");
    assert.deepEqual(await last(), { kind: "ui_confirm", confirmation_id: "confirmation.opaque", accept: false });
    await capture("confirmation-v1-u64-credit");
  });
  await check("unpublished level-up layout bindings remain explicitly incomplete, never a fake quest scroll", async () => {
    await mount();
    await patch(() => {
      const s = window.component.services, w = structuredClone(s.state().world);
      w.ui.reward = { id: "level-up.opaque", kind: "level_up", interface: "interface.level_up.popup", title: "Cooking level 4",
        lines: ["Actual level-up payload"], items: [], xp: [], questPoints: 0, quest: null, skill: "skill.cooking", level: 4,
        continuation: { kind: "ui_dismiss", presentation_id: "level-up.opaque" } };
      s.patchWorld(w);
    });
    assert.match(await page.getByRole("status").innerText(), /Required source layout unavailable for interface.level_up.popup/);
    assert.equal(await page.locator('[data-ui-control="reward-details"]').count(), 0);
    await reset(); await click("notice-close");
    assert.deepEqual(await last(), { kind: "ui_dismiss", presentation_id: "level-up.opaque" });
    assert.equal(await page.evaluate(() => window.component.services.state().world.ui.reward.id), "level-up.opaque");
    await capture("level-up-source-binding-required");
  });
  assert.deepEqual(errors, []);
} finally {
  await writeFile(resolve(results, "gameplay-ui-v1-tests.json"), JSON.stringify({
    scope: "Published game.ui.v1 component integration ONLY; deterministic service double, not backend/protobuf implementation proof",
    cases, errors, browser: browser.version(), finalAcceptance: false,
  }, null, 2) + "\n");
  await browser.close(); await host.close();
  console.log(`${cases.filter(test => test.passed).length}/${cases.length} versioned UI cases passed.`);
}
