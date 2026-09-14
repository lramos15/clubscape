use clubscape_game_types::*;
use clubscape_simulation::{bank, inventory};

use crate::{WorldEngine, invalid_content, invalid_state, runtime, unavailable, unknown};

impl WorldEngine {
    fn bank_access(
        &self,
        world: &WorldState,
        character: &CharacterState,
        banker: &SpawnId,
    ) -> GameResult<()> {
        let (source, index) = runtime::access(character, "bank")?;
        if &source != banker {
            return Err(GameError::new(
                GameErrorCode::NotOwned,
                "Banker does not own the open bank interaction.",
            ));
        }
        let interaction = runtime::interaction(&self.content, &source, index)?;
        if !matches!(interaction.action, InteractionAction::Bank) {
            return Err(invalid_state(
                "Persisted bank session is not a bank interaction.",
            ));
        }
        self.require_target(world, character, &source, interaction)
    }

    pub(crate) fn deposit(
        &self,
        world: &WorldState,
        character: &mut CharacterState,
        banker: &SpawnId,
        slot: usize,
        quantity: Quantity,
    ) -> GameResult<()> {
        self.bank_access(world, character, banker)?;
        let item = &inventory::stack_at(&character.inventory, slot)?.item;
        let available = inventory::count(&character.inventory, &self.content.items, item)?;
        let quantity = Quantity::new(quantity.get().min(available))?;
        transfer_up_to(character, quantity, |draft, quantity| {
            bank::deposit(draft, &self.content, slot, quantity).map(|_| ())
        })?;
        Ok(())
    }

    pub(crate) fn withdraw(
        &self,
        world: &WorldState,
        character: &mut CharacterState,
        banker: &SpawnId,
        slot: usize,
        quantity: Quantity,
        noted: bool,
    ) -> GameResult<()> {
        self.bank_access(world, character, banker)?;
        transfer_up_to(character, quantity, |draft, quantity| {
            bank::withdraw(draft, &self.content, slot, quantity, noted).map(|_| ())
        })?;
        Ok(())
    }

    fn shop_access(
        &self,
        world: &WorldState,
        character: &CharacterState,
        shop: &ShopId,
    ) -> GameResult<()> {
        let (source, index) = runtime::access(character, "shop")?;
        let interaction = runtime::interaction(&self.content, &source, index)?;
        if !matches!(&interaction.action, InteractionAction::Shop { shop: opened } if opened == shop)
        {
            return Err(GameError::new(
                GameErrorCode::NotOwned,
                "Shop does not own the open interaction.",
            ));
        }
        self.require_target(world, character, &source, interaction)
    }

    pub(crate) fn buy(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        shop: &ShopId,
        index: usize,
        quantity: Quantity,
    ) -> GameResult<()> {
        self.shop_access(world, character, shop)?;
        let definition = self
            .content
            .shops
            .get(shop)
            .ok_or_else(|| unknown(format!("Unknown shop {shop}.")))?;
        let row = definition
            .stock
            .get(index)
            .ok_or_else(|| invalid_state("Shop stock index is out of range."))?;
        let state = world
            .shops
            .get(shop)
            .ok_or_else(|| unknown("Shop state is missing."))?;
        let stock = *state
            .stock
            .get(&row.item)
            .ok_or_else(|| invalid_state("Source shop stock row is missing."))?;
        if stock == 0 {
            return Err(GameError::new(
                GameErrorCode::InsufficientItems,
                "The item is out of stock.",
            ));
        }
        if row.buy_price == 0 {
            return Err(invalid_content(
                "Shop purchases cannot use a zero source price.",
            ));
        }
        let coins = inventory::count(
            &character.inventory,
            &self.content.items,
            &definition.currency,
        )?;
        let affordable = coins / row.buy_price;
        if affordable == 0 {
            return Err(GameError::new(
                GameErrorCode::InsufficientItems,
                "Insufficient shop currency.",
            ));
        }
        let requested = Quantity::new(quantity.get().min(stock).min(affordable))?;
        let transferred = transfer_up_to(character, requested, |draft, quantity| {
            let total = row
                .buy_price
                .checked_mul(quantity.get())
                .filter(|total| *total <= MAX_STACK_QUANTITY)
                .ok_or_else(|| {
                    GameError::new(
                        GameErrorCode::StackOverflow,
                        "Purchase price exceeds the currency stack limit.",
                    )
                })?;
            inventory::apply_operations(
                &mut draft.inventory,
                &self.content.items,
                &[
                    inventory::InventoryOperation::Remove(ItemStack {
                        item: definition.currency.clone(),
                        quantity: Quantity::new(total)?,
                    }),
                    inventory::InventoryOperation::Add(ItemStack {
                        item: row.item.clone(),
                        quantity,
                    }),
                ],
            )
        })?;
        world
            .shops
            .get_mut(shop)
            .ok_or_else(|| unknown("Shop state is missing."))?
            .stock
            .insert(row.item.clone(), stock - transferred);
        Ok(())
    }

