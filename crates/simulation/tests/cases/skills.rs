use clubscape_game_types::{
    GameErrorCode::*, GameEvent, SkillRequirement, SkillState, StageId, XpReward,
};
use clubscape_simulation::skills::{self, CurrentLevelPolicy::*, LevelBasis, XpLimits};

use crate::support::*;

#[test]
fn levels_follow_only_explicit_thresholds_at_exact_tenth_boundaries() {
    let definition = skill_definition();
    for (xp, expected) in [
        (0, 1),
        (99, 1),
        (100, 2),
        (300, 2),
        (301, 3),
        (609, 3),
        (610, 4),
        (999, 4),
        (1000, 5),
        (2000, 5),
    ] {
        assert_eq!(skills::level_for_xp(&definition, xp).unwrap(), expected);
    }
    assert_error(skills::level_for_xp(&definition, 2001), InvalidInput);
}

#[test]
fn next_level_distance_has_no_whole_xp_rounding_or_invented_virtual_levels() {
    let definition = skill_definition();
    for (xp, expected) in [
        (0, Some(100)),
        (99, Some(1)),
        (100, Some(201)),
        (300, Some(1)),
        (301, Some(309)),
        (999, Some(1)),
        (1000, None),
        (2000, None),
    ] {
        assert_eq!(skills::xp_to_next_level(&definition, xp).unwrap(), expected);
    }
}

#[test]
fn invalid_source_threshold_tables_and_maxima_are_explicit_errors() {
    for thresholds in [
        vec![],
        vec![1],
        vec![0, 100, 100],
        vec![0, 100, 99],
        vec![0, 2001],
        vec![0; usize::from(u16::MAX) + 1],
    ] {
        let mut definition = skill_definition();
        definition.xp_thresholds_tenths = thresholds;
        assert_error(skills::level_for_xp(&definition, 0), InvalidContent);
    }
}

#[test]
fn a_single_source_level_and_zero_maximum_are_valid_without_defaults() {
    let mut definition = skill_definition();
    definition.xp_thresholds_tenths = vec![0];
    definition.maximum_xp_tenths = 0;
    let mut state = SkillState {
        xp_tenths: 0,
        current_level: 0,
    };
    let before = state.clone();
    let award = skills::award_xp(
        &mut state,
        &definition,
        u64::MAX,
        XpLimits::default(),
        Preserve,
    )
    .unwrap();
    assert_eq!(award.level, 1);
    assert_eq!(award.amount_tenths, 0);
    assert_eq!(state, before);
}

#[test]
fn fractional_threshold_crossing_and_award_events_are_exact() {
    let definition = skill_definition();
    let mut state = SkillState {
        xp_tenths: 300,
        current_level: 2,
    };
    let award = skills::award_xp(
        &mut state,
        &definition,
        1,
        XpLimits::default(),
        AddBaseLevelGains,
    )
    .unwrap();
    assert_eq!(
        state,
        SkillState {
            xp_tenths: 301,
            current_level: 3
        }
    );
    assert_eq!(award.previous_xp_tenths, 300);
    assert_eq!(award.previous_level, 2);
    assert_eq!(award.level, 3);
    assert_eq!(
        award.event(),
        Some(GameEvent::XpGained {
            skill: skill("practice"),
            amount_tenths: 1
        })
    );
}

#[test]
fn exact_xp_cap_clips_an_award_in_tenths() {
    let definition = skill_definition();
    let mut state = SkillState {
        xp_tenths: 300,
        current_level: 8,
    };
    let limits = XpLimits {
        cap_tenths: Some(305),
        stop_level: None,
    };
    let award = skills::award_xp(&mut state, &definition, 20, limits, Preserve).unwrap();
    assert_eq!(award.amount_tenths, 5);
    assert_eq!(
        state,
        SkillState {
            xp_tenths: 305,
            current_level: 8
        }
    );
    let before = state.clone();
    assert_eq!(
        skills::award_xp(&mut state, &definition, 20, limits, Preserve)
            .unwrap()
            .amount_tenths,
        0
    );
    assert_eq!(state, before);
}

