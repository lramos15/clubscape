import assert from "node:assert/strict";
import { test } from "node:test";
import { setImmediate } from "node:timers/promises";
import { AudioFailure, deserializeSourceAudioPreferences, serializeSourceAudioPreferences, sourceAudioPreferenceDefaults } from "../../audio/index.ts";
import type { AudioEvent } from "../../shared/contracts.ts";
import type { SourceAudioPreferenceBinding, SourceMusicSkipResult } from "../../audio/index.ts";
import { AppError } from "../errors.ts";
import { PlayerAudioPreferences } from "../player-audio.ts";
import { PlayerAudioPreferenceStore } from "../player-audio-store.ts";
import { audioFixtureRecord, audioFixtureWorld, FixturePreferenceRuntime } from "./player-audio-fixture.ts";

function fixture() {
  let stored = serializeSourceAudioPreferences(audioFixtureRecord(21));
  const writes: string[] = [], errors: AppError[] = [], projections: SourceAudioPreferenceBinding[] = [];
  const runtime = new FixturePreferenceRuntime();
  const store = new PlayerAudioPreferenceStore({
    read: () => stored, write: (_id, value) => { stored = value; writes.push(value); },
  });
  const preferences = new PlayerAudioPreferences(store, runtime, (binding) => {
    projections.push(binding); runtime.calls.push({ kind: "project", args: [binding] });
  }, (error) => errors.push(error));
  runtime.observe = (binding) => preferences.changed(binding);
  return { preferences, runtime, store, writes, errors, projections, world: audioFixtureWorld() };
}

test("genuine storage finishes before the single synchronous world/apply/UI boundary", async () => {
  const load = Promise.withResolvers<string | null>();
  const runtime = new FixturePreferenceRuntime(), order: string[] = [];
  const record = audioFixtureRecord(21), world = audioFixtureWorld();
  const store = new PlayerAudioPreferenceStore({
    read: () => load.promise, write: (_id, text) => { order.push("save"); assert.deepEqual(JSON.parse(text), record); },
  });
  const preferences = new PlayerAudioPreferences(store, runtime, (binding) => {
    runtime.calls.push({ kind: "project", args: [binding] }); order.push("project");
  }, (error) => { throw error; });
  const pending = preferences.prepare(world.player.id);
  assert.equal(preferences.observe().phase, "loading");
  assert.throws(() => preferences.commit(world, [], undefined, [62, 76]), /superseded/);
  assert(!runtime.calls.some((call) => call.kind === "world"));
  load.resolve(serializeSourceAudioPreferences(record));
  await pending;
  const events: readonly AudioEvent[] = Object.freeze([Object.freeze({
    id: "event.contract_fixture", kind: "interface_closed", sourceId: 153, assetId: null,
    actorId: world.player.id, tile: null, sourceCycle: 12, payload: Object.freeze({ fixture: true }),
  })]);
  const binding = preferences.commit(world, events, undefined, [76, 62]);
  assert.deepEqual(runtime.calls.filter((call) => call.kind !== "disconnect").map((call) => call.kind), ["world", "apply", "project"]);
  assert.equal(runtime.calls.find((call) => call.kind === "world")!.args[0], world);
  assert.equal(runtime.calls.find((call) => call.kind === "world")!.args[1], events);
  assert.deepEqual(runtime.calls.find((call) => call.kind === "apply")!.args, [world.player.id, record, [76, 62]]);
  assert.deepEqual(order, ["project"], "No storage await or empty world update splits the native batch.");
  assert.equal(preferences.read(), binding);
  await preferences.flush();
  assert.deepEqual(order, ["project", "save"]);
  assert.equal(preferences.observe().save, "stored");
});

test("only the native returned canonical preferences are persisted, without runtime binding/authority", async () => {
  const f = fixture();
  f.runtime.applying = () => audioFixtureRecord(44);
  await f.preferences.prepare(f.world.player.id);
  const binding = f.preferences.commit(f.world, [], undefined, [62, 76]);
  await f.preferences.flush();
  assert.equal(binding.preferences.volumes.current.music, 44);
  assert.deepEqual(JSON.parse(f.writes[0]!), binding.preferences);
  assert(!f.writes[0]!.includes("unlockedGroups") && !f.writes[0]!.includes(f.world.player.id));
  assert.deepEqual(f.errors, []);
  assert(Object.isFrozen(f.preferences.observe()));
});

