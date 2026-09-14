use clubscape_game_types::*;
use clubscape_simulation::inventory;

use crate::{RandomSource, WorldEngine, invalid_content, invalid_state, runtime, unknown};

#[derive(Default)]
pub(crate) struct EffectFrame {
    pub events: Vec<GameEvent>,
    pub trigger: Option<GameEvent>,
    pub inventory_slot: Option<usize>,
    pub(crate) depth: usize,
    pub(crate) claim_active: bool,
}

impl WorldEngine {
    pub(crate) fn progress(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        before_action: &CharacterState,
        events: &mut Vec<GameEvent>,
        rng: &mut impl RandomSource,
    ) -> GameResult<()> {
        let snapshot = character.clone();
        let stage = self
            .content
            .tutorial
            .get(&snapshot.tutorial_stage)
            .ok_or_else(|| unknown("Undefined tutorial stage."))?;
        let mut selected = Vec::new();
        if snapshot.tutorial_stage == before_action.tutorial_stage
            && let Some(transition) =
                self.match_transition(world, &snapshot, &stage.transitions, events)?
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
                self.match_transition(world, &snapshot, &quest.transitions, events)?
            {
                selected.push(transition);
            }
        }
        for (transition, trigger) in selected {
            let mut frame = EffectFrame {
                trigger: Some(trigger),
                ..EffectFrame::default()
            };
            self.effects(world, character, &transition.effects, rng, &mut frame)?;
            events.extend(frame.events);
        }
        Ok(())
    }

    fn match_transition<'a>(
        &self,
        world: &WorldState,
        character: &CharacterState,
        transitions: &'a [ProgressTransition],
        events: &[GameEvent],
    ) -> GameResult<Option<(&'a ProgressTransition, GameEvent)>> {
        for event in events {
            let mut selected = None;
            for transition in transitions {
                if transition.event == event.kind()
                    && transition
                        .target
                        .as_deref()
                        .is_none_or(|target| event.primary_target() == Some(target))
                    && self.guard(world, character, &transition.guard, Some(event))?
                {
                    if selected.is_some() {
                        return Err(invalid_content(
                            "Multiple graph transitions match the same event.",
                        ));
                    }
                    selected = Some((transition, event.clone()));
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
        world: &mut WorldState,
        character: &mut CharacterState,
        effects: &[Effect],
        rng: &mut impl RandomSource,
        frame: &mut EffectFrame,
    ) -> GameResult<()> {
        if frame.depth > 64 || effects.len() > 4096 {
            return Err(invalid_content("Effect evaluation exceeds its bound."));
        }
        frame.depth += 1;
        for effect in effects {
            match effect {
                Effect::GiveItems { items } => {
                    inventory::add_batch(&mut character.inventory, &self.content.items, items)?
                }
                Effect::TakeItems { items } => {
                    inventory::remove_batch(&mut character.inventory, &self.content.items, items)?
                }
                Effect::AwardXp { rewards } => {
                    frame.events.extend(self.award_xp(character, rewards)?)
                }
                Effect::SetFlag { name, value } => {
                    if name.starts_with(runtime::PREFIX) {
                        return Err(invalid_content(
                            "Source effects cannot write engine metadata.",
                        ));
                    }
                    character.flags.insert(name.clone(), *value);
                }
                Effect::UnlockInterface { interface } => {
                    if !self.content.interfaces.contains_key(interface) {
                        return Err(unknown("Undefined interface."));
                    }
                    if !character.interfaces.contains(interface) {
                        character.interfaces.push(interface.clone());
                    }
                }
                Effect::SetTutorialStage { stage } => {
                    if !self.content.tutorial.contains_key(stage) {
                        return Err(unknown("Undefined tutorial stage."));
                    }
                    if &character.tutorial_stage != stage {
                        character.tutorial_stage = stage.clone();
                        frame.events.push(GameEvent::TutorialAdvanced {
                            stage: stage.clone(),
                        });
                    }
                }
                Effect::SetQuestStage { quest, stage } => {
                    let definition = self
                        .content
                        .quests
                        .get(quest)
                        .ok_or_else(|| unknown("Unknown quest."))?;
                    if !definition.journal.contains_key(stage) {
                        return Err(unknown("Undefined quest stage."));
                    }
                    let state = character
                        .quests
                        .get_mut(quest)
                        .ok_or_else(|| invalid_state("Missing quest state."))?;
                    if state.stage == definition.completed_stage && &state.stage != stage {
                        return Err(invalid_content("Completed quest cannot be reset."));
                    }
                    if &state.stage != stage {
                        state.stage = stage.clone();
                        frame.events.push(GameEvent::QuestAdvanced {
                            quest: quest.clone(),
                            stage: stage.clone(),
                        });
                    }
                }
                Effect::AddQuestPoints { amount } => {
                    character.quest_points = character
                        .quest_points
                        .checked_add(u32::from(*amount))
                        .ok_or_else(|| invalid_content("Quest point overflow."))?;
                }
                Effect::Travel { region, tile } => {
                    let tile =
                        self.map_instance_tile(world, character.runtime.instance.as_ref(), *tile)?;
                    let location = RuntimeLocation {
                        region: region.clone(),
                        tile,
                        instance: character.runtime.instance.clone(),
                    };
                    self.move_to(world, character, location)?;
                    frame.events.push(GameEvent::Moved { tile });
                }
                Effect::Message { text } => {
                    frame.events.push(GameEvent::Message { text: text.clone() })
                }
                Effect::Conditional { guard, effects } => {
                    if self.guard(world, character, guard, frame.trigger.as_ref())? {
                        self.effects(world, character, effects, rng, frame)?;
                    }
                }
                Effect::Grant { grant } => {
                    if frame.claim_active
                        && self
                            .content
                            .mechanics
                            .grants
                            .get(grant)
                            .is_some_and(|definition| {
                                definition.capacity == CapacityPolicy::OrderedPartial
                            })
                    {
                        return Err(invalid_content(
                            "An atomic entitlement cannot wrap an ordered-partial grant.",
                        ));
                    }
                    frame.events.extend(self.grant(character, grant)?);
                }
                Effect::Once {
                    entitlement,
                    effects,
                } => {
                    let definition = self
                        .content
                        .mechanics
                        .entitlements
                        .get(entitlement)
                        .ok_or_else(|| unknown("Unknown entitlement."))?;
                    if !matches!(definition.purpose, EntitlementPurpose::AtomicReward)
                        || frame.claim_active
                    {
                        return Err(invalid_content(
                            "Invalid or nested atomic reward entitlement.",
                        ));
                    }
                    if character.runtime.entitlements.contains_key(entitlement) {
                        continue;
                    }
                    frame.claim_active = true;
                    self.effects(world, character, effects, rng, frame)?;
                    frame.claim_active = false;
                    character.runtime.entitlements.insert(
                        entitlement.clone(),
                        EntitlementState::Claimed {
                            at_tick: world.tick,
                        },
                    );
                }
                Effect::RestoreVital { vital, restoration } => {
                    self.restore_vital(character, *vital, restoration)?
                }
                Effect::SetCounter { counter, value } => {
                    self.set_counter(world, character, counter, *value, &mut frame.events)?
                }
                Effect::AddCounter { counter, delta } => {
                    let value = self.counter_value(world, character, counter)?;
                    let value = self
                        .content
                        .mechanics
                        .counters
                        .get(counter)
                        .ok_or_else(|| unknown("Unknown counter."))?
                        .checked_add(value, *delta)?;
                    self.set_counter(world, character, counter, value, &mut frame.events)?;
                }
                Effect::TransformObject { transform, state } => {
                    let definition = self
                        .content
                        .mechanics
                        .object_transforms
                        .get(transform)
                        .ok_or_else(|| unknown("Unknown object transform."))?;
                    if !definition.states.contains_key(state) {
                        return Err(unknown("Unknown transform state."));
                    }
                    let states = match definition.scope {
                        CounterScope::World => &mut world.runtime.object_states,
                        CounterScope::Instance => {
                            &mut world
                                .runtime
                                .instances
                                .get_mut(character.runtime.instance.as_ref().ok_or_else(|| {
                                    invalid_state("Instance transform needs an instance.")
                                })?)
                                .ok_or_else(|| invalid_state("Unknown instance."))?
                                .object_states
                        }
                        CounterScope::Character => {
                            return Err(invalid_content("Collision cannot be character-scoped."));
                        }
                    };
                    states.insert(transform.clone(), state.clone());
                    self.collision_for(
                        world,
                        if definition.scope == CounterScope::World {
                            None
                        } else {
                            character.runtime.instance.as_ref()
                        },
                    )?;
                    frame.events.push(GameEvent::ObjectTransformed {
                        transform: transform.clone(),
                        state: state.clone(),
                    });
                }
                Effect::CreateTemporaryObject { definition } => frame
                    .events
                    .push(self.create_temporary(world, character, definition, rng)?),
                Effect::TravelVia { travel } => frame
                    .events
                    .extend(self.start_travel(world, character, travel, rng)?),
                Effect::ReconcileContainers { reconciliation } => {
                    self.reconcile(world, character, reconciliation)?
                }
                Effect::CompleteDeathTopic { topic } => {
                    if !matches!(character.runtime.life, LifeState::FirstDeathOffice { .. }) {
                        return Err(GameError::new(
                            GameErrorCode::RequirementNotMet,
                            "Death topic is not part of an active first-death introduction.",
                        ));
                    }
                    if character.runtime.death_topics.insert(*topic) {
                        frame
                            .events
                            .push(GameEvent::DeathTopicCompleted { topic: *topic });
                    }
                }
                Effect::ConsumeCharges {
                    item,
                    charge_kind,
                    amount,
                    selection,
                } => self.consume_charges(
                    character,
                    item,
                    charge_kind,
                    *amount,
                    *selection,
                    frame.inventory_slot,
                )?,
                Effect::Inspect {
                    target,
                    explanation,
                } => {
                    let interaction = InteractionDefinition {
                        name: "inspect".into(),
                        reach: 1,
                        guard: Guard::Always,
                        action: InteractionAction::Attack,
                    };
                    self.require_target(world, character, target, &interaction)?;
                    frame.events.push(GameEvent::Inspected {
                        target: target.clone(),
                        explanation: explanation.clone(),
                    });
                }
            }
        }
        frame.depth -= 1;
        Ok(())
    }

    fn set_counter(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        id: &CounterId,
        value: CounterValue,
        events: &mut Vec<GameEvent>,
    ) -> GameResult<()> {
        let definition = self
            .content
            .mechanics
            .counters
            .get(id)
            .ok_or_else(|| unknown("Unknown counter."))?;
        definition.validate_value(value)?;
        let counters = match definition.scope {
            CounterScope::Character => &mut character.runtime.counters,
            CounterScope::World => &mut world.runtime.counters,
            CounterScope::Instance => {
                &mut world
                    .runtime
                    .instances
                    .get_mut(
                        character
                            .runtime
                            .instance
                            .as_ref()
                            .ok_or_else(|| invalid_state("Counter needs an instance."))?,
                    )
                    .ok_or_else(|| invalid_state("Unknown instance."))?
                    .counters
            }
        };
        if !counters.contains_key(id) {
            return Err(invalid_state("Missing counter needs migration."));
        }
        counters.insert(id.clone(), value);
        events.push(GameEvent::CounterChanged {
            counter: id.clone(),
            value,
        });
        Ok(())
    }

    fn consume_charges(
        &self,
        character: &mut CharacterState,
        item: &ItemId,
        kind: &ChargeKindId,
        amount: u32,
        selection: ChargeSelection,
        slot: Option<usize>,
    ) -> GameResult<()> {
        // Ordinary M1 uses normal milk. The shared inventory primitive deliberately rejects
        // instance items, so do not hide an alternate container implementation in this engine.
        let _ = (character, item, kind, amount, selection, slot);
        Err(crate::unavailable(
            "Charged-item consumption awaits instance-aware shared inventory primitives; ordinary M1 milk remains supported.",
        ))
    }

    pub(crate) fn check_reward_atomicity(
        &self,
        before: &CharacterState,
        after: &CharacterState,
    ) -> GameResult<()> {
        before.runtime.validate_ledger_successor(&after.runtime)?;
        if after.quest_points > before.quest_points
            && !self.content.quests.iter().any(|(id, definition)| {
                before
                    .quests
                    .get(id)
                    .is_some_and(|state| state.stage != definition.completed_stage)
                    && after
                        .quests
                        .get(id)
                        .is_some_and(|state| state.stage == definition.completed_stage)
            })
        {
            return Err(invalid_content(
                "Quest points require atomic newly completed quest progress.",
            ));
        }
        Ok(())
    }
}
