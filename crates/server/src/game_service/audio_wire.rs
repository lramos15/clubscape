use clubscape_game_types as types;
use clubscape_protocol::game;

pub(super) fn view(value: types::AudioAuthorityView) -> game::AudioAuthority {
    game::AudioAuthority {
        version: value.version,
        profile: value.profile,
        music: Some(game::MusicAuthority {
            history: match value.music.history {
                types::MusicHistoryStatus::FromCreation => game::MusicHistoryStatus::FromCreation,
                types::MusicHistoryStatus::LegacyUntracked => {
                    game::MusicHistoryStatus::LegacyUntracked
                }
            } as i32,
            tracked_from_tick: value.music.tracked_from_tick,
            revision: value.music.revision,
            complete: value.music.complete,
            unlocked_groups: value.music.unlocked_groups,
            tracks: value
                .music
                .tracks
                .into_iter()
                .map(|track| game::MusicUnlock {
                    group: track.group,
                    status: match track.status {
                        types::MusicUnlockStatus::Unlocked => game::MusicUnlockStatus::Unlocked,
                        types::MusicUnlockStatus::Locked => game::MusicUnlockStatus::Locked,
                        types::MusicUnlockStatus::Unknown => game::MusicUnlockStatus::Unknown,
                    } as i32,
                    confirmed_at_tick: track.confirmed_at_tick,
                    rule: track.rule,
                })
                .collect(),
        }),
        varps: value
            .varps
            .into_iter()
            .map(|varp| game::NativeVarp {
                id: varp.id,
                value: varp.value,
                known_bits: varp.known_bits,
                binding: varp.binding,
                unavailable_reason: varp.unavailable_reason,
            })
            .collect(),
    }
}
