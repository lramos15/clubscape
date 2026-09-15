import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import {
  sourceAudioPreferenceDefaults, parseSourceAudioPreferences, parseSourceSavedPlaylist,
  sourceToggleVolumeMute, serializeSourceAudioPreferences, deserializeSourceAudioPreferences,
  sourceSavedPlaylistsFromVarps, sourceSavedPlaylistsToVarps, sourceVolumePreferencesFromVarps,
  sourceAudioPreferencesFromVarps, sourceSelectPlaylist, sourceEditPlaylist, sourcePreferenceUnlocks,
  validateSourcePreferenceUnlocks, SOURCE_STORED_TRACK_IDS, SOURCE_PLAYLIST_LABELS,
} from "./preferences.ts";
import type { SourceVolumePreferences } from "./preferences.ts";
import { sourceSliderToMixer } from "./native-policy.ts";

interface NativeVolumeCase {
  case: string;
  observed: { after_original_option_sync: { current: number[]; remembered: number[]; native_mixer: number[] } };
}
const evidence = JSON.parse(await readFile(new URL("../../research/browser-audio-policy/native-preference-controls.json", import.meta.url), "utf8"));
const channels = ["master", "music", "effects", "area"] as const;
const empty = () => Array<number | null>(100).fill(null);
const positions = (vector: readonly number[]) => ({
  master: vector[0]!, music: vector[1]!, effects: vector[2]!, area: vector[3]!,
});
const volumeCase = (name: string): NativeVolumeCase =>
  evidence.cases.find((item: { case: string }) => item.case === name);

test("new client records distinguish original effective defaults from first-use mute memory", () => {
  const value = sourceAudioPreferenceDefaults();
  assert.deepEqual(value.volumes.current, positions([100, 100, 100, 100]));
  assert.deepEqual(value.volumes.remembered, positions([0, 0, 0, 0]));
  assert.equal(value.music.mode, "area");
  assert.equal(value.music.rememberModeOnLogin, false);
  assert.equal(value.music.repeatInAreaShuffle, false);
  assert.equal(value.music.currentPlaylist, 0);
  for (const key of ["savedPlaylist1", "savedPlaylist2", "savedPlaylist3"] as const) {
    assert.deepEqual(value.music[key], empty());
  }
  assert.equal("unlockedGroups" in value.music, false);
  assert.equal("playerId" in value, false);
});

test("first-use and genuine saved Unmute match executed original9255 and native option sync", () => {
  for (const [name, memory] of [
    ["first-use-zero-memory", [0, 0, 0, 0]],
    ["restore-saved-vector", [37, 21, 66, 83]],
  ] as const) {
    let state: SourceVolumePreferences = { current: positions([0, 0, 0, 0]), remembered: positions(memory) };
    for (const channel of channels) state = sourceToggleVolumeMute(state, channel);
    const original = volumeCase(name).observed.after_original_option_sync;
    assert.deepEqual(state.current, positions(original.current));
    assert.deepEqual(state.remembered, positions(original.remembered));
    assert.deepEqual(["music", "effects", "area"].map((channel) =>
      sourceSliderToMixer(channel as "music" | "effects" | "area", state.current[channel as keyof typeof state.current], state.current.master)),
    original.native_mixer);
  }
});

test("Mute remembers each actual percentage, including native master-before-channel scaling", () => {
  let state: SourceVolumePreferences = { current: positions([37, 21, 66, 83]), remembered: positions([100, 20, 45, 25]) };
  for (const channel of channels) state = sourceToggleVolumeMute(state, channel);
  const native = volumeCase("mute-positive-vector").observed.after_original_option_sync;
  assert.deepEqual(state.current, positions(native.current));
  assert.deepEqual(state.remembered, positions(native.remembered));
  assert.deepEqual(native.native_mixer, [0, 0, 0]);
  for (const channel of channels) state = sourceToggleVolumeMute(state, channel);
  assert.deepEqual(state.current, positions([37, 21, 66, 83]));
});

test("preference serialization is versioned, detached, canonical and deeply immutable", () => {
  const external = structuredClone(sourceAudioPreferenceDefaults());
  const parsed = parseSourceAudioPreferences(external);
  assert.notEqual(parsed, external);
  assert.notEqual(parsed.volumes, external.volumes);
  assert.notEqual(parsed.music.savedPlaylist1, external.music.savedPlaylist1);
  assert.ok(Object.isFrozen(parsed.music.savedPlaylist1));
  assert.throws(() => Object.assign(parsed.volumes.current, { music: 0 }));
  const text = serializeSourceAudioPreferences(parsed);
  assert.equal(serializeSourceAudioPreferences(deserializeSourceAudioPreferences(text)), text);
  assert.deepEqual(deserializeSourceAudioPreferences(text), parsed);
  assert.ok(text.length < 16_384);
});

