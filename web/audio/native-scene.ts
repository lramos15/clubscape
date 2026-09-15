import type { Tile } from "../shared/contracts.ts";
import { AudioFailure, integer, requireAudio } from "./errors.ts";
import { NATIVE_AUDIO_DATA } from "./native-data.ts";
import type { SourcePoint, SourceWorldOwner } from "./native-policy.ts";

export interface SourceObjectSound {
  readonly id: number;
  readonly range: number;
  readonly retain: number;
  readonly visibility: number;
  readonly distanceCurve: number;
  readonly fadeInCurve: number;
  readonly fadeInMs: number;
  readonly fadeOutCurve: number;
  readonly fadeOutMs: number;
}
export interface SourceObjectDefinition {
  readonly id: number;
  readonly sizeX: number;
  readonly sizeY: number;
  readonly varbit: number;
  readonly varp: number;
  readonly transforms: readonly number[] | null;
  readonly sound: SourceObjectSound | null;
  readonly random: { readonly ids: readonly number[] | null; readonly minCycles: number; readonly maxCycles: number } | null;
}
export interface SourceSceneEmitter {
  readonly id: string;
  readonly objectId: number;
  readonly tile: Tile;
  readonly orientation: number;
  readonly instance: string | null;
  readonly owner: SourceWorldOwner | null;
  readonly present: boolean;
}
export interface SourceAudioScene {
  /** The native audio listener point, in 128-unit local/world coordinates. */
  readonly listener: SourcePoint;
  readonly plane: number;
  readonly instance: string | null;
  readonly owner: SourceWorldOwner | null;
  /** Original varp values from the validated bridge, not guessed quest-stage integers. */
  readonly varps: ReadonlyMap<number, number>;
  /** Source placed objects, including audible scenery not represented as interactable entities. */
  readonly emitters: readonly SourceSceneEmitter[];
}

const definitions = new Map<number, SourceObjectDefinition>(
  (NATIVE_AUDIO_DATA.objects as readonly SourceObjectDefinition[]).map((definition) => [definition.id, definition]),
);
const varbits = new Map(NATIVE_AUDIO_DATA.varbits.map((entry) => [entry.id as number, entry]));

export function sourceObjectDefinition(id: number): SourceObjectDefinition | null {
  return definitions.get(id) ?? null;
}

export function resolveSourceObject(id: number, varps: ReadonlyMap<number, number>): SourceObjectDefinition | null {
  const source = definitions.get(id);
  requireAudio(source, "AUDIO_SOURCE_OBJECT", `No calibrated original audio object ${id}.`);
  if (source.transforms === null) return source;
  let index: number;
  if (source.varbit !== -1) {
    const bit = varbits.get(source.varbit);
    requireAudio(bit, "AUDIO_SOURCE_VARIABLE", `Missing original varbit definition ${source.varbit}.`);
    const value = varps.get(bit.varp);
    requireAudio(integer(value, -2147483648, 2147483647), "AUDIO_SOURCE_VARIABLE",
      `Supply authoritative source varp ${bit.varp} for object ${id}; no default is invented.`);
    const width = bit.msb - bit.lsb + 1;
    const mask = width === 32 ? -1 : (2 ** width) - 1;
    index = (value >> bit.lsb) & mask;
  } else {
    const value = varps.get(source.varp);
    requireAudio(integer(value, -2147483648, 2147483647), "AUDIO_SOURCE_VARIABLE",
      `Supply authoritative source varp ${source.varp} for object ${id}.`);
    index = value;
  }
  const selected = index >= 0 && index < source.transforms.length - 1
    ? source.transforms[index]! : source.transforms.at(-1)!;
  if (selected === -1) return null;
  const result = definitions.get(selected);
  requireAudio(result, "AUDIO_SOURCE_OBJECT", `Missing original transformed audio object ${selected}.`);
  return result;
}

