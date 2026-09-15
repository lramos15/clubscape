import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { browserHost, launchBrowser, results } from "./browser-host.mjs";

const host = await browserHost(), browser = await launchBrowser(), cases = [], errors = [];
const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
page.on("pageerror", error => errors.push(error.message));
const frame = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
const mount = async phase => {
  await page.evaluate(async phase => { const fixture = await import("/web/ui/tests/component-fixture.ts"); await fixture.mount(phase); }, phase);
  await frame();
};
const click = async id => { await page.locator(`[data-ui-control="${id}"]`).click(); await frame(); };
const intents = () => page.evaluate(() => window.component.services.intents);
const resetIntents = () => page.evaluate(() => { window.component.services.intents.length = 0; });
const lastIntent = async () => (await intents()).at(-1);
const reject = (message, errorId) => page.evaluate(value => { window.component.services.rejection = value; }, { message, errorId });
async function check(name, test) {
  const start = Date.now();
  try { await test(); cases.push({ name, passed: true, milliseconds: Date.now() - start }); }
  catch (error) { cases.push({ name, passed: false, error: error.message }); throw error; }
}
async function capture(name) {
  await page.mouse.move(600, 600); await frame();
  await page.screenshot({ path: resolve(results, `components/${name}.png`) });
}

try {
  await mkdir(resolve(results, "components"), { recursive: true });
  await page.goto(host.url);
  await check("real password fields, registration mismatch, rejection and retained input", async () => {
    await mount("title"); await click("new-account");
    await page.getByLabel("Login name", { exact: true }).fill("reference_user");
    await page.getByLabel("Password", { exact: true }).fill("component-password");
    await page.getByLabel("Confirm password", { exact: true }).fill("different");
    await click("register-submit");
    assert.match(await page.getByRole("status").innerText(), /passwords do not match/);
    assert.equal(await page.evaluate(() => window.component.services.calls.filter(c => c.method === "register").length), 0);
    await click("entry-retry"); await frame();
    assert.equal(await page.getByLabel("Login name", { exact: true }).inputValue(), "reference_user");
    await page.getByLabel("Confirm password", { exact: true }).fill("component-password");
    await reject("That login name is unavailable.", "test.account.name");
    await click("register-submit"); await capture("registration-rejected");
    assert.match(await page.getByRole("status").innerText(), /test.account.name/);
    await click("entry-retry");
    assert.equal(await page.getByLabel("Password", { exact: true }).getAttribute("type"), "password");
    assert.equal(await page.getByLabel("Password", { exact: true }).inputValue(), "component-password");
    await page.getByLabel("Login name", { exact: true }).fill("another_user");
    await page.getByLabel("Password", { exact: true }).press("Enter"); await frame();
    assert.equal(await page.evaluate(() => window.component.services.calls.filter(c => c.method === "register").length), 2);
    assert.equal(await page.evaluate(() => window.component.services.state().phase), "register");
  });
  await check("login submit, keyboard focus and cancel route through services", async () => {
    await mount("login");
    await page.getByLabel("Login name", { exact: true }).fill("reference_user");
    await page.getByLabel("Password", { exact: true }).fill("component-password");
    await page.getByLabel("Password", { exact: true }).press("Enter"); await frame();
    assert.equal(await page.evaluate(() => window.component.services.calls.at(-1).method), "login");
    await click("entry-cancel");
    assert.equal(await page.evaluate(() => window.component.services.state().phase), "title");
    await capture("title");
  });
  await check("capability/loading/connecting feedback and audio controls use real app state", async () => {
    await mount("title");
    await click("title-audio");
    assert.equal(await page.evaluate(() => window.component.services.calls.filter(c => c.method === "audioVolume").length), 3);
    await page.evaluate(() => { const s = window.component.services; s.publish({ ...s.state(), phase: "capability_check",
      loading: { completed: 2, total: 7, label: "Loading original source sprites" } }); }); await frame();
    await capture("loading-real-counters");
    await page.evaluate(() => { const s = window.component.services; s.publish({ ...s.state(), phase: "connecting",
      loading: { completed: 3, total: 7, label: "Preparing required source assets" } }); }); await frame();
    await capture("connecting");
    await page.evaluate(() => { const s = window.component.services; s.publish({ ...s.state(), phase: "error", loading: null,
      error: { message: "WebGPU is required for ClubScape.", errorId: "test.capability.webgpu", recoverable: false } }); }); await frame();
    assert.equal(await page.locator('[data-ui-control="entry-retry"]').isDisabled(), true);
    assert.match(await page.getByRole("status").innerText(), /test.capability.webgpu/);
    await capture("capability-error");
  });
  await check("native character controls without a fabricated WorldView retain selection on reject", async () => {
    await mount("character");
    await page.getByRole("button", { name: "Body type B", exact: true }).click(); await frame();
    await reject("Character creation is temporarily unavailable.", "test.character.rejected");
    await page.getByRole("button", { name: "Confirm appearance", exact: true }).click(); await frame();
    assert.match(await page.getByRole("status").innerText(), /test.character.rejected/);
    assert.equal(await page.evaluate(() => window.component.services.state().world), null);
    await click("notice-close");
    assert.equal(await page.getByRole("button", { name: "Body type B", exact: true }).getAttribute("aria-pressed"), "true");
    await page.getByRole("button", { name: "Confirm appearance", exact: true }).click(); await frame();
    assert.deepEqual(await page.evaluate(() => window.component.services.calls.at(-1)),
      { method: "createCharacter", args: [{ body_type: 1 }] });
    await capture("character-renderer-preview-pending");
  });
  await check("inventory default, right-click, item selection, cancellation and no optimistic grants", async () => {
    await mount("world");
    await page.waitForFunction(() => document.querySelector('[data-ui-control="inventory-0"]')?.getAttribute("aria-label")?.includes("Wield"));
    await click("inventory-0");
    assert.deepEqual(await lastIntent(), { kind: "equip", inventory_slot: 0 });
    assert.equal(await page.evaluate(() => window.component.services.state().world.player.inventory[0].item.sourceId), 1265);
    await resetIntents();
    await page.locator('[data-ui-control="inventory-0"]').click({ button: "right" }); await frame();
    const labels = await page.locator('[data-ui-control^="menu-"]').allTextContents();
    assert.deepEqual(labels, ["Wield Bronze pickaxe", "Use Bronze pickaxe", "Drop Bronze pickaxe", "Examine Bronze pickaxe", "Cancel"]);
    await page.getByRole("button", { name: "Use Bronze pickaxe", exact: true }).click(); await frame();
    await click("inventory-1");
    assert.deepEqual(await lastIntent(), { kind: "use_item", inventory_slot: 0, target: { kind: "inventory", slot: 1 } });
    await page.locator("canvas").press("Escape"); await frame();
    await capture("inventory");
  });
  await check("drag/swap dispatch, no client mutation, and following click remains live", async () => {
    await mount("world"); await resetIntents();
    const from = await page.locator('[data-ui-control="inventory-0"]').boundingBox();
    const to = await page.locator('[data-ui-control="inventory-12"]').boundingBox();
    await page.mouse.move(from.x + 10, from.y + 10); await page.mouse.down();
    await page.waitForTimeout(130); await page.mouse.move(to.x + 10, to.y + 10, { steps: 5 }); await page.mouse.up(); await frame();
    assert.deepEqual(await lastIntent(), { kind: "move_inventory", from: 0, to: 12 });
    assert.equal(await page.evaluate(() => window.component.services.state().world.player.inventory[12].item), null);
    await click("inventory-2"); assert.deepEqual(await lastIntent(), { kind: "eat", inventory_slot: 2 });
  });
  await check("rejected game action displays actual reason/error ID and retains state", async () => {
    await mount("world"); await reject("You need a higher Attack level.", "test.equipment.requirement");
    await click("inventory-0"); assert.match(await page.getByRole("status").innerText(), /test.equipment.requirement/);
    assert.equal(await page.evaluate(() => window.component.services.state().world.player.inventory[0].item.quantity), 1);
    await capture("action-rejection"); await click("notice-close");
  });
  await check("genuine tutorial lock dispatches no actions for mouse or keyboard", async () => {
    await mount("world");
    await page.evaluate(() => {
      const service = window.component.services, world = structuredClone(service.state().world);
      world.player.tutorialStage = "stage.tutorial.guide_greeting"; world.player.unlockedInterfaces = ["interface.logout", "interface.settings"];
      service.patchWorld(world);
    }); await frame(); await resetIntents();
    assert.equal(await page.locator('[data-ui-control="tab-3"]').isDisabled(), true);
    await page.locator("canvas").press("F1"); await frame(); assert.equal((await intents()).length, 0);
    await capture("tutorial-locked");
  });
  await check("bank quantity, notes, search, cancel, rejected amount and atomic state boundary", async () => {
    await mount("world");
    await page.evaluate(() => {
      const s = window.component.services;
      s.patchWorld({ bank: { banker: "spawn.banker", capacity: 400, allowNotes: true,
        slots: [{ index: 0, item: { ...s.state().world.player.inventory[0].item, quantity: 12 } },
          { index: 1, item: { ...s.state().world.player.inventory[3].item, quantity: 150000 } }] } });
    }); await frame();
    await click("bank-control-25--1"); await click("bank-0");
    assert.deepEqual(await lastIntent(), { kind: "bank_withdraw", banker: "spawn.banker", bank_slot: 0, quantity: 1, noted: true });
    await click("bank-control-42--1");
    await page.getByLabel("Search bank", { exact: true }).fill("coins"); await frame();
    assert.equal(await page.locator('[data-ui-control="bank-0"]').count(), 0);
    assert.equal(await page.locator('[data-ui-control="bank-1"]').count(), 1);
    await page.getByLabel("Search bank", { exact: true }).press("Escape"); await frame();
    await page.locator('[data-ui-control="bank-1"]').click({ button: "right" }); await frame();
    await page.getByRole("button", { name: "Withdraw-X Coins", exact: true }).click(); await frame();
    await page.getByLabel("Enter amount:", { exact: true }).fill("0");
    await page.getByLabel("Enter amount:", { exact: true }).press("Enter"); await frame();
    assert.match(await page.getByRole("status").innerText(), /positive whole number/);
    await click("notice-close"); await page.getByLabel("Enter amount:", { exact: true }).fill("7");
    await reject("The bank has closed.", "test.bank.closed");
    await page.getByLabel("Enter amount:", { exact: true }).press("Enter"); await frame();
    assert.deepEqual(await lastIntent(), { kind: "bank_withdraw", banker: "spawn.banker", bank_slot: 1, quantity: 7, noted: true });
    assert.match(await page.getByRole("status").innerText(), /test.bank.closed/);
    await click("notice-close"); assert.equal(await page.getByLabel("Enter amount:", { exact: true }).inputValue(), "7");
    await page.getByLabel("Enter amount:", { exact: true }).press("Escape"); await frame();
    await capture("bank-search-notes");
    assert.equal(await page.evaluate(() => window.component.services.state().world.bank.slots[1].item.quantity), 150000);
  });
  await check("shop value, buy, sell, zero stock and server rejection", async () => {
    await mount("world");
    await page.evaluate(() => {
      const s = window.component.services;
      s.patchWorld({ shop: { id: "shop.general", name: "General Store", rows: [
        { index: 0, item: s.state().world.player.inventory[0].item, stock: 7, buyPrice: 2, sellPrice: 1 },
        { index: 1, item: s.state().world.player.inventory[1].item, stock: 0, buyPrice: 4, sellPrice: 2 },
      ] } });
    }); await frame(); await resetIntents();
    await click("shop-0"); assert.match(await page.getByRole("status").innerText(), /currently costs 2 coins/);
    assert.equal((await intents()).length, 0); await click("notice-close");
    await click("shop-mode-10"); await click("shop-0");
    assert.deepEqual(await lastIntent(), { kind: "shop_buy", shop: "shop.general", item_index: 0, quantity: 5, expected_item: "item.pickaxe.bronze" });
    assert.equal(await page.locator('[data-ui-control="shop-1"]').isDisabled(), true);
    await click("inventory-0");
    assert.deepEqual(await lastIntent(), { kind: "shop_sell", shop: "shop.general", inventory_slot: 0, quantity: 5 });
    await reject("You don't have enough coins.", "test.shop.coins"); await click("shop-0");
    assert.match(await page.getByRole("status").innerText(), /test.shop.coins/);
    await capture("shop-rejection");
  });
  await check("every buy amount sends the displayed canonical identity, never source ID or price", async () => {
    await mount("world");
    await page.evaluate(() => {
      const s = window.component.services;
      s.patchWorld({ shop: { id: "shop.general", name: "General Store", rows: [
        { index: 0, item: s.state().world.player.inventory[0].item, stock: 75, buyPrice: 23, sellPrice: 9 },
      ] } });
    }); await frame();
    for (const [control, quantity] of [[8, 1], [10, 5], [12, 10], [14, 50]]) {
      await click(`shop-mode-${control}`); await click("shop-0");
      assert.deepEqual(await lastIntent(), { kind: "shop_buy", shop: "shop.general", item_index: 0, quantity,
        expected_item: "item.pickaxe.bronze" });
    }
    await page.locator('[data-ui-control="shop-0"]').click({ button: "right" }); await frame();
    await page.getByRole("button", { name: "Buy-X Bronze pickaxe", exact: true }).click(); await frame();
    await page.getByLabel("Enter amount:", { exact: true }).fill("7");
    await page.getByLabel("Enter amount:", { exact: true }).press("Enter"); await frame();
    assert.deepEqual(await lastIntent(), { kind: "shop_buy", shop: "shop.general", item_index: 0, quantity: 7,
      expected_item: "item.pickaxe.bronze" });
    assert.equal(await page.evaluate(() => window.component.services.state().world.shop.rows[0].stock), 75);
  });
  await check("reused extra rows cannot retarget a held buy menu, quantity prompt or price view", async () => {
    await mount("world");
    const stock = (replacement) => page.evaluate(replacement => {
      const s = window.component.services;
      s.patchWorld({ shop: { id: "shop.general", name: "General Store", rows: [
        { index: 12, item: s.state().world.player.inventory[replacement ? 1 : 0].item,
          stock: 2, buyPrice: replacement ? 49 : 23, sellPrice: 9 },
      ] } });
    }, replacement);
    await stock(false); await frame(); await resetIntents();
    await page.locator('[data-ui-control="shop-12"]').click({ button: "right" }); await frame();
    await stock(true); await frame();
    await page.getByRole("button", { name: "Buy-1 Bronze pickaxe", exact: true }).click(); await frame();
    assert.equal((await intents()).length, 0);
    assert.match(await page.getByRole("status").innerText(), /ui.shop.stale/);
    await click("notice-close");
    await stock(false); await frame();
    await page.locator('[data-ui-control="shop-12"]').click({ button: "right" }); await frame();
    await page.getByRole("button", { name: "Buy-X Bronze pickaxe", exact: true }).click(); await frame();
    await page.getByLabel("Enter amount:", { exact: true }).fill("2");
    await stock(true); await frame();
    await page.getByLabel("Enter amount:", { exact: true }).press("Enter"); await frame();
    assert.equal((await intents()).length, 0);
    assert.match(await page.getByRole("status").innerText(), /Choose the current item again/);
    await click("notice-close");
    await stock(false); await frame();
    await page.locator('[data-ui-control="shop-12"]').click({ button: "right" }); await frame();
    await stock(true); await frame();
    await page.getByRole("button", { name: "Value Bronze pickaxe", exact: true }).click(); await frame();
    assert.match(await page.getByRole("status").innerText(), /ui.shop.stale/);
    assert.doesNotMatch(await page.getByRole("status").innerText(), /49 coins/);
    await click("notice-close");
    await click("shop-mode-8"); await click("shop-12");
    assert.deepEqual(await lastIntent(), { kind: "shop_buy", shop: "shop.general", item_index: 12, quantity: 1,
      expected_item: "item.axe.bronze" });
    await capture("shop-reused-row-new-selection");
  });
  await check("server stale/uncertain rejection preserves the original buy intent without automatic retargeting", async () => {
    for (const errorId of ["StaleCommand", "transport.uncertain"]) {
      await mount("world");
      await page.evaluate(() => {
        const s = window.component.services;
        s.patchWorld({ shop: { id: "shop.general", name: "General Store", rows: [
          { index: 12, item: s.state().world.player.inventory[0].item, stock: 2, buyPrice: 23, sellPrice: 9 },
        ] } });
      }); await frame();
      await click("shop-mode-8"); await resetIntents();
      await reject("Refresh the shop and choose the current item.", errorId);
      await click("shop-12");
      const original = await lastIntent();
      assert.equal(original.expected_item, "item.pickaxe.bronze");
      assert.match(await page.getByRole("status").innerText(), new RegExp(errorId.replace(".", "\\.")));
      await page.evaluate(() => {
        const s = window.component.services;
        s.patchWorld({ shop: { ...s.state().world.shop, rows: [
          { index: 12, item: s.state().world.player.inventory[1].item, stock: 1, buyPrice: 49, sellPrice: 9 },
        ] } });
      }); await frame();
      await click("notice-close"); await frame();
      assert.equal((await intents()).length, 1);
      assert.deepEqual(await lastIntent(), original);
      assert.equal(await page.evaluate(() => window.component.services.state().world.player.inventory[3].item.quantity), 12345);
    }
    await capture("shop-stale-intent-retained");
  });
  await check("equipment, skills XP, combat/auto-retaliate, run and prayer controls", async () => {
    await mount("world"); await click("tab-4"); await click("equipment-slot.weapon");
    assert.deepEqual(await lastIntent(), { kind: "unequip", slot: "slot.weapon" });
    await click("tab-1"); await click("skill-skill.attack");
    assert.match(await page.getByRole("status").innerText(), /Current XP: 0/); await click("notice-close");
    await click("tab-0"); await click("combat-style-1");
    assert.deepEqual(await lastIntent(), { kind: "set_combat_style", style: "style.sword.bronze.stab.aggressive" });
    await click("auto-retaliate");
    assert.deepEqual(await lastIntent(), { kind: "set_setting", setting: { setting: "auto_retaliate", enabled: false } });
    await click("run"); assert.deepEqual(await lastIntent(), { kind: "set_setting", setting: { setting: "run", enabled: true } });
    await click("tab-5"); await page.getByRole("button", { name: "Thick Skin", exact: true }).click(); await frame();
    assert.deepEqual(await lastIntent(), { kind: "set_prayer", prayer: "prayer.thick_skin", enabled: true });
    await capture("prayer");
  });
  await check("spell selection, actual renderer world-pick bridge, unavailable target and minimap", async () => {
    await mount("world"); await click("tab-6"); await click("spell-11"); await resetIntents();
    await page.evaluate(() => window.forwardWorldPointer(window.component.ui, { kind: "primary", x: 600, y: 500,
      pick: { kind: "entity", id: "spawn.cook", tile: { x: 3223, y: 3218, plane: 0 } } })); await frame();
    assert.deepEqual(await lastIntent(), { kind: "cast", spell: "spell.wind_strike", target: "spawn.cook" });
    await page.locator("canvas").press("Escape"); await frame();
    await page.evaluate(() => window.forwardWorldPointer(window.component.ui, { kind: "context", x: 600, y: 500,
      pick: { kind: "entity", id: "spawn.cook", tile: { x: 3223, y: 3218, plane: 0 } } })); await frame();
    assert.equal(await page.getByRole("button", { name: "Attack Cook", exact: true }).isDisabled(), true);
    await page.getByRole("button", { name: "Cancel", exact: true }).click(); await frame();
    await page.locator('[data-ui-control="minimap"]').click({ position: { x: 100, y: 80 } }); await frame();
    assert.equal((await lastIntent()).kind, "walk");
    await click("compass"); assert.deepEqual(await page.evaluate(() => window.component.services.cameraRequests), [0]);
  });
  await check("native prayer and spell filters toggle locally, disable dependent options and reflow", async () => {
    await mount("world"); await click("tab-5"); await resetIntents();
    await click("filters-541");
    assert.equal(await page.locator('[data-ui-control^="filter-prayer-"]').count(), 5);
    assert.equal(await page.locator('[data-ui-control="filter-prayer-1"]').isDisabled(), true);
    await click("filter-prayer-0");
    assert.equal(await page.locator('[data-ui-control="filter-prayer-1"]').isDisabled(), false);
    await click("filter-prayer-3"); await click("filters-541");
    assert.equal(await page.locator('[data-ui-control^="prayer-"]').count(), 1);
    assert.equal((await intents()).length, 0);
    await click("filters-541"); await capture("prayer-filter-native-controls");
    await page.locator("canvas").press("Escape"); await frame();
    assert.equal(await page.locator('[data-ui-control^="filter-prayer-"]').count(), 0);
    assert.equal((await intents()).length, 0);
    await click("tab-6"); await resetIntents(); await click("filters-218");
    assert.equal(await page.locator('[data-ui-control^="filter-magic-"]').count(), 7);
    await click("filter-magic-3"); await click("filters-218");
    assert.equal(await page.locator('[data-ui-control^="spell-"]').count(), 3);
    const wind = await page.locator('[data-ui-control="spell-11"]').boundingBox();
    assert.equal(wind.width, 40);
    await click("filters-218"); await click("filter-magic-0"); await capture("spell-filter-native-controls");
    await click("filters-218");
    assert.equal(await page.locator('[data-ui-control="spell-11"]').count(), 0);
    assert.equal((await intents()).length, 0);
  });
  await check("native recovery selection/amount controls preserve items and do not invent balance fields", async () => {
    await mount("world");
    await page.evaluate(() => {
      const s = window.component.services;
      s.patchWorld({ recovery: { death: "death.component", storage: "death_office", remainingTicks: null, items: [
        { id: "recovery.pickaxe", item: s.state().world.player.inventory[0].item, cost: null },
        { id: "recovery.runes", item: { ...s.state().world.player.inventory[4].item, quantity: 7 }, cost: null },
      ] } });
    }); await frame(); await resetIntents();
    await click("recovery-item-recovery.runes");
    assert.equal((await intents()).length, 0);
    await page.getByRole("button", { name: "Retrieve 5", exact: true }).click(); await frame();
    assert.match(await page.getByRole("status").innerText(), /reclaim_quantity/);
    assert.equal((await intents()).length, 0); await click("notice-close");
    await reject("These items cannot be reclaimed from this location.", "test.recovery.location");
    await page.getByRole("button", { name: "Retrieve All", exact: true }).click(); await frame();
    assert.deepEqual(await lastIntent(), { kind: "reclaim", death: "death.component", storage: "death_office", items: ["recovery.runes"] });
    assert.equal(await page.evaluate(() => window.component.services.state().world.recovery.items[1].item.quantity), 7);
    await click("notice-close"); await capture("recovery-native-fields-pending");
  });
  await check("source title animates without app-state progress and long runtime errors paginate", async () => {
    await mount("title");
    const sample = () => page.evaluate(() => {
      const c = document.querySelector("canvas");
      return Array.from(c.getContext("2d").getImageData(Math.floor((innerWidth - 765) / 2), 9, 110, 254).data).reduce((a, b, i) => (a + b * (i % 127 + 1)) >>> 0, 0);
    });
    const first = await sample();
    await page.waitForTimeout(300);
    assert.notEqual(await sample(), first);
    assert.equal(await page.evaluate(() => window.component.services.calls.length), 0);
    assert.equal(await page.evaluate(() => window.component.services.state().loading), null);
    await page.evaluate(() => {
      const s = window.component.services;
      s.publish({ ...s.state(), phase: "error", error: {
        message: Array(12).fill("The required source resource could not be loaded.").join(" "),
        errorId: "test.long.resource.failure", recoverable: true,
      } });
    }); await frame();
    let pages = 0;
    while (await page.locator('[data-ui-control="entry-error-next"]').count()) {
      await click("entry-error-next"); pages++;
      assert.ok(pages < 20);
    }
    assert.ok(pages > 1);
    assert.equal(await page.locator('[data-ui-control="entry-retry"]').count(), 1);
    assert.match(await page.getByRole("status").innerText(), /test.long.resource.failure/);
    assert.equal(await page.evaluate(() => window.component.services.calls.filter(c => c.method === "enterWorld").length), 0);
    await capture("runtime-error-paged-source-frame");
  });
  await check("dialogue choices/continue, authoritative text and quest journal", async () => {
    await mount("world");
    await page.evaluate(() => window.component.services.patchWorld({ dialogue: { id: "dialogue.fixture", speaker: "spawn.cook",
      speakerName: "Cook", portraitAsset: null, text: "What am I to do?",
      choices: [{ id: "help", text: "What's wrong?" }, { id: "leave", text: "I'm busy." }] } })); await frame();
    await page.getByRole("button", { name: "What's wrong?", exact: true }).click(); await frame();
    assert.deepEqual(await lastIntent(), { kind: "select_dialogue", speaker: "spawn.cook", choice: "help" });
    await page.evaluate(() => window.component.services.patchWorld({ dialogue: { id: "dialogue.fixture.2", speaker: "spawn.cook",
      speakerName: "Cook", portraitAsset: null, text: "I need a bucket of milk, an egg and a pot of flour.", choices: [{ id: "continue", text: "Continue" }] } })); await frame();
    await click("dialogue-continue"); assert.deepEqual(await lastIntent(), { kind: "select_dialogue", speaker: "spawn.cook", choice: "continue" });
    await capture("dialogue");
    await page.evaluate(() => window.component.services.patchWorld({ dialogue: null })); await frame();
    await click("tab-2"); await page.getByRole("button", { name: "Cook's Assistant", exact: true }).click(); await frame();
    assert.deepEqual(await lastIntent(), { kind: "open_interface", interface: "interface.quests" });
    await capture("quest-journal");
  });
  await check("resize anchors, no offscreen required controls, reconnect and disposal", async () => {
    await mount("world");
    for (const [width, height] of [[1024, 768], [2560, 1440], [1920, 1080]]) {
      await page.setViewportSize({ width, height });
      await page.evaluate(([w, h]) => window.component.ui.resize(w, h), [width, height]); await frame();
      const bounds = await page.locator('[data-ui-control="inventory-0"]').boundingBox();
      assert.equal(bounds.x, width - 200); assert.equal(bounds.y, height - 290);
      const clipped = await page.locator('[data-ui-control]').evaluateAll(nodes => nodes.filter(node => {
        const b = node.getBoundingClientRect(); return b.left < 0 || b.top < 0 || b.right > innerWidth || b.bottom > innerHeight;
      }).map(n => n.dataset.uiControl));
      assert.deepEqual(clipped, []);
      await capture(`resize-${width}x${height}`);
    }
    await page.evaluate(() => { const s = window.component.services; s.publish({ ...s.state(), phase: "reconnecting" }); }); await frame(); await resetIntents();
    assert.equal(await page.evaluate(() => window.component.ui.capturesPointer(100, 100)), true);
    assert.equal(await page.locator('[data-ui-control="inventory-0"]').count(), 0);
    await capture("reconnecting");
    await page.evaluate(() => window.component.ui.dispose()); await frame();
    assert.equal(await page.locator("[data-clubscape-ui]").count(), 0);
    assert.equal(await page.evaluate(() => window.component.services.subscriptions), 0);
    assert.equal((await intents()).length, 0);
  });
  assert.deepEqual(errors, []);
} finally {
  await writeFile(resolve(results, "component-tests.json"), JSON.stringify({
    scope: "Real browser Canvas/DOM component interactions against a deterministic AppServices double ONLY",
    browser: browser.version(), cases, browserErrors: errors,
    authoritativeServerJourney: false, sourceAcceptance: false, finalAcceptance: false,
  }, null, 2) + "\n");
  await browser.close(); await host.close();
  console.log(`${cases.filter(c => c.passed).length}/${cases.length} component cases passed.`);
}