test("missing and invalid actual unlocks cannot start a world or rewrite the stored selection", async () => {
  const f = fixture();
  await f.preferences.prepare(f.world.player.id);
  assert.equal(f.preferences.readyFor(f.world.player.id, undefined), false);
  assert.equal(f.preferences.observe().phase, "waiting_unlocks");
  assert.equal(f.preferences.read(), null);
  assert.throws(() => f.preferences.commit(f.world, [], undefined, [62]), /AUDIO_PREFERENCE_UNLOCKS/);
  assert(!f.runtime.calls.some((call) => call.kind === "world"));
  assert.deepEqual(f.writes, []);
  assert.throws(() => f.preferences.readyFor(f.world.player.id, [62, 62]), /AUDIO_PREFERENCE_UNLOCKS/);
  assert.equal(f.preferences.readyFor(f.world.player.id, [76, 62]), true);
  f.preferences.commit(f.world, [], undefined, [76, 62]);
  await f.preferences.flush();
  assert.equal(f.preferences.observe().phase, "ready");
});

test("world, native-apply and UI-projection failures disable controls rather than retaining a ready binding", async () => {
  for (const phase of ["world", "apply", "project"] as const) {
    const runtime = new FixturePreferenceRuntime(), errors: AppError[] = [];
    const fault = new AppError(`Contract fixture ${phase} failed.`, { kind: "audio", errorId: `fixture.${phase}` });
    let fail = false;
    runtime.updating = () => { if (fail && phase === "world") throw fault; };
    runtime.applying = (record) => { if (fail && phase === "apply") throw fault; return record; };
    const preferences = new PlayerAudioPreferences(new PlayerAudioPreferenceStore({
      read: () => serializeSourceAudioPreferences(audioFixtureRecord(21)), write() {},
    }), runtime, () => { if (fail && phase === "project") throw fault; }, (error) => errors.push(error));
    const world = audioFixtureWorld();
    await preferences.prepare(world.player.id);
    preferences.commit(world, [], undefined, [62, 76]);
    const prior = preferences.controls();
    fail = true;
    assert.throws(() => preferences.commit(world, [], undefined, [62, 76]), (error: unknown) => error === fault);
    assert.equal(preferences.observe().phase, "failed");
    assert.equal(preferences.observe().errorId, fault.errorId);
    assert.equal(preferences.read(), null);
    assert.throws(() => prior.setPercent(world.player.id, "music", 9), /superseded/);
    assert.equal(runtime.calls.at(-1)?.kind, "disconnect");
    fail = false;
    await preferences.prepare(world.player.id);
    preferences.commit(world, [], undefined, [62, 76]);
    await preferences.flush();
    assert.equal(preferences.observe().phase, "ready");
  }
});

test("source controls call the exact native command with source slots/percentages and never a mode-flip Skip", async () => {
  const f = fixture();
  await f.preferences.prepare(f.world.player.id);
  f.preferences.commit(f.world, [], undefined, [62, 76]);
  const controls = f.preferences.controls(), player = f.world.player.id;
  controls.setMusic(player, { ...controls.read()!.preferences.music, mode: "shuffle" });
  controls.selectPlaylist(player, 3);
  const slots = [...sourceAudioPreferenceDefaults().music.savedPlaylist1];
  slots[2] = 62; slots[99] = 76;
  controls.setSavedPlaylist(player, 2, slots);
  controls.editSavedPlaylist(player, 2, { kind: "remove", group: 62 });
  controls.toggleMute(player, "master");
  controls.setPercent(player, "effects", 45);
  const before = f.runtime.calls.length;
  const pending = controls.skip(player);
  assert.equal(f.runtime.calls[before]?.kind, "skip", "The native Skip call runs synchronously in the trusted handler stack.");
  assert.equal((await pending).status, "disabled_mode");
  assert(!f.runtime.calls.slice(before).some((call) => call.kind === "music"));
  assert.deepEqual(f.runtime.calls.find((call) => call.kind === "replace")?.args, [player, 2, slots]);
  assert.deepEqual(f.runtime.calls.find((call) => call.kind === "edit")?.args, [player, 2, { kind: "remove", group: 62 }]);
  assert.deepEqual(f.runtime.calls.find((call) => call.kind === "percent")?.args, [player, "effects", 45]);
  await f.preferences.flush();
  const stored = deserializeSourceAudioPreferences(f.writes.at(-1)!);
  assert.equal(stored.music.savedPlaylist2.length, 100);
  assert.equal(stored.music.savedPlaylist2[2], null); assert.equal(stored.music.savedPlaylist2[99], 76);
});

