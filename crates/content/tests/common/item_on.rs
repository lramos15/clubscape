use super::*;

pub fn bound<T>(value: T) -> SourceBinding<T> {
    SourceBinding::Bound {
        value,
        source: sources(),
    }
}

/// Synthetic ore conversion, deliberately not the product water recipe.
pub fn content() -> GameContent {
    let mut content = fixture();
    content
        .spawns
        .get_mut(&id("spawn.test.furnace"))
        .unwrap()
        .interactions
        .clear();
    let recipe = content.recipes.get_mut(&id("recipe.test.bar")).unwrap();
    recipe.ticks = None;
    recipe.item_on_target = Some(bound(ItemOnTargetRule {
        reach: 1,
        guard: Guard::Always,
    }));
    recipe.mechanics = Some(RecipeMechanics {
        method: id("action.test.item_on"),
        guard: Guard::Always,
        chance_skill: None,
        cadence: ActionCadence {
            single: bound(1),
            first: bound(7),
            repeat: bound(11),
            menu_delay: bound(0),
        },
        tool_ownership: OwnershipScope::InventoryAndEquipment,
        failed_xp: Vec::new(),
        success_effects: Vec::new(),
        failure_effects: Vec::new(),
        lifecycle: RecipeLifecycle::InventoryConversion,
    });
    content.initial_state.tile = tile(1004, 1002);
    for stage in content.tutorial.values_mut() {
        stage.allowed_actions = vec!["*".into()];
    }
    content
}
