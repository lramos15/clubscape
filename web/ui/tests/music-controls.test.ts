import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import type { SourceMusicState } from "../../audio/native-scene.ts";
import { decodeUiCatalogue } from "../assets.ts";
import { musicRequest, musicScrollPosition, musicStateProblem, projectMusicControls } from "../music-controls.ts";
import { widgetId } from "../layout.ts";

const catalogue = decodeUiCatalogue(JSON.parse(readFileSync(resolve(import.meta.dirname, "../../../assets/compiled/ui/manifest.json"), "utf8")));
const state: SourceMusicState = Object.freeze({
  mode: "area", areaMode: "modern", unlockedGroups: Object.freeze([2, 64, 76, 145, 163, 327]),
  selectedGroup: null, playlistGroups: Object.freeze([64, 327]), loopEnabled: true,
});

test("native music rows bind source database/group/widget identities without label-based dispatch", () => {
  assert.equal(catalogue.musicTracks.length, 853);
  assert.equal(new Set(catalogue.musicTracks.map(row => row.group)).size, 853);
  const book = catalogue.musicTracks.find(row => row.group === 64)!;
  assert.equal(book.row, 2583);
  assert.equal(book.widgetIndex, 71);
  assert.equal(book.name, "Book of Spells");
  assert.equal(catalogue.musicTracks.find(row => row.group === 76)!.row, 2777);
});

test("music UI requests retain actual unlocks and do not guess a next track", () => {
  const single = musicRequest(state, { kind: "mode", mode: "single" }, 76);
  assert.ok(single.state);
  assert.equal(single.state.selectedGroup, 76);
  assert.deepEqual(single.state.unlockedGroups, state.unlockedGroups);
  assert.notEqual(musicRequest(state, { kind: "mode", mode: "single" }, null).problem, null);
  assert.equal(musicRequest(state, { kind: "mode", mode: "shuffle" }, 76).state!.selectedGroup, null);
  assert.equal(musicRequest(state, { kind: "play", group: 64 }, 76).state!.selectedGroup, 64);
  assert.notEqual(musicRequest(state, { kind: "play", group: 62 }, 76).problem, null);
});

test("current playlist and loop changes preserve source identities and reject invalid states", () => {
  assert.equal(musicRequest(state, { kind: "loop", enabled: false }, 76).state!.loopEnabled, false);
  assert.deepEqual(musicRequest(state, { kind: "add", group: 163 }, 76).state!.playlistGroups, [64, 327, 163]);
  assert.deepEqual(musicRequest(state, { kind: "remove", group: 64 }, 76).state!.playlistGroups, [327]);
  assert.notEqual(musicStateProblem({ ...state, playlistGroups: [62] }), null);
  assert.notEqual(musicStateProblem({ ...state, unlockedGroups: [64, 64] }), null);
  assert.notEqual(musicStateProblem({ ...state, mode: "playlist", playlistGroups: [] }), null);
  assert.deepEqual(state.playlistGroups, [64, 327]);
});

test("music projection replaces fixture unlock counts/colors and scrolls real source rows", () => {
  const projection = projectMusicControls(catalogue, state, 64, 600, false);
  const text = (child: number) => projection.widgets.find(widget => widget.id === widgetId(239, child) && widget.index === -1)!.text;
  assert.equal(text(4), "Book of Spells");
  assert.equal(text(5), "Unlocked: 6 / 853");
  const book = projection.widgets.find(widget => widget.id === widgetId(239, 11) && widget.index === 71)!;
  assert.equal(book.color, 0x0dc10d);
  assert.equal(projection.scroll, 600);
  const playlist = projectMusicControls(catalogue, { ...state, mode: "playlist" }, null, 0, false);
  assert.equal(playlist.widgets.filter(widget => widget.id === widgetId(239, 11) && widget.type === 4).length, 2);
  assert.equal(musicScrollPosition(16, 154, 10, 12647, 0), 0);
  assert.equal(musicScrollPosition(128, 154, 10, 12647, 0), 12647);
});