test("negative, corrupt, non-JSON, duplicate and unknown preference fields are rejected, not defaulted", () => {
  const valid = sourceAudioPreferenceDefaults();
  for (const value of [null, {}, [], { ...valid, version: 2 }, { ...valid, unlockedGroups: [62] },
    { ...valid, music: { ...valid.music, currentPlaylist: 4 } },
    { ...valid, music: { ...valid.music, mode: "single", selectedGroup: null } },
    { ...valid, music: { ...valid.music, selectedGroup: 0 } },
    { ...valid, music: { ...valid.music, savedPlaylist1: [] } },
    { ...valid, music: { ...valid.music, savedPlaylist2: Array(100) } },
  ]) assert.throws(() => parseSourceAudioPreferences(value));
  for (const bad of [-1, 101, 0.5, NaN, Infinity, "20", null]) {
    assert.throws(() => parseSourceAudioPreferences({ ...valid, volumes: { ...valid.volumes,
      current: { ...valid.volumes.current, music: bad } } }));
    assert.throws(() => parseSourceAudioPreferences({ ...valid, volumes: { ...valid.volumes,
      remembered: { ...valid.volumes.remembered, music: bad } } }));
  }
  const accessor = empty();
  Object.defineProperty(accessor, "0", { enumerable: true, get: () => 62 });
  assert.throws(() => parseSourceSavedPlaylist(accessor));
  const duplicate = empty(); duplicate[0] = 62; duplicate[3] = 62;
  assert.throws(() => parseSourceSavedPlaylist(duplicate));
  assert.throws(() => deserializeSourceAudioPreferences("{"));
  assert.throws(() => deserializeSourceAudioPreferences(" ".repeat(16_385)));
});

test("all three saved slots use original source track encodings, never group/row aliases", () => {
  for (const row of evidence.music_identities.tracks) {
    assert.equal(SOURCE_STORED_TRACK_IDS.get(row.group), row.stored_track_id);
    assert.equal(row.stored_track_id, row.unlock_fields[0] * 100 + row.unlock_fields[1]);
  }
  assert.deepEqual(SOURCE_PLAYLIST_LABELS, evidence.music_identities.playlist_menu.stringVals);
  const slots = empty();
  slots[1] = 327; slots[2] = 76; slots[99] = 62;
  const state = { savedPlaylist1: slots, savedPlaylist2: [...slots], savedPlaylist3: [...slots] };
  const encoded = sourceSavedPlaylistsToVarps(state);
  assert.equal(encoded.size, 150);
  for (const slot of [1, 2, 3]) {
    const native = evidence.cases.find((item: { case: string }) => item.case === `saved-playlist-${slot}-sparse-roundtrip`);
    const start = 5239 + (slot - 1) * 50;
    assert.deepEqual([encoded.get(start), encoded.get(start + 1), encoded.get(start + 49)], native.packed_words);
  }
  assert.deepEqual(sourceSavedPlaylistsFromVarps(encoded), state);
  const corrupt = new Map(encoded);
  corrupt.set(5239, 62); // Native saved62 is NOT index6/group62.
  assert.throws(() => sourceSavedPlaylistsFromVarps(corrupt));
  corrupt.delete(5239);
  assert.throws(() => sourceSavedPlaylistsFromVarps(corrupt));
});

test("saved playlist edits reuse the first native hole and never compact on removal", () => {
  const initial = empty(); initial[0] = 2; initial[2] = 76; initial[99] = 62;
  const added = sourceEditPlaylist(initial, { kind: "add", group: 327 });
  assert.equal(added[1], 327);
  assert.deepEqual(sourceEditPlaylist(added, { kind: "add", group: 327 }), added);
  const removed = sourceEditPlaylist(added, { kind: "remove", group: 2 });
  assert.deepEqual([removed[0], removed[1], removed[2], removed[99]], [null, 327, 76, 62]);
  assert.equal(initial[0], 2);
  assert.throws(() => sourceEditPlaylist(initial, { kind: "add", group: 65000 }));
});

test("stored and active playlists cannot grant locked or unpublished music", () => {
  const value = sourceAudioPreferenceDefaults();
  const slots = empty(); slots[0] = 62;
  const withTrack = parseSourceAudioPreferences({ ...value, music: { ...value.music, savedPlaylist3: slots } });
  assert.throws(() => validateSourcePreferenceUnlocks(withTrack, [76]));
  validateSourcePreferenceUnlocks(withTrack, [76, 62]);
  assert.deepEqual(sourcePreferenceUnlocks([76, 62]), sourcePreferenceUnlocks([62, 76]));
  assert.throws(() => sourcePreferenceUnlocks([62, 62]));
  assert.throws(() => sourcePreferenceUnlocks([65000]));
});

