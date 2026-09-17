import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { browserHost, launchBrowser, results } from "./browser-host.mjs";

const host = await browserHost(), browser = await launchBrowser(), errors = [];
try {
  const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
  page.on("pageerror", error => errors.push(error.message));
  await page.goto(host.url);
  await page.waitForFunction(() => typeof window.sourceFixture === "function");
  const names = await page.evaluate(async () => {
    const catalogue = await (await fetch("/assets/ui/manifest.json")).json();
    return Object.keys(catalogue.templates).filter(name =>
      /^native-(production-(choice-|hover|amount-)|smithing-bronze|death-preview-populated|reward-fields)/.test(name));
  });
  for (const kind of ["presentations", "presentation-projections"]) await mkdir(resolve(results, kind), { recursive: true });
  for (const name of names) {
    await page.evaluate(name => window.sourceFixture(name), name);
    await page.screenshot({ path: resolve(results, "presentations", name + ".png") });
    if (name !== "native-smithing-bronze") {
      await page.evaluate(async name => {
        const { presentationProjection } = await import("/web/ui/tests/presentation-fixture.ts");
        await presentationProjection(name);
      }, name);
      await page.screenshot({ path: resolve(results, "presentation-projections", name + ".png") });
    }
  }
  await writeFile(resolve(results, "presentation-browser.json"), JSON.stringify({
    scope: "Original native component frames and explicit-field projection ONLY; synthetic source-only inputs, not live gameplay",
    names, errors, browser: browser.version(), dpr: 1, finalAcceptance: false,
  }, null, 2) + "\n");
  if (errors.length) throw new Error(errors.join("\n"));
} finally { await browser.close(); await host.close(); }
