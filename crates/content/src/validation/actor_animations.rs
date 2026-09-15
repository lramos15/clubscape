use super::*;

impl Validator<'_> {
    pub(super) fn actor_animations(&self, animations: &ActorAnimationDefinition) -> GameResult<()> {
        if animations.version != 1
            || animations.sequences.is_empty()
            || animations.sequences.len() > 256
            || animations.actions.len() > 512
            || animations.recipes.len() > 512
            || animations.styles.len() > 512
            || animations.spells.len() > 256
        {
            return Err(invalid(
                "ui.actor_animations",
                "invalid version or bounded animation registries",
            ));
        }
        for (id, sequence) in &animations.sequences {
            if *id > 65534 || sequence.duration_cycles == 0 || sequence.duration_cycles > 1_000_000
            {
                return Err(invalid(
                    "ui.actor_animations.sequence",
                    "invalid source sequence or duration",
                ));
            }
        }
        for (id, rule) in &animations.recipes {
            if !self.content.recipes.contains_key(id) {
                return Err(invalid(
                    "ui.actor_animations.recipe",
                    "unknown source recipe",
                ));
            }
            self.actor_animation_rule(animations, rule)?;
        }
        for (id, rule) in &animations.styles {
            if !self.content.mechanics.combat_styles.contains_key(id) {
                return Err(invalid(
                    "ui.actor_animations.style",
                    "unknown source combat style",
                ));
            }
            self.actor_animation_rule(animations, rule)?;
        }
        for (id, rule) in &animations.spells {
            let spell = self
                .content
                .mechanics
                .spells
                .get(id)
                .ok_or_else(|| invalid("ui.actor_animations.spell", "unknown source spell"))?;
            self.actor_animation_rule(animations, rule)?;
            if let ActorAnimationRule::Channel { duration_ticks, .. } = rule.rule {
                let SpellAction::Teleport { travel } = &spell.action else {
                    return Err(invalid(
                        "ui.actor_animations.spell",
                        "channel animation needs a source transport",
                    ));
                };
                let duration = self.content.mechanics.travels[travel]
                    .channel_ticks
                    .require()?;
                if duration != &duration_ticks {
                    return Err(invalid(
                        "ui.actor_animations.spell",
                        "animation timeline must preserve the source channel duration",
                    ));
                }
            }
        }
        for rule in animations.actions.values() {
            self.actor_animation_rule(animations, rule)?;
        }
        Ok(())
    }

    fn actor_animation_rule(
        &self,
        definitions: &ActorAnimationDefinition,
        binding: &ActorAnimationBinding,
    ) -> GameResult<()> {
        match &binding.rule {
            ActorAnimationRule::Sequence { sequence } => {
                if !definitions.sequences.contains_key(sequence) {
                    return Err(invalid(
                        "ui.actor_animations",
                        "missing source sequence metadata",
                    ));
                }
            }
            ActorAnimationRule::Channel {
                duration_ticks,
                phases,
            } => {
                if *duration_ticks == 0
                    || *duration_ticks > 2400
                    || phases.is_empty()
                    || phases.len() > 64
                    || phases[0].at_tick != 0
                    || phases
                        .windows(2)
                        .any(|pair| pair[0].at_tick >= pair[1].at_tick)
                    || phases.iter().any(|phase| {
                        phase.at_tick >= *duration_ticks
                            || !definitions.sequences.contains_key(&phase.sequence)
                    })
                {
                    return Err(invalid(
                        "ui.actor_animations",
                        "invalid source animation phase timeline",
                    ));
                }
            }
            ActorAnimationRule::Unverified { reason } => {
                text(reason, "ui.actor_animations.unverified", 2048)?
            }
        }
        Ok(())
    }
}
