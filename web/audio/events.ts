import type { AudioEvent } from "../shared/contracts.ts";
import { integer, requireAudio } from "./errors.ts";
import { validTile } from "./source.ts";

export const EVENT_KINDS = new Set([
  "sound", "animation", "music", "jingle", "quest_complete", "level_up", "interface_closed",
]);
const COMMON_FIELDS = ["committed", "actionId", "cueId"];
const SPATIAL_FIELDS = ["sourceGain", "sourceDistance", "range", "retain", "instance"];
const PAYLOAD_FIELDS: Record<AudioEvent["kind"], ReadonlySet<string>> = {
  sound: new Set([...COMMON_FIELDS, ...SPATIAL_FIELDS, "phase", "delayCycles", "repeatCount",
    "selector", "binding", "ambient", "objectId", "active", "sequenceId", "frame", "iteration", "itemId", "weightRoll"]),
  animation: new Set([...COMMON_FIELDS, ...SPATIAL_FIELDS, "phase", "frame", "iteration", "itemId", "weightRoll"]),
  music: new Set([...COMMON_FIELDS, "mode", "unlocked", "boundary", "playlist", "shuffle"]),
  jingle: new Set([...COMMON_FIELDS, "auxiliary", "causeQuestId"]),
  level_up: new Set([...COMMON_FIELDS, "auxiliary", "causeQuestId", "level"]),
  quest_complete: new Set([...COMMON_FIELDS, "questId"]),
  interface_closed: new Set([...COMMON_FIELDS, "questId", "completionId"]),
};

export function stableId(value: unknown): value is string {
  return typeof value === "string" && /^[A-Za-z0-9._:/@-]{1,256}$/.test(value);
}

export function validateEvent(input: AudioEvent): AudioEvent {
  requireAudio(input && typeof input === "object" && stableId(input.id) &&
    EVENT_KINDS.has(input.kind) &&
    (input.sourceId === null || integer(input.sourceId, -1, 65534)) &&
    (input.assetId === null || stableId(input.assetId)) &&
    (input.actorId === null || stableId(input.actorId)) &&
    (input.tile === null || validTile(input.tile)) &&
    (input.sourceCycle === null || integer(input.sourceCycle, 0, Number.MAX_SAFE_INTEGER)),
  "AUDIO_EVENT", "Invalid authoritative audio event envelope.");
  const payload = input.payload;
  requireAudio(payload && typeof payload === "object" && !Array.isArray(payload) &&
    [Object.prototype, null].includes(Object.getPrototypeOf(payload)) &&
    Object.keys(payload).length <= 32,
  "AUDIO_EVENT", "Invalid audio payload.");
  for (const [key, value] of Object.entries(payload)) {
    requireAudio(PAYLOAD_FIELDS[input.kind].has(key) && /^[A-Za-z][A-Za-z0-9_]{0,63}$/.test(key) &&
      (typeof value === "boolean" ||
        (typeof value === "string" && value.length <= 4096) ||
        (typeof value === "number" && Number.isFinite(value))),
    "AUDIO_EVENT", "Audio payload fields must be supported, bounded immutable scalars; unknown gain/fade/offset/priority controls are not silently ignored.");
  }
  requireAudio(payload.auxiliary === undefined || integer(payload.auxiliary, -2147483648, 2147483647),
    "AUDIO_EVENT", "The unused native auxiliary argument is an integer, not a priority control.");
  return Object.freeze({
    ...input, payload: Object.freeze({ ...payload }),
    tile: input.tile === null ? null : Object.freeze({ ...input.tile }),
  });
}

export function actionId(event: AudioEvent): string {
  const id = event.payload.actionId ?? event.id;
  requireAudio(stableId(id), "AUDIO_EVENT", "Invalid audio action correlation ID.");
  return id;
}

export function cueKey(event: AudioEvent, sequence: number, frame: number): string {
  const iteration = event.payload.iteration ?? 0;
  requireAudio(integer(iteration, 0, 1_000_000), "AUDIO_EVENT", "Invalid source animation iteration.");
  if (event.payload.cueId !== undefined) {
    requireAudio(stableId(event.payload.cueId), "AUDIO_EVENT", "Invalid source cue correlation ID.");
    return event.payload.cueId;
  }
  return `${event.actorId ?? "world"}/${actionId(event)}/${sequence}/${frame}/${iteration}`;
}

export const OBSERVED_SELECTORS: Readonly<Record<string, number>> = Object.freeze({
  shortbow_release: 2693,
  rat_attack: 710,
  rat_hit: 713,
  rat_death: 711,
  goblin_attack: 469,
  goblin_hit: 472,
  goblin_death: 471,
  bronze_smelt_start: 2725,
});

export const LEARNING = "quest.learning_the_ropes";
export const COOK = "quest.cooks_assistant";
