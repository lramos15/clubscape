use clubscape_game_types::*;

use crate::{RandomSource, invalid_content, invalid_state, random, unknown};

pub(crate) const PREFIX: &str = "__world_engine.";

pub(crate) fn schedule(character: &CharacterState) -> GameResult<&EngineSchedule> {
    match &character.runtime.engine {
        EngineMetadata::Typed { schedule } => Ok(schedule),
        EngineMetadata::Legacy => Err(invalid_state("Engine metadata needs checked migration.")),
    }
}

pub(crate) fn schedule_mut(character: &mut CharacterState) -> GameResult<&mut EngineSchedule> {
    match &mut character.runtime.engine {
        EngineMetadata::Typed { schedule } => Ok(schedule),
        EngineMetadata::Legacy => Err(invalid_state("Engine metadata needs checked migration.")),
    }
}

pub(crate) fn deadline(now: u64, delay: u64) -> GameResult<u64> {
    now.checked_add(delay)
        .filter(|tick| *tick <= i64::MAX as u64)
        .ok_or_else(|| invalid_state("Tick deadline overflow."))
}

pub(crate) fn legacy_ticks(ticks: Option<u16>) -> GameResult<u64> {
    ticks
        .filter(|value| *value > 0)
        .map(u64::from)
        .ok_or_else(|| invalid_content("Missing positive legacy cadence."))
}

pub(crate) fn legacy_respawn(ticks: Option<u32>) -> GameResult<u64> {
    ticks
        .filter(|value| *value > 0)
        .map(u64::from)
        .ok_or_else(|| invalid_content("Missing positive legacy respawn."))
}

pub(crate) fn duration(duration: &TickDuration, rng: &mut impl RandomSource) -> GameResult<u64> {
    match duration {
        TickDuration::Fixed { ticks } if *ticks > 0 => Ok(u64::from(*ticks)),
        TickDuration::UniformInclusive { minimum, maximum }
            if *minimum > 0 && minimum <= maximum =>
        {
            let range = maximum
                .checked_sub(*minimum)
                .and_then(|value| value.checked_add(1))
                .ok_or_else(|| invalid_content("Random duration range overflow."))?;
            Ok(u64::from(*minimum) + u64::from(random::draw(rng, range)?))
        }
        _ => Err(invalid_content("Duration must be positive and ordered.")),
    }
}

pub(crate) fn cadence(cadence: &ActionCadence, single: bool, repeat: bool) -> GameResult<u64> {
    let ticks = if repeat {
        cadence.repeat.require()?
    } else if single {
        cadence.single.require()?
    } else {
        cadence.first.require()?
    };
    if *ticks == 0 {
        return Err(invalid_content("Action cadence cannot be zero."));
    }
    Ok(u64::from(*ticks)
        + if repeat {
            0
        } else {
            u64::from(*cadence.menu_delay.require()?)
        })
}

pub(crate) fn close_access(character: &mut CharacterState) -> GameResult<()> {
    schedule_mut(character)?.access = None;
    if let Some(ui) = &mut character.runtime.ui {
        ui.active_interface = None;
        ui.production = None;
        ui.document = None;
        ui.confirmation = None;
        ui.death_preview = false;
    }
    Ok(())
}

pub(crate) fn interrupt(character: &mut CharacterState) -> GameResult<()> {
    character.activity = Activity::Idle;
    character.dialogue = None;
    character.runtime.pending_fire = None;
    character.runtime.combat.target = None;
    let schedule = schedule_mut(character)?;
    schedule.gather_interaction = None;
    schedule.dialogue_interaction = None;
    schedule.access = None;
    if let Some(ui) = &mut character.runtime.ui {
        ui.active_interface = None;
        ui.production = None;
        ui.production_input = None;
        ui.document = None;
        ui.confirmation = None;
        ui.death_preview = false;
    }
    Ok(())
}

pub(crate) fn open_access(
    character: &mut CharacterState,
    bank: bool,
    spawn: &SpawnId,
    index: usize,
) -> GameResult<()> {
    let interaction = u32::try_from(index)
        .ok()
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| invalid_state("Interaction index overflow."))?;
    let session = InteractionSession {
        spawn: spawn.clone(),
        interaction,
    };
    schedule_mut(character)?.access = Some(if bank {
        ContainerSession::Bank { session }
    } else {
        ContainerSession::Shop { session }
    });
    Ok(())
}

pub(crate) fn access(character: &CharacterState, bank: bool) -> GameResult<(&SpawnId, usize)> {
    let session = match (&schedule(character)?.access, bank) {
        (Some(ContainerSession::Bank { session }), true)
        | (Some(ContainerSession::Shop { session }), false) => session,
        _ => {
            return Err(GameError::new(
                GameErrorCode::RequirementNotMet,
                "No matching open container session.",
            ));
        }
    };
    Ok((&session.spawn, session.interaction as usize - 1))
}

pub(crate) fn interaction<'a>(
    content: &'a GameContent,
    spawn: &SpawnId,
    index: usize,
) -> GameResult<&'a InteractionDefinition> {
    content
        .spawns
        .get(spawn)
        .ok_or_else(|| unknown(format!("Unknown spawn {spawn}.")))?
        .interactions
        .get(index)
        .ok_or_else(|| invalid_state("Persisted interaction no longer exists in this revision."))
}

pub(crate) fn location(character: &CharacterState) -> RuntimeLocation {
    RuntimeLocation {
        region: character.region.clone(),
        tile: character.tile,
        instance: character.runtime.instance.clone(),
    }
}

pub(crate) fn entity<'a>(
    world: &'a WorldState,
    instance: Option<&InstanceId>,
    id: &SpawnId,
) -> GameResult<&'a EntityState> {
    let entities = match instance {
        Some(id) => {
            &world
                .runtime
                .instances
                .get(id)
                .ok_or_else(|| invalid_state("Unknown live instance."))?
                .entities
        }
        None => &world.entities,
    };
    entities
        .get(id)
        .ok_or_else(|| unknown(format!("No live entity {id} in this instance.")))
}

pub(crate) fn entity_mut<'a>(
    world: &'a mut WorldState,
    instance: Option<&InstanceId>,
    id: &SpawnId,
) -> GameResult<&'a mut EntityState> {
    let entities = match instance {
        Some(id) => {
            &mut world
                .runtime
                .instances
                .get_mut(id)
                .ok_or_else(|| invalid_state("Unknown live instance."))?
                .entities
        }
        None => &mut world.entities,
    };
    entities
        .get_mut(id)
        .ok_or_else(|| unknown(format!("No live entity {id} in this instance.")))
}
