mod support;

use std::sync::Arc;

use clubscape_game_types::*;
use clubscape_world_engine::WorldEngine;
use support::*;

#[test]
fn dialogue_requires_open_speaker_and_current_node_not_remote_injection() {
    let mut content = content();
    add_dialogue(&mut content);
    let (engine, mut world) = setup(content);
    error_unchanged(
        &engine,
        &mut world,
        select("continue"),
        GameErrorCode::RequirementNotMet,
    );
    apply(&engine, &mut world, interact("cook"));
    next(&engine, &mut world);
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::SelectDialogue {
            speaker: spawn("bank"),
            choice: "continue".into(),
        },
        GameErrorCode::NotOwned,
    );
    error_unchanged(
        &engine,
        &mut world,
        select("finish"),
        GameErrorCode::InvalidInput,
    );
    apply(&engine, &mut world, select("continue"));
    assert_eq!(state(&world).dialogue.as_ref().unwrap().node, "last");
    next(&engine, &mut world);
    error_unchanged(
        &engine,
        &mut world,
        select("continue"),
        GameErrorCode::InvalidInput,
    );
    apply(&engine, &mut world, select("finish"));
    assert!(state(&world).dialogue.is_none());
    assert_eq!(state(&world).flags["finished"], 1);
}

#[test]
fn dialogue_revalidates_range_after_open_and_rejects_locked_choice() {
    let mut content = content();
    add_dialogue(&mut content);
    content.dialogues.get_mut(&dialogue()).unwrap().nodes[0].choices[0].guard = Guard::Flag {
        name: "permission".into(),
        equals: 1,
    };
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("cook"));
    next(&engine, &mut world);
    error_unchanged(
        &engine,
        &mut world,
        select("continue"),
        GameErrorCode::RequirementNotMet,
    );
    state_mut(&mut world).flags.insert("permission".into(), 1);
    world.entities.get_mut(&spawn("cook")).unwrap().tile = tile(20, 20, 0);
    error_unchanged(
        &engine,
        &mut world,
        select("continue"),
        GameErrorCode::OutOfReach,
    );
}

#[test]
fn locked_next_node_rolls_back_choice_effects_and_dialogue() {
    let mut content = content();
    add_dialogue(&mut content);
    content.dialogues.get_mut(&dialogue()).unwrap().nodes[1].guard = Guard::Flag {
        name: "future_unlocked".into(),
        equals: 1,
    };
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("cook"));
    next(&engine, &mut world);
    error_unchanged(
        &engine,
        &mut world,
        select("continue"),
        GameErrorCode::RequirementNotMet,
    );
    assert!(!state(&world).flags.contains_key("visited"));
    assert_eq!(state(&world).dialogue.as_ref().unwrap().node, "entry");
}

#[test]
fn undefined_dialogue_nodes_and_event_names_are_explicit_content_errors() {
    let mut content = content();
    add_dialogue(&mut content);
    content.dialogues.get_mut(&dialogue()).unwrap().nodes[0].choices[0].next_node =
        Some("not_defined".into());
    assert_eq!(
        WorldEngine::new(Arc::new(content)).unwrap_err().code,
        GameErrorCode::UnknownContent
    );
    let mut content = support::content();
    content
        .tutorial
        .get_mut(&stage("start"))
        .unwrap()
        .transitions
        .push(ProgressTransition {
            event: "advance_tutorial".into(),
            target: None,
            guard: Guard::Always,
            effects: vec![],
        });
    assert_eq!(
        WorldEngine::new(Arc::new(content)).unwrap_err().code,
        GameErrorCode::InvalidContent
    );
}

#[test]
fn missing_flag_is_not_true_under_negation_and_guards_are_authoritative() {
    let mut content = content();
    interaction(&mut content, "bank").guard = Guard::Not {
        guard: Box::new(Guard::Flag {
            name: "missing".into(),
            equals: 1,
        }),
    };
    let (engine, mut world) = setup(content);
    error_unchanged(
        &engine,
        &mut world,
        interact("bank"),
        GameErrorCode::RequirementNotMet,
    );
    state_mut(&mut world).flags.insert("missing".into(), 0);
    apply(&engine, &mut world, interact("bank"));
}

