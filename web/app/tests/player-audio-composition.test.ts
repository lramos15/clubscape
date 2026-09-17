import assert from "node:assert/strict";
import { test } from "node:test";
import { setImmediate } from "node:timers/promises";
import { serializeSourceAudioPreferences } from "../../audio/index.ts";
import type { SourceAudioPreferenceBinding, SourceAudioScene } from "../../audio/index.ts";
import type { PlayerAudioControls } from "../player-audio.ts";
import type { PlayerAudioUi } from "../player-audio-composition.ts";
import { PlayerAudioComposition } from "../player-audio-composition.ts";
import { PlayerAudioPreferenceStore } from "../player-audio-store.ts";
import { AppError, deepFreeze } from "../errors.ts";
import { audioFixtureRecord, audioFixtureWorld, FixturePreferenceRuntime } from "./player-audio-fixture.ts";

function fixtureUi(runtime: FixturePreferenceRuntime) {
  const bindings: PlayerAudioControls[] = [], projects: SourceAudioPreferenceBinding[] = [];
  let observers = 0, controls = 0;
  const ui: PlayerAudioUi = {
    async bindAudio() { observers++; return () => { observers--; }; },
    bindPreferences(value) { controls++; bindings.push(value); return () => { controls--; }; },
    project(value) { projects.push(value); runtime.calls.push({ kind: "ui", args: [value] }); },
  };
  return { ui, bindings, projects, active: () => ({ observers, controls }) };
}
const store = () => new PlayerAudioPreferenceStore({
  read: () => serializeSourceAudioPreferences(audioFixtureRecord(21)), write() {},
});

test("composition loads the actual player record and binds UI before the coherent native/UI event batch", async () => {
  const runtime = new FixturePreferenceRuntime(), ui = fixtureUi(runtime);
  const load = Promise.withResolvers<string | null>(), world = audioFixtureWorld(), reported: AppError[] = [];
  const scene: SourceAudioScene = {
    listener: { x: 3094 * 128 + 12, y: 3107 * 128 + 93 }, plane: 0, instance: null,
    owner: null, emitters: [], varps: new Map([[281, 3]]),
  };
  const source = new PlayerAudioComposition(new PlayerAudioPreferenceStore({
    read: () => load.promise, write() {},
  }), runtime, ui.ui, { unlockedGroups: () => [62, 76], scene: () => scene }, (error) => reported.push(error));
  runtime.observe = (binding) => source.changed(binding);
  const ready = source.prepare(world);
  await setImmediate();
  assert.deepEqual(ui.active(), { observers: 0, controls: 0 });
  assert(!runtime.calls.some((call) => call.kind === "world"));
  load.resolve(serializeSourceAudioPreferences(audioFixtureRecord(21)));
  await ready;
  assert.deepEqual(ui.active(), { observers: 1, controls: 1 });
  assert.equal(ui.bindings[0]!.read(), null);
  assert(!runtime.calls.some((call) => call.kind === "world"));
  runtime.calls.push({ kind: "publish-authoritative-world", args: [world] });
  source.events(world, []);
  assert.deepEqual(runtime.calls.filter((call) => call.kind !== "disconnect").map((call) => call.kind),
    ["publish-authoritative-world", "world", "apply", "ui"]);
  assert.equal(runtime.calls.find((call) => call.kind === "world")!.args[2], scene);
  assert.equal(ui.bindings[0]!.read()?.playerId, world.player.id);
  assert.deepEqual(source.observe().appliedWorld, { playerId: world.player.id, revision: world.revision, tick: world.tick });
  assert.equal(source.observe().uiPreferencesBound, true);
  assert.equal(source.observe().preferences.origin, "stored");
  assert.deepEqual(reported, []);
  await source.dispose();
  assert.deepEqual(ui.active(), { observers: 0, controls: 0 });
});