#[test]
fn a_lower_stage_cap_never_resets_existing_xp_or_current_levels() {
    let definition = skill_definition();
    let mut state = SkillState {
        xp_tenths: 610,
        current_level: 1,
    };
    let before = state.clone();
    let award = skills::award_xp(
        &mut state,
        &definition,
        90,
        XpLimits {
            cap_tenths: Some(100),
            stop_level: None,
        },
        AddBaseLevelGains,
    )
    .unwrap();
    assert_eq!(award.amount_tenths, 0);
    assert_eq!(award.event(), None);
    assert_eq!(state, before);
}

#[test]
fn a_stop_level_allows_the_final_award_to_cross_its_threshold() {
    let definition = skill_definition();
    let mut state = SkillState {
        xp_tenths: 99,
        current_level: 1,
    };
    let limits = XpLimits {
        cap_tenths: None,
        stop_level: Some(2),
    };
    let award = skills::award_xp(&mut state, &definition, 500, limits, AddBaseLevelGains).unwrap();
    assert_eq!(award.amount_tenths, 500);
    assert_eq!(
        state,
        SkillState {
            xp_tenths: 599,
            current_level: 3
        }
    );
    let before = state.clone();
    assert_eq!(
        skills::award_xp(&mut state, &definition, 500, limits, AddBaseLevelGains)
            .unwrap()
            .event(),
        None
    );
    assert_eq!(state, before);

    let mut exact = SkillState {
        xp_tenths: 99,
        current_level: 1,
    };
    let award = skills::award_xp(
        &mut exact,
        &definition,
        500,
        XpLimits {
            cap_tenths: Some(100),
            stop_level: None,
        },
        Preserve,
    )
    .unwrap();
    assert_eq!(award.amount_tenths, 1);
    assert_eq!(exact.xp_tenths, 100);
}

#[test]
fn stop_levels_use_base_xp_not_temporary_boosts_or_drains() {
    let definition = skill_definition();
    let limits = XpLimits {
        cap_tenths: None,
        stop_level: Some(2),
    };
    let mut state = SkillState {
        xp_tenths: 0,
        current_level: 9,
    };
    assert_eq!(
        skills::award_xp(&mut state, &definition, 101, limits, Preserve)
            .unwrap()
            .amount_tenths,
        101
    );
    state.current_level = 0;
    let before = state.clone();
    assert_eq!(
        skills::award_xp(&mut state, &definition, 90, limits, Preserve)
            .unwrap()
            .amount_tenths,
        0
    );
    assert_eq!(state, before);
}

#[test]
fn simultaneous_ceiling_and_stop_rules_are_distinct_and_both_apply() {
    let definition = skill_definition();
    let mut state = SkillState {
        xp_tenths: 99,
        current_level: 1,
    };
    let limits = XpLimits {
        cap_tenths: Some(305),
        stop_level: Some(2),
    };
    assert_eq!(
        skills::award_xp(&mut state, &definition, 1000, limits, Preserve)
            .unwrap()
            .amount_tenths,
        206
    );
    assert_eq!(state.xp_tenths, 305);
    let before = state.clone();
    assert_eq!(
        skills::award_xp(&mut state, &definition, 1000, limits, Preserve)
            .unwrap()
            .amount_tenths,
        0
    );
    assert_eq!(state, before);
}

#[test]
fn source_maximum_clips_without_integer_overflow_even_for_maximum_requests() {
    let mut definition = skill_definition();
    let mut state = SkillState {
        xp_tenths: 1999,
        current_level: 5,
    };
    assert_eq!(
        skills::award_xp(
            &mut state,
            &definition,
            u64::MAX,
            XpLimits::default(),
            Preserve
        )
        .unwrap()
        .amount_tenths,
        1,
    );
    assert_eq!(state.xp_tenths, 2000);
    definition.maximum_xp_tenths = u64::MAX;
    state.xp_tenths = u64::MAX - 1;
    assert_eq!(
        skills::award_xp(
            &mut state,
            &definition,
            u64::MAX,
            XpLimits::default(),
            Preserve
        )
        .unwrap()
        .amount_tenths,
        1,
    );
    assert_eq!(state.xp_tenths, u64::MAX);
    assert_eq!(
        skills::award_xp(&mut state, &definition, 1, XpLimits::default(), Preserve)
            .unwrap()
            .amount_tenths,
        0,
    );
}

