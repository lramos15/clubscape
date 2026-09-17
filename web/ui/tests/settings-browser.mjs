import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { browserHost, launchBrowser, results } from "./browser-host.mjs";

const host = await browserHost({ audioAssets: true }), browser = await launchBrowser({ muteAudio: true });
const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
const cases = [], errors = [];
page.on("pageerror", error => errors.push(error.message));
const frame = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
const click = async id => { await page.locator(`[data-ui-control="${id}"]`).click(); await frame(); };
const mount = async () => {
  await page.evaluate(async () => { const fixture = await import("/web/ui/tests/audio-fixture.ts"); await fixture.mountAudio("world"); });
  await frame(); await click("tab-11");
  await page.evaluate(() => { window.component.services.intents.length = 0; });
  await click("settings-32--1");
};
const search = async value => {
  if (await page.locator('[data-ui-control="settings-search-start"]').count()) await click("settings-search-start");
  await page.getByRole("textbox", { name: "Search settings", exact: true }).fill(value); await frame();
};
const setting = id => page.locator(`[data-ui-control^="setting-${id}-"]`).first();
async function check(name, run) {
  try { await run(); cases.push({ name, passed: true }); }
  catch (error) {
    const state = await page.evaluate(() => ({ status: document.querySelector('[role="status"]')?.textContent,
      focus: document.activeElement?.outerHTML.slice(0, 200),
      categories: [...document.querySelectorAll('[data-ui-control^="settings-category-"]')].map(element => element.dataset.uiControl) }));
    cases.push({ name, passed: false, error: error.message, state }); error.message += "\n" + JSON.stringify(state); throw error;
  }
}
try {
  await page.goto(host.url); await mkdir(resolve(results, "settings-components"), { recursive: true });
  await check("All Settings opens the original window with real category pages and no fake server acknowledgement", async () => {
    await mount();
    assert.equal(await page.evaluate(() => window.component.services.intents.length), 0);
    for (const category of [1, 2, 3, 4, 5, 6, 7, 0]) {
      await click(`settings-category-${category}`);
      assert.ok(await page.locator('[data-ui-control="settings-info"]').count());
    }
    const close = page.getByRole("button", { name: "Close interface", exact: true });
    assert.equal(await close.count(), 1);
    await close.click(); await frame();
    assert.equal(await page.locator('[data-ui-control="settings-info"]').count(), 0);
    assert.equal(await page.evaluate(() => window.component.services.intents.length), 0);
    for (const [name, category] of [["Controls", 3], ["Audio", 1], ["Display", 4]]) {
      await page.locator('[data-ui-control="tab-11"]').click({ button: "right" }); await frame();
      await page.getByRole("button", { name, exact: true }).click(); await frame();
      assert.equal(await page.locator('[data-ui-control="settings-info"]').count(), 1);
      assert.equal(await page.locator(`[data-ui-control="settings-category-${category}"]`).count(), 0);
      assert.equal(await page.locator('[data-ui-control^="settings-category-"]').count(), 7);
      await page.getByRole("button", { name: "Close interface", exact: true }).click(); await frame();
    }
    assert.equal(await page.evaluate(() => window.component.services.intents.length), 0);
  });
  await check("native audio sliders use the actual observer and source normalized positions", async () => {
    await mount(); await click("settings-category-1");
    const music = page.getByRole("slider", { name: "Music volume", exact: true });
    assert.equal(await music.getAttribute("aria-valuenow"), "100");
    await music.focus(); await music.press("Home"); await frame();
    for (let step = 0; step < 5; step++) { await music.press("PageUp"); await frame(); }
    assert.equal(await page.evaluate(() => window.audioComponent.state().volumes.music), 0.5);
    assert.equal(await page.evaluate(() => window.audioComponent.state().nativeMixer.music), 44);
    await page.screenshot({ path: resolve(results, "settings-components/audio-native-controls.png") });
  });
  await check("source search, no-results, information and locked filters remain navigable", async () => {
    await mount(); await search("zoom");
    assert.equal(await page.getByRole("textbox", { name: "Search settings" }).inputValue(), "zoom");
    await click("settings-info"); await click("settings-locked");
    await setting(2734).click(); await frame();
    assert.match(await page.getByRole("status").innerText(), /source_setting:2734/);
    await click("notice-close");
    await search("Minimum item value needed for Alchemy spells warning");
    await setting(2784).click(); await frame();
    assert.match(await page.getByRole("status").innerText(), /source_setting:2784/);
    await click("notice-close");
    await search("Public chat"); await setting(2896).click(); await frame();
    assert.match(await page.getByRole("status").innerText(), /source_setting:2896/);
    await click("notice-close");
    await page.screenshot({ path: resolve(results, "settings-components/unknown-source-preferences.png") });
    await search("no such setting");
    assert.equal(await page.locator('[data-ui-control^="setting-"]').count(), 0);
    await page.screenshot({ path: resolve(results, "settings-components/search-no-results.png") });
    await click("settings-category-4");
    assert.ok(await setting(2734).count());
  });
  await check("existing authoritative death settings dispatch requests and never change before a view update", async () => {
    await mount();
    await page.evaluate(() => {
      const services = window.component.services, world = structuredClone(services.state().world);
      world.player.settings.push({ setting: "death_supply_piles", enabled: false }, { setting: "death_auto_equip", enabled: false });
      services.patchWorld(world);
    }); await frame();
    await search("Auto-equip");
    await setting(880).click(); await frame();
    assert.deepEqual(await page.evaluate(() => window.component.services.intents.at(-1)), {
      kind: "set_setting", setting: { setting: "death_auto_equip", enabled: true },
    });
    assert.equal(await page.evaluate(() => window.component.services.state().world.player.settings.find(setting => setting.setting === "death_auto_equip").enabled), false);
    await page.evaluate(() => { window.component.services.rejection = { message: "Actual setting rejection.", errorId: "settings.server.reject" }; });
    await setting(880).click(); await frame();
    assert.match(await page.getByRole("status").innerText(), /settings.server.reject/);
  });
  await check("existing client input settings change real shift/default/menu behavior", async () => {
    await mount(); await search("Shift click to drop");
    await setting(1104).click(); await frame();
    await page.getByRole("button", { name: "Close interface", exact: true }).click(); await frame();
    await click("tab-3");
    await page.evaluate(() => { window.component.services.intents.length = 0; });
    await page.locator('[data-ui-control="inventory-0"]').click({ modifiers: ["Shift"] }); await frame();
    assert.equal(await page.evaluate(() => window.component.services.intents.at(-1).action), "action.component.0.0");
    await click("tab-11"); await click("settings-32--1"); await search("Single mouse button");
    await setting(2769).click(); await frame();
    await page.getByRole("button", { name: "Close interface", exact: true }).click(); await frame();
    await click("tab-3"); await page.evaluate(() => { window.component.services.intents.length = 0; });
    await page.locator('[data-ui-control="inventory-0"]').click(); await frame();
    assert.equal(await page.getByRole("button", { name: "Wield Bronze pickaxe", exact: true }).count(), 1);
    assert.equal(await page.evaluate(() => window.component.services.intents.length), 0);
    await page.locator("canvas").press("Escape"); await frame();
    await page.evaluate(() => window.forwardWorldPointer(window.component.ui, { kind: "primary", x: 600, y: 500,
      pick: { kind: "entity", id: "spawn.cook", tile: { x: 3223, y: 3218, plane: 0 } } })); await frame();
    assert.equal(await page.evaluate(() => window.component.services.intents.length), 0);
    assert.equal(await page.getByRole("button", { name: "Attack Cook", exact: true }).isDisabled(), true);
    await page.getByRole("button", { name: "Talk-to Cook", exact: true }).click(); await frame();
    assert.deepEqual(await page.evaluate(() => window.component.services.intents.at(-1)), {
      kind: "interact_with", target: { kind: "spawn", spawn: "spawn.cook" }, action: "Talk-to",
    });
    await click("tab-11");
    await page.getByRole("button", { name: "Controls", exact: true }).click(); await frame();
    await search("Shift click to drop");
    await setting(1104).click(); await frame();
    await search("Esc"); await setting(2775).click(); await frame();
    await page.locator("canvas").press("Escape"); await frame();
    assert.equal(await page.locator('[data-ui-control="settings-info"]').count(), 1);
    await setting(2775).click(); await frame();
    await page.locator("canvas").press("Escape"); await frame();
    assert.equal(await page.locator('[data-ui-control="settings-info"]').count(), 0);
    await click("tab-3"); await page.evaluate(() => { window.component.services.intents.length = 0; });
    await page.locator('[data-ui-control="inventory-0"]').click({ modifiers: ["Shift"] }); await frame();
    assert.equal(await page.evaluate(() => window.component.services.intents.at(-1).action),
      await page.evaluate(() => window.component.services.state().world.ui.inventoryActions[0].actions.find(action => action.label === "Drop").id));
    await page.evaluate(() => {
      const services = window.component.services, world = structuredClone(services.state().world);
      world.ui.inventoryActions[0].actions.find(action => action.label === "Drop").permission = {
        allowed: false, code: "settings.drop.denied", reason: "The authoritative drop permission is denied.",
      };
      services.intents.length = 0; services.patchWorld(world);
    }); await frame();
    await page.locator('[data-ui-control="inventory-0"]').click({ modifiers: ["Shift"] }); await frame();
    assert.equal(await page.evaluate(() => window.component.services.intents.length), 0);
    assert.match(await page.getByRole("status").innerText(), /authoritative drop permission is denied/);
  });
  await check("display layout and choice paging preserve the approved Classic layout and clear unavailable feedback", async () => {
    await mount(); await search("Game client layout");
    await setting(2732).click(); await frame(); await click("settings-option-2");
    assert.match(await page.getByRole("status").innerText(), /Resizable Modern layout.*not available in this slice/);
    await click("notice-close");
    await search("Quest list sorting");
    await setting(3107).click(); await frame();
    const scroll = page.getByRole("slider", { name: "Setting choices scroll", exact: true });
    await scroll.focus(); await scroll.press("End"); await frame();
    assert.ok(Number(await scroll.getAttribute("aria-valuenow")) > 0);
    await page.locator("canvas").press("Escape"); await frame();
    assert.equal(await page.locator('[data-ui-control^="settings-option-"]').count(), 0);
    await page.screenshot({ path: resolve(results, "settings-components/source-choice-paging.png") });
  });
  assert.deepEqual(errors, []);
} finally {
  await writeFile(resolve(results, "settings-component-tests.json"), JSON.stringify({
    scope: "All Settings component controls only; deterministic services/real audio observer, no live server or renderer setting invention",
    cases, errors, finalAcceptance: false,
  }, null, 2) + "\n");
  await page.evaluate(() => window.audioComponent?.dispose()).catch(error => errors.push(error.message));
  await browser.close(); await host.close();
  console.log(`${cases.filter(test => test.passed).length}/${cases.length} settings component cases passed.`);
}