test("missing authority and unadapted UI are explicit distinct gaps, never an empty-list or legacy-control success", async () => {
  const runtime = new FixturePreferenceRuntime(), world = audioFixtureWorld(), errors: AppError[] = [];
  const ui: PlayerAudioUi = {
    bindAudio: async () => { throw new Error("No world audio binding is allowed."); },
    project: () => { throw new Error("No successful music projection is allowed."); },
  };
  const source = new PlayerAudioComposition(store(), runtime, ui, {}, (error) => errors.push(error));
  await source.prepare(world);
  source.events(world, []);
  const status = source.observe();
  assert.equal(status.preferences.origin, "stored");
  assert.equal(status.preferences.phase, "waiting_unlocks");
  assert.equal(status.sourceUnlocksSupplied, false);
  assert.equal(status.uiPreferencesBound, false);
  assert.equal(status.appliedWorld, null);
  assert.deepEqual(status.issues.map((issue) => issue.errorId).sort(),
    ["audio.preferences.source_unlocks_required", "audio.preferences.ui_adapter_required"]);
  await source.prepare(world); source.events(world, []);
  assert.equal(errors.length, 2, "Repeated polls do not hide or repeatedly announce the same known gap.");
  assert(!runtime.calls.some((call) => call.kind === "world"));
  await source.dispose();
});

test("saved tracks, visible music/settings tabs and current world hints cannot reconstruct authority across restart", async () => {
  let reads = 0, writes = 0;
  const record = serializeSourceAudioPreferences(audioFixtureRecord(21));
  const preferences = new PlayerAudioPreferenceStore({
    read: () => { reads++; return record; }, write() { writes++; },
  });
  const initial = audioFixtureWorld();
  const later = deepFreeze({
    ...initial, revision: "9007199254740995", tick: "9007199254740996",
    player: {
      ...initial.player, region: "region.lumbridge", tile: { x: 3222, y: 3218, plane: 0 },
      tutorialStage: "stage.tutorial.complete", unlockedInterfaces: ["interface.music", "interface.settings"],
      quests: [{ id: "quest.cooks_assistant", name: "Fixture quest", stage: "stage.fixture.complete",
        journal: "Controlled authority-boundary fixture, not game progress.", completed: true }],
    },
  });
  for (const snapshots of [[initial, later], [later]]) {
    const runtime = new FixturePreferenceRuntime(), ui = fixtureUi(runtime), errors: AppError[] = [];
    const source = new PlayerAudioComposition(preferences, runtime, ui.ui, {}, (error) => errors.push(error));
    for (const world of snapshots) {
      await source.prepare(world);
      source.events(world, []);
      const status = source.observe();
      assert.equal(status.preferences.origin, "stored");
      assert.equal(status.preferences.phase, "waiting_unlocks");
      assert.equal(status.sourceUnlocksSupplied, false);
      assert.equal(status.appliedWorld, null);
      assert.deepEqual(status.issues.map((issue) => issue.errorId), ["audio.preferences.source_unlocks_required"]);
    }
    assert(!runtime.calls.some((call) => call.kind === "world" || call.kind === "apply"));
    assert.deepEqual(ui.active(), { observers: 0, controls: 0 }, "A supplied UI adapter cannot bypass missing source facts.");
    assert.equal(errors.length, 1);
    await source.dispose();
  }
  assert.equal(reads, 2, "A fresh composition reloads only the genuine client-preference record.");
  assert.equal(writes, 0, "Visited tiles, quest hints and saved tracks never become client-owned unlock/history/varp storage.");
});

test("an explicitly unavailable authority producer cannot fall back to a legacy supplied music selection", async () => {
  const runtime = new FixturePreferenceRuntime(), world = audioFixtureWorld(), ui = fixtureUi(runtime);
  let legacyReads = 0, sceneReads = 0;
  const source = new PlayerAudioComposition(store(), runtime, ui.ui, {
    unlockedGroups: () => undefined,
    music: () => {
      legacyReads++;
      return { mode: "single", areaMode: "classic", unlockedGroups: [62, 76],
        selectedGroup: 76, playlistGroups: [76], loopEnabled: true };
    },
    scene: () => { sceneReads++; return undefined; },
  }, () => {});
  await source.prepare(world);
  source.events(world, []);
  assert.equal(source.observe().preferences.phase, "waiting_unlocks");
  assert.equal(source.observe().sourceUnlocksSupplied, false);
  assert.equal(legacyReads, 0);
  assert.equal(sceneReads, 0);
  assert(!runtime.calls.some((call) => call.kind === "world" || call.kind === "apply"));
  await source.dispose();
});

