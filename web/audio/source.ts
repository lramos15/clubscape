import type { ClientAssets, Tile } from "../shared/contracts.ts";
import { SOURCE_PACK_SHA256 } from "../shared/contracts.ts";
import { AudioFailure, integer, requireAudio } from "./errors.ts";

export const SOURCE_CYCLE_SECONDS = 0.02;
export const SOURCE_RATE = 22050;
export const APPROVED_PACK = "b62e19704e17d3d3e4e819f803ef49ba7cc54034ae407184b423427c65d9674d";
export const AUDIO_INPUTS = Object.freeze({
  manifest: {
    path: "assets/manifests/osrs/audio-runtime.json",
    sha256: "8e6d1eca465f720829599cae0e1c11b7700a9c7162e53dd5ddec36ceb89787aa",
    bytes: 845835,
  },
  map: {
    path: "research/audio-source/source-map.json",
    sha256: "1d79acd86552ee6d50801161f172e05a343861eeaae7eee768a5bf420c9eb271",
    bytes: 122699,
  },
  reference: {
    path: "research/reference-pack/v1/audio-reference.json",
    sha256: "e4d54300411c6fdffd09b2d5d1c2051d381f9d8a3931fce16e83cbee07f16764",
    bytes: 24348,
  },
});

export type AssetKind = "music" | "jingle" | "sfx";
interface SourceSignal {
  readonly id: string;
  readonly kind: AssetKind;
  readonly sourceId: number;
  readonly frames: number;
  readonly channels: number;
  readonly peak: number;
  readonly firstNonzeroFrame: number | null;
  readonly endFrame: number | null;
  readonly loopStart: number;
  readonly loopEnd: number;
  readonly inputGain: number;
}
export type SourceAsset = SourceSignal & (
  | { readonly playable: true; readonly path: string; readonly sha256: string; readonly bytes: number }
  | { readonly playable: false; readonly path: null; readonly sha256: null; readonly bytes: 0 }
);
export interface FrameCue {
  readonly sequence: number;
  readonly frame: number;
  readonly cycle: number;
  readonly sourceId: number;
  readonly repeats: number;
  readonly range: number;
  readonly retain: number;
  readonly weight: number;
}
export interface AmbientDefinition {
  readonly objectId: number;
  readonly sourceIds: readonly number[];
  readonly range: number;
  readonly retain: number;
  readonly fields: Readonly<Record<string, unknown>>;
}
export interface SourceCatalog {
  readonly assets: ReadonlyMap<string, SourceAsset>;
  readonly groups: ReadonlyMap<string, SourceAsset>;
  readonly sequences: ReadonlyMap<number, readonly FrameCue[]>;
  readonly ambient: ReadonlyMap<number, AmbientDefinition>;
}

function object(value: unknown): Record<string, any> {
  requireAudio(value !== null && typeof value === "object" && !Array.isArray(value),
    "AUDIO_MANIFEST", "Invalid source metadata object.");
  return value as Record<string, any>;
}

export function assetUrl(assets: ClientAssets, id: string): string {
  const url = new URL(assets.url(id), location.href);
  requireAudio(
    (url.protocol === "http:" || url.protocol === "https:") &&
      url.origin === location.origin && !url.username && !url.password && !url.hash,
    "AUDIO_ORIGIN", "Audio inputs must use the application's same-origin asset transport.",
  );
  return url.href;
}

