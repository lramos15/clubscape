import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { browserHost, launchBrowser, results } from "./browser-host.mjs";

const host = await browserHost(), browser = await launchBrowser(), errors = [];
const names = ["ui4-book-first", "ui4-book-last", "ui4-map-tutors-hidden", "ui4-map-tutors-shown"];
try {
  const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
  page.on("pageerror", error => errors.push(error.message));
  await page.goto(host.url); await page.waitForFunction(() => typeof window.sourceFixture === "function");
  for (const folder of ["documents", "document-projections"]) await mkdir(resolve(results, folder), { recursive: true });
  for (const name of names) {
    await page.evaluate(name => window.sourceFixture(name), name);
    await page.screenshot({ path: resolve(results, "documents", name + ".png") });
    await page.evaluate(async name => {
      const { documentProjectionFixture } = await import("/web/ui/tests/document-fixture.ts");
      await documentProjectionFixture(name);
    }, name);
    await page.screenshot({ path: resolve(results, "document-projections", name + ".png") });
  }
  assert.deepEqual(errors, []);
} finally {
  await writeFile(resolve(results, "documents-browser.json"), JSON.stringify({
    scope: "Original document frame/text/tutor-control fixtures only. Native map captures have a genuinely hidden marker; visible-marker placement/animation are not certified by these images.",
    names, errors, finalAcceptance: false,
  }, null, 2) + "\n");
  await browser.close(); await host.close();
}