export interface SourceMusicRow {
  readonly group: number;
  readonly row: number;
  readonly name: string;
  readonly durationTicks: number;
  readonly area: number | null;
  readonly defaultArea: boolean;
  readonly sourceSha256: string;
}
export const SOURCE_MUSIC_ROWS: ReadonlyMap<number, SourceMusicRow> = new Map(
  (NATIVE_AUDIO_DATA.music as readonly SourceMusicRow[]).map((row) => [row.group, row]),
);
export const SOURCE_LUMBRIDGE_GROUPS = Object.freeze([2, 64, 327, 163, 76, 145]);
export const SOURCE_SUPPLEMENT_M1_MUSIC = Object.freeze([64, 327, 163, 145]);
/** Kept for callers of the earlier calibration component; all four inputs are now published. */
export const SOURCE_UNPUBLISHED_M1_MUSIC: readonly number[] = Object.freeze([]);
export const SOURCE_MIX_REPRESENTATION_NEEDS = Object.freeze(
  NATIVE_AUDIO_DATA.nativeMixComparisons.filter((entry) => entry.additional_clip_samples > 0)
    .map((entry) => Object.freeze({ index: entry.index, group: entry.group, additionalClipsAt255: entry.additional_clip_samples })),
);
export type SourceMusicMode = "area" | "shuffle" | "single";
export const SOURCE_MUSIC_MODE_IDS = Object.freeze({ area: 0, shuffle: 1, single: 2 });
export const SOURCE_BACKGROUND_TRANSITION = Object.freeze({
  fadeOutDelayCycles: 0, fadeOutCycles: 60, fadeInDelayCycles: 60, fadeInCycles: 0,
});
export const SOURCE_TITLE_TRANSITION = Object.freeze({
  fadeOutDelayCycles: 0, fadeOutCycles: 0, fadeInDelayCycles: 0, fadeInCycles: 100,
});
export const SOURCE_JINGLE_TRANSITION = Object.freeze({
  fadeOutDelayCycles: 0, fadeOutCycles: 0, fadeInDelayCycles: 0, fadeInCycles: 0,
});
export type SourceMusicTransition = typeof SOURCE_BACKGROUND_TRANSITION | {
  readonly fadeOutDelayCycles: number; readonly fadeOutCycles: number;
  readonly fadeInDelayCycles: number; readonly fadeInCycles: number;
};
export interface SourceMusicRequest {
  readonly previousGroup: number;
  readonly mode: "area" | "single" | "playlist";
  readonly region: SourceMusicRegion | null;
  readonly durationTicks: number | null;
}
export interface SourceMusicSelection {
  readonly group: number;
  readonly transition?: SourceMusicTransition;
}
export type SourceMusicSelector = (request: SourceMusicRequest) => Promise<SourceMusicSelection>;
export interface SourceMusicState {
  readonly mode: "area" | "single" | "shuffle" | "playlist";
  readonly areaMode: "modern" | "classic";
  /** Source-unlocked groups for manual/shuffle selection; the audio component grants no unlocks. */
  readonly unlockedGroups: readonly number[];
  readonly selectedGroup: number | null;
  readonly playlistGroups: readonly number[];
  /** Legacy continuation directive, not native4137. Bound preferences apply Single's unconditional re-entry. */
  readonly loopEnabled: boolean;
}

function polygonContains(x: number, y: number, points: readonly (readonly number[])[]): boolean {
  let inside = false;
  for (let i = 0, j = points.length - 1; i < points.length; j = i++) {
    const a = points[i]!, b = points[j]!;
    const ax = a[0]!, ay = a[1]!, bx = b[0]!, by = b[1]!;
    const cross = (x - ax) * (by - ay) - (y - ay) * (bx - ax);
    if (cross === 0 && x >= Math.min(ax, bx) && x <= Math.max(ax, bx) &&
      y >= Math.min(ay, by) && y <= Math.max(ay, by)) return true;
    if ((ay > y) !== (by > y) && x < (bx - ax) * (y - ay) / (by - ay) + ax) inside = !inside;
  }
  return inside;
}

export interface SourceMusicRegion {
  readonly name: string;
  readonly areaId: number | null;
  readonly groups: readonly number[];
  readonly defaultGroup: number;
  readonly classification: "dated_public_source_geography_with_native_music_table_corroboration";
}

export function sourceMusicRegion(tile: Tile, areaMode: "modern" | "classic" = "modern"): SourceMusicRegion | null {
  requireAudio(integer(tile.x, 0, 16383) && integer(tile.y, 0, 16383) && integer(tile.plane, 0, 3),
    "AUDIO_SOURCE_REGION", "Invalid source music coordinate.");
  for (const area of NATIVE_AUDIO_DATA.geography) {
    if (!area.polygons.some((shape) => polygonContains(tile.x, tile.y, shape))) continue;
    let groups: readonly number[] = area.groups;
    let defaultGroup: number = area.groups[0]!;
    let areaId: number | null = null;
    if (area.name === "Lumbridge") {
      areaId = 1;
      defaultGroup = 76;
      if (areaMode === "classic") {
        if (tile.x >= 3200 && tile.x < 3264 && tile.y >= 3264 && tile.y < 3328) {
          groups = [2]; defaultGroup = 2;
        } else if (tile.x >= 3200 && tile.x < 3264 && tile.y >= 3200 && tile.y < 3264) groups = [76];
        else throw new AudioFailure("AUDIO_SOURCE_REGION",
          "This Classic square needs its exact source selection; it is not inferred from Modern membership.");
      }
    }
    return Object.freeze({
      name: area.name, areaId, groups: Object.freeze([...groups]), defaultGroup,
      classification: "dated_public_source_geography_with_native_music_table_corroboration",
    });
  }
  return null;
}

export function sourceMusicDurationSeconds(group: number): number | null {
  const row = SOURCE_MUSIC_ROWS.get(group);
  return row ? row.durationTicks * 0.6 : null;
}
