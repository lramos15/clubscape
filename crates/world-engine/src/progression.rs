use clubscape_game_types::*;
use clubscape_simulation::{inventory, skills};

use crate::{WorldEngine, invalid_content, runtime, unknown};

impl WorldEngine {
    pub(crate) fn progress(
        &self,
        character: &mut CharacterState,
        before_action: &CharacterState,
        events: &mut Vec<GameEvent>,
    ) -> GameResult<()> {
        let snapshot = character.clone();
        let stage = self
            .content
            .tutorial
            .get(&snapshot.tutorial_stage)
            .ok_or_else(|| unknown("Undefined current tutorial stage."))?;
        let mut selected = Vec::new();
        if snapshot.tutorial_stage == before_action.tutorial_stage
            && let Some(transition) =
                self.match_transition(&snapshot, &stage.transitions, events)?
        {
            selected.push(transition);
        }
        for quest in self.content.quests.values() {
            if snapshot.quests.get(&quest.id).map(|state| &state.stage)
                != before_action
                    .quests
                    .get(&quest.id)
                    .map(|state| &state.stage)
            {
                continue;
            }
            if let Some(transition) =
                self.match_transition(&snapshot, &quest.transitions, events)?
            {
                selected.push(transition);
            }
        }
        // Match from one post-action snapshot: no cascading stage skips on emitted XP/progress events.
        for transition in selected {
            self.effects(character, &transition.effects, events, 0)?;
        }
        Ok(())
    }

    fn match_transition<'a>(
        &self,
        character: &CharacterState,
        transitions: &'a [ProgressTransition],
        events: &[GameEvent],
    ) -> GameResult<Option<&'a ProgressTransition>> {
        for event in events {
            let mut selected = None;
            for transition in transitions {
                if transition.event == event.kind()
                    && transition
                        .target
                        .as_deref()
                        .is_none_or(|target| event.primary_target() == Some(target))
                    && self.guard(character, &transition.guard)?
                {
                    if selected.is_some() {
                        return Err(invalid_content(
                            "Multiple graph transitions match the same authoritative event.",
                        ));
                    }
                    selected = Some(transition);
                }
            }
            if selected.is_some() {
                return Ok(selected);
            }
        }
        Ok(None)
    }

    pub(crate) fn effects(
        &self,
        character: &mut CharacterState,
        effects: &[Effect],
        events: &mut Vec<GameEvent>,
        depth: usize,
    ) -> GameResult<()> {
        if depth > 64 || effects.len() > 4096 {
            return Err(invalid_content("Effect evaluation exceeds its bound."));
        }
        for effect in effects {
            match effect {
                Effect::GiveItems { items } => {
                    inventory::add_batch(&mut character.inventory, &self.content.items, items)?
                }
                Effect::TakeItems { items } => {
                    inventory::remove_batch(&mut character.inventory, &self.content.items, items)?
                }
                Effect::AwardXp { rewards } => events.extend(skills::award_character_xp(
                    character,
                    &self.content,
                    rewards,
                    skills::CurrentLevelPolicy::AddBaseLevelGains,
                )?),
                Effect::SetFlag { name, value } => {
                    if name.starts_with(runtime::PREFIX) {
                        return Err(invalid_content(
                            "Content cannot overwrite engine runtime metadata.",
                        ));
                    }
                    character.flags.insert(name.clone(), *value);
                }
                Effect::UnlockInterface { interface } => {
                    if !self.content.interfaces.contains_key(interface) {
                        return Err(unknown(format!("Undefined interface {interface}.")));
                    }
                    if !character.interfaces.contains(interface) {
                        character.interfaces.push(interface.clone());
                    }
                }
                Effect::SetTutorialStage { stage } => {
                    if !self.content.tutorial.contains_key(stage) {
                        return Err(unknown(format!("Undefined tutorial stage {stage}.")));
                    }
                    if &character.tutorial_stage != stage {
                        character.tutorial_stage = stage.clone();
                        events.push(GameEvent::TutorialAdvanced {
                            stage: stage.clone(),
                        });
                    }
                }
                Effect::SetQuestStage { quest, stage } => {
                    let definition = self
                        .content
                        .quests
                        .get(quest)
                        .ok_or_else(|| unknown(format!("Unknown quest {quest}.")))?;
                    if !definition.journal.contains_key(stage) {
                        return Err(unknown(format!("Undefined quest stage {stage}.")));
                    }
                    let state = character.quests.get_mut(quest).ok_or_else(|| {
                        unknown(format!("Character has no state for quest {quest}."))
                    })?;
                    if state.stage == definition.completed_stage && &state.stage != stage {
                        return Err(invalid_content(
                            "A completed quest cannot be reset by a transition.",
                        ));
                    }
                    if &state.stage != stage {
                        state.stage = stage.clone();
                        events.push(GameEvent::QuestAdvanced {
                            quest: quest.clone(),
                            stage: stage.clone(),
                        });
                    }
                }
                Effect::AddQuestPoints { amount } => {
                    character.quest_points = character
                        .quest_points
                        .checked_add(u32::from(*amount))
                        .ok_or_else(|| invalid_content("Quest point reward overflow."))?;
                }
                Effect::Travel { region, tile } => {
                    self.validate_destination(region, *tile)?;
                    runtime::interrupt(character);
                    character.region = region.clone();
                    character.tile = *tile;
                    events.push(GameEvent::Moved { tile: *tile });
                }
                Effect::Message { text } => events.push(GameEvent::Message { text: text.clone() }),
                Effect::Conditional { guard, effects } => {
                    if self.guard(character, guard)? {
                        self.effects(character, effects, events, depth + 1)?;
                    }
                }
                Effect::Grant { .. }
                | Effect::Once { .. }
                | Effect::RestoreVital { .. }
                | Effect::SetCounter { .. }
                | Effect::AddCounter { .. }
                | Effect::TransformObject { .. }
                | Effect::CreateTemporaryObject { .. }
                | Effect::TravelVia { .. }
                | Effect::ReconcileContainers { .. }
                | Effect::CompleteDeathTopic { .. }
                | Effect::ConsumeCharges { .. }
                | Effect::Inspect { .. } => {
                    return Err(crate::unavailable(
                        "This typed source effect requires mechanics-v2 execution.",
                    ));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn check_reward_atomicity(
        &self,
        before: &CharacterState,
        after: &CharacterState,
    ) -> GameResult<()> {
        if after.quest_points > before.quest_points {
            let completed = self.content.quests.iter().any(|(id, definition)| {
                before
                    .quests
                    .get(id)
                    .is_some_and(|state| state.stage != definition.completed_stage)
                    && after
                        .quests
                        .get(id)
                        .is_some_and(|state| state.stage == definition.completed_stage)
            });
            if !completed {
                return Err(invalid_content(
                    "Quest points require an atomic, newly completed quest.",
                ));
            }
        }
        Ok(())
    }
}
