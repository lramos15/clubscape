use clubscape_game_types::*;

use crate::{WorldEngine, invalid_state};

impl WorldEngine {
    fn animation_definitions(&self) -> Option<&ActorAnimationDefinition> {
        self.content
            .ui
            .as_ref()
            .and_then(|ui| ui.actor_animations.as_ref())
    }

    pub(crate) fn animation_binding(
        &self,
        identity: &ObservedAction,
    ) -> Option<&ActorAnimationBinding> {
        let definitions = self.animation_definitions()?;
        identity
            .spell_id
            .as_ref()
            .and_then(|id| definitions.spells.get(id))
            .or_else(|| {
                identity
                    .recipe_id
                    .as_ref()
                    .and_then(|id| definitions.recipes.get(id))
            })
            .or_else(|| {
                identity
                    .action_id
                    .as_ref()
                    .and_then(|id| definitions.actions.get(id))
            })
            .or_else(|| {
                identity
                    .style_id
                    .as_ref()
                    .and_then(|id| definitions.styles.get(id))
            })
    }

    pub(crate) fn completed_animation_visible(
        &self,
        tick: u64,
        action: &ActionObservation,
    ) -> bool {
        let Some(completed) = action.completed_at_tick else {
            return false;
        };
        let sequence = action
            .identity
            .animation
            .as_ref()
            .and_then(|value| value.parse::<u32>().ok())
            .or_else(|| {
                self.animation_binding(&action.identity)
                    .and_then(|binding| match binding.rule {
                        ActorAnimationRule::Sequence { sequence } => Some(sequence),
                        _ => None,
                    })
            });
        let duration = sequence
            .and_then(|sequence| self.animation_definitions()?.sequences.get(&sequence))
            .map(|sequence| u64::from(sequence.duration_cycles));
        match duration {
            Some(duration) => tick
                .checked_sub(action.cycle_started_at_tick)
                .is_some_and(|elapsed| elapsed.saturating_mul(30) < duration),
            None => completed == tick,
        }
    }

    pub(crate) fn project_actor_animation(
        &self,
        tick: u64,
        stored: &ActionObservation,
        view: &mut ActorActionView,
    ) -> GameResult<()> {
        if view.animation.is_some() {
            return Ok(());
        }
        let Some(binding) = self.animation_binding(&stored.identity) else {
            return Ok(());
        };
        match &binding.rule {
            ActorAnimationRule::Sequence { sequence } => {
                view.animation = Some(sequence.to_string())
            }
            ActorAnimationRule::Channel {
                duration_ticks,
                phases,
            } => {
                let elapsed = tick
                    .checked_sub(stored.started_at_tick)
                    .ok_or_else(|| invalid_state("Observed channel phase is future-dated."))?;
                if elapsed >= u64::from(*duration_ticks) {
                    return Ok(());
                }
                let phase = phases
                    .iter()
                    .rev()
                    .find(|phase| u64::from(phase.at_tick) <= elapsed)
                    .ok_or_else(|| {
                        invalid_state("Source channel lacks its initial animation phase.")
                    })?;
                view.animation = Some(phase.sequence.to_string());
                view.cycle_started_at_tick =
                    (stored.started_at_tick + u64::from(phase.at_tick)).to_string();
            }
            ActorAnimationRule::Unverified { .. } => {}
        }
        Ok(())
    }

    pub(crate) fn record_item_animation(
        &self,
        tick: u64,
        character: &mut CharacterState,
        item: &ItemId,
        action: &str,
    ) -> GameResult<()> {
        let Some(definitions) = self.animation_definitions() else {
            return Ok(());
        };
        let name = format!("action.item.{}.{}", &item.as_str()[5..], action);
        let Some(id) = definitions
            .actions
            .keys()
            .find(|id| id.as_str() == name)
            .cloned()
        else {
            return Ok(());
        };
        let observation = character
            .runtime
            .observation
            .get_or_insert_with(ActorObservation::default);
        let ordinal = observation.next_id;
        observation.next_id = ordinal
            .checked_add(1)
            .filter(|next| *next <= i64::MAX as u64)
            .ok_or_else(|| invalid_state("Actor animation identity exhausted."))?;
        observation.overlay = Some(ActionObservation {
            ordinal,
            identity: ObservedAction {
                activity: "inventory_action".into(),
                action_id: Some(id),
                target: None,
                recipe_id: None,
                style_id: None,
                spell_id: None,
                animation: None,
            },
            started_at_tick: tick,
            cycle_started_at_tick: tick,
            next_action_tick: None,
            completed_at_tick: Some(tick),
        });
        observation.validate(tick)
    }
}