test("native14817/12426/12427/12428 readers preserve actual memory and require area override authority", () => {
  const source = new Map([[3796, 0], [168, 0], [169, 0], [872, 0], [5588, 0],
    [3797, 37], [3109, 21 | (66 << 8) | (83 << 15)]]);
  const actual = sourceVolumePreferencesFromVarps(source);
  assert.deepEqual(actual.remembered, positions([37, 21, 66, 83]));
  assert.deepEqual(actual.current, positions([0, 0, 0, 0]));
  source.set(5588, 1); source.set(5589, 19); source.delete(872);
  assert.equal(sourceVolumePreferencesFromVarps(source).current.area, 19);
  source.delete(5588);
  assert.throws(() => sourceVolumePreferencesFromVarps(source));
  source.set(5588, 0); source.set(872, 100); source.set(3797, 101);
  assert.throws(() => sourceVolumePreferencesFromVarps(source));
});

test("full native preference projection keeps source flags, row identity and all numbered slots separate", () => {
  assert.deepEqual(evidence.music_identities.area_mode_enums.map((entry: { definition: { id: number; keys: number[]; stringVals: string[] } }) =>
    [entry.definition.id, entry.definition.keys, entry.definition.stringVals]), [[684, [0, 1], ["Modern", "Classic"]]]);
  const value = sourceAudioPreferenceDefaults();
  const source = new Map(sourceSavedPlaylistsToVarps(value.music));
  for (const [id, number] of [[3796, 100], [168, 20], [169, 45], [872, 25], [5588, 0],
    [3797, 0], [3109, 0], [18, 2], [3883, 2777],
    [19, 1 | (1 << 10) | (3 << 14) | (1 << 21) | (1 << 23)]]) source.set(id!, number!);
  const result = sourceAudioPreferencesFromVarps(source);
  assert.equal(result.music.mode, "single");
  assert.equal(result.music.areaMode, "classic");
  assert.equal(result.music.selectedGroup, 76);
  assert.equal(result.music.currentPlaylist, 3);
  assert.equal(result.music.repeatInAreaShuffle, true);
  assert.equal(result.music.rememberModeOnLogin, true);
  assert.equal(result.music.keepPlayingOnPlaylistChange, true);
});

test("numbered selection follows source9297 including Single membership and keep-playing branches", () => {
  const original = sourceAudioPreferenceDefaults().music;
  const slots = empty(); slots[0] = 62;
  const area = { ...original, savedPlaylist1: slots };
  const actual = sourceSelectPlaylist(area, 1, 76);
  const native = evidence.cases.find((item: { case: string }) => item.case === "actual-numbered-menu-selection");
  assert.equal(native.after_mode, 1);
  assert.equal(actual.mode, "shuffle");
  assert.equal(actual.currentPlaylist, native.after_slot);
  const single = { ...area, mode: "single" as const, selectedGroup: 62 };
  assert.equal(sourceSelectPlaylist(single, 1, 62).mode, "single");
  assert.equal(sourceSelectPlaylist(single, 1, 76).mode, "shuffle");
  assert.equal(sourceSelectPlaylist(single, 1, null).mode, "single");
  assert.equal(sourceSelectPlaylist(single, 0, 62).mode, "area");
  const keep = { ...single, keepPlayingOnPlaylistChange: true };
  assert.equal(sourceSelectPlaylist(keep, 2, 62).mode, "single");
  assert.equal(sourceSelectPlaylist(keep, 0, 62).mode, "single");
});

test("the bounded source probe pins twelve states and preserves original Skip/jingle/mute boundaries", () => {
  assert.equal(evidence.result, "passed");
  assert.equal(evidence.controlled_native_states, 12);
  assert.equal(evidence.cases.length, 12);
  for (const mode of [0, 2, 1]) {
    const sample = evidence.cases.find((item: { case: string }) => item.case === `skip-mode-${mode}`);
    assert.equal(sample.primary_click_queued, mode === 1 ? 1 : 0);
    assert.equal(sample.nonprimary_click_queued, 0);
    assert.equal(sample.callback_changes_music, false);
  }
  const jingle = evidence.cases.find((item: { case: string }) => item.case === "background-request-during-jingle");
  assert.deepEqual(jingle.same_group_noop.transition, [0, 0, 0, 0]);
  assert.deepEqual(jingle.same_group_noop.remembered, [62]);
  assert.deepEqual(jingle.first.requested, [152]);
  assert.deepEqual(jingle.last.requested, [152]);
  assert.deepEqual(jingle.last.remembered, [327]);
  assert.deepEqual(jingle.last.transition, [0, 60, 60, 0]);
  const muted = evidence.cases.find((item: { case: string }) => item.case === "muted-shuffle-request");
  assert.equal(muted.native_mixer, 0);
  assert.equal(muted.next_selection_consumed, false);
  assert.equal(evidence.acceptance, false);
});