#[test]
fn guard_item_duplicates_require_the_sum_not_reusing_one_item_twice() {
    let mut content = content();
    interaction(&mut content, "bank").guard = Guard::HasItems {
        items: vec![stack("pick", 1), stack("pick", 1)],
    };
    let (engine, mut world) = setup(content);
    error_unchanged(
        &engine,
        &mut world,
        interact("bank"),
        GameErrorCode::RequirementNotMet,
    );
}

#[test]
fn stage_action_permissions_and_unlocked_interfaces_are_not_client_assertions() {
    let mut content = content();
    content
        .tutorial
        .get_mut(&stage("start"))
        .unwrap()
        .allowed_actions = vec!["walk".into(), "open_interface".into()];
    content.initial_state.interfaces.clear();
    let (engine, mut world) = setup(content);
    error_unchanged(
        &engine,
        &mut world,
        interact("rock"),
        GameErrorCode::RequirementNotMet,
    );
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::OpenInterface {
            interface: interface(),
        },
        GameErrorCode::RequirementNotMet,
    );
    apply(&engine, &mut world, GameIntent::RequestLogout);
}

#[test]
fn effects_roll_back_item_xp_flag_and_stage_together() {
    let mut content = content();
    give_initial(&mut content, &[stack("pebble", 26)]);
    add_object(
        &mut content,
        "reward",
        InteractionAction::Effects {
            effects: vec![
                Effect::SetFlag {
                    name: "received".into(),
                    value: 1,
                },
                Effect::SetTutorialStage {
                    stage: stage("next"),
                },
                Effect::AwardXp {
                    rewards: vec![XpReward {
                        skill: skill(),
                        amount_tenths: 99,
                    }],
                },
                Effect::GiveItems {
                    items: vec![stack("egg", 1)],
                },
            ],
        },
    );
    let (engine, mut world) = setup(content);
    error_unchanged(
        &engine,
        &mut world,
        interact("reward"),
        GameErrorCode::InventoryFull,
    );
}

#[test]
fn canonical_targets_are_exact_and_none_is_a_wildcard() {
    for (target, advances) in [
        (Some(item("ore").to_string()), false),
        (Some(spawn("rock").to_string()), true),
        (None, true),
    ] {
        let mut content = content();
        content
            .tutorial
            .get_mut(&stage("start"))
            .unwrap()
            .transitions
            .push(ProgressTransition {
                event: "gathered".into(),
                target,
                guard: Guard::Always,
                effects: vec![Effect::SetTutorialStage {
                    stage: stage("next"),
                }],
            });
        let (engine, mut world) = setup(content);
        apply(&engine, &mut world, interact("rock"));
        ticks(&engine, &mut world, 8, &mut Fixed(0));
        assert_eq!(
            state(&world).tutorial_stage,
            if advances {
                stage("next")
            } else {
                stage("start")
            }
        );
    }
}

#[test]
fn one_operation_cannot_chain_through_a_second_stage_or_repeat_activity_xp() {
    let mut content = content();
    for (from, to) in [("start", "next"), ("next", "last")] {
        content
            .tutorial
            .get_mut(&stage(from))
            .unwrap()
            .transitions
            .push(ProgressTransition {
                event: "gathered".into(),
                target: None,
                guard: Guard::Always,
                effects: vec![Effect::SetTutorialStage { stage: stage(to) }],
            });
    }

    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("rock"));
    let events = ticks(&engine, &mut world, 8, &mut Fixed(0));
    assert_eq!(state(&world).tutorial_stage, stage("next"));
    assert_eq!(state(&world).skills[&skill()].xp_tenths, 175);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event.event, GameEvent::TutorialAdvanced { .. }))
            .count(),
        1
    );
    // Effective stage permissions preserve an earlier activity for legitimate recovery.
    ticks(&engine, &mut world, 4, &mut NeverDraw);
    apply(&engine, &mut world, interact("rock"));
    ticks(&engine, &mut world, 8, &mut Fixed(0));
    assert_eq!(state(&world).tutorial_stage, stage("last"));
    assert_eq!(state(&world).skills[&skill()].xp_tenths, 350);
}