    pub(crate) fn sell(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        shop: &ShopId,
        slot: usize,
        quantity: Quantity,
    ) -> GameResult<()> {
        self.shop_access(world, character, shop)?;
        let definition = self
            .content
            .shops
            .get(shop)
            .ok_or_else(|| unknown(format!("Unknown shop {shop}.")))?;
        let item = inventory::stack_at(&character.inventory, slot)?
            .item
            .clone();
        let item_definition = self
            .content
            .items
            .get(&item)
            .ok_or_else(|| unknown("Unknown sale item."))?;
        if !item_definition.tradable || item == definition.currency {
            return Err(GameError::new(
                GameErrorCode::RequirementNotMet,
                "This shop cannot buy the item.",
            ));
        }
        let row = definition.stock.iter().find(|row| row.item == item).ok_or_else(|| {
            if definition.accepts_general_items {
                unavailable("General-item pricing and per-row restock policies are not represented for unstocked items.")
            } else {
                GameError::new(GameErrorCode::RequirementNotMet, "The shop does not buy this item.")
            }
        })?;
        let stock = *world
            .shops
            .get(shop)
            .ok_or_else(|| unknown("Shop state is missing."))?
            .stock
            .get(&item)
            .ok_or_else(|| invalid_state("Shop row is missing."))?;
        let room = MAX_STACK_QUANTITY
            .checked_sub(stock)
            .filter(|room| *room > 0)
            .ok_or_else(|| GameError::new(GameErrorCode::StackOverflow, "Shop stock is full."))?;
        let available = inventory::count(&character.inventory, &self.content.items, &item)?;
        let coins = inventory::count(
            &character.inventory,
            &self.content.items,
            &definition.currency,
        )?;
        let currency_room = (MAX_STACK_QUANTITY - coins)
            .checked_div(row.sell_price)
            .unwrap_or(MAX_STACK_QUANTITY);
        if currency_room == 0 {
            return Err(GameError::new(
                GameErrorCode::StackOverflow,
                "No room for sale currency.",
            ));
        }
        let requested = Quantity::new(quantity.get().min(room).min(available).min(currency_room))?;
        let transferred = transfer_up_to(character, requested, |draft, quantity| {
            let selected_quantity = inventory::stack_at(&draft.inventory, slot)?
                .quantity
                .get()
                .min(quantity.get());
            inventory::remove_from_slot(
                &mut draft.inventory,
                &self.content.items,
                slot,
                Quantity::new(selected_quantity)?,
            )?;
            if quantity.get() > selected_quantity {
                inventory::remove(
                    &mut draft.inventory,
                    &self.content.items,
                    &ItemStack {
                        item: item.clone(),
                        quantity: Quantity::new(quantity.get() - selected_quantity)?,
                    },
                )?;
            }
            let total = row
                .sell_price
                .checked_mul(quantity.get())
                .filter(|total| *total <= MAX_STACK_QUANTITY)
                .ok_or_else(|| {
                    GameError::new(
                        GameErrorCode::StackOverflow,
                        "Sale price exceeds the currency stack limit.",
                    )
                })?;
            if total > 0 {
                inventory::add(
                    &mut draft.inventory,
                    &self.content.items,
                    &ItemStack {
                        item: definition.currency.clone(),
                        quantity: Quantity::new(total)?,
                    },
                )?;
            }
            Ok(())
        })?;
        world
            .shops
            .get_mut(shop)
            .ok_or_else(|| unknown("Shop state is missing."))?
            .stock
            .insert(item, stock + transferred);
        Ok(())
    }

    pub(crate) fn next_restock(&self, shop: &ShopDefinition, tick: u64) -> GameResult<u64> {
        let mut next = i64::MAX as u64;
        for row in &shop.stock {
            let period = u64::from(row.restock_ticks);
            if period == 0 {
                return Err(invalid_content("Restocking needs a positive period."));
            }
            next = next.min(runtime::deadline(tick, period - tick % period)?);
        }
        Ok(next)
    }

    pub(crate) fn restock(&self, world: &mut WorldState) -> GameResult<()> {
        for (id, definition) in &self.content.shops {
            let state = world
                .shops
                .get_mut(id)
                .ok_or_else(|| unknown(format!("Missing shop state {id}.")))?;
            if world.tick < state.next_restock_tick {
                continue;
            }
            for row in &definition.stock {
                if world.tick.is_multiple_of(u64::from(row.restock_ticks)) {
                    let stock = state
                        .stock
                        .get_mut(&row.item)
                        .ok_or_else(|| invalid_state("Shop stock row is missing."))?;
                    match (*stock).cmp(&row.base_stock) {
                        std::cmp::Ordering::Less => *stock += 1,
                        std::cmp::Ordering::Greater => *stock -= 1,
                        std::cmp::Ordering::Equal => {}
                    }
                }
            }
            state.next_restock_tick = self.next_restock(definition, world.tick)?;
        }
        Ok(())
    }
}

/// The primitive is exact; source UI quantities mean "up to". Capacity is monotone.
/// Binary search bounds the work even for a maximum-sized requested stack.
fn transfer_up_to(
    character: &mut CharacterState,
    quantity: Quantity,
    operation: impl Fn(&mut CharacterState, Quantity) -> GameResult<()>,
) -> GameResult<u32> {
    let mut full = character.clone();
    let first_error = match operation(&mut full, quantity) {
        Ok(()) => {
            *character = full;
            return Ok(quantity.get());
        }
        Err(error) if capacity_error(&error.code) => error,
        Err(error) => return Err(error),
    };
    let mut low = 1;
    let mut high = quantity.get() - 1;
    let mut best = None;
    while low <= high {
        let mid = low + (high - low) / 2;
        let mut draft = character.clone();
        match operation(&mut draft, Quantity::new(mid)?) {
            Ok(()) => {
                best = Some((mid, draft));
                low = mid + 1;
            }
            Err(error) if capacity_error(&error.code) => high = mid - 1,
            Err(error) => return Err(error),
        }
    }
    if let Some((count, draft)) = best {
        *character = draft;
        Ok(count)
    } else {
        Err(first_error)
    }
}

fn capacity_error(code: &GameErrorCode) -> bool {
    matches!(
        code,
        GameErrorCode::InsufficientItems
            | GameErrorCode::InventoryFull
            | GameErrorCode::StackOverflow
    )
}
