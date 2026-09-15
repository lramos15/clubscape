import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { browserHost, launchBrowser, results } from "./browser-host.mjs";

const host = await browserHost(), browser = await launchBrowser();
try {
  const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  await page.goto(host.url);
  await page.waitForFunction(() => typeof window.flameFixture === "function");
  await mkdir(resolve(results, "flames"), { recursive: true });
  for (const cycle of [1, 2, 3, 10, 40, 128, 257, 320, 640, 1024, 2048]) {
    await page.evaluate(cycle => window.flameFixture(cycle), cycle);
    await page.locator("canvas").screenshot({ path: resolve(results, `flames/cycle-${cycle}.png`) });
  }
  await page.evaluate(() => window.reconnectFixture());
  await page.locator("canvas").screenshot({ path: resolve(results, "flames/reconnect.png") });
  const names = await page.evaluate(async () => {
    const catalogue = await (await fetch("/assets/ui/manifest.json")).json();
    return Object.keys(catalogue.templates).filter(name => /-mask-|^native-retrieval-|filters$/.test(name));
  });
  await mkdir(resolve(results, "modes"), { recursive: true });
  for (const name of names) {
    await page.evaluate(name => window.sourceFixture(name), name);
    await page.screenshot({ path: resolve(results, `modes/${name}.png`) });
  }
  await mkdir(resolve(results, "projections"), { recursive: true });
  for (const kind of ["prayer", "magic"]) {
    const masks = kind === "prayer" ? [0, 1, 2, 4, 8, 16, 31] : [0, 1, 2, 4, 8, 16, 32, 64, 72, 88, 127];
    for (const mask of masks) for (const open of [false, true]) {
      await page.evaluate(({ kind, mask, open }) => window.filterProjection(kind, mask, open), { kind, mask, open });
      await page.screenshot({ path: resolve(results, `projections/native-${kind}-mask-${mask}${open ? "-filters" : ""}.png`) });
    }
  }
  for (const name of names.filter(name => name.startsWith("native-retrieval-"))) {
    await page.evaluate(name => window.recoveryProjection(name), name);
    await page.screenshot({ path: resolve(results, `projections/${name}.png`) });
  }
  await writeFile(resolve(results, "mode-browser.json"), JSON.stringify({
    scope: "Native UI effect/component validation only; not gameplay or completed startup acceptance",
    browser: browser.version(), errors, names, dpr: 1, finalAcceptance: false,
  }, null, 2) + "\n");
  if (errors.length) throw new Error(errors.join("\n"));
} finally { await browser.close(); await host.close(); }
