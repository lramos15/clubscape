import assert from "node:assert/strict";
import { test } from "node:test";
import { setImmediate } from "node:timers/promises";
import { sourceAudioPreferenceDefaults, deserializeSourceAudioPreferences, serializeSourceAudioPreferences } from "../../audio/index.ts";
import { AppError } from "../errors.ts";
import { browserPlayerAudioStorage, PlayerAudioPreferenceStore, PLAYER_AUDIO_PREFERENCE_PREFIX } from "../player-audio-store.ts";
import { audioFixtureRecord } from "./player-audio-fixture.ts";

const player = "actor.audio_fixture";

test("only a confirmed absent per-player record selects native new-record defaults", async () => {
  const reads: string[] = [], writes: string[] = [];
  const store = new PlayerAudioPreferenceStore({
    read: (id) => { reads.push(id); return null; },
    write: (_id, value) => { writes.push(value); },
  });
  const loaded = await store.load(player);
  assert.deepEqual(loaded, { origin: "confirmed_absent", preferences: sourceAudioPreferenceDefaults() });
  assert.deepEqual(reads, [player]);
  assert.deepEqual(writes, [], "Reading absence does not fabricate a successful save.");
  assert(Object.isFrozen(loaded.preferences.music.savedPlaylist1));
});

test("browser storage uses the real actor only as a namespaced key and never stores authority or credentials", async () => {
  const data = new Map<string, string>();
  const storage = browserPlayerAudioStorage(() => ({
    getItem: (key) => data.get(key) ?? null, setItem: (key, value) => { data.set(key, value); },
  }));
  const store = new PlayerAudioPreferenceStore(storage), record = audioFixtureRecord(21);
  const receipt = await store.save(player, record);
  assert.equal(receipt.status, "stored");
  const text = data.get(`${PLAYER_AUDIO_PREFERENCE_PREFIX}${player}`)!;
  assert.deepEqual(JSON.parse(text), record);
  assert(!/actor\.|playerId|account|unlocked|playhead|permission|privacyMute|token|password/i.test(text));
  assert.equal(text, serializeSourceAudioPreferences(record));
  assert.deepEqual(await store.load(player), { preferences: record, origin: "stored" });
  assert.equal((await store.load("actor.other")).origin, "confirmed_absent");
  for (const id of ["", "account.example", "../actor.other", "actor..other", "actor.Other", `actor.${"x".repeat(161)}`]) {
    await assert.rejects(store.load(id), /Invalid player identity/);
  }
});

test("failed, denied and invalid storage reads never become confirmed absence", async () => {
  const privateText = "credential-like-corrupt-storage-must-not-be-echoed";
  const denied = new PlayerAudioPreferenceStore(browserPlayerAudioStorage(() => { throw new Error(privateText); }));
  const failed = new PlayerAudioPreferenceStore({
    read: async () => { throw new Error(privateText); }, write() { throw new Error("No write expected."); },
  });
  const invalid = new PlayerAudioPreferenceStore({
    // @ts-expect-error Deliberately invalid external storage result, not a valid storage implementation.
    read: async () => undefined, write() { throw new Error("No write expected."); },
  });
  for (const store of [denied, failed, invalid]) {
    await assert.rejects(store.load(player), (error: unknown) => {
      assert(error instanceof AppError);
      assert.equal(error.kind, "audio_preferences_read");
      assert(!error.message.includes(privateText));
      return true;
    });
  }
});

test("corrupt, oversized, unknown-version and compacted records are preserved without defaults or rewrite", async () => {
  const valid = sourceAudioPreferenceDefaults();
  const invalid = [
    '{"credential":"do-not-echo-this-private-string"',
    " ".repeat(16_385), "null",
    JSON.stringify({ ...valid, version: 2 }),
    JSON.stringify({ ...valid, playerId: player }),
    JSON.stringify({ ...valid, unlockedGroups: [62] }),
    JSON.stringify({ ...valid, visitHistory: ["region.lumbridge"] }),
    JSON.stringify({ ...valid, sourceVarps: { 281: 100 } }),
    JSON.stringify({ ...valid, music: { ...valid.music, unlockedGroups: [62, 76] } }),
    JSON.stringify({ ...valid, music: { ...valid.music, history: [62, 76] } }),
    JSON.stringify({ ...valid, music: { ...valid.music, savedPlaylist1: [62] } }),
    JSON.stringify({ ...valid, volumes: { ...valid.volumes, remembered: { ...valid.volumes.remembered, master: 101 } } }),
  ];
  for (const text of invalid) {
    let writes = 0;
    const store = new PlayerAudioPreferenceStore({ read: () => text, write() { writes++; } });
    await assert.rejects(store.load(player), (error: unknown) =>
      error instanceof AppError && error.errorId === "audio.preferences.invalid_record" && !error.message.includes(text));
    assert.equal(writes, 0);
  }
  const sparse = { ...valid, music: { ...valid.music, savedPlaylist1: Array<number | null>(100) } };
  const store = new PlayerAudioPreferenceStore({ read: () => null, write() { throw new Error("Invalid record reached storage."); } });
  await assert.rejects(store.save(player, sparse), /100 explicit/);
  for (const authority of [
    { ...valid, unlockedGroups: [62, 76] },
    { ...valid, visitHistory: ["region.lumbridge"] },
    { ...valid, sourceVarps: { 281: 100 } },
  ]) await assert.rejects(store.save(player, authority), /missing, extra, or accessor fields/);
});

