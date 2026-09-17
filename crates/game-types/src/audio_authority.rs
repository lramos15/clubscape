//! Server-owned source observations, separate from client audio preferences and playback.
use serde::{Deserialize, Serialize};

pub const AUDIO_AUTHORITY_VERSION: u32 = 1;
pub const AUDIO_AUTHORITY_CAPABILITY: &str = "game.audio.authority.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioAuthorityView {
    pub version: u32,
    pub profile: String,
    pub music: MusicAuthorityView,
    pub varps: Vec<NativeVarpView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MusicHistoryStatus {
    FromCreation,
    LegacyUntracked,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MusicUnlockStatus {
    Unlocked,
    Locked,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicAuthorityView {
    pub history: MusicHistoryStatus,
    pub tracked_from_tick: String,
    pub revision: String,
    /// True only when every declared track has a known state; legacy absence is not locked.
    pub complete: bool,
    pub unlocked_groups: Vec<u32>,
    pub tracks: Vec<MusicUnlockView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicUnlockView {
    pub group: u32,
    pub status: MusicUnlockStatus,
    /// First authoritative confirmation, not an invented timestamp for an earlier visit.
    pub confirmed_at_tick: Option<String>,
    pub rule: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeVarpView {
    pub id: u32,
    pub value: Option<i32>,
    /// Only these bits are source-bound; consumers must not interpret the remaining word.
    pub known_bits: u32,
    pub binding: String,
    pub unavailable_reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_unknown_is_distinct_from_locked_or_a_fresh_empty_history() {
        let view = MusicAuthorityView {
            history: MusicHistoryStatus::LegacyUntracked,
            tracked_from_tick: "9007199254740993".into(),
            revision: "0".into(),
            complete: false,
            unlocked_groups: Vec::new(),
            tracks: vec![MusicUnlockView {
                group: 62,
                status: MusicUnlockStatus::Unknown,
                confirmed_at_tick: None,
                rule: None,
            }],
        };
        let json = serde_json::to_string(&view).unwrap();
        assert!(json.contains("\"status\":\"unknown\""));
        assert_eq!(
            serde_json::from_str::<MusicAuthorityView>(&json).unwrap(),
            view
        );
        assert!(
            serde_json::from_str::<MusicAuthorityView>(
                r#"{"history":"from_creation","unlocked_groups":[]}"#
            )
            .is_err()
        );
    }
}
