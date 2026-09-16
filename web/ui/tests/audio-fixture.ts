import type { AudioSnapshot } from "../../audio/index.ts";
import { createAudio, readAudioState, sourceSliderToMixer, setSourceMasterVolume,
  applySourceAudioPreferences, readSourceAudioPreferences } from "../../audio/index.ts";
import * as nativeAudio from "../../audio/index.ts";
import type { SourceMusicState } from "../../audio/index.ts";
import { SOURCE_PACK_SHA256 } from "../../shared/contracts.ts";
import { bindUiAudio, bindUiAudioPreferences, getUiMusicState, onUiMusicStateChange, setUiMusicState } from "../index.ts";
import { PlayerAudioPreferences } from "../../app/player-audio.ts";
import { PlayerAudioPreferenceStore } from "../../app/player-audio-store.ts";
import type { PlayerAudioStorage } from "../../app/player-audio-store.ts";
import type { WorldView } from "../../shared/contracts.ts";
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
  preferences?: { playerId: string; storage: PlayerAudioStorage; unlockedGroups: readonly number[] }): Promise<void> {
  await disposeAudio?.();
  const component = await mount(phase);
  if (!component) throw new Error("Component mount failed.");
  if (phase === "world") component.services.enableUi();
  if (preferences) {
    const state = component.services.state(), world = state.world;
    if (!world) throw new Error("Player preference fixtures require an explicit component world.");
    component.services.publish({ ...state, world: { ...world, player: { ...world.player, id: preferences.playerId } } });
  }
  const failures: Error[] = [];
  const handle = await createAudio({
    ...testAssets, baseUrl: location.origin,
    url: id => location.origin + "/audio-asset/" + encodeURIComponent(id),
  }, error => { failures.push(error); });
  const stop = await bindUiAudio(component.ui, handle);
  const store = preferences ? new PlayerAudioPreferenceStore(preferences.storage) : null;
  const manager = store ? new PlayerAudioPreferences(store, {
    update: (world, events) => handle.update(world, [...events]),
    disconnected: () => handle.disconnected(),
    preferences: {
      read: () => readSourceAudioPreferences(handle),
      apply: (player, value, unlocked) => applySourceAudioPreferences(handle, player, value, unlocked),
      music: (player, value) => nativeAudio.setSourceMusicPreferences(handle, player, value),
      playlist: (player, slot) => nativeAudio.selectSourcePlaylist(handle, player, slot),
      replace: (player, slot, entries) => nativeAudio.setSourceSavedPlaylist(handle, player, slot, entries),
      edit: (player, slot, edit) => nativeAudio.editSourceSavedPlaylist(handle, player, slot, edit),
      toggle: (player, channel) => nativeAudio.toggleSourceAudioMute(handle, player, channel),
      percent: (player, channel, value) => nativeAudio.setSourceAudioPercent(handle, player, channel, value),
      skip: (player) => nativeAudio.requestSourceMusicSkip(handle, player),
    },
  }, binding => setUiMusicState(component.ui, binding.playerId, binding.musicState),
  error => { failures.push(error); component.services.report(error, error.errorId); }) : null;
  let stopPreferences: (() => void) | null = null;
  const stopPreferencesObserver = manager ? nativeAudio.observeAudioState(handle, value => manager.changed(value.preferences)) : null;
  const enter = async (world: WorldView) => {
    if (!manager || !preferences) throw new Error("The actual preference manager is not part of this fixture.");
    stopPreferences?.(); manager.invalidate(true);
    await manager.prepare(world.player.id);
    stopPreferences = bindUiAudioPreferences(component.ui, manager.controls());
    manager.commit(world, [], undefined, preferences.unlockedGroups);
  };
  component.services.audioVolume = (channel, value) => {
    component.services.calls.push({ method: "audioVolume", args: [channel, value] });
    handle.volume(channel, value);
  };
  component.services.unlockAudio = () => {
    component.services.calls.push({ method: "unlockAudio", args: [] });
    return handle.unlock();
  };
  disposeAudio = async () => {
    stopPreferences?.(); stopPreferencesObserver?.(); stop();
    manager?.invalidate(true);
    try { await store?.flush(); }
    finally { await handle.dispose(); }
  };
  try {
    if (phase === "world") {
      const world = component.services.state().world!;
      if (manager) await enter(world);
      else handle.update(world, []);
    }
    else handle.update(null, []);
  } catch (error) {
    await disposeAudio();
    disposeAudio = null;
    throw error;
  }
  const musicChanges: Array<{ playerId: string; state: SourceMusicState }> = [];
  onUiMusicStateChange(component.ui, (playerId, state) => { musicChanges.push({ playerId, state }); });
  Object.assign(window, { audioComponent: {
    handle, failures, state: () => readAudioState(handle), stop,
    master: (percent: number) => setSourceMasterVolume(handle, percent),
    music: (state: SourceMusicState, playerId = component.services.state().world!.player.id) => setUiMusicState(component.ui, playerId, state),
    musicState: () => getUiMusicState(component.ui), musicChanges,
    preferences: () => readSourceAudioPreferences(handle),
    preferenceManager: manager, preferenceStore: store, enter,
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
