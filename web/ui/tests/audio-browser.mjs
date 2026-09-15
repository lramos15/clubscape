import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { browserHost, launchBrowser, results } from "./browser-host.mjs";

const host = await browserHost({ audioAssets: true });
const browser = await launchBrowser({ muteAudio: true });
const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
const errors = [], cases = [];
page.on("pageerror", error => errors.push(error.message));
const frame = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
const click = async id => { await page.locator(`[data-ui-control="${id}"]`).click(); await frame(); };
const state = () => page.evaluate(() => window.audioComponent.state());
const mount = async (phase = "world") => {
  await page.evaluate(async phase => { const fixture = await import("/web/ui/tests/audio-fixture.ts"); await fixture.mountAudio(phase); }, phase);
  await frame();
};
const settings = async () => { await click("tab-11"); await click("settings-page-66"); };
async function check(name, run) {
  try { await run(); cases.push({ name, passed: true }); }
  catch (error) {
    const state = await page.evaluate(() => ({
      focus: document.activeElement?.outerHTML.slice(0, 280),
      status: document.querySelector('[role="status"]')?.textContent,
      calls: window.component?.services.calls.slice(-8),
      audio: window.audioComponent ? { volumes: window.audioComponent.state().volumes, disposed: window.audioComponent.state().disposed } : null,
    }));
    cases.push({ name, passed: false, error: error.message, state });
    error.message += "\n" + JSON.stringify(state); throw error;
  }
}
try {
  await page.goto(host.url);
  await mkdir(resolve(results, "audio-components"), { recursive: true });
  await check("real observer defaults and integer source position, not linear gain", async () => {
    await mount(); await settings();
    assert.deepEqual((await state()).nativeMixer, { music: 255, effects: 127, area: 127 });
    for (const label of ["Master Volume", "Music Volume", "Sound Effects Volume", "Area Sounds Volume"])
      assert.equal(await page.getByRole("slider", { name: label, exact: true }).getAttribute("aria-valuenow"), "100");
    const music = page.getByRole("slider", { name: "Music Volume", exact: true });
    await music.focus(); await music.press("Home"); await frame();
    for (let index = 0; index < 5; index++) { await music.press("PageUp"); await frame(); }
    assert.equal((await state()).volumes.music, 0.5);
    assert.equal((await state()).nativeMixer.music, 44);
    assert.deepEqual(await page.evaluate(() => window.component.services.calls.filter(call => call.method === "audioVolume").at(-1).args), ["music", 0.5]);
    assert.equal((await state()).unlocked, false);
    assert.equal(await page.evaluate(() => window.component.services.calls.filter(call => call.method === "unlockAudio").length), 0);
    assert.equal((await state()).voices.length, 0);
    await page.screenshot({ path: resolve(results, "audio-components/native-volume-50.png") });
    await page.evaluate(() => {
      const slider = document.querySelector('[data-ui-control="audio-slider-music"]');
      slider.focus();
      for (const key of ["Home", "PageUp", "PageUp", "PageUp"])
        slider.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true }));
    });
    await frame();
    assert.equal((await state()).volumes.music, 0.3);
  });
  await check("native master is pre-lookup and channel mute restores an observed value", async () => {
    await mount(); await settings();
    const master = page.getByRole("slider", { name: "Master Volume", exact: true });
    await master.focus(); await master.press("Home"); await frame();
    for (let index = 0; index < 5; index++) { await master.press("PageUp"); await frame(); }
    assert.equal((await state()).masterPercent, 50);
    assert.deepEqual((await state()).nativeMixer, { music: 44, effects: 22, area: 22 });
    await click("audio-mute-music"); assert.equal((await state()).volumes.music, 0);
    await click("audio-mute-music"); assert.equal((await state()).volumes.music, 1);
    assert.equal((await state()).nativeMixer.music, 44);
    await master.focus(); await master.press("Home"); await frame();
    assert.equal((await state()).volumes.effects, 1);
    assert.equal((await state()).nativeMixer.effects, 0);
    await page.screenshot({ path: resolve(results, "audio-components/native-master-zero.png") });
  });
  await check("drag, focus, Escape and external updates preserve source slider values", async () => {
    await mount(); await settings();
    const slider = page.getByRole("slider", { name: "Area Sounds Volume", exact: true });
    const box = await slider.boundingBox();
    await page.mouse.move(box.x + box.width - 8, box.y + 8); await page.mouse.down();
    await page.mouse.move(box.x + 8, box.y + 8, { steps: 3 }); await frame();
    assert.equal((await state()).volumes.area, 0);
    await page.keyboard.press("Escape");
    await page.mouse.move(box.x + box.width - 8, box.y + 8); await page.mouse.up(); await frame();
    assert.equal((await state()).volumes.area, 0);
    await page.evaluate(() => window.audioComponent.handle.volume("area", 0.25)); await frame();
    assert.equal(await slider.getAttribute("aria-valuenow"), "25");
    await slider.focus(); await slider.press("ArrowRight"); await frame();
    assert.equal((await state()).volumes.area, 0.26);
    for (const viewport of [{ width: 1024, height: 768 }, { width: 2560, height: 1440 }, { width: 1920, height: 1080 }]) {
      await page.setViewportSize(viewport);
      await page.evaluate(({ width, height }) => window.component.ui.resize(width, height), viewport); await frame();
      for (const label of ["Master Volume", "Music Volume", "Sound Effects Volume", "Area Sounds Volume"]) {
        const rect = await page.getByRole("slider", { name: label, exact: true }).boundingBox();
        assert.ok(rect.x >= 0 && rect.y >= 0 && rect.x + rect.width <= viewport.width && rect.y + rect.height <= viewport.height);
        assert.equal(rect.width, 112); assert.equal(rect.height, 16);
      }
    }
  });
  await check("actual audio failures surface exact codes and unbound/disposed controls cannot dispatch", async () => {
    await mount(); await settings();
    await page.evaluate(() => {
      window.audioComponent.restoreVolumeRoute = window.component.services.audioVolume;
      window.component.services.audioVolume = () => { throw Object.assign(new Error("The shell rejected the audio setting."), { errorId: "shell.audio.rejected" }); };
    });
    await page.getByRole("slider", { name: "Music Volume", exact: true }).focus();
    await page.getByRole("slider", { name: "Music Volume", exact: true }).press("Home"); await frame();
    assert.equal((await state()).volumes.music, 1);
    assert.match(await page.getByRole("status").innerText(), /shell.audio.rejected/);
    await click("notice-close");
    await page.evaluate(() => { window.component.services.audioVolume = window.audioComponent.restoreVolumeRoute; });
    await page.evaluate(() => window.audioComponent.handle.volume("music", NaN)); await frame();
    assert.match(await page.getByRole("status").innerText(), /AUDIO_VOLUME/);
    await click("notice-close");
    await page.evaluate(() => window.audioComponent.handle.volume("music", 0)); await frame();
    await click("audio-mute-music");
    assert.equal((await state()).volumes.music, 0);
    assert.match(await page.getByRole("status").innerText(), /ui.audio.remembered_mute/);
    await click("notice-close");
    await page.evaluate(() => window.audioComponent.stop()); await frame();
    const slider = page.getByRole("slider", { name: "Music Volume", exact: true });
    assert.equal(await slider.isDisabled(), true);
    assert.equal(await slider.getAttribute("aria-valuenow"), null);
    const before = (await state()).volumes.music;
    await slider.press("End"); await frame();
    assert.equal((await state()).volumes.music, before);
  });
  await check("native music mode IDs retain required authoritative selection feedback, without fabricated tracks", async () => {
    await mount(); await click("tab-13");
    assert.equal(await page.getByRole("button", { name: "Area Mode", exact: true }).getAttribute("data-ui-control"), "music-mode-0");
    assert.equal(await page.getByRole("button", { name: "Shuffle Mode", exact: true }).getAttribute("data-ui-control"), "music-mode-1");
    assert.equal(await page.getByRole("button", { name: "Single Mode", exact: true }).getAttribute("data-ui-control"), "music-mode-2");
    await click("music-mode-1");
    assert.match(await page.getByRole("status").innerText(), /authoritative_music_selection_and_unlocks/);
    assert.equal(await page.evaluate(() => window.component.services.intents.some(intent => intent.kind === "music")), false);
  });
  await check("title toggle uses real activation and global mute without rewriting source channel positions", async () => {
    await mount("title");
    await page.evaluate(() => { window.audioComponent.handle.volume("music", 0); window.audioComponent.handle.volume("effects", 0.45); });
    await click("title-audio");
    await page.waitForFunction(() => window.audioComponent.state().unlocked);
    assert.equal((await state()).volumes.effects, 0.45);
    assert.equal((await state()).volumes.music, 0);
    assert.equal((await state()).muted, false);
    await click("title-audio"); assert.equal((await state()).muted, true);
    assert.equal((await state()).volumes.effects, 0.45);
    await page.screenshot({ path: resolve(results, "audio-components/title-source-mute.png") });
  });
  await check("UI disposal detaches the observer but does not dispose the shell's audio graph", async () => {
    await page.evaluate(() => window.audioComponent.uiDispose()); await frame();
    await page.evaluate(() => window.audioComponent.handle.volume("area", 0.3));
    assert.equal((await state()).disposed, false);
    assert.equal(await page.locator("[data-clubscape-ui]").count(), 0);
  });
  await page.evaluate(() => window.audioComponent.dispose());
  const native = await page.evaluate(async () => {
    const catalogue = await (await fetch("/assets/ui/manifest.json")).json();
    return catalogue.nativeAudioControls;
  });
  for (const folder of ["audio-source", "audio-projections"]) await mkdir(resolve(results, folder), { recursive: true });
  for (const record of native) {
    await page.evaluate(name => window.sourceFixture(name), record.case);
    await page.screenshot({ path: resolve(results, "audio-source", record.case + ".png") });
    await page.evaluate(async percentages => { const fixture = await import("/web/ui/tests/audio-fixture.ts"); await fixture.audioProjection(percentages); }, record.percentages);
    await page.screenshot({ path: resolve(results, "audio-projections", record.case + ".png") });
  }
  assert.deepEqual(errors, []);
} finally {
  await writeFile(resolve(results, "audio-ui-tests.json"), JSON.stringify({
    scope: "Source audio UI/real WebAudio observer controls ONLY; browser output muted for shared-host privacy, not audible world/M1 acceptance",
    cases, errors, browser: browser.version(), browserOutputMuted: true, finalAcceptance: false,
  }, null, 2) + "\n");
  await page.evaluate(() => window.audioComponent?.dispose()).catch(error => errors.push(error.message));
  await browser.close(); await host.close();
  console.log(`${cases.filter(row => row.passed).length}/${cases.length} source audio UI cases passed.`);
}
