import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { browserHost, launchBrowser, results } from "./browser-host.mjs";

const host = await browserHost();
const browser = await launchBrowser();
try {
  const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  await page.goto(host.url);
  await page.waitForFunction(() => typeof window.sourceFixture === "function");
  const cases = process.argv.slice(2);
  const names = cases.length ? cases : ["native-inventory", "native-equipment", "native-skills", "native-combat",
    "native-prayer", "native-magic", "native-quest-list", "native-bank", "native-shop", "native-guide-dialogue",
    "family-guide", "family-survival", "family-quest-guide", "family-combat", "family-prayer", "family-magic"];
  await mkdir(resolve(results, "source"), { recursive: true });
  for (const name of names) {
    await page.evaluate(name => window.sourceFixture(name), name);
    await page.screenshot({ path: resolve(results, `source/${name}.png`) });
  }
  const compositions = cases.length ? [] : ["registration", "registration-rejected", "connecting", "capability", "runtime-error", "unavailable", "branding"];
  for (const name of compositions) {
    await page.evaluate(name => window.ownerFixture(name), name);
    await page.screenshot({ path: resolve(results, `source/owner-${name}.png`) });
  }
  await writeFile(resolve(results, "source-captures.json"), JSON.stringify({
    scope: "Component source-widget replay on transparent world surface, NOT a game or full-frame scene comparison",
    browser: browser.version(), viewport: [1920, 1080], dpr: 1, uiScale: 1, names, compositions, errors,
    serverGameplay: false, finalAcceptance: false,
  }, null, 2) + "\n");
  if (errors.length) throw new Error(errors.join("\n"));
  console.log(`Captured ${names.length} source component cases; no browser exceptions.`);
} finally { await browser.close(); await host.close(); }
