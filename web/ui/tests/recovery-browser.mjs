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
const reset = () => page.evaluate(() => { window.component.services.intents.length = 0; });
const patch = async change => { await page.evaluate(change); await frame(); };
const mount = async (empty = false) => {
  await page.evaluate(async empty => {
    const { mount } = await import("/web/ui/tests/component-fixture.ts");
    const { recoveryContextFixture } = await import("/web/ui/tests/recovery-fixture.ts");
    await mount("world");
    window.component.services.patchWorld(recoveryContextFixture(empty).world);
  }, empty);
  await frame();
};
const retrieve = async amount => {
  await page.getByRole("button", { name: `Retrieve ${amount}`, exact: true }).click();
  await frame();
};
const amountField = () => page.getByRole("textbox", { name: "Retrieve how many?", exact: true });
async function check(name, run) {
  try { await run(); cases.push({ name, passed: true }); }
  catch (error) { cases.push({ name, passed: false, error: error.message }); throw error; }
}

try {
  await mkdir(resolve(results, "recovery"), { recursive: true });
  await page.goto(host.url);
  await check("full Office includes every record and selected partial requests retain Office rather than physical grave storage", async () => {
    await mount();
    assert.equal(await page.locator('[data-ui-control^="recovery-item-"]').count(), 2);
    assert.equal(await page.evaluate(() => window.component.services.state().world.recovery.items.length), 1);
    await click("recovery-item-entry.two");
    assert.equal(await page.locator('[data-ui-control="recovery-item-entry.two"]').getAttribute("aria-pressed"), "true");
    for (const [label, amount] of [["1", { kind: "quantity", quantity: 1 }], ["5", { kind: "quantity", quantity: 5 }], ["All", { kind: "all" }]]) {
      await reset(); await retrieve(label);
      assert.deepEqual(await intents(), [{ kind: "recovery_take", death: "death.two", storage: "death_office",
        items: [{ id: "entry.two", amount }] }]);
    }
    await reset(); await retrieve("X");
    await amountField().fill("3"); await amountField().press("Enter"); await frame();
    assert.deepEqual(await intents(), [{ kind: "recovery_take", death: "death.two", storage: "death_office",
      items: [{ id: "entry.two", amount: { kind: "quantity", quantity: 3 } }] }]);
    assert.equal(await page.evaluate(() => window.component.services.state().world.ui.recovery.management.context.slots[1].entry.item.quantity), 7);
    await page.keyboard.press("Escape"); await frame();
    await page.screenshot({ path: resolve(results, "recovery/multiple-records.png") });
  });
  await check("whole-context Take-All sends exactly one unchanged observation, not entry or record fan-out", async () => {
    await mount(); await reset();
    const selection = await page.evaluate(() => window.component.services.state().world.ui.recovery.management.context.takeAll.selection);
    await page.getByRole("button", { name: "Take-All", exact: true }).click(); await frame();
    assert.deepEqual(await intents(), [{ kind: "recovery_take_all", selection }]);
    assert.equal(await page.evaluate(() => window.component.services.state().world.ui.recovery.management.context.slots.length), 2);
    assert.equal(await page.evaluate(() => window.component.services.state().world.ui.recovery.cofferBalance), "9007199254740993");
  });
  await check("a held Take-All menu does not silently rebind after the same context changes quantities", async () => {
    await mount(); await reset();
    await page.getByRole("button", { name: "Take-All", exact: true }).click({ button: "right" }); await frame();
    assert.equal(await page.locator('[data-ui-control^="menu-"]').filter({ hasText: /^Take-All$/ }).count(), 1);
    await patch(() => {
      const services = window.component.services, world = structuredClone(services.state().world);
      const management = world.ui.recovery.management, context = management.context;
      const entry = context.slots[1].entry;
      entry.item.quantity = 6; entry.inventoryCapacity = 6; entry.bankCapacity = 6; entry.fullStackFee = "264";
      context.slots[0].selectedTypeCaption.quantity = "13"; context.slots[0].selectedTypeCaption.totalFee = "546";
      context.slots[1].selectedTypeCaption.quantity = "13"; context.slots[1].selectedTypeCaption.totalFee = "572";
      context.takeAll.selection.records[1].entries[0].quantity = 6;
      context.takeAll.plan.transfers[1].quantity = 6; context.takeAll.plan.transfers[1].fee = "264";
      context.takeAll.plan.totalFee = "558";
      management.panels[1].entries = [entry]; management.panels[1].fullSelectionFee = "264";
      services.patchWorld(world);
    });
    await page.locator('[data-ui-control^="menu-"]').filter({ hasText: /^Take-All$/ }).click(); await frame();
    assert.deepEqual(await intents(), []);
    assert.match(await page.getByRole("status").innerText(), /full recovery context or item selection changed/);
  });
  await check("semantic context identity retains local selection, while a different Office cancels selection and amount entry", async () => {
    await mount(); await reset(); await click("recovery-item-entry.two"); await retrieve("X");
    await amountField().fill("5");
    await patch(() => {
      const services = window.component.services, world = structuredClone(services.state().world);
      services.patchWorld(world);
    });
    assert.equal(await amountField().inputValue(), "5");
    await page.keyboard.press("Escape"); await frame();
    assert.equal(await page.locator('[data-ui-control="recovery-item-entry.two"]').getAttribute("aria-pressed"), "true");
    await retrieve("X"); await amountField().fill("5");
    await patch(() => {
      const services = window.component.services, world = structuredClone(services.state().world);
      world.player.instance = "instance.reentered-office";
      world.ui.recovery.management.context.identity.instance = world.player.instance;
      world.ui.recovery.management.context.takeAll.selection.context.instance = world.player.instance;
      services.patchWorld(world);
    });
    assert.equal(await amountField().count(), 0);
    assert.equal(await page.locator('[data-ui-control="recovery-item-entry.two"]').getAttribute("aria-pressed"), "false");
    assert.deepEqual(await intents(), []);
    await click("recovery-item-entry.two"); await retrieve("X"); await amountField().fill("2");
    await page.keyboard.press("Escape"); await frame();
    assert.equal(await amountField().count(), 0);
    assert.deepEqual(await intents(), []);
  });
  await check("empty Office remains open with zero actual rows and source-denied Take-All even without a legacy recovery", async () => {
    await mount(true); await reset();
    assert.equal(await page.evaluate(() => window.component.services.state().world.recovery), null);
    assert.equal(await page.locator('[data-ui-control^="recovery-item-"]').count(), 0);
    const take = page.getByRole("button", { name: "Take-All", exact: true });
    assert.equal(await take.isDisabled(), true);
    assert.match(await take.getAttribute("aria-description"), /no items in this source recovery context/);
    await page.screenshot({ path: resolve(results, "recovery/empty-office.png") });
    await page.locator('[data-ui-control^="recovery-option-"]').filter({ hasText: /^Close$/ }).click(); await frame();
    assert.deepEqual(await intents(), [{ kind: "close_interface" }]);
  });
  await check("a mismatched Office projection is explicit and cannot fall through to a singular legacy panel", async () => {
    await mount(); await reset();
    await patch(() => {
      const services = window.component.services, world = structuredClone(services.state().world);
      world.ui.recovery.management.context.identity.instance = "foreign-instance";
      world.ui.recovery.management.context.takeAll.selection.context.instance = "foreign-instance";
      services.patchWorld(world);
    });
    assert.match(await page.getByRole("status").innerText(), /different source instance/);
    assert.deepEqual(await intents(), []);
    assert.equal(await page.locator('[data-ui-control^="recovery-item-"]').count(), 0);
  });
  await check("native selected-type caption data preserves the original short and 80-slot Office images", async () => {
    await page.evaluate(() => window.component.ui.dispose());
    await mkdir(resolve(results, "projections"), { recursive: true });
    for (const [name, quantity, totalFee] of [
      ["native-retrieval-669-12345-7-42", "7", "294"],
      ["native-retrieval-scroll-669", "35", "1470"],
    ]) {
      await page.evaluate(({ name, quantity, totalFee }) => window.recoveryProjection(name,
        { kind: "source", sourceId: 882, quantity, unitFee: "42", totalFee }), { name, quantity, totalFee });
      await page.screenshot({ path: resolve(results, `projections/${name}.png`) });
    }
  });
  assert.deepEqual(errors, []);
} finally {
  await writeFile(resolve(results, "recovery-context-browser.json"), JSON.stringify({
    scope: "Native recovery UI component and original-source projection checks; controlled services, not a player journey",
    cases, errors, browser: browser.version(), finalAcceptance: false,
  }, null, 2) + "\n");
  await browser.close(); await host.close();
  console.log(`${cases.filter(test => test.passed).length}/${cases.length} recovery context UI cases passed.`);
}