export async function verifiedBytes(
  assets: ClientAssets,
  id: string,
  bytes: number,
  sha256: string,
  signal: AbortSignal,
): Promise<ArrayBuffer> {
  const response = await fetch(assetUrl(assets, id), {
    signal, redirect: "error", credentials: "same-origin", cache: "force-cache",
  });
  if (!response.ok) {
    await response.body?.cancel();
    throw new AudioFailure("AUDIO_NETWORK", `Audio input ${id} returned HTTP ${response.status}.`);
  }
  const size = response.headers.get("content-length");
  if (size !== null && Number(size) !== bytes) {
    await response.body?.cancel();
    throw new AudioFailure("AUDIO_BYTES", `Wrong length for ${id}.`);
  }
  requireAudio(response.body, "AUDIO_NETWORK", `Audio input ${id} has no response body.`);
  const reader = response.body.getReader();
  const output = new Uint8Array(bytes);
  let offset = 0;
  try {
    for (;;) {
      const chunk = await reader.read();
      if (chunk.done) break;
      requireAudio(offset + chunk.value.length <= bytes, "AUDIO_BYTES", `Oversized input ${id}.`);
      output.set(chunk.value, offset);
      offset += chunk.value.length;
    }
  } catch (error) {
    await reader.cancel().catch(() => undefined);
    throw error;
  } finally {
    reader.releaseLock();
  }
  requireAudio(offset === bytes, "AUDIO_BYTES", `Truncated input ${id}.`);
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", output));
  const actual = Array.from(digest, (v) => v.toString(16).padStart(2, "0")).join("");
  requireAudio(actual === sha256, "AUDIO_INTEGRITY", `Source hash mismatch for ${id}.`);
  return output.buffer;
}

async function documentInput(
  assets: ClientAssets, input: { path: string; bytes: number; sha256: string }, signal: AbortSignal,
): Promise<Record<string, any>> {
  const bytes = await verifiedBytes(assets, input.path, input.bytes, input.sha256, signal);
  return object(JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes)));
}

export async function loadCatalog(assets: ClientAssets, signal: AbortSignal): Promise<SourceCatalog> {
  requireAudio(SOURCE_PACK_SHA256 === APPROVED_PACK, "AUDIO_PACK", "The audio implementation requires approved pack v1.3.0.");
  const [manifest, map, reference] = await Promise.all([
    documentInput(assets, AUDIO_INPUTS.manifest, signal),
    documentInput(assets, AUDIO_INPUTS.map, signal),
    documentInput(assets, AUDIO_INPUTS.reference, signal),
  ]);
  requireAudio(manifest.settings.native_startup_percussion_bank === 128 &&
    manifest.settings.native_startup_percussion_channel === 9 &&
    manifest.assets.length === 264, "AUDIO_PACK", "Missing corrected native audio initialization.");
  const byId = new Map<string, SourceAsset>();
  const groups = new Map<string, SourceAsset>();
  const add = (asset: SourceAsset) => {
    requireAudio(!byId.has(asset.id) && !groups.has(`${asset.kind}:${asset.sourceId}`),
      "AUDIO_MANIFEST", "Duplicate source audio identity.");
    byId.set(asset.id, Object.freeze(asset));
    groups.set(`${asset.kind}:${asset.sourceId}`, asset);
  };
  for (const raw of manifest.assets) {
    const a = object(raw);
    requireAudio(["music", "jingle", "sfx"].includes(a.kind) &&
      integer(a.source_group, 0, 65534) &&
      a.asset_id === `asset.source.osrs.cache2695.audio-runtime.${a.kind}.${a.source_group}` &&
      a.path === `assets/source/osrs/audio-runtime/${a.kind}/${a.source_group}.flac` &&
      a.signal.sample_rate === SOURCE_RATE && a.source_decode_verified === true,
    "AUDIO_MANIFEST", "Invalid source audio asset identity or format.");
    add({
      id: a.asset_id, kind: a.kind, sourceId: a.source_group, path: a.path,
      sha256: a.sha256, bytes: a.size_bytes, frames: a.signal.frames, channels: a.signal.channels,
      peak: a.signal.peak, firstNonzeroFrame: a.signal.first_nonzero_frame,
      endFrame: a.kind === "sfx" ? null : a.loop.source_engine_end_frame,
      loopStart: a.loop.loop_start_frame ?? 0, loopEnd: a.loop.loop_end_frame ?? 0,
      inputGain: 1, playable: true,
    });
  }
  // These two approved original WAVs are NOT among the 264 FLACs. Match the
  // FLAC effects' reversible half-gain at the node, without modifying either file.
  for (const raw of reference.reference_templates) {
    const a = object(raw);
    requireAudio([710, 2693].includes(a.source_group) && a.source_index === 4 &&
      a.signal.sample_rate === SOURCE_RATE && a.copied_without_conversion === true,
    "AUDIO_MANIFEST", "Unexpected reference audio template.");
    add({
      id: a.id, kind: "sfx", sourceId: a.source_group, path: a.path,
      sha256: a.sha256, bytes: a.size_bytes, frames: a.signal.frames, channels: 1,
      peak: a.signal.peak, firstNonzeroFrame: null, endFrame: null,
      loopStart: 0, loopEnd: 0, inputGain: 0.5, playable: true,
    });
  }
  requireAudio(manifest.source_silences[0].source_group === 2411 &&
    manifest.source_silences[0].frames === 110,
  "AUDIO_MANIFEST", "The original weighted silence is missing.");
  add({
    id: manifest.source_silences[0].asset_id, kind: "sfx", sourceId: 2411,
    playable: false, path: null, sha256: null, bytes: 0, frames: 110, channels: 1, peak: 0,
    firstNonzeroFrame: null, endFrame: null, loopStart: 0, loopEnd: 0, inputGain: 1,
  });
  const sequences = new Map<number, FrameCue[]>();
  for (const raw of map.sequence_sound_events) {
    const a = object(raw);
    const cue: FrameCue = Object.freeze({
      sequence: a.sequence_id, frame: a.frame, cycle: a.source_frame_start_cycle_sum,
      sourceId: a.id, repeats: a.loops, range: a.location, retain: a.retain, weight: a.weight,
    });
    requireAudio(cue.sourceId === 2411 || groups.has(`sfx:${cue.sourceId}`),
      "AUDIO_MANIFEST", "A sequence references an unavailable sound.");
    const cues = sequences.get(cue.sequence) ?? [];
    cues.push(cue);
    sequences.set(cue.sequence, cues);
  }
  for (const [sequence, cues] of sequences) sequences.set(sequence, Object.freeze(cues) as FrameCue[]);
  const ambient = new Map<number, AmbientDefinition>();
  for (const raw of map.ambient_objects) {
    const a = object(raw);
    const fields = object(a.source_fields);
    const ids = [fields.ambientSoundId, ...(fields.ambientSoundIds ?? [])]
      .filter((id) => integer(id, 0, 65534));
    ambient.set(a.object_id, Object.freeze({
      objectId: a.object_id, sourceIds: Object.freeze(ids),
      range: fields.ambientSoundDistance, retain: fields.ambientSoundRetain,
      fields: Object.freeze({ ...fields }),
    }));
  }
  return { assets: byId, groups, sequences, ambient };
}

