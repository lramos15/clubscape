//! Approved provisional adaptation, not verified OSRS probabilities or product content.
mod support;

use clubscape_game_types::*;
use support::{v2 as v, *};

#[test]
fn approved_independent_potion_event_preserves_the_128_primary_pool_and_materializes_one_dose() {
    let mut content = v::content();
    v::with_combat(&mut content);
    v::armed(&mut content, false);
    let approval = SourceRecord {
        reference:
            "milestones/m1-goblin-loot-approval.json#adaptation.m1.goblin_energy_potion_provisional"
                .into(),
        revision: "594a4fd".into(),
        status: EvidenceStatus::ApprovedAdaptation,
        notes:
            "Owner-approved provisional independent 1/16, equal 1-4 doses; not verified OSRS odds."
                .into(),
    };
    for dose in 1..=4 {
        let mut definition = content.items[&item("pot")].clone();
        definition.id = item(&format!("potion_{dose}"));
        definition.name = format!("Synthetic approved-policy dose {dose}");
        definition.source.push(approval.clone());
        content.items.insert(definition.id.clone(), definition);
    }
    let entry = |name: &str, amount: u32| LootEntry {
        item: item(name),
        minimum: quantity(amount),
        maximum: quantity(amount),
    };
    let primary = LootPool::Exclusive {
        total_weight: 128,
        entries: vec![
            WeightedLoot {
                weight: 64,
                items: vec![entry("coins", 2)],
            },
            WeightedLoot {
                weight: 64,
                items: vec![entry("arrow", 3)],
            },
        ],
    };
    // A separate 64-way draw is exactly a 1/16 event with four equal conditional doses.
    let mut potion_entries: Vec<_> = (1..=4)
        .map(|dose| WeightedLoot {
            weight: 1,
            items: vec![entry(&format!("potion_{dose}"), 1)],
        })
        .collect();
    potion_entries.push(WeightedLoot {
        weight: 60,
        items: vec![],
    });
    let npc = content.npcs.get_mut(&v::npc()).unwrap();
    npc.source.push(approval);
    let combat = npc.combat.as_mut().unwrap();
    combat.hitpoints = 1;
    combat.mechanics.as_mut().unwrap().loot = vec![
        LootPool::Guaranteed {
            items: vec![entry("egg", 1)],
        },
        primary.clone(),
        LootPool::Exclusive {
            total_weight: 64,
            entries: potion_entries,
        },
    ];
    let mut doses = [0; 4];
    for roll in 0..64 {
        let (engine, mut world) = setup(content.clone());
        engine
            .apply_intent(
                &mut world,
                &actor(),
                &interact("enemy"),
                &mut v::Rolls::new(&[816, 0, 1, 100, roll]),
            )
            .unwrap();
        assert_eq!(
            engine.content().npcs[&v::npc()]
                .combat
                .as_ref()
                .unwrap()
                .mechanics
                .as_ref()
                .unwrap()
                .loot[1],
            primary
        );
        assert!(
            world
                .ground_items
                .iter()
                .any(|ground| ground.stack == stack("arrow", 3))
        );
        assert!(
            world
                .ground_items
                .iter()
                .any(|ground| ground.stack == stack("egg", 1))
        );
        assert!(
            !world
                .ground_items
                .iter()
                .any(|ground| ground.stack.item == item("coins"))
        );
        let potion_count = world
            .ground_items
            .iter()
            .filter(|ground| ground.stack.item.as_str().contains("potion_"))
            .count();
        assert_eq!(potion_count, usize::from(roll < 4));
        if roll < 4 {
            doses[roll as usize] += 1;
            assert!(
                world
                    .ground_items
                    .iter()
                    .any(|ground| ground.stack == stack(&format!("potion_{}", roll + 1), 1))
            );
        }
        let before = world.ground_items.clone();
        world = serde_json::from_str(&serde_json::to_string(&world).unwrap()).unwrap();
        v::tick(&engine, &mut world, &mut NeverDraw);
        error_unchanged(&engine, &mut world, interact("enemy"), GameErrorCode::Busy);
        assert_eq!(world.ground_items, before);
    }
    assert_eq!(doses, [1, 1, 1, 1]);
}