test("valid authority cannot activate the UI's old mute/playlist implementation without its real preference adapter", async () => {
  const runtime = new FixturePreferenceRuntime(), world = audioFixtureWorld();
  const source = new PlayerAudioComposition(store(), runtime, {
    bindAudio: async () => { throw new Error("Legacy audio controls must stay detached."); }, project() {},
  }, { unlockedGroups: () => [62, 76] }, () => {});
  await source.prepare(world); source.events(world, []);
  assert.equal(source.observe().sourceUnlocksSupplied, true);
  assert.equal(source.observe().preferences.phase, "failed");
  assert.equal(source.observe().preferences.errorId, "audio.preferences.ui_adapter_required");
  assert(!runtime.calls.some((call) => call.kind === "world"));
  await source.dispose();
});

test("the real complete source authority drives native binding without an inferred unlock producer", async () => {
  const runtime = new FixturePreferenceRuntime(), ui = fixtureUi(runtime), base = audioFixtureWorld();
  const world = {
    ...base,
    audioAuthority: {
      version: 1 as const, profile: "source.audio.fixture",
      music: {
        history: "from_creation" as const, trackedFromTick: "0", revision: "2", complete: true,
        unlockedGroups: [62, 76],
        tracks: [62, 76].map((group) => ({ group, status: "unlocked" as const, confirmedAtTick: "0", rule: `source.${group}` })),
      },
      varps: [],
    },
  };
  const source = new PlayerAudioComposition(store(), runtime, ui.ui, {}, (error) => { throw error; });
  await source.prepare(world);
  source.events(world, []);
  assert.equal(source.observe().preferences.phase, "ready");
  assert.equal(source.observe().sourceUnlocksSupplied, true);
  assert.equal(source.observe().sourceAuthority, "complete");
  assert.deepEqual(runtime.binding?.unlockedGroups, [62, 76]);
  await source.dispose();
});

test("unknown legacy source history cannot use client selection or a legacy producer as an unlock grant", async () => {
  const runtime = new FixturePreferenceRuntime(), ui = fixtureUi(runtime), base = audioFixtureWorld();
  const errors: AppError[] = [];
  const world = {
    ...base,
    audioAuthority: {
      version: 1 as const, profile: "source.audio.fixture",
      music: {
        history: "legacy_untracked" as const, trackedFromTick: "20", revision: "1", complete: false,
        unlockedGroups: [62],
        tracks: [
          { group: 62, status: "unlocked" as const, confirmedAtTick: "20", rule: "source.62" },
          { group: 76, status: "unknown" as const, confirmedAtTick: null, rule: null },
        ],
      },
      varps: [],
    },
  };
  const source = new PlayerAudioComposition(store(), runtime, ui.ui, { unlockedGroups: () => [62, 76] }, (error) => errors.push(error));
  await source.prepare(world); source.events(world, []);
  assert.equal(source.observe().preferences.errorId, "audio.authority.history_unknown");
  assert.equal(source.observe().sourceAuthority, "partial_history");
  assert.equal(world.audioAuthority.music.tracks[1]!.status, "unknown");
  assert(!runtime.calls.some((call) => call.kind === "world" || call.kind === "apply"));
  assert.equal(errors.length, 1);
  await source.dispose();
});

test("the earlier source-music producer supplies only actual unlocks, never guessed saved slots or native flags", async () => {
  const runtime = new FixturePreferenceRuntime(), world = audioFixtureWorld(), ui = fixtureUi(runtime);
  const record = audioFixtureRecord(21);
  const source = new PlayerAudioComposition(store(), runtime, ui.ui, {
    music: () => ({ mode: "single", areaMode: "classic", unlockedGroups: [62, 76],
      selectedGroup: 76, playlistGroups: [76], loopEnabled: true }),
  }, (error) => { throw error; });
  await source.prepare(world); source.events(world, []);
  const args = runtime.calls.find((call) => call.kind === "apply")!.args;
  assert.deepEqual(args, [world.player.id, record, [62, 76]]);
  assert.equal(source.controls().read()!.preferences.music.currentPlaylist, 0);
  assert.equal(source.controls().read()!.preferences.music.mode, "area");
  await source.dispose();
});