export function validTile(tile: unknown): tile is Tile {
  if (!tile || typeof tile !== "object") return false;
  const t = tile as Tile;
  return integer(t.x, 0, 16383) && integer(t.y, 0, 16383) && integer(t.plane, 0, 3);
}

export function regionalTrack(region: string): number | null {
  // Named journey scopes, not a made-up Modern-area polygon. In particular
  // 12595 (the mill extraction seed) is NOT Autumn Voyage's Classic square.
  return new Map([
    ["region.osrs.12336", 62], ["region.osrs.12592", 62],
    ["region.osrs.12436", 144], ["region.osrs.12850", 76],
    ["region.osrs.12851", 2],
  ]).get(region) ?? null;
}

export function selectWeighted(cues: readonly FrameCue[], roll: number): FrameCue {
  requireAudio(integer(roll, 0, 99) && cues.reduce((sum, cue) => sum + cue.weight, 0) === 100,
    "AUDIO_EVENT", "A source sound selection requires an unchanged 100-weight distribution.");
  let cumulative = 0;
  for (const cue of cues) {
    cumulative += cue.weight;
    if (roll < cumulative) return cue;
  }
  throw new AudioFailure("AUDIO_EVENT", "Invalid source sound distribution.");
}

export function sourceRandomBelow(bound: number): number {
  requireAudio(integer(bound, 1, 65536), "AUDIO_EVENT", "Invalid source random choice bound.");
  const value = new Uint32Array(1);
  const ceiling = Math.floor(4_294_967_296 / bound) * bound;
  do crypto.getRandomValues(value);
  while (value[0]! >= ceiling);
  return value[0]! % bound;
}

export function sourceRoll(): number { return sourceRandomBelow(100); }
