import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { browserHost, launchBrowser, results } from "./browser-host.mjs";

const host = await browserHost({ audioAssets: true }), browser = await launchBrowser({ muteAudio: true });
const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
const cases = [], errors = [];
page.on("pageerror", error => errors.push(error.message));
const frame = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
const click = async id => { await page.locator(`[data-ui-control="${id}"]`).click(); await frame(); };
const declared = () => ({ mode: "area", areaMode: "modern", unlockedGroups: [2, 64, 76, 145, 163, 327],
  selectedGroup: null, playlistGroups: [64, 327], loopEnabled: true });
const music = () => page.evaluate(() => window.audioComponent.musicState());
const graph = () => page.evaluate(() => window.audioComponent.state());
const supply = async state => { await page.evaluate(state => window.audioComponent.music(state), state); await frame(); };
const mount = async () => {
  await page.evaluate(async () => { const fixture = await import("/web/ui/tests/audio-fixture.ts"); await fixture.mountAudio("world"); });
  await frame(); await supply(declared()); await click("tab-13");
};
const reveal = async group => {
  const position = await page.evaluate(async group => {
    const { decodeUiCatalogue } = await import("/web/ui/assets.ts");
    const c = decodeUiCatalogue(await (await fetch("/assets/ui/manifest.json")).json());
    const track = c.musicTracks.find(track => track.group === group);
    const row = c.templates["native-music"].find(row => row.id === 239 * 65536 + 11 && row.index === track.widgetIndex);
    const root = c.templates["native-music"].find(row => row.id === 239 * 65536 + 11 && row.index === -1);
    return row.y - root.y;
  }, group);
  const control = page.getByRole("slider", { name: "Music list scroll", exact: true });
  const box = await control.boundingBox(), maximum = Number(await control.getAttribute("aria-valuemax"));
  const desired = Math.max(0, Math.min(maximum, position - 60));
  await page.mouse.click(box.x + 8, box.y + 16 + Math.round(desired * (box.height - 42) / maximum) + 5);
  await frame();
  assert.ok(await page.locator(`[data-ui-control="music-track-${group}"]`).count(), `Source track ${group} must be reachable using the native scrollbar`);
};
async function check(name, run) {
  try { await run(); cases.push({ name, passed: true }); }
  catch (error) {
    const detail = await page.evaluate(() => ({ status: document.querySelector('[role="status"]')?.textContent,
      music: window.audioComponent?.musicState(), traces: window.audioComponent?.state().traces.slice(-4) }));
    cases.push({ name, passed: false, error: error.message, detail }); error.message += "\n" + JSON.stringify(detail); throw error;
  }
}
try {
  await page.goto(host.url); await mkdir(resolve(results, "music-components"), { recursive: true });
  await check("four metadata routes and all nine exact additive asset identities resolve without aliasing base files", async () => {
    const manifestId = "assets/manifests/osrs/audio-m1-supplement.json";
    const response = await page.request.get(host.url + "/audio-asset/" + encodeURIComponent(manifestId));
    const bytes = await response.body();
    assert.equal(createHash("sha256").update(bytes).digest("hex"), "840aef91bac9a1fd042bdb1c3662ff92a378e279f48335108730e168af550d91");
    const manifest = JSON.parse(bytes.toString());
    assert.equal(manifest.assets.length, 9);
    for (const asset of manifest.assets) {
      const response = await page.request.get(host.url + "/audio-asset/" + encodeURIComponent(asset.asset_id));
      assert.equal(response.status(), 200);
      assert.equal(createHash("sha256").update(await response.body()).digest("hex"), asset.sha256);
    }
    assert.notEqual(manifest.assets.find(a => a.kind === "music" && a.source_group === 64).asset_id,
      manifest.assets.find(a => a.kind === "jingle" && a.source_group === 64).asset_id);
  });
  await check("declared native modes and manual source selection use the actual audio engine, not fabricated music events", async () => {
    await mount();
    await supply({ ...declared(), unlockedGroups: [64, 327], playlistGroups: [64] });
    await click("music-mode-2");
    assert.equal((await music()).selectedGroup, 64);
    await supply(declared());
    await click("music-mode-2");
    assert.equal((await music()).mode, "single");
    assert.equal((await music()).selectedGroup, 76);
    await click("music-mode-1");
    assert.equal((await music()).mode, "shuffle");
    assert.deepEqual((await music()).unlockedGroups, declared().unlockedGroups);
    await click("music-mode-0");
    assert.equal((await music()).mode, "area");
    await reveal(64); await click("music-track-64");
    await page.waitForFunction(() => window.audioComponent.state().voices.some(voice =>
      voice.kind === "music" && voice.sourceId === 64 && voice.renderedNativeLevel === 255), null, { timeout: 20000 });
    const voice = (await graph()).voices.find(voice => voice.kind === "music" && voice.sourceId === 64);
    assert.equal(voice.assetId, "asset.source.osrs.cache2695.audio-supplement.music.64.native255");
    assert.ok(voice.gain <= 1);
    assert.equal((await music()).selectedGroup, 64);
    await supply(await music());
    assert.equal((await graph()).voices.find(current => current.kind === "music" && current.sourceId === 64).id, voice.id);
    assert.equal(await page.evaluate(() => window.component.services.intents.some(intent => intent.kind === "music")), false);
    await page.screenshot({ path: resolve(results, "music-components/native255-book-of-spells.png") });
  });
  await check("held track actions recheck current unlock identity and never silently retarget", async () => {
    await mount(); await reveal(64);
    await page.locator('[data-ui-control="music-track-64"]').click({ button: "right" }); await frame();
    await supply({ ...declared(), unlockedGroups: [76], playlistGroups: [] });
    await page.getByRole("button", { name: "Play Book of Spells", exact: true }).click(); await frame();
    assert.match(await page.getByRole("status").innerText(), /no longer source-unlocked/);
    assert.equal((await music()).mode, "area"); assert.equal((await music()).selectedGroup, null);
    await click("notice-close");
    assert.equal(await page.locator('[data-ui-control="music-track-64"]').isDisabled(), true);
    await page.evaluate(async () => {
      const { onUiMusicStateChange } = await import("/web/ui/index.ts");
      onUiMusicStateChange(window.component.ui, async () => {
        throw Object.assign(new Error("The preference save was rejected."), { errorId: "music.preference.save" });
      });
    });
    await page.locator('[data-ui-control="music-mode-2"]').click({ button: "right" }); await frame();
    await page.getByRole("button", { name: "Disable looping", exact: true }).click(); await frame();
    assert.match(await page.getByRole("status").innerText(), /music.preference.save/);
    assert.equal((await music()).loopEnabled, false);
  });
  await check("current playlist and loop controls retain supplied groups and report missing numbered-slot bindings explicitly", async () => {
    await mount(); await reveal(64);
    await page.locator('[data-ui-control="music-track-64"]').click({ button: "right" }); await frame();
    await page.getByRole("button", { name: "Remove from current playlist", exact: true }).click(); await frame();
    assert.deepEqual((await music()).playlistGroups, [327]);
    await page.locator('[data-ui-control="music-mode-1"]').click({ button: "right" }); await frame();
    await page.getByRole("button", { name: "Play current playlist", exact: true }).click(); await frame();
    assert.equal((await music()).mode, "playlist");
    await page.locator('[data-ui-control="music-mode-2"]').click({ button: "right" }); await frame();
    await page.getByRole("button", { name: "Disable looping", exact: true }).click(); await frame();
    assert.equal((await music()).loopEnabled, false);
    await click("music-list-filter"); await click("music-filter-2");
    assert.match(await page.getByRole("status").innerText(), /numbered_music_playlists/);
    assert.deepEqual((await music()).playlistGroups, [327]);
    await click("notice-close"); await page.locator("canvas").press("Escape"); await frame();
    await page.screenshot({ path: resolve(results, "music-components/current-playlist.png") });
  });
  await check("player-scoped state rejects foreign updates and clears on character change", async () => {
    await mount();
    const error = await page.evaluate(() => {
      try { window.audioComponent.music(window.audioComponent.musicState(), "different.player"); return null; }
      catch (error) { return error.errorId; }
    });
    assert.equal(error, "ui.music.owner"); assert.deepEqual((await music()).unlockedGroups, declared().unlockedGroups);
    await click("notice-close");
    await page.evaluate(() => {
      const services = window.component.services, world = structuredClone(services.state().world);
      world.player.id = "another.player"; services.patchWorld(world);
      window.audioComponent.handle.update(services.state().world, []);
    }); await frame();
    assert.equal(await music(), null);
    await click("tab-13"); await click("music-mode-1");
    assert.match(await page.getByRole("status").innerText(), /ui.music.binding/);
  });
  await check("native scrollbar, dropdown cancellation and skip placement remain real controls", async () => {
    await mount();
    const scroll = page.getByRole("slider", { name: "Music list scroll", exact: true });
    await scroll.focus(); await scroll.press("End"); await frame();
    assert.equal(await scroll.getAttribute("aria-valuenow"), await scroll.getAttribute("aria-valuemax"));
    await scroll.press("Home"); await frame();
    assert.equal(await scroll.getAttribute("aria-valuenow"), "0");
    await scroll.press("ArrowDown"); await frame();
    assert.equal(await scroll.getAttribute("aria-valuenow"), "4");
    assert.equal(await page.locator('[data-ui-control="music-skip"]').isDisabled(), true);
    await click("music-mode-1"); await click("music-skip");
    assert.match(await page.getByRole("status").innerText(), /native_music_skip_request/);
    await click("notice-close"); await click("music-list-filter");
    await page.locator("canvas").press("Escape"); await frame();
    assert.equal(await page.locator('[data-ui-control="music-filter-0"]').count(), 0);
  });
  await page.evaluate(() => window.audioComponent.uiDispose());
  await page.evaluate(() => window.audioComponent.dispose());
  const names = ["native-music-mode-0", "native-music-mode-1", "native-music-mode-2", "native-music-filter-open"];
  for (const folder of ["music-source", "music-projections"]) await mkdir(resolve(results, folder), { recursive: true });
  for (const name of names) {
    await page.evaluate(name => window.sourceFixture(name), name);
    await page.screenshot({ path: resolve(results, "music-source", name + ".png") });
    await page.evaluate(async name => { const fixture = await import("/web/ui/tests/audio-fixture.ts"); await fixture.musicProjection(name); }, name);
    await page.screenshot({ path: resolve(results, "music-projections", name + ".png") });
  }
  assert.deepEqual(errors, []);
} finally {
  await writeFile(resolve(results, "music-ui-tests.json"), JSON.stringify({
    scope: "Real source music control/adapter component ONLY; supplied test unlocks, browser output muted, not live-server or M1 acceptance",
    cases, errors, browser: browser.version(), browserOutputMuted: true, finalAcceptance: false,
  }, null, 2) + "\n");
  await page.evaluate(() => window.audioComponent?.dispose()).catch(error => errors.push(error.message));
  await browser.close(); await host.close();
  console.log(`${cases.filter(row => row.passed).length}/${cases.length} source music UI cases passed.`);
}
