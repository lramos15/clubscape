import type { AudioSnapshot } from "../../audio/index.ts";
import { createAudio, readAudioState, sourceSliderToMixer, setSourceMasterVolume,
  applySourceAudioPreferences, readSourceAudioPreferences } from "../../audio/index.ts";
import type { SourceMusicState } from "../../audio/index.ts";
import { SOURCE_PACK_SHA256 } from "../../shared/contracts.ts";
import { bindUiAudio, bindUiAudioPreferencePersistence, getUiMusicState, onUiMusicStateChange, setUiMusicState } from "../index.ts";
import type { UiAudioPreferencePersistence } from "../audio-preference-storage.ts";
import { observedAudio, projectAudioControls } from "../audio-controls.ts";
import { projectMusicControls } from "../music-controls.ts";
import { UiAssets } from "../assets.ts";
import { SourceRaster } from "../raster.ts";
import { paintNativeTree } from "../layout.ts";
import { MinimapPainter } from "../minimap.ts";
import { mount } from "./component-fixture.ts";
import { testAssets } from "./source-fixture.ts";

let disposeAudio: (() => Promise<void>) | null = null;

export async function mountAudio(phase: "world" | "title" = "world",
  preferences?: { persistence: UiAudioPreferencePersistence; unlockedGroups: readonly number[] }): Promise<void> {
  await disposeAudio?.();
  const component = await mount(phase);
  if (!component) throw new Error("Component mount failed.");
  if (phase === "world") component.services.enableUi();
  const record = preferences ? await preferences.persistence.load(component.services.state().world!.player.id) : null;
  const failures: Error[] = [];
  const handle = await createAudio({
    ...testAssets, baseUrl: location.origin,
    url: id => location.origin + "/audio-asset/" + encodeURIComponent(id),
  }, error => { failures.push(error); });
  const stop = await bindUiAudio(component.ui, handle);
  const stopPersistence = preferences ? bindUiAudioPreferencePersistence(component.ui, preferences.persistence) : null;
  component.services.audioVolume = (channel, value) => {
    component.services.calls.push({ method: "audioVolume", args: [channel, value] });
    handle.volume(channel, value);
  };
  component.services.unlockAudio = () => {
    component.services.calls.push({ method: "unlockAudio", args: [] });
    return handle.unlock();
  };
  if (phase === "world") {
    const world = component.services.state().world!;
    handle.update(world, []);
    if (preferences && record) {
      const binding = applySourceAudioPreferences(handle, world.player.id, record, preferences.unlockedGroups);
      setUiMusicState(component.ui, binding.playerId, binding.musicState);
    }
  }
  else handle.update(null, []);
  const musicChanges: Array<{ playerId: string; state: SourceMusicState }> = [];
  onUiMusicStateChange(component.ui, (playerId, state) => { musicChanges.push({ playerId, state }); });
  disposeAudio = async () => { stopPersistence?.(); await handle.dispose(); };
  Object.assign(window, { audioComponent: {
    handle, failures, state: () => readAudioState(handle), stop,
    master: (percent: number) => setSourceMasterVolume(handle, percent),
    music: (state: SourceMusicState, playerId = component.services.state().world!.player.id) => setUiMusicState(component.ui, playerId, state),
    musicState: () => getUiMusicState(component.ui), musicChanges,
    preferences: () => readSourceAudioPreferences(handle),
    dispose: () => handle.dispose(), uiDispose: () => component.ui.dispose(),
  } });
}

export function snapshotFor(percentages: readonly number[]): AudioSnapshot {
  if (percentages.length !== 4 || percentages.some(value => !Number.isInteger(value) || value < 0 || value > 100))
    throw new Error("Native audio fixture needs four explicit integer percentages.");
  const [master, music, effects, area] = [percentages[0]!, percentages[1]!, percentages[2]!, percentages[3]!];
  return {
    sourcePackSha256: SOURCE_PACK_SHA256, contextState: "suspended", currentTime: 0, sampleRate: 22050,
    pendingGesture: true, unlocked: false, muted: false, connected: false, disposed: false, outputEnabled: true,
    volumes: { music: music / 100, effects: effects / 100, area: area / 100 },
    nativeMixer: { music: sourceSliderToMixer("music", music, master), effects: sourceSliderToMixer("effects", effects, master),
      area: sourceSliderToMixer("area", area, master) },
    masterPercent: master, preferences: null, queueSize: 0, background: { groups: [], cursor: 0, mode: "once", exhausted: false, failed: false },
    voices: [], cache: { decodedBytes: 0, cached: 0, pending: 0 }, policyLimits: [], traces: [],
  };
}

let loaded: Promise<UiAssets> | null = null;
export async function audioProjection(percentages: readonly number[]): Promise<void> {
  const canvas = document.querySelector("canvas")!;
  canvas.width = 1920; canvas.height = 1080;
  const assets = await (loaded ??= UiAssets.load(testAssets, error => { throw error; }));
  const result = observedAudio(snapshotFor(percentages));
  if (!result.value) throw new Error(result.problem);
  const widgets = projectAudioControls(assets.catalogue, result.value);
  await assets.preloadItems(widgets.filter(widget => widget.item >= 0).map(widget => widget.item));
  await Promise.all(["ui/minimaps/compass.png", "ui/minimaps/3168-3168-0.png"].map(id => assets.require(id)));
  const raster = new SourceRaster(canvas, assets), minimap = new MinimapPainter(raster);
  paintNativeTree(raster, widgets, 1920, 1080, widget => widget.contentType === 1337 ||
    minimap.draw(widget, { x: 3222, y: 3218, plane: 0 }));
}

export async function musicProjection(name: string): Promise<void> {
  const canvas = document.querySelector("canvas")!;
  canvas.width = 1920; canvas.height = 1080;
  const assets = await (loaded ??= UiAssets.load(testAssets, error => { throw error; }));
  const source = assets.catalogue.templates[name]!;
  const unlocked = new Set(source.filter(widget => widget.id === 239 * 65536 + 11 && widget.type === 4 && widget.color === 0x0dc10d)
    .map(widget => widget.index));
  const mode = name === "native-music-mode-1" ? "shuffle" : name === "native-music-mode-2" ? "single" : "area";
  const state: SourceMusicState = { mode, areaMode: "modern",
    unlockedGroups: assets.catalogue.musicTracks.filter(track => unlocked.has(track.widgetIndex)).map(track => track.group),
    selectedGroup: null, playlistGroups: [], loopEnabled: true };
  const projected = projectMusicControls(assets.catalogue, state, null, 0, name === "native-music-filter-open").widgets;
  const widgets = [...source.filter(widget => widget.id >> 16 !== 239), ...projected.filter(widget => widget.id >> 16 === 239)];
  await assets.preloadItems(widgets.filter(widget => widget.item >= 0).map(widget => widget.item));
  await Promise.all(["ui/minimaps/compass.png", "ui/minimaps/3168-3168-0.png"].map(id => assets.require(id)));
  const raster = new SourceRaster(canvas, assets), minimap = new MinimapPainter(raster);
  paintNativeTree(raster, widgets, 1920, 1080, widget => widget.contentType === 1337 ||
    minimap.draw(widget, { x: 3222, y: 3218, plane: 0 }));
}