#[test]
fn direct_dialogue_stage_effect_does_not_enable_a_second_transition_in_that_operation() {
    let mut content = content();
    add_dialogue(&mut content);
    content.dialogues.get_mut(&dialogue()).unwrap().nodes[0].choices[0]
        .effects
        .push(Effect::SetTutorialStage {
            stage: stage("next"),
        });
    content
        .tutorial
        .get_mut(&stage("next"))
        .unwrap()
        .transitions
        .push(ProgressTransition {
            event: "dialogue_selected".into(),
            target: None,
            guard: Guard::Always,
            effects: vec![Effect::SetTutorialStage {
                stage: stage("last"),
            }],
        });
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("cook"));
    next(&engine, &mut world);
    apply(&engine, &mut world, select("continue"));
    assert_eq!(state(&world).tutorial_stage, stage("next"));
}

#[test]
fn ambiguous_graph_edges_reject_the_whole_tick_including_resource_depletion() {
    let mut content = content();
    for target in [None, Some(spawn("rock").to_string())] {
        content
            .tutorial
            .get_mut(&stage("start"))
            .unwrap()
            .transitions
            .push(ProgressTransition {
                event: "gathered".into(),
                target,
                guard: Guard::Always,
                effects: vec![Effect::SetTutorialStage {
                    stage: stage("next"),
                }],
            });
    }
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("rock"));
    ticks(&engine, &mut world, 7, &mut NeverDraw);
    let before = world.clone();
    assert_eq!(
        engine.tick(&mut world, &mut Fixed(0)).unwrap_err().code,
        GameErrorCode::InvalidContent
    );
    assert_eq!(world, before);
}

#[test]
fn a_failed_progression_grant_cancels_activity_without_publishing_gather_or_xp() {
    let mut content = content();
    give_initial(&mut content, &[stack("pebble", 25)]);
    content
        .tutorial
        .get_mut(&stage("start"))
        .unwrap()
        .transitions
        .push(ProgressTransition {
            event: "gathered".into(),
            target: None,
            guard: Guard::Always,
            effects: vec![Effect::GiveItems {
                items: vec![stack("egg", 1)],
            }],
        });
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("rock"));
    let events = ticks(&engine, &mut world, 8, &mut Fixed(0));
    assert_eq!(count(&engine, &world, "ore"), 0);
    assert_eq!(state(&world).skills[&skill()].xp_tenths, 0);
    assert_eq!(world.entities[&spawn("rock")].available_at_tick, 0);
    assert!(
        events
            .iter()
            .all(|event| matches!(event.event, GameEvent::Message { .. }))
    );
}

