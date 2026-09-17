import { integer, requireAudio } from "./errors.ts";

export const NATIVE_AUDIO_RUNTIME_SHA256 = "7fdedf1194261cc5b99faa35e0d2b4e45b6d56665402ccbde7f3aa6207c3f947";
export type SourceAudioChannel = "music" | "effects" | "area";

// Actual dm.ae / dm.ab initializers, not a fitted logarithmic curve.
export const NATIVE_MUSIC_LEVELS = Object.freeze([
  0,1,1,1,1,1,1,1,1,2,2,2,2,2,2,2,3,3,3,3,3,3,4,4,4,4,4,4,4,5,5,5,
  5,5,5,6,6,6,6,6,7,7,7,7,7,8,8,8,8,8,9,9,9,9,10,10,10,10,10,11,11,11,12,12,
  12,13,13,13,14,14,14,15,15,15,16,16,16,17,17,17,18,18,18,19,19,19,20,20,20,21,21,22,22,23,23,24,
  24,25,25,26,27,27,28,28,29,29,30,30,31,32,32,33,34,34,35,35,36,37,37,38,38,39,40,40,41,42,42,43,
  44,44,45,46,46,47,48,49,49,50,51,52,53,53,54,56,56,57,58,59,60,61,62,63,64,65,66,67,68,69,71,71,
  72,74,75,76,77,78,79,81,81,82,84,85,86,88,89,90,92,93,94,96,97,98,100,101,103,105,106,107,109,111,112,114,
  116,117,119,122,123,125,127,129,130,131,132,134,136,139,141,142,145,147,148,151,153,155,158,159,162,164,166,169,170,173,175,178,
  180,182,185,187,190,193,194,197,200,202,205,208,210,213,216,217,219,221,224,227,230,233,236,240,243,246,248,250,252,255,255,255,
]);
export const NATIVE_EFFECT_LEVELS = Object.freeze([
  0,1,1,1,1,2,2,2,2,2,3,3,3,3,3,3,4,4,4,4,4,5,5,5,5,6,6,6,6,7,7,7,
  8,8,8,9,9,9,10,10,10,11,11,11,12,12,12,13,13,14,14,15,15,16,16,17,17,18,18,19,19,20,21,21,
  22,22,23,24,24,25,26,26,27,28,29,30,30,31,32,33,34,35,36,37,38,39,40,41,42,44,45,46,48,49,50,52,
  53,55,57,58,60,61,63,65,67,69,71,73,75,77,80,82,84,87,90,92,95,97,101,103,106,109,112,116,119,122,125,127,
]);

export interface SourceAudioDefaults {
  readonly sliders: Readonly<Record<SourceAudioChannel | "master", number>>;
  readonly rawNewPreferences: Readonly<Record<SourceAudioChannel, number>>;
  readonly mixer: Readonly<Record<SourceAudioChannel, number>>;
  readonly assetGain: Readonly<Record<SourceAudioChannel, number>>;
}

export function sourceSliderToMixer(channel: SourceAudioChannel, percent: number, masterPercent = 100): number {
  requireAudio(["music", "effects", "area"].includes(channel) &&
    integer(percent, 0, 100) && integer(masterPercent, 0, 100),
  "AUDIO_SOURCE_VOLUME", "Source sliders are integer percentages in [0,100].");
  const max = channel === "music" ? 255 : 127;
  const master = Math.fround(masterPercent / 100);
  const fraction = Math.fround(Math.fround(percent / 100) * master);
  const index = Math.min(max, Math.max(0, Math.floor(Math.fround(fraction * max) + 0.5)));
  return (channel === "music" ? NATIVE_MUSIC_LEVELS : NATIVE_EFFECT_LEVELS)[index]!;
}

export function sourceMixerToAssetGain(volume: number, renderedNativeLevel: 128 | 255 = 128): number {
  requireAudio(integer(volume, 0, 255) && (renderedNativeLevel === 128 || renderedNativeLevel === 255),
    "AUDIO_SOURCE_VOLUME", "Invalid native mixer level or original representation.");
  // Music was rendered at native 128. Effects' native device result is s16*V/256;
  // the published effects are already at half gain (including the WAV voice adapter).
  return volume / renderedNativeLevel;
}