test("failed or corrupt storage does not feed even a single world update or apply new-record defaults", async () => {
  for (const bad of ["read_failed", "corrupt"]) {
    const runtime = new FixturePreferenceRuntime(), ui = fixtureUi(runtime), errors: AppError[] = [];
    const source = new PlayerAudioComposition(new PlayerAudioPreferenceStore({
      read: () => { if (bad === "read_failed") throw new Error("private-read-detail"); return '{"private-invalid-record"'; },
      write() { throw new Error("No record rewrite is permitted."); },
    }), runtime, ui.ui, { unlockedGroups: () => [62, 76] }, (error) => errors.push(error));
    await source.prepare(audioFixtureWorld());
    source.events(audioFixtureWorld(), []);
    assert(!runtime.calls.some((call) => call.kind === "world" || call.kind === "apply"));
    assert.equal(source.observe().preferences.phase, "failed");
    assert.equal(source.observe().preferences.origin, null);
    assert.equal(errors.length, 1);
    assert(!errors[0]!.message.includes("private"));
    await source.dispose();
  }
});

test("late native UI binding is cleaned before re-entry binding and cannot block requested logout indefinitely", async () => {
  const runtime = new FixturePreferenceRuntime(), world = audioFixtureWorld();
  const firstStarted = Promise.withResolvers<void>(), release = Promise.withResolvers<void>();
  let binds = 0, active = 0, max = 0;
  const source = new PlayerAudioComposition(store(), runtime, {
    async bindAudio() {
      const id = ++binds;
      if (id === 1) { firstStarted.resolve(); await release.promise; }
      active++; max = Math.max(max, active);
      return () => { active--; };
    },
    bindPreferences: () => () => {}, project() {},
  }, { unlockedGroups: () => [62, 76] }, (error) => { throw error; });
  const old = source.prepare(world);
  const rejected = assert.rejects(old, (error: unknown) => error instanceof AppError && error.kind === "cancelled");
  await firstStarted.promise;
  source.disconnected();
  await rejected;
  const current = source.prepare(world);
  await setImmediate();
  assert.equal(binds, 1, "A new observer does not race the still-pending old bind.");
  release.resolve();
  await current;
  source.events(world, []);
  assert.equal(binds, 2);
  assert.equal(active, 1); assert.equal(max, 1);
  assert.equal(source.observe().preferences.phase, "ready");
  await source.dispose();
  assert.equal(active, 0);
});

test("title clears the native binding and same-actor re-entry rejects the previous UI control object", async () => {
  const runtime = new FixturePreferenceRuntime(), world = audioFixtureWorld(), ui = fixtureUi(runtime);
  const source = new PlayerAudioComposition(store(), runtime, ui.ui,
    { unlockedGroups: () => [62, 76] }, (error) => { throw error; });
  await source.prepare(world); source.events(world, []);
  const previous = ui.bindings[0]!;
  const title = source.title();
  assert.equal(runtime.binding, null, "The genuine title reset is synchronous, before any binding await.");
  assert.equal(source.observe().preferences.playerId, null);
  await title;
  assert.deepEqual(ui.active(), { observers: 1, controls: 0 });
  await source.prepare(world); source.events(world, []);
  assert.deepEqual(ui.active(), { observers: 1, controls: 1 });
  assert.throws(() => previous.setPercent(world.player.id, "music", 17), /superseded/);
  ui.bindings.at(-1)!.setPercent(world.player.id, "music", 19);
  await source.dispose();
});

test("source inputs are tied to the exact immutable snapshot and never reused for another event batch", async () => {
  const runtime = new FixturePreferenceRuntime(), ui = fixtureUi(runtime), errors: AppError[] = [];
  const source = new PlayerAudioComposition(store(), runtime, ui.ui,
    { unlockedGroups: () => [62, 76] }, (error) => errors.push(error));
  await source.prepare(audioFixtureWorld());
  source.events(audioFixtureWorld(), []);
  assert.equal(source.observe().appliedWorld, null);
  assert(errors.some((error) => error.errorId === "audio.preferences.world_superseded"));
  assert(!runtime.calls.some((call) => call.kind === "world"));
  assert.deepEqual(ui.active(), { observers: 0, controls: 0 });
  await source.dispose();
});
