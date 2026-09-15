import assert from "node:assert/strict";

export async function preferenceChecks({ page, check, snapshot, waitVoice, emit, faults, manifest, native, quick }) {
  const unlocked = [2, 62, 64, 76, 144, 145, 163, 327];
  const slots = (...groups) => [...groups, ...Array(100 - groups.length).fill(null)];
  const read = () => page.evaluate(() => audioFixture.controls.read());
  const starts = () => page.evaluate(() => audioFixture.native.starts.length);
  const press = async (id, keyboard = false) => {
    if (keyboard) {
      await page.locator(id).focus();
      await page.keyboard.press("Enter");
    } else await page.click(id);
    const result = await page.evaluate(() => audioFixture.lastControl);
    assert.equal(result.success, true, JSON.stringify(result));
    return result.result;
  };
  const music = async (patch) => page.evaluate((patch) =>
    audioFixture.controls.music({ ...audioFixture.controls.read().preferences.music, ...patch }), patch);
  const oneMusic = async (groups = null) => {
    await page.waitForFunction((groups) => {
      const state = audioFixture.snapshot(), voices = state.voices.filter((voice) => voice.kind === "music");
      return voices.length === 1 && voices[0].when <= state.currentTime &&
        (!groups || groups.includes(voices[0].sourceId));
    }, groups);
    return (await snapshot()).voices.find((voice) => voice.kind === "music");
  };
  const setPercentages = async (current) => page.evaluate((current) => {
    const binding = audioFixture.controls.read();
    return audioFixture.controls.apply({
      ...binding.preferences, volumes: { ...binding.preferences.volumes, current },
    }, binding.unlockedGroups);
  }, current);

  await check("client preferences require the real character and detach all three slots without autoplay or persistence", async () => {
    await page.evaluate(async () => {
      if (audioFixture.context.state === "running") await audioFixture.context.suspend();
      audioFixture.setSourceMusicSelector(null, "modern");
      const world = audioFixture.syntheticWorld("region.osrs.12850");
      world.player.id = "player.audio-preferences";
      audioFixture.update(world);
    });
    const before = await starts();
    const state = await page.evaluate((unlocked) => {
      const input = structuredClone(audioFixture.controls.defaults());
      input.music.mode = "shuffle";
      input.music.rememberModeOnLogin = true;
      input.music.selectedGroup = 62;
      input.volumes.current = { master: 100, music: 50, effects: 45, area: 25 };
      const first = audioFixture.controls.apply(input, unlocked);
      input.volumes.current.music = 99;
      input.music.savedPlaylist1[0] = 76;
      for (let i = 0; i < 20; i++) audioFixture.controls.apply(
        structuredClone(first.preferences), i % 2 ? [...unlocked].reverse() : unlocked,
      );
      let wrongCharacter;
      try { audioFixture.controls.apply(first.preferences, unlocked, "player.not-active"); }
      catch (error) { wrongCharacter = error.code; }
      return {
        first, after: audioFixture.controls.read(), wrongCharacter,
        localStorageKeys: Object.keys(localStorage), state: audioFixture.snapshot().contextState,
      };
    }, unlocked);
    assert.equal(state.wrongCharacter, "AUDIO_PREFERENCE_CHARACTER");
    assert.equal(state.state, "suspended");
    assert.deepEqual(state.after, state.first);
    assert.deepEqual(state.localStorageKeys, []);
    assert.equal(state.after.preferences.volumes.current.music, 50);
    assert.equal(state.after.preferences.music.savedPlaylist1[0], null);
    assert.equal((await snapshot()).pendingGesture, true);
    assert.equal(await starts(), before);
    return { version: 1, nativeSavedSlots: [1, 2, 3], slotsPerPlaylist: 100,
      detached: true, idempotentReapplications: 20, autoplaySources: 0, audioOwnedPersistenceKeys: 0 };
  });

  await check("direct Skip unlocks from a real keyboard gesture and obeys source Area/Single eligibility", async () => {
    await page.waitForFunction(() => !navigator.userActivation.isActive);
    const before = await starts();
    const rejected = await page.evaluate(async () => {
      try { await audioFixture.controls.skip(); return "unexpected-success"; }
      catch (error) { return error.code; }
    });
    assert.equal(rejected, "AUDIO_GESTURE_REQUIRED");
    assert.equal(await starts(), before);
    for (const group of [2, 64, 76, 144, 145, 163, 327]) {
      const id = group === 2 || group === 76 || group === 144
        ? `asset.source.osrs.cache2695.audio-runtime.music.${group}`
        : `asset.source.osrs.cache2695.audio-supplement.music.${group}.native255`;
      faults.set(id, { kind: "delay", ms: 180 });
    }
    const requested = await press("#skip", true);
    assert.equal(requested.status, "requested");
    assert.equal(requested.previousGroup, 62);
    assert.ok(unlocked.includes(requested.nextGroup) && requested.nextGroup !== 62);
    const voice = await oneMusic([requested.nextGroup]);
    const start = (await snapshot()).traces.find((entry) => entry.type === "music_started" && entry.data.voiceId === voice.id);
    const delayErrorMs = Math.abs(voice.when - start.audioTime - 60 * 0.02) * 1000;
    assert.ok(delayErrorMs <= 20, String(delayErrorMs));
    assert.equal((await snapshot()).contextState, "running");
    assert.equal((await snapshot()).pendingGesture, false);
    assert.equal(await page.evaluate(() => audioFixture.originalsConnected()), true);
    const monitor = await page.evaluate(() => audioFixture.stats());
    assert.ok(monitor.nonzero > 0);
    const disabled = [];
    for (const mode of ["area", "single"]) {
      await music({ mode, selectedGroup: mode === "single" ? 62 : null });
      const current = await oneMusic(mode === "single" ? [62] : [76]);
      const count = await starts();
      assert.equal((await press("#skip")).status, "disabled_mode");
      assert.equal(await starts(), count);
      assert.equal((await snapshot()).voices.find((entry) => entry.kind === "music").id, current.id);
      disabled.push(mode);
    }
    const gesture = await page.evaluate(() => audioFixture.native.gestures.findLast((entry) => entry.control === "skip" && entry.trusted));
    assert.equal(gesture.active, true);
    return { originalCallback: 9292, originalClick: 2266, rejectedSyntheticUnlock: rejected,
      trustedGesture: gesture, requested, actualStartedGroup: voice.sourceId, actualStart: voice.when,
      nativeTransitionCycles: [0, 60, 60, 0], delayErrorMs, disabledModes: disabled };
  });

  await check("three native saved playlists keep holes, selection rules and explicit empty/unpublished rejection", async () => {
    const before = await oneMusic([62]);
    const total = await starts();
    const input = [slots(2, null, 76), slots(62), slots(163)];
    input[1][99] = 144;
    await page.evaluate((input) => {
      for (let slot = 1; slot <= 3; slot++) audioFixture.controls.saved(slot, input[slot - 1]);
      audioFixture.controls.edit(1, { kind: "add", group: 327 });
      audioFixture.controls.edit(1, { kind: "remove", group: 2 });
    }, input);
    assert.equal(await starts(), total);
    assert.equal((await snapshot()).voices.find((entry) => entry.kind === "music").id, before.id);
    const state = await read();
    assert.deepEqual(state.preferences.music.savedPlaylist1.slice(0, 3), [null, 327, 76]);
    assert.equal(state.preferences.music.savedPlaylist2[99], 144);
    assert.equal(state.preferences.music.savedPlaylist3[0], 163);
    const invalid = await page.evaluate(() => {
      const before = audioFixture.controls.serialize(audioFixture.controls.read().preferences), codes = [];
      for (const operation of [
        () => audioFixture.controls.saved(4, Array(100).fill(null)),
        () => audioFixture.controls.saved(1, [65000, ...Array(99).fill(null)]),
        () => audioFixture.controls.apply({ ...audioFixture.controls.read().preferences, version: 2 }, [62]),
      ]) {
        try { operation(); codes.push("unexpected-success"); }
        catch (error) { codes.push(error.code); }
      }
      return { codes, unchanged: before === audioFixture.controls.serialize(audioFixture.controls.read().preferences) };
    });
    assert.deepEqual(invalid.codes, ["AUDIO_PREFERENCES", "AUDIO_PREFERENCE_TRACK", "AUDIO_PREFERENCE_VERSION"]);
    assert.equal(invalid.unchanged, true);
    const selection = await press("#playlist-1");
    assert.equal(selection.preferences.music.mode, "shuffle");
    assert.equal(selection.preferences.music.currentPlaylist, 1);
    await oneMusic([327, 76]);
    await press("#playlist-0");
    await oneMusic([76]);
    assert.equal((await read()).preferences.music.mode, "area");
    await page.evaluate(() => audioFixture.controls.saved(3, Array(100).fill(null)));
    await press("#playlist-3");
    assert.equal((await read()).preferences.music.currentPlaylist, 3);
    assert.equal((await snapshot()).voices.some((voice) => voice.kind === "music"), false);
    assert.equal(await page.evaluate(() => audioFixture.errors.some((error) => error.code === "AUDIO_PLAYLIST_EMPTY")), true);
    assert.equal((await press("#skip")).status, "no_alternative");
    return { inactivePlaylistStarts: 0, holeAfterRemoval: 1, firstHoleInsertion: 2, slot2LastGroup: 144,
      sourceSelectionMode: 1, allMusicSelectionMode: 0, emptyPlaylistReported: true, invalid };
  });

  await check("native remembered mute vectors, first-use fallbacks and slider-zero differ from global privacy mute", async () => {
    await music({ mode: "single", selectedGroup: 62 });
    await page.evaluate(() => {
      const binding = audioFixture.controls.read();
      audioFixture.controls.apply({ ...binding.preferences, volumes: {
        current: { master: 0, music: 0, effects: 0, area: 0 },
        remembered: { master: 0, music: 0, effects: 0, area: 0 },
      } }, binding.unlockedGroups);
    });
    for (const channel of ["master", "music", "effects", "area"]) await press(`#toggle-${channel}`);
    const original = native.cases.find((entry) => entry.case === "first-use-zero-memory").observed.after_original_option_sync;
    const first = await read();
    const vector = (values) => [values.master, values.music, values.effects, values.area];
    assert.deepEqual(vector(first.preferences.volumes.current), original.current);
    assert.deepEqual(vector(first.preferences.volumes.remembered), original.remembered);
    assert.deepEqual(Object.values((await snapshot()).nativeMixer), original.native_mixer);
    await oneMusic([62]);
    const custom = { master: 37, music: 21, effects: 66, area: 83 };
    await setPercentages(custom);
    assert.deepEqual((await read()).preferences.volumes.remembered, { master: 100, music: 20, effects: 45, area: 25 });
    await page.locator("#music").focus();
    await page.keyboard.press("Home");
    assert.equal((await read()).preferences.volumes.current.music, 0);
    assert.equal((await read()).preferences.volumes.remembered.music, 20);
    await press("#toggle-music");
    assert.equal((await read()).preferences.volumes.current.music, 20);
    await setPercentages(custom);
    for (const channel of ["master", "music", "effects", "area"]) await press(`#toggle-${channel}`);
    const muted = (await read()).preferences.volumes;
    assert.deepEqual(vector(muted.current), [0, 0, 0, 0]);
    assert.deepEqual(muted.remembered, custom);
    for (const channel of ["master", "music", "effects", "area"]) await press(`#toggle-${channel}`);
    assert.deepEqual((await read()).preferences.volumes.current, custom);
    assert.deepEqual(Object.values((await snapshot()).nativeMixer),
      native.cases.find((entry) => entry.case === "restore-saved-vector").observed.after_original_option_sync.native_mixer);
    const serialized = await page.evaluate(() => audioFixture.controls.serialize(audioFixture.controls.read().preferences));
    await page.click("#mute");
    const silence = await page.evaluate(() => audioFixture.capture(4096));
    assert.equal(Math.max(...silence.samples.map(Math.abs)), 0);
    await page.click("#unmute");
    assert.equal(await page.evaluate(() => audioFixture.controls.serialize(audioFixture.controls.read().preferences)), serialized);
    await oneMusic([62]);
    return { originalRestore: original.current, restoredSavedVector: vector(custom),
      restoredNativeMixer: Object.values((await snapshot()).nativeMixer), sliderZeroDidNotRewriteMemory: true,
      globalMutePersistedAsChannelZero: false, mutedMonitorPeak: 0 };
  });

  await check("Skip during a jingle updates remembered source and fades without consuming another bag entry", async () => {
    await setPercentages({ master: 100, music: 50, effects: 45, area: 25 });
    await page.evaluate((entries) => audioFixture.controls.saved(1, entries), slots(62, 76, 2));
    await music({ mode: "shuffle", currentPlaylist: 1, selectedGroup: 62 });
    const initial = await oneMusic([62]);
    const source = manifest.assets.find((asset) => asset.kind === "jingle" && asset.source_group === 33);
    await music({ keepPlayingOnPlaylistChange: true });
    await emit({ kind: "jingle", sourceId: 33, payload: { committed: true } });
    await waitVoice(33, "jingle");
    const keptJingle = (await snapshot()).voices.find((voice) => voice.kind === "jingle");
    await press("#playlist-2");
    assert.equal((await snapshot()).voices.find((voice) => voice.kind === "jingle").id, keptJingle.id);
    const keptBackground = await oneMusic([62]);
    const keptEnd = keptJingle.when + source.loop.source_engine_end_frame / 22050;
    const sameGroupReentryErrorMs = Math.abs(keptBackground.when - keptEnd) * 1000;
    assert.ok(sameGroupReentryErrorMs <= 20, String(sameGroupReentryErrorMs));
    await music({ currentPlaylist: 1, keepPlayingOnPlaylistChange: false });
    await oneMusic([62]);
    const expected = (await snapshot()).background.groups[1];
    const eventId = await emit({ kind: "jingle", sourceId: 33, payload: { committed: true } });
    await waitVoice(33, "jingle");
    const jingle = (await snapshot()).voices.find((voice) => voice.kind === "jingle");
    const count = await starts();
    const first = await press("#skip");
    assert.equal(first.status, "requested");
    assert.equal(first.nextGroup, expected);
    await page.evaluate(() => audioFixture.controls.edit(2, { kind: "remove", group: 144 }));
    const second = await press("#skip");
    const third = await press("#skip");
    assert.equal(second.status, "pending");
    assert.equal(third.status, "pending");
    assert.equal(second.nextGroup, expected);
    assert.equal(third.nextGroup, expected);
    assert.equal((await snapshot()).voices.find((voice) => voice.kind === "jingle").id, jingle.id);
    const background = await oneMusic([expected]);
    const end = jingle.when + source.loop.source_engine_end_frame / 22050;
    const errorMs = Math.abs(background.when - end - 1.2) * 1000;
    assert.ok(errorMs <= 20, JSON.stringify({ background, end, errorMs }));
    const state = await snapshot();
    assert.equal(state.background.cursor, 1);
    assert.equal(state.traces.filter((entry) => entry.type === "started" && entry.data.eventId === eventId).length, 1);
    return { previous: initial.sourceId, remembered: expected, repeatedRequests: [second.status, third.status],
      nativeTransition: native.cases.find((entry) => entry.case === "background-request-during-jingle").first.transition,
      expectedJingleEnd: end, nextSourceStart: background.when, transitionErrorMs: errorMs,
      sameRememberedTrackRetainedItsZeroTransition: true, sameGroupReentryErrorMs,
      jingleNativeVoiceId: jingle.id, jingleRestarts: 0, nativeStartsIncludingBoundClicks: (await starts()) - count };
  });

  await check("Shuffle preserves every entry across pending Skip, mute, reconnect and bag rollover", async () => {
    await page.evaluate(({ unlocked, entries }) => {
      const previous = structuredClone(audioFixture.controls.read().preferences);
      const world = audioFixture.syntheticWorld("region.osrs.12850");
      world.player.id = "player.audio-preference-bag";
      audioFixture.update(world);
      previous.music = { ...previous.music, mode: "shuffle", currentPlaylist: 1, selectedGroup: 62,
        savedPlaylist1: entries, rememberModeOnLogin: true };
      previous.volumes.current = { master: 100, music: 50, effects: 0, area: 0 };
      audioFixture.controls.apply(previous, unlocked);
    }, { unlocked, entries: slots(62, 76, 2) });
    await oneMusic([62]);
    const order = (await snapshot()).background.groups;
    assert.equal(order[0], 62);
    const first = await press("#skip");
    assert.equal(first.nextGroup, order[1]);
    await page.click("#mute");
    assert.equal((await snapshot()).voices.length, 0);
    assert.equal((await press("#skip")).status, "muted");
    await page.click("#unmute");
    const repeated = await press("#skip");
    assert.equal(repeated.status, "pending");
    assert.equal(repeated.nextGroup, order[1]);
    const afterMute = await oneMusic([order[1]]);
    const preferences = await read();
    await page.evaluate(() => audioFixture.handle.disconnected());
    const disconnected = await page.evaluate(async () => {
      try { await audioFixture.controls.skip(); return "unexpected-success"; }
      catch (error) { return error.code; }
    });
    assert.equal(disconnected, "AUDIO_DISCONNECTED");
    await page.evaluate(() => audioFixture.update(audioFixture.world));
    const afterReconnect = await oneMusic([order[1]]);
    assert.notEqual(afterReconnect.id, afterMute.id);
    assert.deepEqual(await read(), preferences);
    const second = await press("#skip");
    assert.equal(second.nextGroup, order[2]);
    await oneMusic([order[2]]);
    const wrap = await press("#skip");
    assert.ok(order.includes(wrap.nextGroup) && wrap.nextGroup !== order[2]);
    await oneMusic([wrap.nextGroup]);
    const before = await starts();
    await page.evaluate(() => {
      const binding = audioFixture.controls.read();
      for (let i = 0; i < 30; i++) {
        audioFixture.update(audioFixture.world);
        audioFixture.controls.apply(structuredClone(binding.preferences), [...binding.unlockedGroups].reverse());
      }
    });
    assert.equal(await starts(), before);
    return { order, firstBagActuallyPlayed: [62, afterMute.sourceId, second.nextGroup],
      pendingSkipPreservedThroughMute: repeated.nextGroup, reconnectRestartedOnlyCurrentGroup: afterReconnect.sourceId,
      nextBagFirst: wrap.nextGroup, skippedOrDoubleConsumedEntries: 0, snapshotStarts: 0 };
  });

  await check("native keep-playing and Single membership selection retain nodes; one-song Shuffle cannot invent an alternative", async () => {
    await page.evaluate((entries) => audioFixture.controls.saved(2, entries), slots(144));
    await music({ mode: "single", selectedGroup: 62, keepPlayingOnPlaylistChange: true });
    const current = await oneMusic([62]);
    const before = await starts();
    const kept = await press("#playlist-2");
    assert.equal(kept.preferences.music.mode, "single");
    assert.equal(kept.preferences.music.currentPlaylist, 2);
    assert.equal((await snapshot()).voices.find((voice) => voice.kind === "music").id, current.id);
    assert.equal(await starts(), before);
    await music({ keepPlayingOnPlaylistChange: false });
    await press("#playlist-2");
    const single = await oneMusic([144]);
    assert.equal((await read()).preferences.music.mode, "shuffle");
    const count = await starts();
    assert.equal((await press("#skip")).status, "no_alternative");
    assert.equal(await starts(), count);
    assert.equal((await snapshot()).voices.find((voice) => voice.kind === "music").id, single.id);
    return { retainedNativeVoiceId: current.id, keepPlayingRestartCount: 0,
      nonmemberSelectionSwitchedToNativeShuffle: true, onlyEligibleGroup: 144, inventedNextTrack: false };
  });

  await check("gesture failure and cross-character reset cannot inherit preferences, tracks or pending commands", async () => {
    await page.evaluate(async () => {
      await audioFixture.context.suspend();
      audioFixture.context.savedControlResume = audioFixture.context.resume;
      audioFixture.context.resume = () => Promise.reject(new DOMException("Injected source control resume denial", "NotAllowedError"));
    });
    const before = await snapshot();
    await page.click("#skip");
    const failed = await page.evaluate(() => audioFixture.lastControl);
    assert.equal(failed.success, false);
    assert.equal(failed.code, "AUDIO_UNLOCK");
    assert.equal((await snapshot()).contextState, "suspended");
    assert.equal((await snapshot()).pendingGesture, true);
    assert.deepEqual((await snapshot()).background, before.background);
    await page.evaluate(() => {
      audioFixture.context.resume = audioFixture.context.savedControlResume;
      delete audioFixture.context.savedControlResume;
    });
    await page.click("#unlock");
    assert.equal((await page.evaluate(() => audioFixture.lastUnlock)).success, true);
    await oneMusic([144]);
    await page.evaluate(async () => {
      await audioFixture.context.suspend();
      const original = audioFixture.context.resume;
      audioFixture.context.savedControlResume = original;
      audioFixture.context.resume = () => new Promise((resolve, reject) => {
        audioFixture.finishDelayedControlResume = () => original.call(audioFixture.context).then(resolve, reject);
      });
    });
    await page.click("#skip");
    await page.evaluate(async () => {
      const binding = audioFixture.controls.read(), world = audioFixture.world;
      const preferences = structuredClone(binding.preferences);
      preferences.music = { ...preferences.music, mode: "shuffle", currentPlaylist: 1,
        selectedGroup: 62, rememberModeOnLogin: true };
      audioFixture.update(null);
      audioFixture.update(world);
      audioFixture.controls.apply(preferences, binding.unlockedGroups);
      audioFixture.context.resume = audioFixture.context.savedControlResume;
      delete audioFixture.context.savedControlResume;
      await audioFixture.finishDelayedControlResume();
      delete audioFixture.finishDelayedControlResume;
    });
    const stale = await page.evaluate(() => audioFixture.lastControl);
    assert.equal(stale.success, false);
    assert.equal(stale.code, "AUDIO_CONTROL_SUPERSEDED");
    await oneMusic([62]);
    assert.equal((await snapshot()).background.groups[(await snapshot()).background.cursor], 62);
    const old = await read();
    const reset = await page.evaluate(() => {
      const oldId = audioFixture.world.player.id;
      const world = audioFixture.syntheticWorld("region.osrs.12850");
      world.player.id = "player.audio-preference-new-character";
      audioFixture.update(world);
      let rejected;
      try { audioFixture.controls.toggle("music", oldId); }
      catch (error) { rejected = error.code; }
      return { binding: audioFixture.controls.read(), state: audioFixture.snapshot(), rejected };
    });
    assert.equal(reset.binding, null);
    assert.equal(reset.rejected, "AUDIO_PREFERENCE_CHARACTER");
    assert.deepEqual(reset.state.volumes, { music: 1, effects: 1, area: 1 });
    assert.equal(reset.state.masterPercent, 100);
    const fresh = await page.evaluate((unlocked) => {
      const input = structuredClone(audioFixture.controls.defaults());
      input.music.mode = "shuffle"; // A saved last mode is not an instruction to remember it on login.
      input.music.currentPlaylist = 1;
      input.music.savedPlaylist1[0] = 62;
      input.volumes.current = { master: 100, music: 50, effects: 0, area: 0 };
      return audioFixture.controls.apply(input, unlocked);
    }, unlocked);
    assert.equal(fresh.preferences.music.mode, "area");
    assert.equal(fresh.preferences.music.currentPlaylist, 0);
    assert.deepEqual(fresh.preferences.volumes.remembered, { master: 0, music: 0, effects: 0, area: 0 });
    assert.notEqual(fresh.playerId, old.playerId);
    await oneMusic([76]);
    return { injectedResumeFailure: failed.code, nativeBagChangedOnFailure: false,
      deferredResumeAcrossSameIdReset: stale.code, staleSkipAppliedToNewSession: false,
      previousPlayer: old.playerId, newPlayer: fresh.playerId, inheritedBinding: false,
      freshLoginMode: fresh.preferences.music.mode, nativeCurrentPlaylist: 0 };
  });

  if (!quick) await check("bound native Single repeats at 194 source ticks even when native4137 is clear, without extra clipping", async () => {
    const clips = (await page.evaluate(() => audioFixture.stats())).clips;
    await music({ mode: "single", selectedGroup: 163, repeatInAreaShuffle: false });
    const first = await oneMusic([163]), boundary = first.when + 194 * 0.6;
    assert.equal((await read()).preferences.music.repeatInAreaShuffle, false);
    assert.equal((await read()).musicState.loopEnabled, true);
    await page.waitForFunction(({ boundary, id }) => audioFixture.context.currentTime >= boundary + 0.1 &&
      audioFixture.snapshot().voices.some((voice) => voice.kind === "music" && voice.sourceId === 163 &&
        voice.id !== id && voice.when <= audioFixture.context.currentTime), { boundary, id: first.id }, { timeout: 130_000 });
    const second = await oneMusic([163]);
    const errorMs = Math.abs(second.when - boundary) * 1000;
    assert.ok(errorMs <= 20);
    assert.equal(second.loop, false);
    assert.equal(second.renderedNativeLevel, 255);
    assert.equal(second.appliedNativeLevel, 44);
    assert.equal((await page.evaluate(() => audioFixture.stats())).clips - clips, 0);
    return { sourceScript: 9630, native4137: 0, nativeMode: 2, durationTicks: 194,
      firstStart: first.when, secondStart: second.when, timingErrorMs: errorMs,
      releasePaddingSampleLoop: false, addedClipSamples: 0, actualNativeMixer: 44 };
  });
}
