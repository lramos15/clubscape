import type { AudioSnapshot } from "../../audio/index.ts";
import { createAudio, readAudioState, sourceSliderToMixer, setSourceMasterVolume } from "../../audio/index.ts";
import { SOURCE_PACK_SHA256 } from "../../shared/contracts.ts";
import { bindUiAudio } from "../index.ts";
import { observedAudio, projectAudioControls } from "../audio-controls.ts";
import { UiAssets } from "../assets.ts";
import { SourceRaster } from "../raster.ts";
import { paintNativeTree } from "../layout.ts";
import { MinimapPainter } from "../minimap.ts";
import { mount } from "./component-fixture.ts";
import { testAssets } from "./source-fixture.ts";

let disposeAudio: (() => Promise<void>) | null = null;

export async function mountAudio(phase: "world" | "title" = "world"): Promise<void> {
  await disposeAudio?.();
  const component = await mount(phase);
  if (!component) throw new Error("Component mount failed.");
  if (phase === "world") component.services.enableUi();
  const failures: Error[] = [];
  const handle = await createAudio({
    ...testAssets, baseUrl: location.origin,
    url: id => location.origin + "/audio-asset/" + encodeURIComponent(id),
  }, error => { failures.push(error); });
  const stop = await bindUiAudio(component.ui, handle);
  component.services.audioVolume = (channel, value) => {
    component.services.calls.push({ method: "audioVolume", args: [channel, value] });
    handle.volume(channel, value);
  };
  component.services.unlockAudio = () => {
    component.services.calls.push({ method: "unlockAudio", args: [] });
    return handle.unlock();
  };
  if (phase === "world") handle.update(component.services.state().world, []);
  else handle.update(null, []);
  disposeAudio = () => handle.dispose();
  Object.assign(window, { audioComponent: {
    handle, failures, state: () => readAudioState(handle), stop,
    master: (percent: number) => setSourceMasterVolume(handle, percent),
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
    masterPercent: master, queueSize: 0, background: { groups: [], cursor: 0, mode: "once", exhausted: false, failed: false },
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
