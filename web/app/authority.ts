import { AUDIO_AUTHORITY_CAPABILITY, UI_AMOUNTS_CAPABILITY, UI_RECOVERY_CAPABILITY } from "../shared/contracts.ts";
import type { AudioAuthorityView, CurrentSceneView, WorldView } from "../shared/contracts.ts";
import { NATIVE_AUDIO_DATA } from "../audio/native-data.ts";
import { sourceObjectDefinition } from "../audio/native-scene.ts";
import type { SourceAudioScene } from "../audio/index.ts";
import { AppError, invariant } from "./errors.ts";
import { decimal } from "./gameplay-ui.ts";

export function validateScene(scene: CurrentSceneView, world: WorldView): void {
  invariant(scene && typeof scene.region === "string" && scene.region === world.player.region
    && Object.hasOwn(scene, "instance") && Object.hasOwn(scene, "instanceTemplate")
    && scene.instance === world.player.instance
    && (scene.instance === null ? scene.instanceTemplate === null
      : typeof scene.instance === "string" && scene.instance.length > 0
        && typeof scene.instanceTemplate === "string" && scene.instanceTemplate.startsWith("instance_template.")),
  "The authoritative region/opaque instance/template pair is incomplete or inconsistent.", "protocol");
}

export function validateAudioAuthority(value: AudioAuthorityView): void {
  invariant(value && value.version === 1 && typeof value.profile === "string" && value.profile.length > 0
    && value.profile.length <= 256 && value.music && Array.isArray(value.varps),
  "The versioned source audio authority projection is missing.", "protocol");
  const music = value.music;
  invariant(["from_creation", "legacy_untracked"].includes(music.history)
    && typeof music.complete === "boolean" && Array.isArray(music.tracks) && Array.isArray(music.unlockedGroups)
    && music.tracks.length <= 4096 && value.varps.length <= 4096,
  "Invalid source music history projection.", "protocol");
  decimal(music.trackedFromTick); decimal(music.revision);
  const groups = new Set<number>(), unlocked = new Set<number>();
  let unknown = false;
  for (const track of music.tracks) {
    invariant(track && Number.isSafeInteger(track.group) && track.group > 0 && track.group <= 0xffffffff
      && !groups.has(track.group) && ["unlocked", "locked", "unknown"].includes(track.status)
      && Object.hasOwn(track, "confirmedAtTick") && Object.hasOwn(track, "rule"),
    "Invalid source music track identity/status; title0 is not an unlock row.", "protocol");
    groups.add(track.group);
    if (track.status === "unlocked") {
      decimal(track.confirmedAtTick);
      invariant(typeof track.rule === "string" && track.rule.length > 0,
        "An unlocked source track omitted its confirming rule.", "protocol");
      unlocked.add(track.group);
    } else {
      invariant(track.confirmedAtTick === null && track.rule === null
        && (track.status === "locked" ? music.history === "from_creation" : music.history === "legacy_untracked"),
      "Legacy unknown music history cannot be reclassified as locked or newly created.", "protocol");
      unknown ||= track.status === "unknown";
    }
  }
  invariant(music.complete === !unknown && music.unlockedGroups.length === unlocked.size
    && new Set(music.unlockedGroups).size === unlocked.size && music.unlockedGroups.every((group) => unlocked.has(group)),
  "Source music completeness/unlock identities disagree with the actual track observations.", "protocol");
  const ids = new Set<number>();
  for (const variable of value.varps) {
    invariant(variable && Number.isSafeInteger(variable.id) && variable.id >= 0 && variable.id <= 0xffffffff
      && !ids.has(variable.id) && Number.isSafeInteger(variable.knownBits)
      && variable.knownBits >= 0 && variable.knownBits <= 0xffffffff
      && typeof variable.binding === "string" && variable.binding.length > 0 && variable.binding.length <= 256
      && (variable.value === null
        ? variable.knownBits === 0 && typeof variable.unavailableReason === "string" && variable.unavailableReason.length > 0
        : Number.isSafeInteger(variable.value) && variable.value >= -2147483648 && variable.value <= 2147483647
          && variable.knownBits !== 0 && variable.unavailableReason === null),
    "Invalid partial native-varp authority; absent bits are not zero-valued facts.", "protocol");
    ids.add(variable.id);
  }
}

