mod common;

use clubscape_content::{ValidationMode, compile_content, encode_compiled, load_compiled};
use clubscape_game_types::*;
use common::*;

#[test]
fn advancement_events_flags_and_unlocks_cannot_bootstrap_their_own_only_producer() {
    let mutations: [fn(&mut GameContent); 4] = [
        |content| {
            let transition = tutorial_transition(content);
            transition.event = "tutorial_advanced".into();
            transition.target = Some("stage.test.start".into());
        },
        |content| {
            let transition = quest_transition(content);
            transition.event = "quest_advanced".into();
            transition.target = Some("quest.test.errand".into());
        },
        |content| {
            let transition = tutorial_transition(content);
            transition.guard = Guard::Flag {
                name: "test_seen".into(),
                equals: 1,
            };
            transition.effects.push(Effect::SetFlag {
                name: "test_seen".into(),
                value: 1,
            });
        },
        |content| {
            let transition = tutorial_transition(content);
            transition.guard = Guard::InterfaceUnlocked {
                interface: id("interface.test.circular"),
            };
            transition.effects.push(Effect::UnlockInterface {
                interface: id("interface.test.circular"),
            });
        },
    ];
    for mutate in mutations {
        let mut content = fixture();
        mutate(&mut content);
        let error = compile_content(content, ValidationMode::TestFixture).unwrap_err();
        assert!(
            error.message.contains("rooted event/flag/interface"),
            "{error}"
        );
    }
}

#[test]
fn an_external_gameplay_event_can_root_a_chain_of_stage_advancement_events() {
    let mut content = fixture();
    let transition = &mut content
        .tutorial
        .get_mut(&id("stage.test.learn"))
        .unwrap()
        .transitions[0];
    transition.event = "tutorial_advanced".into();
    transition.target = Some("stage.test.learn".into());
    compile_content(content, ValidationMode::TestFixture).unwrap();
}

#[test]
fn later_conditionals_observe_an_earlier_stage_write_in_the_same_atomic_effect_batch() {
    let mut content = fixture();
    content
        .quests
        .values_mut()
        .next()
        .unwrap()
        .transitions
        .clear();
    tutorial_transition(&mut content)
        .effects
        .push(Effect::Conditional {
            guard: Guard::TutorialStage {
                stage: id("stage.test.learn"),
            },
            effects: vec![Effect::SetQuestStage {
                quest: id("quest.test.errand"),
                stage: id("stage.test.quest_done"),
            }],
        });
    compile_content(content, ValidationMode::TestFixture).unwrap();
}

#[test]
fn contradictory_nested_guards_without_intervening_writes_are_not_progression() {
    let mut content = fixture();
    content
        .spawns
        .get_mut(&id("spawn.test.guide"))
        .unwrap()
        .interactions
        .push(InteractionDefinition {
            name: "SetFlag".into(),
            reach: 1,
            guard: Guard::Always,
            action: InteractionAction::Effects {
                effects: vec![Effect::SetFlag {
                    name: "test_seen".into(),
                    value: 1,
                }],
            },
        });
    let transition = tutorial_transition(&mut content);
    transition.guard = Guard::Flag {
        name: "test_seen".into(),
        equals: 0,
    };
    transition.effects = vec![Effect::Conditional {
        guard: Guard::Flag {
            name: "test_seen".into(),
            equals: 1,
        },
        effects: std::mem::take(&mut transition.effects),
    }];
    let error = compile_content(content, ValidationMode::TestFixture).unwrap_err();
    assert!(error.message.contains("unreachable"), "{error}");
}

#[test]
fn the_supported_guard_and_effect_depth_boundary_round_trips_through_the_loader() {
    let mut content = fixture();
    let transition = tutorial_transition(&mut content);
    let mut guard = Guard::Always;
    let mut effect = transition.effects.pop().unwrap();
    for _ in 0..31 {
        guard = Guard::All {
            guards: vec![guard],
        };
        effect = Effect::Conditional {
            guard: Guard::Always,
            effects: vec![effect],
        };
    }
    transition.guard = guard;
    transition.effects.push(effect);
    let compiled = compile_content(content, ValidationMode::TestFixture).unwrap();
    load_compiled(
        &encode_compiled(&compiled).unwrap(),
        ValidationMode::TestFixture,
    )
    .unwrap();
}

#[test]
fn graph_analysis_has_a_work_budget_not_unbounded_stage_by_rule_fanout() {
    let mut content = fixture();
    let terminal = content.tutorial[&id("stage.test.done")].clone();
    for number in 0..60 {
        let mut stage = terminal.clone();
        stage.id = id(&format!("stage.test.budget{number}"));
        content
            .spawns
            .get_mut(&id("spawn.test.guide"))
            .unwrap()
            .interactions
            .push(InteractionDefinition {
                name: format!("Reach{number}"),
                reach: 1,
                guard: Guard::Always,
                action: InteractionAction::Effects {
                    effects: vec![Effect::SetTutorialStage {
                        stage: stage.id.clone(),
                    }],
                },
            });
        content.tutorial.insert(stage.id.clone(), stage);
    }
    content
        .spawns
        .get_mut(&id("spawn.test.guide"))
        .unwrap()
        .interactions
        .push(InteractionDefinition {
            name: "Budget".into(),
            reach: 1,
            guard: Guard::All {
                guards: vec![Guard::Always; 50_000],
            },
            action: InteractionAction::Effects {
                effects: vec![Effect::SetTutorialStage {
                    stage: id("stage.test.learn"),
                }],
            },
        });
    let error = compile_content(content, ValidationMode::TestFixture).unwrap_err();
    assert!(
        error.message.contains("graph-analysis work budget"),
        "{error}"
    );
}

#[test]
fn source_locators_cannot_be_empty_resources_disguised_as_nonempty_strings() {
    for reference in [".", "...", "https://", "fixture:"] {
        let mut content = fixture();
        content.initial_state.source[0].reference = reference.into();
        assert!(
            compile_content(content, ValidationMode::TestFixture).is_err(),
            "{reference}"
        );
    }
}