test("observing unchanged native binding is idempotent and never writes unlocks/playhead", async () => {
  const f = fixture();
  await f.preferences.prepare(f.world.player.id);
  const binding = f.preferences.commit(f.world, [], undefined, [62, 76]);
  await f.preferences.flush();
  for (let index = 0; index < 10; index++) f.preferences.changed(binding);
  await f.preferences.flush();
  assert.equal(f.writes.length, 1);
  assert.equal(f.projections.length, 1);
  f.runtime.preferences.percent(f.world.player.id, "music", 17);
  await f.preferences.flush();
  assert.equal(f.writes.length, 2);
  assert.equal(deserializeSourceAudioPreferences(f.writes[1]!).volumes.current.music, 17);
});

test("disconnect keeps genuine loaded preferences, while logout/re-entry reloads even the same actor", async () => {
  let reads = 0;
  const runtime = new FixturePreferenceRuntime(), world = audioFixtureWorld();
  const preferences = new PlayerAudioPreferences(new PlayerAudioPreferenceStore({
    read: () => { reads++; return serializeSourceAudioPreferences(audioFixtureRecord(21)); }, write() {},
  }), runtime, () => {}, (error) => { throw error; });
  await preferences.prepare(world.player.id);
  preferences.commit(world, [], undefined, [62, 76]);
  preferences.invalidate(false);
  assert.equal(preferences.observe().phase, "disconnected");
  await preferences.prepare(world.player.id);
  preferences.commit(world, [], undefined, [62, 76]);
  assert.equal(reads, 1);
  const old = preferences.controls();
  preferences.invalidate(true);
  runtime.update(null, []);
  await preferences.prepare(world.player.id);
  preferences.commit(world, [], undefined, [62, 76]);
  assert.equal(reads, 2);
  assert.throws(() => old.toggleMute(world.player.id, "music"), /superseded/);
  assert.throws(() => old.read(), /superseded/);
  preferences.controls().setPercent(world.player.id, "music", 31);
  await preferences.flush();
});

test("stale asynchronous loads are cancelled promptly and cannot overwrite a same-actor later entry", async () => {
  const first = Promise.withResolvers<string | null>(), second = Promise.withResolvers<string | null>();
  let reads = 0;
  const runtime = new FixturePreferenceRuntime(), world = audioFixtureWorld();
  const preferences = new PlayerAudioPreferences(new PlayerAudioPreferenceStore({
    read: () => ++reads === 1 ? first.promise : second.promise, write() {},
  }), runtime, () => {}, (error) => { throw error; });
  const stale = preferences.prepare(world.player.id);
  const rejected = assert.rejects(stale, (error: unknown) => error instanceof AppError && error.kind === "cancelled");
  await setImmediate();
  preferences.invalidate(true);
  await rejected;
  const current = preferences.prepare(world.player.id);
  second.resolve(serializeSourceAudioPreferences(audioFixtureRecord(83)));
  await current;
  first.resolve(serializeSourceAudioPreferences(audioFixtureRecord(7)));
  await setImmediate();
  preferences.commit(world, [], undefined, [62, 76]);
  assert.equal(preferences.read()!.preferences.volumes.current.music, 83);
  await preferences.flush();
});

test("a pending Skip cannot project or save into logout/re-entry with the same actor identity", async () => {
  const f = fixture(), skip = Promise.withResolvers<SourceMusicSkipResult>();
  f.runtime.skipResult = skip.promise;
  await f.preferences.prepare(f.world.player.id);
  f.preferences.commit(f.world, [], undefined, [62, 76]);
  const pending = f.preferences.controls().skip(f.world.player.id);
  const rejected = assert.rejects(pending, (error: unknown) => error instanceof AppError && error.kind === "cancelled");
  f.preferences.invalidate(true);
  f.runtime.update(null, []);
  await f.preferences.prepare(f.world.player.id);
  f.preferences.commit(f.world, [], undefined, [62, 76]);
  const projections = f.projections.length;
  skip.resolve({ status: "requested", previousGroup: 62, nextGroup: 76 });
  await rejected;
  assert.equal(f.projections.length, projections);
  await f.preferences.flush();
});

test("a late save receipt is fenced from a same-actor re-entry without cancelling the real ordered write", async () => {
  const started = Promise.withResolvers<void>(), release = Promise.withResolvers<void>();
  let hold = false, stored = serializeSourceAudioPreferences(audioFixtureRecord(21));
  const runtime = new FixturePreferenceRuntime(), world = audioFixtureWorld();
  const preferences = new PlayerAudioPreferences(new PlayerAudioPreferenceStore({
    read: () => stored,
    async write(_id, text) { if (hold) { started.resolve(); await release.promise; } stored = text; },
  }), runtime, () => {}, (error) => { throw error; });
  await preferences.prepare(world.player.id);
  preferences.commit(world, [], undefined, [62, 76]);
  await preferences.flush();
  hold = true;
  preferences.setPercent(world.player.id, "music", 44);
  const pending = preferences.persistCurrent(world.player.id);
  const rejected = assert.rejects(pending, /superseded/);
  await started.promise;
  preferences.invalidate(true);
  const reentry = preferences.prepare(world.player.id);
  assert.equal(preferences.observe().phase, "loading");
  hold = false; release.resolve();
  await rejected; await reentry;
  assert.equal(preferences.observe().save, "unchanged", "An old completion cannot mark this entry saved.");
  preferences.commit(world, [], undefined, [62, 76]);
  assert.equal(preferences.read()!.preferences.volumes.current.music, 44, "Re-entry reads the completed real save, not a stale record.");
  await preferences.flush();
});

