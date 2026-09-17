use clubscape_game_types::{GameError, GameErrorCode::*, GameEvent, XpReward};
use clubscape_simulation::{
    bank, equipment, inventory,
    skills::{self, CurrentLevelPolicy},
    transact_character,
};

use crate::support::*;

#[test]
fn a_composed_inventory_and_xp_reward_commits_once_on_success() {
    let content = content();
    let mut character = character(&content);
    put(&mut character.inventory, 0, stack("shard", 1));
    let events = transact_character(&mut character, |draft| {
        inventory::apply_operations(
            &mut draft.inventory,
            &content.items,
            &[
                inventory::InventoryOperation::Remove(stack("shard", 1)),
                inventory::InventoryOperation::Add(stack("pebble", 1)),
            ],
        )?;
        skills::award_character_xp(
            draft,
            &content,
            &[XpReward {
                skill: skill("practice"),
                amount_tenths: 10,
            }],
            CurrentLevelPolicy::Preserve,
        )
    })
    .unwrap();
    assert_eq!(
        inventory::stack_at(&character.inventory, 0).unwrap(),
        &stack("pebble", 1)
    );
    assert_eq!(
        character.skills.get(&skill("practice")).unwrap().xp_tenths,
        620
    );
    assert_eq!(
        events,
        vec![GameEvent::XpGained {
            skill: skill("practice"),
            amount_tenths: 10
        }]
    );
    assert_eq!(character.hitpoints, 3);
}

#[test]
fn a_later_failed_reward_rolls_back_prior_inventory_mutations() {
    let content = content();
    let mut character = character(&content);
    put(&mut character.inventory, 0, stack("shard", 1));
    let before = character.clone();
    assert_error(
        transact_character(&mut character, |draft| {
            inventory::remove(&mut draft.inventory, &content.items, &stack("shard", 1))?;
            skills::award_character_xp(
                draft,
                &content,
                &[XpReward {
                    skill: skill("unknown"),
                    amount_tenths: 10,
                }],
                CurrentLevelPolicy::Preserve,
            )
        }),
        UnknownContent,
    );
    assert_eq!(character, before);
}

#[test]
fn composed_equipment_and_bank_operations_roll_back_on_a_later_guard_failure() {
    let content = content();
    let mut character = character(&content);
    put(&mut character.inventory, 0, stack("blade", 1));
    put(&mut character.inventory, 1, stack("tokens", 5));
    let before = character.clone();
    let result: Result<(), GameError> = transact_character(&mut character, |draft| {
        equipment::equip(draft, &content, 0)?;
        bank::deposit(draft, &content, 1, quantity(3))?;
        Err(GameError::new(
            RequirementNotMet,
            "Synthetic final guard failed.",
        ))
    });
    assert_error(result, RequirementNotMet);
    assert_eq!(character, before);
}
