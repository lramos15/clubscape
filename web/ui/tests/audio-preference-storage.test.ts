import assert from "node:assert/strict";
import test from "node:test";
import { UiAudioPreferencePersistence } from "../audio-preference-storage.ts";
import { deserializeSourceAudioPreferences, parseSourceAudioPreferences, serializeSourceAudioPreferences,
  sourceAudioPreferenceDefaults } from "../../audio/preferences.ts";
import type { SourceAudioPreferences } from "../../audio/preferences.ts";

function deferred() {
  let resolve!: () => void, reject!: (error: unknown) => void;
  const promise = new Promise<void>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
function record(percent: number): SourceAudioPreferences {
  const base = sourceAudioPreferenceDefaults();
  return parseSourceAudioPreferences({ ...base, volumes: { ...base.volumes, current: { ...base.volumes.current, music: percent } } });
}
const turn = () => new Promise<void>(resolve => queueMicrotask(resolve));

test("only a confirmed absent record uses defaults; corrupt or failed reads remain explicit failures", async () => {
  const failure = Object.assign(new Error("Actual player storage read failed"), { errorId: "storage.read.actual" });
  let value: string | null = null, fail = false, reads = 0;
  const storage = new UiAudioPreferencePersistence({
    async load(player) { assert.equal(player, "player-A"); reads++; if (fail) throw failure; return value; },
    async save() { throw new Error("Loading must not write a guessed record"); },
  });
  assert.deepEqual(await storage.load("player-A"), sourceAudioPreferenceDefaults());
  value = serializeSourceAudioPreferences(record(37));
  assert.equal((await storage.load("player-A")).volumes.current.music, 37);
  value = "{corrupt";
  await assert.rejects(storage.load("player-A"), /not valid JSON/);
  await assert.rejects(storage.flush("player-A"), /not valid JSON/);
  fail = true;
  await assert.rejects(storage.load("player-A"), error => error === failure);
  assert.equal(storage.status("player-A").error, failure);
  assert.equal(reads, 4);
});

test("same-character writes serialize and coalesce intermediate values without older completion overwriting newer preferences", async () => {
  const calls: Array<{ player: string; text: string; pending: ReturnType<typeof deferred> }> = [];
  const saved = new Map<string, string>();
  const storage = new UiAudioPreferencePersistence({
    async load(player) { return saved.get(player) ?? null; },
    async save(player, text) {
      const pending = deferred(); calls.push({ player, text, pending });
      await pending.promise; saved.set(player, text);
    },
  });
  const first = storage.save("A", record(11)), second = storage.save("A", record(22)), latest = storage.save("A", record(33));
  assert.equal(calls.length, 1);
  calls[0]!.pending.resolve(); await turn(); await turn();
  assert.equal(calls.length, 2);
  assert.equal(deserializeSourceAudioPreferences(calls[1]!.text).volumes.current.music, 33);
  calls[1]!.pending.resolve();
  assert.equal((await first).status, "saved");
  assert.equal((await second).status, "superseded");
  assert.equal((await latest).status, "saved");
  assert.equal(deserializeSourceAudioPreferences(saved.get("A")!).volumes.current.music, 33);
  assert.equal(storage.status("A").dirty, false);
});

test("different characters have independent queues and never share record keys", async () => {
  const calls: Array<{ player: string; pending: ReturnType<typeof deferred> }> = [];
  const storage = new UiAudioPreferencePersistence({
    async load() { return null; },
    async save(player) { const pending = deferred(); calls.push({ player, pending }); await pending.promise; },
  });
  const a = storage.save("A", record(10)), b = storage.save("B", record(20));
  assert.deepEqual(calls.map(call => call.player), ["A", "B"]);
  calls[1]!.pending.resolve(); await b;
  assert.equal(storage.status("A").dirty, true); assert.equal(storage.status("B").dirty, false);
  calls[0]!.pending.resolve(); await a;
});

test("a failed write rejects dependent work, retains the latest exact record and requires an explicit retry", async () => {
  const failure = Object.assign(new Error("Actual write failure"), { errorId: "storage.write.actual" });
  const gate = deferred(); let first = true, reads = 0; const writes: string[] = [];
  const storage = new UiAudioPreferencePersistence({
    async load() { reads++; return null; },
    async save(_player, text) { writes.push(text); if (first) { first = false; await gate.promise; } },
  });
  const a = storage.save("A", record(10)), b = storage.save("A", record(21)), c = storage.save("A", record(37));
  const settled = Promise.allSettled([a, b, c]);
  gate.reject(failure);
  const results = await settled;
  assert.ok(results.every(result => result.status === "rejected" && result.reason === failure));
  assert.equal(storage.status("A").dirty, true);
  assert.equal(storage.status("A").error, failure);
  await assert.rejects(storage.load("A"), error => error === failure);
  assert.equal(reads, 0);
  const retry = await storage.retry("A");
  assert.equal(retry.status, "saved");
  assert.equal(deserializeSourceAudioPreferences(writes.at(-1)!).volumes.current.music, 37);
  assert.equal(storage.status("A").dirty, false);
});

test("re-entry loads cannot overtake an older pending save", async () => {
  const gate = deferred(); let stored: string | null = null; const order: string[] = [];
  const storage = new UiAudioPreferencePersistence({
    async load() { order.push("load"); return stored; },
    async save(_player, text) { order.push("save"); await gate.promise; stored = text; },
  });
  const save = storage.save("same-player", record(66));
  const loaded = storage.load("same-player");
  assert.deepEqual(order, ["save"]);
  gate.resolve(); await save;
  assert.equal((await loaded).volumes.current.music, 66);
  assert.deepEqual(order, ["save", "load"]);
});

test("saved playlist holes and slot100 survive exact serialization without actor/unlock/privacy state", async () => {
  const base = sourceAudioPreferenceDefaults();
  const slots: (number | null)[] = Array.from({ length: 100 }, () => null);
  slots[1] = 64; slots[99] = 327;
  const preferences = parseSourceAudioPreferences({ ...base, music: { ...base.music, savedPlaylist2: slots } });
  let saved = "";
  const storage = new UiAudioPreferencePersistence({ async load() { return null; }, async save(_player, text) { saved = text; } });
  await storage.save("actual-character-key", preferences);
  const result = deserializeSourceAudioPreferences(saved);
  assert.equal(result.music.savedPlaylist2.length, 100);
  assert.equal(result.music.savedPlaylist2[0], null);
  assert.equal(result.music.savedPlaylist2[1], 64);
  assert.equal(result.music.savedPlaylist2[99], 327);
  assert.deepEqual(Object.keys(JSON.parse(saved)).sort(), ["music", "version", "volumes"]);
  assert.ok(!saved.includes("actual-character-key") && !saved.includes("unlockedGroups") && !saved.includes("muted"));
});

test("invalid records never reach storage and a retry without dirty data cannot report fake success", async () => {
  let writes = 0;
  const storage = new UiAudioPreferencePersistence({ async load() { return null; }, async save() { writes++; } });
  const invalid = structuredClone(sourceAudioPreferenceDefaults());
  Reflect.set(invalid, "playerId", "forbidden");
  assert.throws(() => storage.save("A", invalid), /extra/);
  assert.equal(writes, 0);
  await assert.rejects(storage.retry("A"), /no unsaved/);
});

test("even a null storage rejection remains failed and cannot fall through to a default read", async () => {
  let reads = 0;
  const storage = new UiAudioPreferencePersistence({
    async load() { reads++; return null; },
    async save() { throw null; },
  });
  const result = await Promise.allSettled([storage.save("A", record(20))]);
  assert.equal(result[0]!.status, "rejected");
  assert.equal(storage.status("A").state, "failed");
  const loaded = await Promise.allSettled([storage.load("A")]);
  assert.equal(loaded[0]!.status, "rejected");
  assert.equal(reads, 0);
});