test("save failures are visible and do not become success or endless observer retries", async () => {
  let writes = 0, fail = true;
  const errors: AppError[] = [], runtime = new FixturePreferenceRuntime(), world = audioFixtureWorld();
  const preferences = new PlayerAudioPreferences(new PlayerAudioPreferenceStore({
    read: () => serializeSourceAudioPreferences(audioFixtureRecord(21)),
    write() { writes++; if (fail) throw new Error("storage unavailable"); },
  }), runtime, () => {}, (error) => errors.push(error));
  await preferences.prepare(world.player.id);
  const binding = preferences.commit(world, [], undefined, [62, 76]);
  await assert.rejects(preferences.flush(), /save failed/);
  await setImmediate();
  assert.equal(preferences.observe().save, "failed");
  assert.equal(errors.length, 1);
  for (let index = 0; index < 10; index++) preferences.changed(binding);
  await setImmediate();
  assert.equal(writes, 1);
  fail = false;
  await preferences.persistCurrent(world.player.id);
  assert.equal(preferences.observe().save, "stored");
  assert.equal(preferences.observe().errorId, null);
});

test("a reconnect during an outstanding save obtains its own receipt instead of remaining pending forever", async () => {
  const started = Promise.withResolvers<void>(), release = Promise.withResolvers<void>();
  let hold = false, stored = serializeSourceAudioPreferences(audioFixtureRecord(21));
  const runtime = new FixturePreferenceRuntime(), world = audioFixtureWorld();
  const preferences = new PlayerAudioPreferences(new PlayerAudioPreferenceStore({
    read: () => stored,
    async write(_id, text) { if (hold) { started.resolve(); await release.promise; } stored = text; },
  }), runtime, () => {}, (error) => { throw error; });
  await preferences.prepare(world.player.id);
  preferences.commit(world, [], undefined, [62, 76]);
  await preferences.flush();
  hold = true;
  preferences.setPercent(world.player.id, "music", 31);
  await started.promise;
  preferences.invalidate(false);
  await preferences.prepare(world.player.id);
  preferences.commit(world, [], undefined, [62, 76]);
  assert.equal(preferences.observe().save, "pending");
  release.resolve(); hold = false;
  await preferences.flush();
  assert.equal(preferences.observe().save, "stored");
  assert.equal(deserializeSourceAudioPreferences(stored).volumes.current.music, 31);
});

test("a late native Skip rejection is classified as a superseded entry, not current-player feedback", async () => {
  const f = fixture(), skip = Promise.withResolvers<SourceMusicSkipResult>();
  f.runtime.skipResult = skip.promise;
  await f.preferences.prepare(f.world.player.id);
  f.preferences.commit(f.world, [], undefined, [62, 76]);
  const pending = f.preferences.skip(f.world.player.id);
  const rejected = assert.rejects(pending, (error: unknown) => error instanceof AppError && error.kind === "cancelled");
  f.preferences.invalidate(false);
  skip.reject(new AudioFailure("AUDIO_CONTROL_SUPERSEDED", "Fixture native epoch changed."));
  await rejected;
  await f.preferences.flush();
});

test("a genuine empty unlock list reaches native validation unchanged, not an invented default track", async () => {
  const runtime = new FixturePreferenceRuntime(), world = audioFixtureWorld();
  runtime.applying = () => { throw new AudioFailure("AUDIO_SOURCE_MUSIC", "No source-unlocked track is available in this area."); };
  const preferences = new PlayerAudioPreferences(new PlayerAudioPreferenceStore({ read: () => null, write() {} }),
    runtime, () => { throw new Error("No successful UI projection expected."); }, () => {});
  await preferences.prepare(world.player.id);
  assert.throws(() => preferences.commit(world, [], undefined, []), /No source-unlocked/);
  assert.deepEqual(runtime.calls.find((call) => call.kind === "apply")?.args[2], []);
  assert.equal(preferences.observe().phase, "failed");
  assert.equal(preferences.read(), null);
});
