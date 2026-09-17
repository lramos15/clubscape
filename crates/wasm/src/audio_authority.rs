use std::collections::BTreeSet;

use clubscape_protocol::game;
use serde_json::{Value, json};

use crate::{BridgeError, gameplay_ui::decimal};

pub(crate) const CAPABILITY: &str = "game.audio.authority.v1";

fn invalid() -> BridgeError {
    BridgeError::protocol("The source audio authority view is incomplete or inconsistent.")
}

pub(crate) fn project(value: &game::AudioAuthority) -> Result<Value, BridgeError> {
    if value.version != 1 || value.profile.is_empty() || value.profile.len() > 256 {
        return Err(invalid());
    }
    let music = value.music.as_ref().ok_or_else(invalid)?;
    let history = match game::MusicHistoryStatus::try_from(music.history).map_err(|_| invalid())? {
        game::MusicHistoryStatus::FromCreation => "from_creation",
        game::MusicHistoryStatus::LegacyUntracked => "legacy_untracked",
        game::MusicHistoryStatus::MusicHistoryUnspecified => return Err(invalid()),
    };
    let mut groups = BTreeSet::new();
    let mut unlocked = BTreeSet::new();
    let mut unknown = false;
    if music.tracks.len() > 4096 || value.varps.len() > 4096 {
        return Err(invalid());
    }
    let tracks = music.tracks.iter().map(|track| {
        if track.group == 0 || !groups.insert(track.group) {
            return Err(invalid());
        }
        let status = match game::MusicUnlockStatus::try_from(track.status).map_err(|_| invalid())? {
            game::MusicUnlockStatus::Unlocked => {
                unlocked.insert(track.group);
                if track.confirmed_at_tick.is_none() || track.rule.as_ref().is_none_or(|rule| rule.is_empty()) {
                    return Err(invalid());
                }
                "unlocked"
            }
            game::MusicUnlockStatus::Locked => {
                if history != "from_creation" || track.confirmed_at_tick.is_some() || track.rule.is_some() {
                    return Err(invalid());
                }
                "locked"
            }
            game::MusicUnlockStatus::Unknown => {
                unknown = true;
                if history != "legacy_untracked" || track.confirmed_at_tick.is_some() || track.rule.is_some() {
                    return Err(invalid());
                }
                "unknown"
            }
            game::MusicUnlockStatus::MusicUnlockUnspecified => return Err(invalid()),
        };
        Ok(json!({
            "group":track.group,"status":status,
            "confirmedAtTick":track.confirmed_at_tick.as_deref().map(|value| decimal(value, false)).transpose()?,
            "rule":track.rule,
        }))
    }).collect::<Result<Vec<_>, BridgeError>>()?;
    if music.complete == unknown
        || music
            .unlocked_groups
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            != unlocked
        || music.unlocked_groups.len() != unlocked.len()
    {
        return Err(invalid());
    }
    let mut ids = BTreeSet::new();
    let varps = value
        .varps
        .iter()
        .map(|varp| {
            if !ids.insert(varp.id)
                || varp.binding.is_empty()
                || varp.binding.len() > 256
                || match varp.value {
                    Some(_) => varp.known_bits == 0 || varp.unavailable_reason.is_some(),
                    None => {
                        varp.known_bits != 0
                            || varp
                                .unavailable_reason
                                .as_ref()
                                .is_none_or(|reason| reason.is_empty())
                    }
                }
            {
                return Err(invalid());
            }
            Ok(json!({
                "id":varp.id,"value":varp.value,"knownBits":varp.known_bits,
                "binding":varp.binding,"unavailableReason":varp.unavailable_reason,
            }))
        })
        .collect::<Result<Vec<_>, BridgeError>>()?;
    Ok(json!({
        "version":value.version,"profile":value.profile,
        "music":{
            "history":history,"trackedFromTick":decimal(&music.tracked_from_tick, false)?,
            "revision":decimal(&music.revision, false)?,"complete":music.complete,
            "unlockedGroups":music.unlocked_groups,"tracks":tracks,
        },
        "varps":varps,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy() -> game::AudioAuthority {
        game::AudioAuthority {
            version: 1,
            profile: "source.audio.fixture".into(),
            music: Some(game::MusicAuthority {
                history: game::MusicHistoryStatus::LegacyUntracked as i32,
                tracked_from_tick: "9007199254740993".into(),
                revision: "18446744073709551615".into(),
                complete: false,
                unlocked_groups: vec![62],
                tracks: vec![
                    game::MusicUnlock {
                        group: 62,
                        status: game::MusicUnlockStatus::Unlocked as i32,
                        confirmed_at_tick: Some("9007199254740993".into()),
                        rule: Some("music.62.actual".into()),
                    },
                    game::MusicUnlock {
                        group: 76,
                        status: game::MusicUnlockStatus::Unknown as i32,
                        ..Default::default()
                    },
                ],
            }),
            varps: vec![game::NativeVarp {
                id: 491,
                value: Some(0),
                known_bits: 20,
                binding: "source.491.bits2and4".into(),
                unavailable_reason: None,
            }],
        }
    }

    #[test]
    fn unknown_history_and_partial_zero_word_remain_explicit_with_exact_u64_strings() {
        let view = project(&legacy()).unwrap();
        assert_eq!(view["music"]["history"], "legacy_untracked");
        assert_eq!(view["music"]["tracks"][1]["status"], "unknown");
        assert_eq!(view["music"]["revision"], "18446744073709551615");
        assert_eq!(view["music"]["trackedFromTick"], "9007199254740993");
        assert_eq!(view["varps"][0]["knownBits"], 20);
        assert_eq!(view["varps"][0]["value"], 0);
    }

    #[test]
    fn missing_authority_or_inconsistent_completeness_grants_and_known_bits_reject() {
        let mut value = legacy();
        value.music = None;
        assert!(project(&value).is_err());
        value = legacy();
        value.music.as_mut().unwrap().complete = true;
        assert!(project(&value).is_err());
        value = legacy();
        value.music.as_mut().unwrap().unlocked_groups.push(76);
        assert!(project(&value).is_err());
        value = legacy();
        value.music.as_mut().unwrap().tracks[1].status = game::MusicUnlockStatus::Locked as i32;
        assert!(project(&value).is_err());
        value = legacy();
        value.varps[0].value = None;
        assert!(project(&value).is_err());
        value = legacy();
        value.varps[0].known_bits = 0;
        assert!(project(&value).is_err());
    }
}
