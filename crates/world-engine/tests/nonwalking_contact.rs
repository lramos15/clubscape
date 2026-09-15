mod support;

use clubscape_game_types::*;
use support::{v2 as v, *};

fn fixture(anchor: StationaryAnchor) -> GameContent {
    let mut content = v::content();
    let id = NpcId::new("npc.synthetic.nonwalking").unwrap();
    content.npcs.insert(
        id.clone(),
        NpcDefinition {
            id: id.clone(),
            name: "Declared nonwalking source fixture".into(),
            source_id: 3317,
            size: 1,
            navigation: NpcNavigation::Stationary { anchor },
            morph: None,
            combat: None,
            asset: None,
            source: source(),
        },
    );
    content.spawns.insert(
        spawn("nonwalking"),
        SpawnDefinition {
            id: spawn("nonwalking"),
            region: content.initial_state.region.clone(),
            tile: tile(11, 10, 0),
            facing: 0,
            placement: None,
            kind: SpawnKind::Npc { npc: id },
            interactions: vec![InteractionDefinition {
                name: "use".into(),
                reach: 1,
                guard: Guard::Always,
                action: InteractionAction::Effects {
                    effects: vec![Effect::Message {
                        text: "Reached the declared actor face.".into(),
                    }],
                },
            }],
            source: source(),
        },
    );
    let target = cell(&mut content, tile(11, 10, 0));
    target.walkable = false;
    target.blocked_movement = u8::MAX;
    target.blocked_sight = 0;
    content
}

fn resource() -> StationaryAnchor {
    StationaryAnchor::NonWalkingResource {
        access_tiles: vec![tile(10, 10, 0), tile(11, 11, 0)],
    }
}

#[test]
fn declared_resource_scripted_and_scenery_faces_work_without_erasing_collision() {
    for anchor in [
        resource(),
        StationaryAnchor::ScriptedActor {
            access_tiles: vec![tile(10, 10, 0)],
            source: source(),
        },
        StationaryAnchor::SceneryBound {
            object: object("bank"),
            access_tiles: vec![tile(10, 10, 0)],
        },
    ] {
        let content = fixture(anchor);
        let regions = content.regions.clone();
        let (engine, mut world) = setup(content);
        let before = world.clone();
        let options = engine
            .interaction_options(
                &world,
                &actor(),
                &WorldTarget::Spawn {
                    spawn: spawn("nonwalking"),
                },
            )
            .unwrap();
        assert!(
            options
                .iter()
                .any(|option| option.name == "use" && option.permission.allowed)
        );
        assert_eq!(world, before);
        let events = apply(&engine, &mut world, interact("nonwalking"));
        assert!(events.iter().any(|event| matches!(&event.event, GameEvent::Message { text } if text == "Reached the declared actor face.")));
        assert_eq!(engine.content().regions, regions);
        let map =
            clubscape_simulation::navigation::CollisionMap::from_regions(regions.values()).unwrap();
        assert!(!map.can_step(tile(10, 10, 0), tile(11, 10, 0)));
    }
}

#[test]
fn stationary_contact_preserves_declared_tiles_walls_sight_plane_and_source_guard() {
    for case in [
        "unlisted",
        "movement_wall",
        "sight_wall",
        "target_sight",
        "target_edge",
        "blocked_land",
        "plane",
        "guard",
    ] {
        let mut content = fixture(resource());
        match case {
            "unlisted" => content.initial_state.tile = tile(12, 10, 0),
            "movement_wall" => {
                cell(&mut content, tile(10, 10, 0)).blocked_movement = Direction::East.mask()
            }
            "sight_wall" => {
                cell(&mut content, tile(10, 10, 0)).blocked_sight = Direction::East.mask()
            }
            "target_sight" => {
                cell(&mut content, tile(11, 10, 0)).blocked_sight = Direction::West.mask()
            }
            "target_edge" => {
                cell(&mut content, tile(11, 10, 0)).blocked_movement = Direction::West.mask()
            }
            "blocked_land" => {}
            "plane" => {
                content.initial_state.region = RegionId::new("region.synthetic.floor_1").unwrap();
                content.initial_state.tile = tile(10, 10, 1);
            }
            "guard" => {
                interaction(&mut content, "nonwalking").guard = Guard::Not {
                    guard: Box::new(Guard::Always),
                }
            }
            _ => unreachable!(),
        }
        let (engine, mut world) = setup(content);
        if case == "blocked_land" {
            state_mut(&mut world).tile = tile(11, 10, 0);
        }
        error_unchanged(
            &engine,
            &mut world,
            interact("nonwalking"),
            if case == "guard" {
                GameErrorCode::RequirementNotMet
            } else {
                GameErrorCode::OutOfReach
            },
        );
    }
}

#[test]
fn ordinary_walkable_anchor_cannot_claim_nonwalking_contact() {
    let content = fixture(StationaryAnchor::Walkable);
    let (engine, mut world) = setup(content);
    error_unchanged(
        &engine,
        &mut world,
        interact("nonwalking"),
        GameErrorCode::OutOfReach,
    );
}

#[test]
fn declared_scripted_access_reaches_the_side_of_a_multitile_actor() {
    let mut content = fixture(StationaryAnchor::ScriptedActor {
        access_tiles: vec![tile(10, 11, 0)],
        source: source(),
    });
    content.initial_state.tile = tile(10, 11, 0);
    content
        .npcs
        .get_mut(&NpcId::new("npc.synthetic.nonwalking").unwrap())
        .unwrap()
        .size = 2;
    for x in 11..=12 {
        for y in 10..=11 {
            let target = cell(&mut content, tile(x, y, 0));
            target.walkable = false;
            target.blocked_movement = u8::MAX;
            target.blocked_sight = 0;
        }
    }
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("nonwalking"));
    assert_eq!(state(&world).tile, tile(10, 11, 0));
}
