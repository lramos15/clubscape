import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { sourceAudioPreferenceDefaults, parseSourceAudioPreferences, serializeSourceAudioPreferences } from "../../audio/preferences.ts";
import { browserHost, launchBrowser, results } from "./browser-host.mjs";

const host = await browserHost({ audioAssets: true }), browser = await launchBrowser({ muteAudio: true });
const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
const cases = [], errors = [];
page.on("pageerror", error => errors.push(error.message));
const frame = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
const click = async id => { await page.locator(`[data-ui-control="${id}"]`).click(); await frame(); };
const binding = () => page.evaluate(() => window.audioComponent.preferences());
const flush = () => page.evaluate(() => window.audioComponent.preferenceStore.flush(window.preferenceFixture.player));
const saveCalls = () => page.evaluate(() => window.preferenceFixture.state.calls.filter(call => call.kind === "save").length);
const acknowledgeSettledAudio = async () => {
  await page.waitForFunction(() => {
    const state = window.audioComponent.state();
    return state.queueSize === 0 && state.cache.pending === 0;
  });
  await frame();
  if (await page.locator('[data-ui-control="notice-close"]').count()) await click("notice-close");
};
const saved = () => page.evaluate(() => JSON.parse(window.preferenceFixture.state.records.get(window.preferenceFixture.player)));
function record(change = {}) {
  const value = structuredClone(sourceAudioPreferenceDefaults());
  Object.assign(value.music, { rememberModeOnLogin: true }, change);
  return parseSourceAudioPreferences(value);
}
const mount = async value => {
  await page.evaluate(async stored => {
    const fixture = await import("/web/ui/tests/preference-fixture.ts"); await fixture.mountPreferenceAudio(stored);
    const audio = window.audioComponent.handle, original = audio.unlock.bind(audio);
    const gesture = { calls: [], hold: null };
    audio.unlock = () => {
      gesture.calls.push({ trustedStack: Boolean(window.event?.isTrusted), active: navigator.userActivation.isActive });
      const actual = original();
      return gesture.hold ? Promise.all([actual, gesture.hold]).then(() => {}) : actual;
    };
    window.preferenceGestures = gesture;
  }, value === null ? null : serializeSourceAudioPreferences(value));
  await frame();
};
const musicTab = async () => { await click("tab-13"); };
const audioTab = async () => { await click("tab-11"); await click("settings-page-66"); };
const settingsSearch = async text => {
  await click("tab-11"); await click("settings-32--1");
  if (await page.locator('[data-ui-control="settings-search-start"]').count()) await click("settings-search-start");
  await page.getByRole("textbox", { name: "Search settings", exact: true }).fill(text); await frame();
};
const setting = id => page.locator(`[data-ui-control^="setting-${id}-"]`).first();
const closeSettings = async () => { await page.getByRole("button", { name: "Close interface", exact: true }).click(); await frame(); };
const capture = async name => {
  await page.mouse.move(600, 100); await page.waitForLoadState("networkidle"); await frame();
  await page.screenshot({ path: resolve(results, "preference-components", name + ".png") });
};
async function check(name, operation) {
  try {
    await operation();
    const audioQualifications = await page.evaluate(() => window.preferenceFixture?.ready
      ? window.audioComponent.state().traces.filter(trace => trace.type === "error").map(trace => ({ code: trace.data.code, message: trace.data.message }))
      : []);
    cases.push({ name, passed: true, audioQualifications });
  }
  catch (error) {
    cases.push({ name, passed: false, error: error.message, status: await page.getByRole("status").innerText() });
    throw error;
  }
}
try {
  await page.goto(host.url); await mkdir(resolve(results, "preference-components"), { recursive: true });
  await check("confirmed record absence is distinct from failed/corrupt reads and is resolved before the first audio world binding", async () => {
    await mount(null);
    assert.equal((await binding()).preferences.version, 1);
    assert.equal((await binding()).preferences.music.savedPlaylist1.length, 100);
    assert.equal(await page.evaluate(() => window.preferenceFixture.state.calls[0].kind), "load");
    await flush();
    assert.equal(await page.evaluate(() => window.preferenceFixture.state.calls.filter(call => call.kind === "save").length), 1);
    assert.deepEqual(await saved(), (await binding()).preferences, "The actual manager persists its applied native record, not a UI default.");
    const failed = await page.evaluate(async () => {
      const fixture = await import("/web/ui/tests/preference-fixture.ts");
      try { await fixture.mountPreferenceAudio(null, true); return null; }
      catch (error) { return { id: error.errorId, ready: window.preferenceFixture.ready }; }
    });
    assert.deepEqual(failed, { id: "preference.read.actual", ready: false });
    const corrupt = await page.evaluate(async () => {
      const fixture = await import("/web/ui/tests/preference-fixture.ts");
      try { await fixture.mountPreferenceAudio("{invalid"); return null; }
      catch (error) { return { id: error.errorId, ready: window.preferenceFixture.ready }; }
    });
    assert.deepEqual(corrupt, { id: "audio.preferences.invalid_record", ready: false });
  });

  await check("native first-use Unmute restores100/20/45/25 with real-gesture unlock and source mixer9/18/8", async () => {
    const value = structuredClone(record());
    value.volumes.current = { master: 0, music: 0, effects: 0, area: 0 };
    await mount(parseSourceAudioPreferences(value)); await audioTab();
    for (const channel of ["master", "music", "effects", "area"]) await click(`audio-mute-${channel}`);
    await flush();
    assert.deepEqual((await binding()).preferences.volumes.current, { master: 100, music: 20, effects: 45, area: 25 });
    assert.deepEqual((await binding()).preferences.volumes.remembered, { master: 100, music: 20, effects: 45, area: 25 });
    assert.deepEqual(await page.evaluate(() => window.audioComponent.state().nativeMixer), { music: 9, effects: 18, area: 8 });
    const gestures = await page.evaluate(() => window.preferenceGestures.calls);
    assert.equal(gestures.length, 4); assert.ok(gestures.every(call => call.trustedStack && call.active), JSON.stringify(gestures));
    assert.deepEqual((await saved()).volumes.current, { master: 100, music: 20, effects: 45, area: 25 });
    await capture("native-unmute-fallbacks");
  });

  await check("numbered playlists retain exact100 slots and holes through add/remove, and empty selection stays genuinely empty", async () => {
    const one = Array(100).fill(null), two = Array(100).fill(null);
    one[1] = 64; one[99] = 327; two[0] = 76;
    await mount(record({ mode: "shuffle", currentPlaylist: 1, savedPlaylist1: one, savedPlaylist2: two }));
    await musicTab();
    assert.equal((await binding()).preferences.music.currentPlaylist, 1);
    await click("music-list-filter"); await click("music-filter-2");
    assert.equal((await binding()).preferences.music.currentPlaylist, 2);
    await page.locator('[data-ui-control="music-track-76"]').click({ button: "right" }); await frame();
    await page.getByRole("button", { name: "Add to playlist 1", exact: true }).click(); await frame();
    assert.equal((await binding()).preferences.music.savedPlaylist1[0], 76);
    assert.equal((await binding()).preferences.music.savedPlaylist1[1], 64);
    assert.equal((await binding()).preferences.music.savedPlaylist1[99], 327);
    await page.locator('[data-ui-control="music-track-76"]').click({ button: "right" }); await frame();
    await page.getByRole("button", { name: "Remove from playlist 1", exact: true }).click(); await frame();
    await flush();
    const slots = (await saved()).music.savedPlaylist1;
    assert.equal(slots.length, 100); assert.equal(slots[0], null); assert.equal(slots[1], 64); assert.equal(slots[99], 327);
    await click("music-list-filter"); await click("music-filter-3");
    assert.equal((await binding()).preferences.music.currentPlaylist, 3);
    assert.deepEqual((await binding()).musicState.playlistGroups, []);
    assert.equal(await page.evaluate(() => window.audioComponent.state().traces.some(trace =>
      trace.type === "error" && trace.data.code === "AUDIO_PLAYLIST_EMPTY")), true);
    await page.waitForFunction(() => {
      const latest = window.audioComponent.state().traces.findLast(trace => trace.type === "error");
      return document.querySelector('[role="status"]').textContent.includes(latest.data.code);
    });
    await click("notice-close");
    assert.equal(await page.locator('[data-ui-control^="music-track-"]').count(), 0);
    await capture("native-empty-playlist");
  });

  await check("All Settings uses native inverted login/playlist flags and explicit saved-slot clearing without rewriting another slot", async () => {
    const one = Array(100).fill(null), two = Array(100).fill(null); one[99] = 64; two[7] = 76;
    await mount(record({ savedPlaylist1: one, savedPlaylist2: two }));
    await settingsSearch("Default to area music mode on login"); await setting(6395).click(); await frame();
    assert.equal((await binding()).preferences.music.rememberModeOnLogin, false);
    await closeSettings(); await settingsSearch("Update music when changing playlists"); await setting(6397).click(); await frame();
    assert.equal((await binding()).preferences.music.keepPlayingOnPlaylistChange, true);
    await closeSettings(); await settingsSearch("Wipe custom music playlist 1");
    await setting(6405).click(); await frame(); await flush();
    assert.equal((await saved()).music.savedPlaylist1.length, 100);
    assert.ok((await saved()).music.savedPlaylist1.every(value => value === null));
    assert.equal((await saved()).music.savedPlaylist2[7], 76);
    await closeSettings(); await settingsSearch("Music playlist");
    await setting(6394).click(); await frame();
    await page.locator("canvas").press("Escape"); await frame();
    assert.equal((await binding()).preferences.music.currentPlaylist, 0);
    await capture("native-preference-settings");
  });

  await check("native repeat is not legacy Single-loop cancellation and different legacy state is rejected after preference binding", async () => {
    await mount(record({ mode: "single", selectedGroup: 64 })); await musicTab();
    assert.equal((await binding()).musicState.loopEnabled, true);
    await page.locator('[data-ui-control="music-mode-2"]').click({ button: "right" }); await frame();
    await page.getByRole("button", { name: "Allow repeated selections outside Single", exact: true }).click(); await frame();
    assert.equal((await binding()).preferences.music.repeatInAreaShuffle, true);
    await page.locator('[data-ui-control="music-mode-2"]').click({ button: "right" }); await frame();
    await page.getByRole("button", { name: "Prevent repeated selections outside Single", exact: true }).click(); await frame();
    assert.equal((await binding()).preferences.music.repeatInAreaShuffle, false);
    assert.equal((await binding()).musicState.loopEnabled, true);
    const rejected = await page.evaluate(() => {
      const state = window.audioComponent.musicState();
      try { window.audioComponent.music({ ...state, loopEnabled: false }); return null; }
      catch (error) { return error.code; }
    });
    assert.equal(rejected, "AUDIO_PREFERENCE_BINDING");
    await click("notice-close");
    assert.equal((await binding()).preferences.music.mode, "single");
    assert.equal(await page.locator('[data-ui-control="music-skip"]').isDisabled(), true);
  });

  await check("direct Skip uses the native command without mode flips or playhead persistence and reports muted requests explicitly", async () => {
    await mount(record({ mode: "shuffle" })); await musicTab();
    await flush();
    const beforeSaves = await saveCalls();
    await click("music-skip");
    await page.waitForFunction(() => window.audioComponent.state().traces.some(trace => trace.type === "source_control_click" && trace.data.binding === "skip/9292"));
    assert.equal((await binding()).preferences.music.mode, "shuffle");
    assert.equal(await saveCalls(), beforeSaves);
    assert.ok((await page.evaluate(() => window.preferenceGestures.calls)).every(call => call.trustedStack));
    await acknowledgeSettledAudio();
    await audioTab();
    const slider = page.getByRole("slider", { name: "Music Volume", exact: true });
    await slider.focus(); await slider.press("Home"); await frame(); await flush();
    await musicTab(); await click("music-skip");
    assert.equal(await page.evaluate(async () => {
      const { getUiMusicSkipResult } = await import("/web/ui/index.ts");
      return getUiMusicSkipResult(window.component.ui).status;
    }), "muted");
    await acknowledgeSettledAudio();
  });

  await check("rapid sliders coalesce per character and repeated in-flight values still become the latest real preference", async () => {
    await mount(record()); await audioTab();
    await flush();
    const beforeSaves = await saveCalls();
    await page.evaluate(() => { window.preferenceFixture.state.holdSaves = true; });
    const slider = page.getByRole("slider", { name: "Music Volume", exact: true });
    await slider.focus(); await slider.press("Home"); await frame();
    await slider.press("PageUp"); await frame(); await slider.press("PageUp"); await frame(); await slider.press("Home"); await frame();
    assert.equal((await binding()).preferences.volumes.current.music, 0);
    assert.equal(await page.evaluate(() => window.preferenceFixture.state.pending.length), 1);
    await page.evaluate(() => window.preferenceFixture.state.pending.shift().resolve());
    await page.waitForFunction(() => window.preferenceFixture.state.pending.length === 1);
    await page.evaluate(() => { window.preferenceFixture.state.holdSaves = false; window.preferenceFixture.state.pending.shift().resolve(); });
    await flush();
    assert.equal((await saved()).volumes.current.music, 0);
    assert.equal(await saveCalls(), beforeSaves + 2);
  });

  await check("actual persistence rejection keeps current values and dirty state with a working native retry control", async () => {
    await mount(record()); await audioTab();
    await page.evaluate(() => { window.preferenceFixture.state.failNextSave = true; });
    const slider = page.getByRole("slider", { name: "Music Volume", exact: true });
    await slider.focus(); await slider.press("Home"); await frame();
    assert.equal((await binding()).preferences.volumes.current.music, 0);
    assert.match(await page.getByRole("status").innerText(), /preference.storage.actual/);
    assert.equal(await page.evaluate(() => window.audioComponent.preferenceManager.observe().save), "failed");
    await page.getByRole("button", { name: "Retry saving audio preferences", exact: true }).click(); await frame(); await flush();
    assert.equal((await saved()).volumes.current.music, 0);
    assert.equal(await page.evaluate(() => window.audioComponent.preferenceManager.observe().save), "stored");
  });

  await check("pending Skip cannot execute after logout/re-entry with the same character identity", async () => {
    await mount(record({ mode: "shuffle" })); await musicTab();
    await page.evaluate(() => {
      window.preferenceGestures.hold = new Promise(resolve => { window.releaseOldPreferenceGesture = resolve; });
    });
    await click("music-skip");
    await page.evaluate(async () => {
      const s = window.component.services, world = structuredClone(s.state().world), audio = window.audioComponent.handle;
      s.publish({ ...s.state(), phase: "title", world: null }); audio.update(null, []);
      s.publish({ ...s.state(), phase: "world", world });
      await window.audioComponent.enter(world);
    }); await frame();
    const before = await binding();
    await page.evaluate(() => { window.preferenceGestures.hold = null; window.releaseOldPreferenceGesture(); });
    await frame(); await frame();
    assert.deepEqual(await binding(), before);
    assert.equal(await page.evaluate(() => window.component.services.errors.some(error =>
      error.errorId === "AUDIO_CONTROL_SUPERSEDED" || error.errorId === "audio.preferences.entry_superseded")), true);
  });
  assert.deepEqual(errors, []);
} finally {
  await writeFile(resolve(results, "preference-component-tests.json"), JSON.stringify({
    scope: "Real WebAudio/native preference UI controls through the actual shell preference manager/store with deterministic storage callbacks; no live account, world-scene or speaker/M1 acceptance.",
    browser: browser.version(), browserOutputMuted: true, cases, errors, finalAcceptance: false,
  }, null, 2) + "\n");
  await page.evaluate(() => window.audioComponent?.dispose()).catch(error => errors.push(error.message));
  await browser.close(); await host.close();
  console.log(`${cases.filter(row => row.passed).length}/${cases.length} preference component cases passed.`);
}