test("all three exact100-slot arrays retain holes, native flags and current/remembered volumes", async () => {
  let text: string | null = null;
  const record = audioFixtureRecord(21);
  const full = { ...record, music: { ...record.music,
    savedPlaylist2: [...record.music.savedPlaylist1], savedPlaylist3: [...record.music.savedPlaylist1],
    currentPlaylist: 3 as const, repeatInAreaShuffle: true, keepPlayingOnPlaylistChange: true,
  } };
  const store = new PlayerAudioPreferenceStore({ read: () => text, write: (_id, value) => { text = value; } });
  await store.save(player, full);
  const loaded = await store.load(player);
  assert.deepEqual(loaded.preferences, full);
  for (const list of [loaded.preferences.music.savedPlaylist1, loaded.preferences.music.savedPlaylist2, loaded.preferences.music.savedPlaylist3]) {
    assert.equal(list.length, 100);
    assert.equal(list[0], null); assert.equal(list[1], 62); assert.equal(list[2], null); assert.equal(list[99], 76);
    assert.equal(Object.keys(list).length, 100);
  }
});

test("pending writes coalesce, while an older active write always finishes before the newest value", async () => {
  const firstStarted = Promise.withResolvers<void>(), releaseFirst = Promise.withResolvers<void>();
  const secondStarted = Promise.withResolvers<void>(), releaseSecond = Promise.withResolvers<void>();
  const written: number[] = [];
  let active = 0, maxActive = 0, persisted: string | null = null;
  const store = new PlayerAudioPreferenceStore({
    read: () => persisted,
    async write(_id, text) {
      active++; maxActive = Math.max(maxActive, active);
      const percent = deserializeSourceAudioPreferences(text).volumes.current.music;
      written.push(percent);
      if (written.length === 1) { firstStarted.resolve(); await releaseFirst.promise; }
      else { secondStarted.resolve(); await releaseSecond.promise; }
      persisted = text; active--;
    },
  });
  const first = store.save(player, audioFixtureRecord(10));
  await firstStarted.promise;
  const second = store.save(player, audioFixtureRecord(20));
  const third = store.save(player, audioFixtureRecord(30));
  let flushed = false;
  const flush = store.flush(player).then(() => { flushed = true; });
  await setImmediate();
  assert.deepEqual(written, [10]); assert.equal(flushed, false);
  releaseFirst.resolve();
  await secondStarted.promise;
  assert.deepEqual(written, [10, 30]); assert.equal(flushed, false);
  releaseSecond.resolve();
  const receipts = await Promise.all([first, second, third]);
  await flush;
  assert.equal(maxActive, 1);
  assert.deepEqual(receipts, [
    { status: "stored", sequence: 1, persistedSequence: 1 },
    { status: "superseded", sequence: 2, persistedSequence: 3 },
    { status: "stored", sequence: 3, persistedSequence: 3 },
  ]);
  assert.equal((await store.load(player)).preferences.volumes.current.music, 30);
});

test("same-turn writes save only the newest record and pending loads wait for that actual save", async () => {
  const started = Promise.withResolvers<void>(), release = Promise.withResolvers<void>();
  let persisted: string | null = null, reads = 0, writes = 0;
  const store = new PlayerAudioPreferenceStore({
    read: () => { reads++; return persisted; },
    async write(_id, value) { writes++; started.resolve(); await release.promise; persisted = value; },
  });
  const first = store.save(player, audioFixtureRecord(10)), latest = store.save(player, audioFixtureRecord(99));
  const loaded = store.load(player);
  await started.promise;
  assert.equal(reads, 0);
  release.resolve();
  assert.equal((await loaded).preferences.volumes.current.music, 99);
  assert.equal(writes, 1);
  assert.equal((await first).status, "superseded");
  assert.equal((await latest).status, "stored");
});

test("independent characters are not blocked behind another character's asynchronous save", async () => {
  const started = Promise.withResolvers<void>(), release = Promise.withResolvers<void>();
  const data = new Map<string, string>();
  const store = new PlayerAudioPreferenceStore({
    read: (id) => data.get(id) ?? null,
    async write(id, value) { if (id === player) { started.resolve(); await release.promise; } data.set(id, value); },
  });
  const slow = store.save(player, audioFixtureRecord(10));
  await started.promise;
  await store.save("actor.other", audioFixtureRecord(20));
  await store.flush("actor.other");
  assert.equal((await store.load("actor.other")).preferences.volumes.current.music, 20);
  assert.equal(data.has(player), false);
  release.resolve(); await slow;
});

test("failed active and queued saves reject explicitly; a later read cannot silently restore stale storage", async () => {
  const started = Promise.withResolvers<void>(), release = Promise.withResolvers<void>();
  let fail = true, attempts = 0, text = serializeSourceAudioPreferences(audioFixtureRecord(7));
  const store = new PlayerAudioPreferenceStore({
    read: () => text,
    async write(_id, value) {
      attempts++; started.resolve(); await release.promise;
      if (fail) throw new Error("storage-denial-details-not-public");
      text = value;
    },
  });
  const first = store.save(player, audioFixtureRecord(10));
  await started.promise;
  const latest = store.save(player, audioFixtureRecord(90));
  const results = Promise.allSettled([first, latest]);
  release.resolve();
  assert((await results).every((result) => result.status === "rejected"
    && result.reason instanceof AppError && result.reason.errorId === "audio.preferences.storage_write"));
  await assert.rejects(store.load(player), /save failed/);
  await assert.rejects(store.flush(), /save failed/);
  assert.equal(attempts, 1, "A failure is not a background retry loop or success-shaped queued save.");
  assert.equal(deserializeSourceAudioPreferences(text).volumes.current.music, 7);
  fail = false;
  await store.save(player, audioFixtureRecord(90));
  assert.equal((await store.load(player)).preferences.volumes.current.music, 90);
});