#[test]
fn current_level_policy_is_explicit_and_never_resets_a_boost_or_drain() {
    let definition = skill_definition();
    for (current, policy, expected) in [
        (9, Preserve, 9),
        (0, Preserve, 0),
        (9, AddBaseLevelGains, 11),
        (0, AddBaseLevelGains, 2),
    ] {
        let mut state = SkillState {
            xp_tenths: 99,
            current_level: current,
        };
        skills::award_xp(&mut state, &definition, 202, XpLimits::default(), policy).unwrap();
        assert_eq!(state.xp_tenths, 301);
        assert_eq!(state.current_level, expected);
    }
}

#[test]
fn current_level_overflow_aborts_the_entire_award() {
    let definition = skill_definition();
    let mut state = SkillState {
        xp_tenths: 99,
        current_level: u16::MAX,
    };
    let before = state.clone();
    assert_error(
        skills::award_xp(
            &mut state,
            &definition,
            1,
            XpLimits::default(),
            AddBaseLevelGains,
        ),
        InvalidInput,
    );
    assert_eq!(state, before);
}

#[test]
fn invalid_xp_or_limits_never_clamp_or_partially_change_existing_state() {
    let definition = skill_definition();
    let mut state = SkillState {
        xp_tenths: 2001,
        current_level: 5,
    };
    let before = state.clone();
    assert_error(
        skills::award_xp(&mut state, &definition, 1, XpLimits::default(), Preserve),
        InvalidInput,
    );
    assert_eq!(state, before);
    state.xp_tenths = 0;
    let before = state.clone();
    for limits in [
        XpLimits {
            cap_tenths: Some(2001),
            stop_level: None,
        },
        XpLimits {
            cap_tenths: None,
            stop_level: Some(0),
        },
        XpLimits {
            cap_tenths: None,
            stop_level: Some(6),
        },
    ] {
        assert_error(
            skills::award_xp(&mut state, &definition, 1, limits, Preserve),
            InvalidContent,
        );
        assert_eq!(state, before);
    }
}

#[test]
fn zero_awards_have_no_event_and_leave_temporary_levels_untouched() {
    let definition = skill_definition();
    let mut state = SkillState {
        xp_tenths: 100,
        current_level: 0,
    };
    let before = state.clone();
    let award = skills::award_xp(
        &mut state,
        &definition,
        0,
        XpLimits::default(),
        AddBaseLevelGains,
    )
    .unwrap();
    assert_eq!(award.event(), None);
    assert_eq!(state, before);
}

#[test]
fn character_awards_read_stage_limits_and_preserve_hp_flags_and_other_progress() {
    let mut content = content();
    let mut character = character(&content);
    content
        .tutorial
        .get_mut(&character.tutorial_stage)
        .unwrap()
        .xp_caps_tenths
        .insert(skill("practice"), 650);
    let mut expected = character.clone();
    expected
        .skills
        .get_mut(&skill("practice"))
        .unwrap()
        .xp_tenths = 650;
    let events = skills::award_character_xp(
        &mut character,
        &content,
        &[XpReward {
            skill: skill("practice"),
            amount_tenths: 200,
        }],
        AddBaseLevelGains,
    )
    .unwrap();
    assert_eq!(
        events,
        vec![GameEvent::XpGained {
            skill: skill("practice"),
            amount_tenths: 40
        }]
    );
    assert_eq!(character, expected);
}

#[test]
fn repeated_character_rewards_remain_separate_for_stop_level_evaluation() {
    let mut content = content();
    let mut character = character(&content);
    let state = character.skills.get_mut(&skill("practice")).unwrap();
    state.xp_tenths = 99;
    state.current_level = 0;
    content
        .tutorial
        .get_mut(&character.tutorial_stage)
        .unwrap()
        .xp_stop_levels
        .insert(skill("practice"), 2);
    let events = skills::award_character_xp(
        &mut character,
        &content,
        &[
            XpReward {
                skill: skill("practice"),
                amount_tenths: 202,
            },
            XpReward {
                skill: skill("practice"),
                amount_tenths: 202,
            },
        ],
        AddBaseLevelGains,
    )
    .unwrap();
    assert_eq!(
        events,
        vec![GameEvent::XpGained {
            skill: skill("practice"),
            amount_tenths: 202
        }]
    );
    assert_eq!(
        character.skills.get(&skill("practice")).unwrap(),
        &SkillState {
            xp_tenths: 301,
            current_level: 2
        }
    );
    assert_eq!(character.hitpoints, 3);
}