#[test]
fn preowned_ingredients_and_all_six_partial_delivery_orders_reward_exactly_once() {
    for order in [
        ["milk", "flour", "egg"],
        ["milk", "egg", "flour"],
        ["flour", "milk", "egg"],
        ["flour", "egg", "milk"],
        ["egg", "milk", "flour"],
        ["egg", "flour", "milk"],
    ] {
        let mut content = content();
        add_quest(&mut content);
        give_initial(
            &mut content,
            &[stack("milk", 1), stack("flour", 1), stack("egg", 1)],
        );
        let (engine, mut world) = setup(content);
        apply(&engine, &mut world, interact("cook"));
        next(&engine, &mut world);
        apply(&engine, &mut world, select("accept"));
        for ingredient in order {
            next(&engine, &mut world);
            apply(&engine, &mut world, interact("cook"));
            next(&engine, &mut world);
            apply(
                &engine,
                &mut world,
                select(&format!("deliver_{ingredient}")),
            );
            assert_eq!(count(&engine, &world, ingredient), 0);
            assert_eq!(state(&world).quest_points, 0);
            assert_eq!(state(&world).skills[&skill()].xp_tenths, 0);
        }
        next(&engine, &mut world);
        apply(&engine, &mut world, interact("cook"));
        next(&engine, &mut world);
        let events = apply(&engine, &mut world, select("thanks"));
        assert_eq!(state(&world).quest_points, 1);
        assert_eq!(state(&world).skills[&skill()].xp_tenths, 3000);
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event.event, GameEvent::XpGained { .. }))
                .count(),
            1
        );
        next(&engine, &mut world);
        world = serde_json::from_str(&serde_json::to_string(&world).unwrap()).unwrap();
        apply(&engine, &mut world, interact("cook"));
        next(&engine, &mut world);
        error_unchanged(
            &engine,
            &mut world,
            select("thanks"),
            GameErrorCode::InvalidInput,
        );
        assert_eq!(state(&world).quest_points, 1);
        assert_eq!(
            state(&world).quests[&quest()].stage,
            stage("quest_completed")
        );
    }
}

#[test]
fn quest_points_without_new_quest_completion_are_rejected_atomically() {
    let mut content = content();
    add_object(
        &mut content,
        "bad_reward",
        InteractionAction::Effects {
            effects: vec![
                Effect::AddQuestPoints { amount: 1 },
                Effect::GiveItems {
                    items: vec![stack("coins", 100)],
                },
            ],
        },
    );
    let (engine, mut world) = setup(content);
    error_unchanged(
        &engine,
        &mut world,
        interact("bad_reward"),
        GameErrorCode::InvalidContent,
    );
}

#[test]
fn completed_quests_cannot_be_reset_for_reward_replay() {
    let mut content = content();
    add_quest(&mut content);
    content
        .initial_state
        .quests
        .get_mut(&quest())
        .unwrap()
        .stage = stage("quest_completed");
    add_object(
        &mut content,
        "reset",
        InteractionAction::Effects {
            effects: vec![quest_effect("quest_0")],
        },
    );
    let (engine, mut world) = setup(content);
    error_unchanged(
        &engine,
        &mut world,
        interact("reset"),
        GameErrorCode::InvalidContent,
    );
}

#[test]
fn synthetic_office_dialogue_requires_all_three_topics_before_guarded_portal() {
    // This tests declarative guards/travel, NOT death retention, a grave, or a real Office journey.
    let mut content = content();
    add_dialogue(&mut content);
    let topics = ["fees", "timer", "kept_items"];
    for name in topics {
        content.initial_state.flags.insert(name.into(), 0);
    }
    let node = &mut content.dialogues.get_mut(&dialogue()).unwrap().nodes[0];
    node.choices = topics
        .iter()
        .map(|name| DialogueChoice {
            id: (*name).into(),
            text: format!("Synthetic {name} topic"),
            guard: Guard::Always,
            effects: vec![Effect::SetFlag {
                name: (*name).into(),
                value: 1,
            }],
            next_node: Some("entry".into()),
        })
        .collect();
    add_object(
        &mut content,
        "portal",
        InteractionAction::Travel {
            destination: tile(10, 10, 1),
            region: RegionId::new("region.synthetic.floor_1").unwrap(),
        },
    );
    interaction(&mut content, "portal").guard = Guard::All {
        guards: topics
            .iter()
            .map(|name| Guard::Flag {
                name: (*name).into(),
                equals: 1,
            })
            .collect(),
    };
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("cook"));
    for (index, topic) in topics.into_iter().enumerate() {
        next(&engine, &mut world);
        error_unchanged(
            &engine,
            &mut world,
            interact("portal"),
            GameErrorCode::RequirementNotMet,
        );
        apply(&engine, &mut world, select(topic));
        if index < 2 {
            assert_eq!(state(&world).tile.plane(), 0);
        }
    }
    next(&engine, &mut world);
    apply(&engine, &mut world, interact("portal"));
    assert_eq!(state(&world).tile.plane(), 1);
}
