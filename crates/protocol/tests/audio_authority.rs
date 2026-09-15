use clubscape_protocol::game;
use prost::Message;

#[test]
fn read_only_audio_authority_preserves_unknown_history_and_explicit_zero_bits() {
    let snapshot = game::WorldSnapshot {
        audio_authority: Some(game::AudioAuthority {
            version: 1,
            profile: "source_audio.m1".into(),
            music: Some(game::MusicAuthority {
                history: game::MusicHistoryStatus::LegacyUntracked as i32,
                tracked_from_tick: "9007199254740993".into(),
                revision: "0".into(),
                complete: false,
                unlocked_groups: Vec::new(),
                tracks: vec![game::MusicUnlock {
                    group: 62,
                    status: game::MusicUnlockStatus::Unknown as i32,
                    confirmed_at_tick: None,
                    rule: None,
                }],
            }),
            varps: vec![game::NativeVarp {
                id: 491,
                value: Some(0),
                known_bits: 20,
                binding: "native_varp.abyssal_warp".into(),
                unavailable_reason: None,
            }],
        }),
        ..Default::default()
    };
    let restored = game::WorldSnapshot::decode(snapshot.encode_to_vec().as_slice()).unwrap();
    assert_eq!(restored, snapshot);
    let authority = restored.audio_authority.unwrap();
    assert_eq!(authority.varps[0].value, Some(0));
    assert_eq!(authority.varps[0].known_bits, 20);
    assert_eq!(
        authority.music.unwrap().tracks[0].status,
        game::MusicUnlockStatus::Unknown as i32
    );
    assert!(
        game::WorldSnapshot::decode([].as_slice())
            .unwrap()
            .audio_authority
            .is_none()
    );
}
