use clubscape_game_types::*;
use std::collections::BTreeMap;

use crate::{WorldEngine, invalid_content, invalid_state, unavailable};

impl WorldEngine {
    fn audio_definition(&self) -> Option<&AudioAuthorityDefinition> {
        self.content
            .ui
            .as_ref()
            .and_then(|ui| ui.audio_authority.as_ref())
    }

    pub(crate) fn initialize_audio_history(
        &self,
        character: &mut CharacterState,
        tick: u64,
        history: MusicHistoryStatus,
    ) -> GameResult<()> {
        let Some(definition) = self.audio_definition() else {
            return Ok(());
        };
        if character.runtime.audio_authority.is_some() {
            return Err(invalid_state(
                "Audio history must not be initialized twice.",
            ));
        }
        character.runtime.audio_authority = Some(AudioAuthorityRuntime {
            version: AUDIO_AUTHORITY_VERSION,
            profile: definition.profile.clone(),
            history,
            tracked_from_tick: tick,
            revision: 0,
            unlocks: BTreeMap::new(),
        });
        self.observe_audio_position(character, tick, character.tile)?;
        self.observe_audio_facts(character, tick)
    }

    pub(crate) fn observe_audio_position(
        &self,
        character: &mut CharacterState,
        tick: u64,
        tile: Tile,
    ) -> GameResult<()> {
        let Some(definition) = self.audio_definition() else {
            if character.runtime.audio_authority.is_some() {
                return Err(unavailable(
                    "Existing audio history requires its source profile.",
                ));
            }
            return Ok(());
        };
        let history =
            character.runtime.audio_authority.as_ref().ok_or_else(|| {
                invalid_state("Untracked audio history requires explicit migration.")
            })?;
        let areas: BTreeMap<_, _> = definition
            .areas
            .iter()
            .map(|(id, area)| {
                (
                    id,
                    character.runtime.instance.is_none() && area.contains(tile),
                )
            })
            .collect();
        let confirmations = definition
            .tracks
            .iter()
            .filter(|(group, _)| !history.unlocks.contains_key(group))
            .filter_map(|(group, track)| {
                if track.automatic {
                    Some((*group, format!("music.{group}.automatic")))
                } else {
                    track
                        .area
                        .as_ref()
                        .filter(|area| areas.get(area).copied() == Some(true))
                        .map(|area| (*group, format!("music.{group}.area.{area}")))
                }
            })
            .collect::<Vec<_>>();
        confirm(character, tick, confirmations)
    }

    pub(crate) fn observe_audio_facts(
        &self,
        character: &mut CharacterState,
        tick: u64,
    ) -> GameResult<()> {
        let Some(definition) = self.audio_definition() else {
            return Ok(());
        };
        let history = character
            .runtime
            .audio_authority
            .as_ref()
            .ok_or_else(|| invalid_state("Audio fact observation lost its history."))?;
        let mut confirmations = Vec::new();
        for (group, track) in &definition.tracks {
            if history.unlocks.contains_key(group) {
                continue;
            }
            if let Some(counter) = &track.conserved {
                match character.runtime.counters.get(counter) {
                    Some(CounterValue::Boolean(true)) => {
                        confirmations.push((*group, format!("music.{group}.conserved")))
                    }
                    Some(CounterValue::Boolean(false)) => {}
                    _ => {
                        return Err(invalid_state(
                            "A conserved music fact is missing or has the wrong type.",
                        ));
                    }
                }
            }
        }
        confirm(character, tick, confirmations)
    }

    pub(crate) fn observe_world_audio(&self, world: &mut WorldState) -> GameResult<()> {
        if self.audio_definition().is_none() {
            return Ok(());
        }
        for character in world.characters.values_mut() {
            self.observe_audio_position(character, world.tick, character.tile)?;
            self.observe_audio_facts(character, world.tick)?;
        }
        Ok(())
    }

    pub(crate) fn migrate_audio_history(&self, world: &mut WorldState) -> GameResult<()> {
        let Some(_) = self.audio_definition() else {
            if world.runtime.audio_authority_version.is_some()
                || world
                    .characters
                    .values()
                    .any(|character| character.runtime.audio_authority.is_some())
            {
                return Err(invalid_state(
                    "A content migration cannot discard audio history.",
                ));
            }
            return Ok(());
        };
        if world.runtime.audio_authority_version.is_none() {
            if world
                .characters
                .values()
                .any(|character| character.runtime.audio_authority.is_some())
            {
                return Err(invalid_state(
                    "An unversioned world contains audio history.",
                ));
            }
            for character in world.characters.values_mut() {
                self.initialize_audio_history(
                    character,
                    world.tick,
                    MusicHistoryStatus::LegacyUntracked,
                )?;
            }
            world.runtime.audio_authority_version = Some(AUDIO_AUTHORITY_VERSION);
        }
        Ok(())
    }

