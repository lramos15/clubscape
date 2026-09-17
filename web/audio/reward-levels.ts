import type { AudioEvent, SkillView } from "../shared/contracts.ts";
import { integer, requireAudio } from "./errors.ts";
import { stableId } from "./events.ts";

export interface BaseSkillState {
  readonly baseLevel: number;
  readonly xpTenths: string;
}
export type BaseSkills = ReadonlyMap<string, BaseSkillState>;
export interface CommittedLevelDelta {
  readonly skillId: string;
  readonly before: BaseSkillState;
  readonly after: BaseSkillState;
}
export interface RewardLevels {
  readonly completionId: string;
  readonly skills: ReadonlyMap<string, CommittedLevelDelta>;
  readonly acceptedSkills: Set<string>;
}

export function baseSkills(skills: readonly SkillView[]): BaseSkills {
  requireAudio(Array.isArray(skills), "AUDIO_LEVEL_EVIDENCE", "Authoritative skill views are missing.");
  const result = new Map<string, BaseSkillState>();
  for (const skill of skills) {
    requireAudio(stableId(skill.id) && !result.has(skill.id) &&
      integer(skill.baseLevel, 1, 65535) && typeof skill.xpTenths === "string" &&
      /^(0|[1-9][0-9]{0,19})$/.test(skill.xpTenths),
    "AUDIO_LEVEL_EVIDENCE", "Invalid authoritative base-level/XP observation.");
    result.set(skill.id, Object.freeze({ baseLevel: skill.baseLevel, xpTenths: skill.xpTenths }));
  }
  return result;
}

/** Capture the already-committed transaction; never calculate/grant XP or infer a jingle. */
export function observeRewardLevels(completionId: string, before: BaseSkills, after: BaseSkills): RewardLevels {
  const skills = new Map<string, CommittedLevelDelta>();
  for (const [skillId, current] of after) {
    const previous = before.get(skillId);
    if (previous) skills.set(skillId, Object.freeze({ skillId, before: previous, after: current }));
  }
  return { completionId, skills, acceptedSkills: new Set() };
}

export function committedRewardLevel(
  event: AudioEvent, reward: RewardLevels | null, current: BaseSkills,
): CommittedLevelDelta | null {
  requireAudio(event.kind === "level_up" && event.payload.committed === true &&
    stableId(event.payload.skillId) && reward !== null,
  "AUDIO_LEVEL_EVIDENCE", "Cook reward level audio needs its committed skill delta, not a golden-case level.");
  requireAudio(event.payload.completionId === undefined || event.payload.completionId === reward.completionId,
    "AUDIO_LEVEL_EVIDENCE", "The level notification does not belong to this committed completion.");
  const delta = reward.skills.get(event.payload.skillId);
  const actual = current.get(event.payload.skillId);
  requireAudio(delta && actual && delta.after.baseLevel >= delta.before.baseLevel &&
    BigInt(delta.after.xpTenths) >= BigInt(delta.before.xpTenths) &&
    actual.baseLevel >= delta.after.baseLevel && BigInt(actual.xpTenths) >= BigInt(delta.after.xpTenths),
  "AUDIO_LEVEL_EVIDENCE", "The declared reward skill does not match the authoritative completion transaction.");
  requireAudio((event.payload.level === undefined || event.payload.level === delta.after.baseLevel) &&
    (event.payload.previousLevel === undefined || event.payload.previousLevel === delta.before.baseLevel),
  "AUDIO_LEVEL_EVIDENCE", "The declared base-level delta disagrees with the committed skill observation.");
  if (delta.after.baseLevel === delta.before.baseLevel) return null;
  requireAudio(BigInt(delta.after.xpTenths) > BigInt(delta.before.xpTenths),
    "AUDIO_LEVEL_EVIDENCE", "A reward base-level increase needs a committed XP increase.");
  requireAudio(event.sourceId !== null, "AUDIO_ASSET_ID",
    "Preserve the source-selected jingle (or its -1 no-request sentinel); no level-to-jingle guess is permitted.");
  return delta;
}