#[test]
fn multi_skill_awards_are_atomic_when_later_definition_or_state_is_missing() {
    let mut content = content();
    let mut character = character(&content);
    let rewards = [
        XpReward {
            skill: skill("practice"),
            amount_tenths: 100,
        },
        XpReward {
            skill: skill("second"),
            amount_tenths: 100,
        },
    ];
    let before = character.clone();
    assert_error(
        skills::award_character_xp(&mut character, &content, &rewards, Preserve),
        UnknownContent,
    );
    assert_eq!(character, before);
    let mut second = skill_definition();
    second.id = skill("second");
    content.skills.insert(second.id.clone(), second);
    assert_error(
        skills::award_character_xp(&mut character, &content, &rewards, Preserve),
        InvalidInput,
    );
    assert_eq!(character, before);
}

#[test]
fn multi_skill_awards_roll_back_when_later_stage_limits_are_invalid() {
    let mut content = content();
    let mut character = character(&content);
    let mut second = skill_definition();
    second.id = skill("second");
    content.skills.insert(second.id.clone(), second);
    character.skills.insert(
        skill("second"),
        SkillState {
            xp_tenths: 0,
            current_level: 1,
        },
    );
    content
        .tutorial
        .get_mut(&character.tutorial_stage)
        .unwrap()
        .xp_caps_tenths
        .insert(skill("second"), 2001);
    let before = character.clone();
    assert_error(
        skills::award_character_xp(
            &mut character,
            &content,
            &[
                XpReward {
                    skill: skill("practice"),
                    amount_tenths: 100,
                },
                XpReward {
                    skill: skill("second"),
                    amount_tenths: 100,
                },
            ],
            Preserve,
        ),
        InvalidContent,
    );
    assert_eq!(character, before);
}

#[test]
fn unknown_or_mismatched_stage_and_skill_keys_are_explicit_errors() {
    let mut content = content();
    let mut character = character(&content);
    let rewards = [XpReward {
        skill: skill("practice"),
        amount_tenths: 1,
    }];
    let before = character.clone();
    content
        .tutorial
        .get_mut(&character.tutorial_stage)
        .unwrap()
        .id = StageId::new("stage.synthetic.other").unwrap();
    assert_error(
        skills::award_character_xp(&mut character, &content, &rewards, Preserve),
        InvalidContent,
    );
    assert_eq!(character, before);
    content
        .tutorial
        .get_mut(&character.tutorial_stage)
        .unwrap()
        .id = character.tutorial_stage.clone();
    content.skills.get_mut(&skill("practice")).unwrap().id = skill("other");
    assert_error(
        skills::award_character_xp(&mut character, &content, &rewards, Preserve),
        InvalidContent,
    );
    assert_eq!(character, before);
    content.tutorial.clear();
    assert_error(
        skills::award_character_xp(&mut character, &content, &rewards, Preserve),
        UnknownContent,
    );
    assert_eq!(character, before);
}

#[test]
fn generic_requirements_can_explicitly_use_boosted_current_levels() {
    let content = content();
    let mut character = character(&content);
    character
        .skills
        .get_mut(&skill("practice"))
        .unwrap()
        .current_level = 7;
    let requirements = [SkillRequirement {
        skill: skill("practice"),
        level: 6,
        basis: clubscape_game_types::SkillLevelBasis::Base,
    }];
    skills::check_requirements(
        &character.skills,
        &content.skills,
        &requirements,
        LevelBasis::Current,
    )
    .unwrap();
    assert_error(
        skills::check_requirements(
            &character.skills,
            &content.skills,
            &requirements,
            LevelBasis::Base,
        ),
        RequirementNotMet,
    );
}

#[test]
fn all_synthetic_xp_values_have_monotonic_levels_and_exact_next_thresholds() {
    let definition = skill_definition();
    let mut previous = 1;
    for xp in 0..=2000 {
        let actual = skills::level_for_xp(&definition, xp).unwrap();
        let expected = match xp {
            0..=99 => 1,
            100..=300 => 2,
            301..=609 => 3,
            610..=999 => 4,
            _ => 5,
        };
        assert_eq!(actual, expected);
        assert!(actual >= previous);
        previous = actual;
    }
}
