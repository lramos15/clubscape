use super::*;
use clubscape_simulation::inventory;

impl WorldEngine {
    pub(crate) fn production_inventory_permission(
        &self,
        character: &CharacterState,
        selection: &ProductionInventorySelection,
        recipes: &[RecipeId],
    ) -> GameResult<()> {
        self.check_production_inventory(&character.inventory, selection)?;
        self.production_inventory_recipes(selection, recipes)
    }

    pub(super) fn production_inventory_recipes(
        &self,
        selection: &ProductionInventorySelection,
        recipes: &[RecipeId],
    ) -> GameResult<()> {
        selection.validate_shape()?;
        for id in recipes {
            let recipe = self
                .content
                .recipes
                .get(id)
                .ok_or_else(|| unknown("Unknown inventory production recipe."))?;
            let uses = |item: &ItemId| {
                recipe.inputs.iter().any(|input| &input.item == item) || recipe.tools.contains(item)
            };
            if !recipe.target_objects.is_empty()
                || recipe.mechanics.as_ref().is_none_or(|mechanics| {
                    !matches!(mechanics.lifecycle, RecipeLifecycle::InventoryConversion)
                })
                || !uses(&selection.used.item)
                || !uses(&selection.target.item)
                || (selection.used.item == selection.target.item
                    && !recipe.inputs.iter().any(|input| {
                        input.item == selection.used.item && input.quantity.get() >= 2
                    }))
            {
                return Err(invalid_state(
                    "Inventory selection does not belong to the targetless source recipe.",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn check_production_inventory(
        &self,
        container: &Inventory,
        selection: &ProductionInventorySelection,
    ) -> GameResult<()> {
        selection.validate_shape()?;
        for (slot, expected) in [
            (selection.used_slot, &selection.used),
            (selection.target_slot, &selection.target),
        ] {
            if container
                .slots
                .get(usize::from(slot))
                .and_then(Option::as_ref)
                != Some(expected)
            {
                return Err(GameError::new(
                    GameErrorCode::StaleCommand,
                    "A selected production input slot, stack or instance changed.",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn remove_selected_recipe_inputs(
        &self,
        container: &mut Inventory,
        recipe: &RecipeDefinition,
        selection: &ProductionInventorySelection,
    ) -> GameResult<()> {
        self.check_production_inventory(container, selection)?;
        self.production_inventory_recipes(selection, std::slice::from_ref(&recipe.id))?;
        let mut draft = container.clone();
        for input in &recipe.inputs {
            let mut remaining = input.quantity.get();
            for (slot, expected) in [
                (selection.used_slot, &selection.used),
                (selection.target_slot, &selection.target),
            ] {
                if expected.item != input.item || remaining == 0 {
                    continue;
                }
                let count = remaining.min(expected.quantity.get());
                inventory::remove_from_slot(
                    &mut draft,
                    &self.content.items,
                    usize::from(slot),
                    Quantity::new(count)?,
                )?;
                remaining -= count;
            }
            if remaining > 0 {
                inventory::remove(
                    &mut draft,
                    &self.content.items,
                    &ItemStack {
                        item: input.item.clone(),
                        quantity: Quantity::new(remaining)?,
                        instance: input.instance.clone(),
                    },
                )?;
            }
        }
        *container = draft;
        Ok(())
    }
}