    pub fn audio_authority_view(
        &self,
        world: &WorldState,
        actor: &ActorId,
    ) -> GameResult<AudioAuthorityView> {
        self.check_world(world)?;
        let definition = self
            .audio_definition()
            .ok_or_else(|| unavailable("game.audio.authority.v1 is not configured."))?;
        let character = world.characters.get(actor).ok_or_else(|| {
            GameError::new(GameErrorCode::NotOwned, "Unknown audio authority owner.")
        })?;
        let history = character
            .runtime
            .audio_authority
            .as_ref()
            .ok_or_else(|| invalid_state("Source audio authority has no tracked history."))?;
        let tracks: Vec<_> = definition
            .tracks
            .keys()
            .map(|group| {
                let confirmed = history.unlocks.get(group);
                MusicUnlockView {
                    group: *group,
                    status: if confirmed.is_some() {
                        MusicUnlockStatus::Unlocked
                    } else if history.history == MusicHistoryStatus::FromCreation {
                        MusicUnlockStatus::Locked
                    } else {
                        MusicUnlockStatus::Unknown
                    },
                    confirmed_at_tick: confirmed.map(|entry| entry.tick.to_string()),
                    rule: confirmed.map(|entry| entry.rule.clone()),
                }
            })
            .collect();
        let varps = definition
            .varps
            .iter()
            .map(|(id, variable)| self.native_varp_view(world, character, *id, variable))
            .collect::<GameResult<_>>()?;
        Ok(AudioAuthorityView {
            version: AUDIO_AUTHORITY_VERSION,
            profile: definition.profile.clone(),
            music: MusicAuthorityView {
                history: history.history.clone(),
                tracked_from_tick: history.tracked_from_tick.to_string(),
                revision: history.revision.to_string(),
                complete: tracks
                    .iter()
                    .all(|track| track.status != MusicUnlockStatus::Unknown),
                unlocked_groups: history.unlocks.keys().copied().collect(),
                tracks,
            },
            varps,
        })
    }

    fn native_varp_view(
        &self,
        world: &WorldState,
        character: &CharacterState,
        id: u32,
        definition: &NativeVarpDefinition,
    ) -> GameResult<NativeVarpView> {
        let mut word = 0_u32;
        let mut known_bits = 0_u32;
        for field in &definition.fields {
            if field.width == 0
                || field.width > 32
                || u16::from(field.lsb) + u16::from(field.width) > 32
            {
                return Err(invalid_content(
                    "Native variable has an invalid source bit width.",
                ));
            }
            let mask = (u32::MAX >> (32 - field.width)) << field.lsb;
            if known_bits & mask != 0 {
                return Err(invalid_content(
                    "Native variable source bit fields overlap.",
                ));
            }
            let mut selected = None;
            for case in &field.cases {
                if self.guard(world, character, &case.guard, None)?
                    && selected.replace(case.value).is_some()
                {
                    return Err(invalid_content("Native variable source cases overlap."));
                }
            }
            let Some(value) = selected else {
                return Ok(NativeVarpView {
                    id,
                    value: None,
                    known_bits: 0,
                    binding: definition.binding.clone(),
                    unavailable_reason: Some(
                        "No source-bound variable case matches the actual game facts.".into(),
                    ),
                });
            };
            if value > u32::MAX >> (32 - field.width) {
                return Err(invalid_content(
                    "Native variable source value exceeds its field.",
                ));
            }
            word |= value << field.lsb;
            known_bits |= mask;
        }
        Ok(NativeVarpView {
            id,
            value: Some(i32::from_ne_bytes(word.to_ne_bytes())),
            known_bits,
            binding: definition.binding.clone(),
            unavailable_reason: None,
        })
    }
}

fn confirm(
    character: &mut CharacterState,
    tick: u64,
    confirmations: Vec<(u32, String)>,
) -> GameResult<()> {
    let history = character
        .runtime
        .audio_authority
        .as_mut()
        .ok_or_else(|| invalid_state("Audio confirmation lost its history."))?;
    if tick < history.tracked_from_tick || tick > i64::MAX as u64 {
        return Err(invalid_state(
            "Audio confirmation is outside its authoritative clock.",
        ));
    }
    let mut changed = false;
    for (group, rule) in confirmations {
        if let std::collections::btree_map::Entry::Vacant(entry) = history.unlocks.entry(group) {
            entry.insert(MusicConfirmation { tick, rule });
            changed = true;
        }
    }
    if changed {
        history.revision = history
            .revision
            .checked_add(1)
            .ok_or_else(|| invalid_state("Audio history revision overflow."))?;
    }
    history.validate_shape()
}
