use std::collections::BTreeMap;

use clubscape_game_types::{
    CharacterState, GameContent, GameError, GameErrorCode, GameEvent, GameResult, SkillDefinition,
    SkillId, SkillRequirement, SkillState, TutorialStageDefinition, XpReward,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct XpLimits {
    pub cap_tenths: Option<u64>,
    pub stop_level: Option<u16>,
}

impl XpLimits {
    /// A missing entry means this stage imposes no extra restriction on the skill.
    pub fn from_stage(stage: &TutorialStageDefinition, skill: &SkillId) -> Self {
        Self {
            cap_tenths: stage.xp_caps_tenths.get(skill).copied(),
            stop_level: stage.xp_stop_levels.get(skill).copied(),
        }
    }
}

/// The caller must choose the source's current-level behavior explicitly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurrentLevelPolicy {
    Preserve,
    AddBaseLevelGains,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LevelBasis {
    Base,
    Current,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XpAward {
    pub skill: SkillId,
    pub amount_tenths: u64,
    pub previous_xp_tenths: u64,
    pub xp_tenths: u64,
    pub previous_level: u16,
    pub level: u16,
}

impl XpAward {
    pub fn event(&self) -> Option<GameEvent> {
        (self.amount_tenths != 0).then(|| GameEvent::XpGained {
            skill: self.skill.clone(),
            amount_tenths: self.amount_tenths,
        })
    }
}

pub fn validate_definition(definition: &SkillDefinition) -> GameResult<()> {
    if definition.xp_thresholds_tenths.first() != Some(&0)
        || definition.xp_thresholds_tenths.len() > usize::from(u16::MAX)
        || definition
            .xp_thresholds_tenths
            .windows(2)
            .any(|pair| match pair {
                [left, right] => left >= right,
                _ => true,
            })
        || definition
            .xp_thresholds_tenths
            .last()
            .is_some_and(|last| *last > definition.maximum_xp_tenths)
    {
        return Err(GameError::new(
            GameErrorCode::InvalidContent,
            format!(
                "Skill {} needs 1..=65535 strictly increasing thresholds starting at zero, within its XP maximum.",
                definition.id
            ),
        ));
    }
    Ok(())
}

/// Threshold index zero is level one; all values are exact integer XP tenths.
pub fn level_for_xp(definition: &SkillDefinition, xp_tenths: u64) -> GameResult<u16> {
    validate_definition(definition)?;
    if xp_tenths > definition.maximum_xp_tenths {
        return Err(GameError::new(
            GameErrorCode::InvalidInput,
            format!("XP exceeds the source maximum for {}.", definition.id),
        ));
    }
    u16::try_from(
        definition
            .xp_thresholds_tenths
            .partition_point(|threshold| *threshold <= xp_tenths),
    )
    .map_err(|_| {
        GameError::new(
            GameErrorCode::InvalidContent,
            "Skill level is out of range.",
        )
    })
}

pub fn xp_to_next_level(definition: &SkillDefinition, xp_tenths: u64) -> GameResult<Option<u64>> {
    let level = level_for_xp(definition, xp_tenths)?;
    Ok(definition
        .xp_thresholds_tenths
        .get(usize::from(level))
        .map(|next| next - xp_tenths))
}

/// Awards once. A stop level is tested before the award, not used as an XP ceiling.
/// Neither policy changes character hitpoints, prayer points, or other progress.
pub fn award_xp(
    state: &mut SkillState,
    definition: &SkillDefinition,
    amount_tenths: u64,
    limits: XpLimits,
    current_level_policy: CurrentLevelPolicy,
) -> GameResult<XpAward> {
    let previous_level = level_for_xp(definition, state.xp_tenths)?;
    validate_limits(definition, limits)?;
    let ceiling = limits.cap_tenths.unwrap_or(definition.maximum_xp_tenths);
    let room = ceiling.saturating_sub(state.xp_tenths);
    let stopped = limits
        .stop_level
        .is_some_and(|level| previous_level >= level);
    let awarded = if stopped { 0 } else { amount_tenths.min(room) };
    let xp_tenths = state.xp_tenths.checked_add(awarded).ok_or_else(|| {
        GameError::new(
            GameErrorCode::InvalidInput,
            "Award would overflow skill XP.",
        )
    })?;
    let level = level_for_xp(definition, xp_tenths)?;
    let current_level = match current_level_policy {
        CurrentLevelPolicy::Preserve => state.current_level,
        CurrentLevelPolicy::AddBaseLevelGains => state
            .current_level
            .checked_add(level - previous_level)
            .ok_or_else(|| {
                GameError::new(
                    GameErrorCode::InvalidInput,
                    "Base level gain would overflow the temporary current level.",
                )
            })?,
    };
    let award = XpAward {
        skill: definition.id.clone(),
        amount_tenths: awarded,
        previous_xp_tenths: state.xp_tenths,
        xp_tenths,
        previous_level,
        level,
    };
    state.xp_tenths = xp_tenths;
    state.current_level = current_level;
    Ok(award)
}

/// Applies ordered rewards and the character's explicit tutorial stage atomically.
/// Repeated rewards for a skill remain separate awards for stop-level purposes.
pub fn award_character_xp(
    character: &mut CharacterState,
    content: &GameContent,
    rewards: &[XpReward],
    current_level_policy: CurrentLevelPolicy,
) -> GameResult<Vec<GameEvent>> {
    let stage = content
        .tutorial
        .get(&character.tutorial_stage)
        .ok_or_else(|| {
            GameError::new(
                GameErrorCode::UnknownContent,
                format!("Unknown tutorial stage {}.", character.tutorial_stage),
            )
        })?;
    if stage.id != character.tutorial_stage {
        return Err(GameError::new(
            GameErrorCode::InvalidContent,
            "Tutorial stage key does not match its definition.",
        ));
    }
    let mut skills = character.skills.clone();
    let mut events = Vec::new();
    for reward in rewards {
        let definition = skill_definition(&content.skills, &reward.skill)?;
        let state = skills.get_mut(&reward.skill).ok_or_else(|| {
            GameError::new(
                GameErrorCode::InvalidInput,
                format!("Character has no skill state for {}.", reward.skill),
            )
        })?;
        let award = award_xp(
            state,
            definition,
            reward.amount_tenths,
            XpLimits::from_stage(stage, &reward.skill),
            current_level_policy,
        )?;
        events.extend(award.event());
    }
    character.skills = skills;
    Ok(events)
}

pub fn check_requirements(
    states: &BTreeMap<SkillId, SkillState>,
    definitions: &BTreeMap<SkillId, SkillDefinition>,
    requirements: &[SkillRequirement],
    basis: LevelBasis,
) -> GameResult<()> {
    for requirement in requirements {
        if requirement.level == 0 {
            return Err(GameError::new(
                GameErrorCode::InvalidContent,
                "A skill requirement level must be positive.",
            ));
        }
        let definition = skill_definition(definitions, &requirement.skill)?;
        let state = states.get(&requirement.skill).ok_or_else(|| {
            GameError::new(
                GameErrorCode::RequirementNotMet,
                format!("Character lacks required skill {}.", requirement.skill),
            )
        })?;
        let base = level_for_xp(definition, state.xp_tenths)?;
        let actual = match basis {
            LevelBasis::Base => base,
            LevelBasis::Current => state.current_level,
        };
        if actual < requirement.level {
            return Err(GameError::new(
                GameErrorCode::RequirementNotMet,
                format!(
                    "{} requires level {}.",
                    requirement.skill, requirement.level
                ),
            ));
        }
    }
    Ok(())
}

fn skill_definition<'a>(
    definitions: &'a BTreeMap<SkillId, SkillDefinition>,
    skill: &SkillId,
) -> GameResult<&'a SkillDefinition> {
    let definition = definitions.get(skill).ok_or_else(|| {
        GameError::new(
            GameErrorCode::UnknownContent,
            format!("Unknown skill {skill}."),
        )
    })?;
    if definition.id != *skill {
        return Err(GameError::new(
            GameErrorCode::InvalidContent,
            format!("Skill definition key does not match {skill}."),
        ));
    }
    Ok(definition)
}

fn validate_limits(definition: &SkillDefinition, limits: XpLimits) -> GameResult<()> {
    if limits
        .cap_tenths
        .is_some_and(|cap| cap > definition.maximum_xp_tenths)
        || limits.stop_level.is_some_and(|level| {
            level == 0 || usize::from(level) > definition.xp_thresholds_tenths.len()
        })
    {
        return Err(GameError::new(
            GameErrorCode::InvalidContent,
            format!("Invalid XP limits for {}.", definition.id),
        ));
    }
    Ok(())
}