export function validateWorldAuthority(capabilities: readonly string[], world: WorldView | null): void {
  if (world === null) return;
  const audio = capabilities.includes(AUDIO_AUTHORITY_CAPABILITY);
  invariant(audio === (world.audioAuthority !== undefined),
    "Source audio authority requires exact capability/version negotiation, not a fresh empty history.", "protocol");
  if (world.audioAuthority !== undefined) validateAudioAuthority(world.audioAuthority);
  if (world.scene !== undefined) validateScene(world.scene, world);
  else invariant(!audio && !capabilities.includes(UI_AMOUNTS_CAPABILITY) && !capabilities.includes(UI_RECOVERY_CAPABILITY),
    "The current server omitted its authoritative scene identity.", "protocol");
}

export function authoritativeMusicUnlocks(world: WorldView): readonly number[] | undefined {
  const authority = world.audioAuthority;
  if (authority === undefined) return undefined;
  validateAudioAuthority(authority);
  if (!authority.music.complete) {
    throw new AppError("Source music history is partially unknown. Client preferences cannot turn unknown tracks into locked/unlocked rows or a new-record history.",
      { kind: "audio_authority", errorId: "audio.authority.history_unknown" });
  }
  return authority.music.unlockedGroups;
}

const calibratedMasks = new Map<number, number>();
for (const bit of NATIVE_AUDIO_DATA.varbits) {
  const width = bit.msb - bit.lsb + 1;
  const mask = (((2 ** width) - 1) << bit.lsb) >>> 0;
  calibratedMasks.set(bit.varp, ((calibratedMasks.get(bit.varp) ?? 0) | mask) >>> 0);
}

/** A bounded bit observation, deliberately not a complete native-varp word or stored preference. */
export function calibratedNativeVarpBits(authority: AudioAuthorityView, id: number, requiredMask: number): Readonly<{
  id: number; value: number; knownBits: number; binding: string;
}> {
  validateAudioAuthority(authority);
  const mask = calibratedMasks.get(id);
  if (!Number.isSafeInteger(requiredMask) || requiredMask <= 0 || requiredMask > 0xffffffff
    || mask === undefined || ((mask & requiredMask) >>> 0) !== requiredMask) {
    throw new AppError("The requested native-varp bits have no published audio calibration.",
      { kind: "audio_authority", errorId: "audio.authority.uncalibrated_bits" });
  }
  const variable = authority.varps.find((entry) => entry.id === id);
  if (!variable || variable.value === null || ((variable.knownBits & requiredMask) >>> 0) !== requiredMask) {
    throw new AppError("The backend has not supplied all calibrated bits needed by this native audio binding.",
      { kind: "audio_authority", errorId: "audio.authority.bits_unknown" });
  }
  return Object.freeze({ id, value: variable.value & requiredMask, knownBits: requiredMask, binding: variable.binding });
}

/** Only the calibrated consumers in this actual scene may read the projected bit subset. */
export function sourceSceneAuthority(world: WorldView, scene: SourceAudioScene): SourceAudioScene {
  if (!world.audioAuthority) throw new AppError("The native scene cannot use client-guessed variable words without audio authority.",
    { kind: "audio_authority", errorId: "audio.authority.scene_variables_required" });
  validateAudioAuthority(world.audioAuthority);
  const required = new Map<number, number>();
  for (const emitter of scene.emitters) {
    const definition = sourceObjectDefinition(emitter.objectId);
    if (!definition) throw new AppError("A placed audio emitter has no published original calibration.",
      { kind: "audio_authority", errorId: "audio.authority.emitter_unbound" });
    if (definition.transforms === null) continue;
    const bit = NATIVE_AUDIO_DATA.varbits.find((value) => value.id === definition.varbit);
    if (!bit || definition.varp !== -1) throw new AppError("The scene requests an uncalibrated full native variable word.",
      { kind: "audio_authority", errorId: "audio.authority.uncalibrated_bits" });
    const mask = ((((2 ** (bit.msb - bit.lsb + 1)) - 1) << bit.lsb) >>> 0);
    required.set(bit.varp, ((required.get(bit.varp) ?? 0) | mask) >>> 0);
  }
  const varps = new Map<number, number>();
  for (const [id, mask] of required) varps.set(id, calibratedNativeVarpBits(world.audioAuthority, id, mask).value);
  return { ...scene, varps };
}
