import type { AudioSnapshot } from "../audio/index.ts";
import type { SourceAudioPreferenceBinding } from "../audio/preferences.ts";
import { sourceAudioDefaults, sourceSliderToMixer, sourceMixerToAssetGain } from "../audio/native-policy.ts";
import type { SourceAudioChannel } from "../audio/native-policy.ts";
import { SOURCE_PACK_SHA256 } from "../shared/contracts.ts";
import type { NativeWidget, UiCatalogue } from "./assets.ts";
import { widgetId } from "./layout.ts";

export type UiAudioChannel = SourceAudioChannel | "master";
export const AUDIO_CONTROLS = [
  { channel: "master", label: "Master Volume", track: 94, thumb: 95, mute: 96 },
  { channel: "music", label: "Music Volume", track: 108, thumb: 109, mute: 110 },
  { channel: "effects", label: "Sound Effects Volume", track: 122, thumb: 123, mute: 124 },
  { channel: "area", label: "Area Sounds Volume", track: 136, thumb: 137, mute: 138 },
] as const;

export interface UiAudioView {
  readonly percentages: Readonly<Record<UiAudioChannel, number>>;
  readonly mixer: Readonly<Record<SourceAudioChannel, number>>;
  readonly musicalVoices: readonly {
    assetId: string; sourceId: number; renderedNativeLevel: 128 | 255; appliedNativeLevel: number; calibrationGain: number;
  }[];
  readonly muted: boolean;
  readonly pendingGesture: boolean;
  readonly disposed: boolean;
  readonly outputEnabled: boolean;
  readonly enabled: boolean;
  readonly playingGroup: number | null;
  readonly plannedGroup: number | null;
  readonly preferences: SourceAudioPreferenceBinding | null;
}

export function observedAudio(snapshot: AudioSnapshot): { value: UiAudioView; problem: null } | { value: null; problem: string } {
  if (snapshot.sourcePackSha256 !== SOURCE_PACK_SHA256)
    return { value: null, problem: "The audio observer does not match the approved source pack." };
  if (snapshot.preferences === undefined)
    return { value: null, problem: "The audio observer omitted its player-preference binding state." };
  const percentages = { master: snapshot.masterPercent, music: Math.round(snapshot.volumes.music * 100),
    effects: Math.round(snapshot.volumes.effects * 100), area: Math.round(snapshot.volumes.area * 100) };
  if (Object.values(percentages).some(value => !Number.isInteger(value) || value < 0 || value > 100))
    return { value: null, problem: "The audio observer did not provide native integer slider positions." };
  for (const channel of ["music", "effects", "area"] as const) {
    const maximum = sourceSliderToMixer(channel, sourceAudioDefaults().sliders[channel]);
    if (!Number.isInteger(snapshot.nativeMixer[channel]) || snapshot.nativeMixer[channel] < 0 || snapshot.nativeMixer[channel] > maximum)
      return { value: null, problem: `The ${channel} mixer observation is outside the native range.` };
  }
  const enabled = snapshot.outputEnabled && snapshot.unlocked && !snapshot.pendingGesture && !snapshot.disposed &&
    !snapshot.muted && snapshot.contextState === "running";
  const voice = enabled ? snapshot.voices.find(voice => voice.kind === "music" && voice.when <= snapshot.currentTime) : undefined;
  const musicalVoices: UiAudioView["musicalVoices"][number][] = [];
  for (const voice of snapshot.voices.filter(voice => voice.kind === "music" || voice.kind === "jingle")) {
    if (voice.renderedNativeLevel !== 128 && voice.renderedNativeLevel !== 255)
      return { value: null, problem: "The audio observer did not identify the actual musical representation." };
    if (!Number.isInteger(voice.appliedNativeLevel) || voice.appliedNativeLevel < 0 || voice.appliedNativeLevel > 255)
      return { value: null, problem: "The audio observer provided an invalid applied native level." };
    musicalVoices.push({ assetId: voice.assetId, sourceId: voice.sourceId, renderedNativeLevel: voice.renderedNativeLevel,
      appliedNativeLevel: voice.appliedNativeLevel,
      calibrationGain: sourceMixerToAssetGain(voice.appliedNativeLevel, voice.renderedNativeLevel) });
  }
  return { problem: null, value: {
    percentages, mixer: { ...snapshot.nativeMixer }, musicalVoices,
    enabled, muted: snapshot.muted, pendingGesture: snapshot.pendingGesture, disposed: snapshot.disposed,
    outputEnabled: snapshot.outputEnabled, playingGroup: voice?.sourceId ?? null,
    plannedGroup: snapshot.background.groups[snapshot.background.cursor] ?? null,
    preferences: snapshot.preferences,
  } };
}

export function audioSliderPercent(x: number, width: number, grab = 0): number {
  if (!Number.isFinite(x) || !Number.isInteger(width) || width <= 16 || !Number.isFinite(grab))
    throw new Error("Invalid native slider coordinates.");
  const travel = Math.max(1, width - 16);
  return Math.trunc(Math.max(0, Math.min(travel, Math.trunc(x - grab))) * 100 / travel);
}

export function audioTooltip(view: UiAudioView, channel: UiAudioChannel): string {
  return `Adjust ${AUDIO_CONTROLS.find(control => control.channel === channel)!.label} (${view.percentages[channel]}%)`;
}

/** Source9234/9240/9246/9252 positions and9253 mute artwork; no mixer or gameplay simulation. */
export function projectAudioControls(catalogue: UiCatalogue, view: UiAudioView | null): NativeWidget[] {
  const source = catalogue.templates["native-audio-default"];
  if (!source) throw new Error("Original audio control geometry is missing.");
  let widgets = source.map(widget => ({ ...widget }));
  for (const control of AUDIO_CONTROLS) {
    const thumb = widgets.find(widget => widget.id === widgetId(116, control.thumb) && widget.index === -1)!;
    const track = widgets.find(widget => widget.id === widgetId(116, control.track) && widget.index === -1)!;
    if (!view) {
      widgets = widgets.filter(widget => widget !== thumb);
      continue;
    }
    const percent = view.percentages[control.channel];
    thumb.originalX = Math.trunc(percent * (track.width - thumb.width) / 100);
    thumb.x = track.x + thumb.originalX;
    thumb.sprite = control.channel !== "master" && view.percentages.master === 0 ? 4894 : 2860;
    const icon = widgets.find(widget => widget.id === widgetId(116, control.mute) && widget.index === -1)!;
    icon.actions = [percent === 0 ? "Unmute" : "Mute"];
    if (percent === 0) {
      const muted = catalogue.templates[`native-audio-${control.channel}-0`]!;
      const overlay = muted.find(widget => widget.id === icon.id && widget.index === 1)!;
      widgets.push({ ...overlay });
    }
  }
  return widgets.sort((a, b) => a.id - b.id || a.index - b.index);
}

export function audioSourceControl(widget: NativeWidget): typeof AUDIO_CONTROLS[number] | undefined {
  return widget.index === -1 && widget.id >> 16 === 116
    ? AUDIO_CONTROLS.find(control => widget.id === widgetId(116, control.track) || widget.id === widgetId(116, control.mute))
    : undefined;
}
