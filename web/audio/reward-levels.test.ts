import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import type { AudioEvent, SkillView } from "../shared/contracts.ts";
import { baseSkills, committedRewardLevel, observeRewardLevels } from "./reward-levels.ts";
import { validateEvent } from "./events.ts";

const boundary = JSON.parse(await readFile(new URL(
  "../../research/browser-audio-policy/cook-reward-boundary.json", import.meta.url), "utf8"));
function skill(state: { baseLevel: number; xpTenths: string }, currentLevel = state.baseLevel): SkillView {
  return { id: "skill.cooking", name: "Cooking", ...state, currentLevel, iconAsset: null };
}
function event(level: number, sourceId: number | null = 34): AudioEvent {
  return {
    id: "committed/cook/level", kind: "level_up", sourceId, assetId: null, actorId: "player.test",
    tile: null, sourceCycle: null,
    payload: { committed: true, causeQuestId: "quest.cooks_assistant", skillId: "skill.cooking",
      completionId: "committed/cook", level },
  };
}

test("source-supported extra cooking changes the committed Cook base-level delta, not the XP rule", () => {
  assert.equal(boundary.source_reward_xp_tenths_unchanged, 3000);
  assert.equal(boundary.gameplay_xp_changed, false);
  assert.equal(boundary.quest_events_created_by_audio, false);
  for (const sample of boundary.boundaries) {
    const before = baseSkills([skill(sample.before)]);
    const after = baseSkills([skill(sample.after)]);
    const observation = observeRewardLevels("committed/cook", before, after);
    const result = committedRewardLevel(event(sample.after.baseLevel), observation, after);
    if (sample.levelChanged) {
      assert.deepEqual(result, { skillId: "skill.cooking", before: sample.before, after: sample.after });
    } else assert.equal(result, null);
  }
});

test("the observed 33/level4 example does not restrict a pre-trained 2-to-5 reward", () => {
  const sample = boundary.boundaries.find((value: any) => value.case === "pretrained-level5");
  const before = baseSkills([skill(sample.before)]), after = baseSkills([skill(sample.after)]);
  const reward = observeRewardLevels("committed/cook", before, after);
  const sourceSelected = validateEvent(event(5, 34));
  assert.equal(committedRewardLevel(sourceSelected, reward, after)?.after.baseLevel, 5);
  assert.equal(sourceSelected.sourceId, 34);
  // This gate never selects a sound from a level; it only preserves the input.
  assert.equal(committedRewardLevel(event(5, 33), reward, after)?.after.baseLevel, 5);
  assert.throws(() => committedRewardLevel(event(4, 33), reward, after), /disagrees/);
});

test("a committed reward with XP but no base-level gain has no level-up cue, including boosted current levels", () => {
  const sample = boundary.boundaries.find((value: any) => value.case === "pretrained-no-level");
  const before = baseSkills([skill(sample.before, sample.before.baseLevel - 2)]);
  const after = baseSkills([skill(sample.after, sample.after.baseLevel + 3)]);
  const reward = observeRewardLevels("committed/cook", before, after);
  assert.equal(committedRewardLevel(event(sample.after.baseLevel), reward, after), null);
  assert.equal(committedRewardLevel(event(sample.after.baseLevel, null), reward, after), null);
});

test("reward events cannot invent a completion association, skill, before level or XP delta", () => {
  const before = baseSkills([skill({ baseLevel: 2, xpTenths: "1000" })]);
  const after = baseSkills([skill({ baseLevel: 5, xpTenths: "4000" })]);
  const reward = observeRewardLevels("committed/cook", before, after);
  assert.throws(() => committedRewardLevel(event(5), null, after), /committed skill delta/);
  for (const payload of [
    { completionId: "other-completion" }, { skillId: "skill.mining" },
    { previousLevel: 4 }, { committed: false },
  ]) assert.throws(() => committedRewardLevel({ ...event(5), payload: { ...event(5).payload, ...payload } }, reward, after));
  const invalid = observeRewardLevels("committed/cook", before,
    baseSkills([skill({ baseLevel: 5, xpTenths: "1000" })]));
  assert.throws(() => committedRewardLevel(event(5), invalid, after), /XP increase/);
});

test("deferral retains the committed reward delta even if later training raises the current snapshot", () => {
  const before = baseSkills([skill({ baseLevel: 2, xpTenths: "1000" })]);
  const committed = baseSkills([skill({ baseLevel: 5, xpTenths: "4000" })]);
  const later = baseSkills([skill({ baseLevel: 6, xpTenths: "5300" })]);
  const reward = observeRewardLevels("committed/cook", before, committed);
  assert.equal(committedRewardLevel(event(5), reward, later)?.after.baseLevel, 5);
});