export function sourceAudioDefaults(): SourceAudioDefaults {
  return Object.freeze({
    sliders: Object.freeze({ master: 100, music: 100, effects: 100, area: 100 }),
    rawNewPreferences: Object.freeze({ music: 127, effects: 127, area: 127 }),
    mixer: Object.freeze({ music: 255, effects: 127, area: 127 }),
    assetGain: Object.freeze({ music: 255 / 128, effects: 127 / 128, area: 127 / 128 }),
  });
}

export interface SourcePoint { readonly x: number; readonly y: number }
export interface SourceBounds { readonly minX: number; readonly minY: number; readonly maxX: number; readonly maxY: number }
export interface SourceSpatialResult {
  readonly distance: number;
  readonly radius: number;
  readonly retainedRadius: number;
  readonly volume: number;
  readonly audible: boolean;
}

function point(value: SourcePoint): void {
  requireAudio(value && Number.isFinite(value.x) && Number.isFinite(value.y) &&
    Math.abs(value.x) <= 2_097_152 && Math.abs(value.y) <= 2_097_152,
  "AUDIO_SOURCE_POSITION", "Expected bounded original coordinates (128 source units per tile).");
}
function spatialFields(radius: number, retain: number, volume: number): void {
  requireAudio(integer(radius, 0, 255) && integer(retain, 0, 255) && integer(volume, 0, 127),
    "AUDIO_SOURCE_POSITION", "Invalid native effect range, retention, or mixer volume.");
}

export function sourcePacketSpatial(
  listener: SourcePoint, emitter: SourcePoint, range: number, retain: number, areaVolume: number,
): SourceSpatialResult {
  point(listener);
  point(emitter);
  spatialFields(range, retain, areaVolume);
  const x = (Math.trunc(emitter.x) >> 7) * 128 + 64;
  const y = (Math.trunc(emitter.y) >> 7) * 128 + 64;
  const distance = Math.max(0, Math.abs(x - Math.trunc(listener.x)) + Math.abs(y - Math.trunc(listener.y)) - 128);
  const radius = range * 128;
  const retainedRadius = Math.max(0, ((retain & 31) - 1) * 128);
  let volume = 0;
  if (distance < radius) {
    const fraction = retainedRadius >= radius ? 1
      : Math.min(1, Math.max(0, Math.fround((radius - distance) / (radius - retainedRadius))));
    volume = Math.ceil(Math.fround(fraction * areaVolume));
  }
  return Object.freeze({ distance, radius, retainedRadius, volume, audible: volume > 0 });
}

export function sourceObjectBounds(
  tile: SourcePoint, sizeX: number, sizeY: number, orientation = 0,
): SourceBounds {
  requireAudio(integer(sizeX, 1, 255) && integer(sizeY, 1, 255) && integer(orientation, 0, 3),
    "AUDIO_SOURCE_POSITION", "Invalid original object footprint.");
  const width = (orientation & 1) === 0 ? sizeX : sizeY;
  const height = (orientation & 1) === 0 ? sizeY : sizeX;
  const bounds = { minX: tile.x * 128, minY: tile.y * 128, maxX: (tile.x + width) * 128, maxY: (tile.y + height) * 128 };
  point({ x: bounds.minX, y: bounds.minY });
  point({ x: bounds.maxX, y: bounds.maxY });
  return Object.freeze(bounds);
}

export function sourceAmbientSpatial(
  listener: SourcePoint, bounds: SourceBounds, range: number, retain: number, areaVolume: number,
): SourceSpatialResult {
  point(listener);
  point({ x: bounds.minX, y: bounds.minY });
  point({ x: bounds.maxX, y: bounds.maxY });
  spatialFields(range, retain, areaVolume);
  requireAudio(bounds.minX <= bounds.maxX && bounds.minY <= bounds.maxY,
    "AUDIO_SOURCE_POSITION", "Invalid source object bounds.");
  const x = Math.trunc(listener.x), y = Math.trunc(listener.y);
  const distance = Math.max(0,
    Math.max(0, bounds.minX - x, x - bounds.maxX) +
    Math.max(0, bounds.minY - y, y - bounds.maxY) - 64);
  const radius = range * 128, retainedRadius = retain * 128;
  let volume = 0;
  if (distance <= radius) {
    const fraction = retainedRadius >= radius ? 1 : Math.min(1, Math.max(0, (radius - distance) / (radius - retainedRadius)));
    volume = Math.ceil(fraction * areaVolume);
  }
  return Object.freeze({ distance, radius, retainedRadius, volume, audible: volume > 0 });
}

