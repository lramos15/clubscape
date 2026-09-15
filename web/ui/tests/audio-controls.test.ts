import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { decodeUiCatalogue } from "../assets.ts";
import { audioSliderPercent, audioTooltip, observedAudio, projectAudioControls } from "../audio-controls.ts";
import { snapshotFor } from "./audio-fixture.ts";
import { nativeTree, widgetId } from "../layout.ts";

const catalogue = decodeUiCatalogue(JSON.parse(readFileSync(resolve(import.meta.dirname, "../../../assets/compiled/ui/manifest.json"), "utf8")));

test("UI source percentages retain the observed nonlinear mixer, never invert it into a linear slider", () => {
  const result = observedAudio(snapshotFor([100, 50, 100, 100]));
  assert.ok(result.value);
  assert.equal(result.value.percentages.music, 50);
  assert.equal(result.value.mixer.music, 44);
  assert.deepEqual(result.value.musicalVoices, []);
  assert.equal(audioTooltip(result.value, "music"), "Adjust Music Volume (50%)");
  const master = observedAudio(snapshotFor([50, 100, 100, 100])).value!;
  assert.deepEqual(master.mixer, { music: 44, effects: 22, area: 22 });
});

test("musical observations preserve native255 denominators and distinguish configured from applied levels", () => {
  const snapshot = snapshotFor([100, 100, 100, 100]);
  const result = observedAudio({ ...snapshot, voices: [
    { id: 1, sourceId: 64, kind: "music", channel: "music", eventId: "native255", when: 0, gain: 1, loop: false, loopEnd: 0,
      assetId: "asset.source.osrs.cache2695.audio-supplement.music.64.native255", renderedNativeLevel: 255, appliedNativeLevel: 255 },
    { id: 2, sourceId: 40, kind: "jingle", channel: "music", eventId: "safe128", when: 0, gain: 44 / 128, loop: false, loopEnd: 0,
      assetId: "asset.source.osrs.cache2695.audio-runtime.jingle.40", renderedNativeLevel: 128, appliedNativeLevel: 44 },
  ] });
  assert.ok(result.value);
  assert.equal(result.value.mixer.music, 255);
  assert.equal(result.value.musicalVoices[0]!.calibrationGain, 1);
  assert.equal(result.value.musicalVoices[1]!.calibrationGain, 44 / 128);
  assert.equal(result.value.musicalVoices[1]!.appliedNativeLevel, 44);
});
test("source audio observation distinguishes real permission/output and accepts intermediate master refreshes", () => {
  const snapshot = snapshotFor([100, 100, 100, 100]);
  assert.equal(observedAudio({ ...snapshot, contextState: "running" }).value!.enabled, false);
  assert.equal(observedAudio({ ...snapshot, contextState: "running", unlocked: true, pendingGesture: false }).value!.enabled, true);
  assert.equal(observedAudio({ ...snapshot, contextState: "running", unlocked: true, pendingGesture: false, outputEnabled: false }).value!.enabled, false);
  assert.equal(observedAudio({ ...snapshot, masterPercent: 50 }).problem, null);
  assert.notEqual(observedAudio({ ...snapshot, masterPercent: 101 }).problem, null);
  assert.notEqual(observedAudio({ ...snapshot, nativeMixer: { ...snapshot.nativeMixer, music: 256 } }).problem, null);
});

test("native slider hit conversion uses floor scaling and retains the original thumb grab offset", () => {
  assert.equal(audioSliderPercent(48, 112), 50);
  assert.equal(audioSliderPercent(56, 112, 8), 50);
  assert.equal(audioSliderPercent(56, 112), 58);
  assert.equal(audioSliderPercent(3.9, 112), 3);
  assert.equal(audioSliderPercent(-200, 112), 0);
  assert.equal(audioSliderPercent(300, 112), 100);
  assert.throws(() => audioSliderPercent(NaN, 112), /Invalid native slider/);
});

test("source audio projection follows exact thumb/mute geometry and never invents unobserved positions", () => {
  const snapshot = observedAudio(snapshotFor([0, 50, 75, 25])).value!;
  const widgets = projectAudioControls(catalogue, snapshot);
  const master = widgets.find(widget => widget.id === widgetId(116, 95))!;
  const music = widgets.find(widget => widget.id === widgetId(116, 109))!;
  assert.equal(master.x, 1757);
  assert.equal(music.x, 1805);
  assert.equal(music.sprite, 4894);
  assert.ok(widgets.some(widget => widget.id === widgetId(116, 96) && widget.index === 1 && widget.sprite === 7426));
  const unknown = projectAudioControls(catalogue, null);
  assert.ok(!unknown.some(widget => [95, 109, 123, 137].includes(widget.id & 65535) && widget.id >> 16 === 116));
});

test("native audio controls remain unclipped within supported Classic HUD anchors", () => {
  const widgets = projectAudioControls(catalogue, observedAudio(snapshotFor([37, 21, 66, 83])).value);
  for (const [width, height] of [[1024, 768], [1920, 1080], [2560, 1440]] as const) {
    const tree = nativeTree(widgets, width, height);
    for (const child of [94, 108, 122, 136]) {
      const track = tree.find(widget => widget.id === widgetId(116, child) && widget.index === -1)!;
      assert.ok(track.x >= track.clip.x && track.y >= track.clip.y);
      assert.ok(track.x + track.width <= track.clip.x + track.clip.width);
      assert.ok(track.y + track.height <= track.clip.y + track.clip.height);
      assert.equal(track.width, 112);
    }
  }
});
