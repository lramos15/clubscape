import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { browserHost, launchBrowser, results } from "./browser-host.mjs";

const host = await browserHost(), browser = await launchBrowser(), errors = [];
try {
  const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
  page.on("pageerror", error => errors.push(error.message));
  await page.goto(host.url); await page.waitForFunction(() => typeof window.sourceFixture === "function");
  const inputs = await page.evaluate(async () => (await (await fetch("/assets/ui/manifest.json")).json()).boundedUiInputs);
  assert.equal(inputs.limit, 18);
  assert.ok(inputs.states.length <= 18);
  assert.equal(new Set(inputs.states.map(state => state.case)).size, inputs.states.length);
  for (const directory of ["bounded-source", "bounded-projections"]) await mkdir(resolve(results, directory), { recursive: true });
  for (const state of inputs.states) {
    await page.evaluate(name => window.sourceFixture(name), state.case);
    await page.screenshot({ path: resolve(results, "bounded-source", state.case + ".png") });
    if (state.group === 134) {
      await page.evaluate(async input => {
        const { settingsProjectionFixture } = await import("/web/ui/tests/bounded-fixture.ts");
        await settingsProjectionFixture({ category: input.category, search: input.search, moreInfo: input.moreInfo,
          hideLocked: input.hideLocked, scroll: 0, choice: null });
      }, state.inputs);
      await page.screenshot({ path: resolve(results, "bounded-projections", state.case + ".png") });
    } else {
      await page.evaluate(async name => {
        const { independentProjectionFixture } = await import("/web/ui/tests/bounded-fixture.ts");
        await independentProjectionFixture(name);
      }, state.case);
      await page.screenshot({ path: resolve(results, "bounded-projections", state.case + ".png") });
    }
  }
  await writeFile(resolve(results, "bounded-browser.json"), JSON.stringify({ scope: "Original controlled independent UI states only",
    browser: browser.version(), inputs, errors, finalAcceptance: false }, null, 2) + "\n");
  assert.deepEqual(errors, []);
} finally { await browser.close(); await host.close(); }