export interface SourceWorldOwner {
  readonly id: string;
  readonly exteriorPlane: number;
  readonly audibleInteriorPlane: number;
}

/** Exact rq.az rule. Admission of packet recipients is still authoritative server work. */
export function sourcePacketOwnerVisible(
  listener: SourceWorldOwner | null, emitter: SourceWorldOwner | null, crossOwner: boolean,
): boolean {
  return listener?.id === emitter?.id || emitter === null || (listener !== null && crossOwner);
}

/** Exact dz.aj plane/visibility rule; M1 ordinary main-world objects have null owners. */
export function sourceAmbientVisible(
  listenerPlane: number, emitterPlane: number,
  listenerOwner: SourceWorldOwner | null, emitterOwner: SourceWorldOwner | null, visibility: number,
): boolean {
  requireAudio(integer(listenerPlane, 0, 3) && integer(emitterPlane, 0, 3) &&
    integer(visibility, 0, 2), "AUDIO_SOURCE_POSITION", "Invalid native sound plane or visibility selector.");
  const same = listenerOwner?.id === emitterOwner?.id;
  if (same) {
    if (listenerPlane !== emitterPlane) return false;
  } else {
    if (listenerOwner && listenerPlane !== listenerOwner.audibleInteriorPlane) return false;
    if (emitterOwner && emitterPlane !== emitterOwner.audibleInteriorPlane) return false;
    if ((listenerOwner?.exteriorPlane ?? listenerPlane) !== (emitterOwner?.exteriorPlane ?? emitterPlane)) return false;
  }
  return visibility === 0 || same || (visibility === 2 && emitterOwner === null);
}

export function sourceAmbientFadeDuration(start: number, target: number, baseVolume: number, durationMs: number): number {
  requireAudio(integer(start, 0, 127) && integer(target, 0, 127) &&
    integer(baseVolume, 0, 127) && integer(durationMs, 0, 65535),
  "AUDIO_SOURCE_FADE", "Invalid source ambient fade.");
  // wc.af passes the SIGNED current-minus-target difference. In this original
  // build an increase therefore has a negative duration and completes immediately.
  const difference = start - target;
  return difference < baseVolume
    ? Math.trunc(Math.fround(durationMs * Math.fround(difference / baseVolume))) : durationMs;
}

export function sourceAmbientFadeVolume(start: number, target: number, durationMs: number, elapsedMs: number): number {
  requireAudio(integer(start, 0, 127) && integer(target, 0, 127) &&
    Number.isFinite(durationMs) && Number.isFinite(elapsedMs),
  "AUDIO_SOURCE_FADE", "Invalid native ambient fade position.");
  if (durationMs <= 0 || elapsedMs >= durationMs) return target;
  const progress = Math.max(0, elapsedMs / durationMs);
  return start <= target
    ? start + Math.trunc(progress * (target - start))
    : target + Math.trunc((start - target) * (1 - progress));
}

/** Original music task float32 accumulation followed by the native int-volume assignment. */
export function sourceMusicFade(volume: number, cycles: number, direction: "in" | "out"): readonly number[] {
  requireAudio(integer(volume, 0, 255) && integer(cycles, 0, 65535),
    "AUDIO_SOURCE_FADE", "Invalid native music fade.");
  let current = direction === "in" ? 0 : volume;
  const step = cycles === 0 ? volume : Math.fround(volume / cycles);
  const output: number[] = [current];
  for (let cycle = 0; cycle <= cycles; cycle++) {
    if ((direction === "in" && current >= volume) || (direction === "out" && current <= 0)) break;
    current = direction === "in"
      ? Math.min(volume, Math.fround(current + (step === 0 ? volume : step)))
      : Math.max(0, Math.fround(current - (step === 0 ? volume : step)));
    output.push(Math.trunc(current));
  }
  return Object.freeze(output);
}
