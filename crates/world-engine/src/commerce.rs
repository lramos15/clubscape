use clubscape_game_types::*;
use clubscape_simulation::{bank, inventory};

use crate::{WorldEngine, invalid_content, invalid_state, runtime, source_math, unknown};

impl WorldEngine {
    fn bank_access(
        &self,
        world: &WorldState,
        character: &CharacterState,
        banker: &SpawnId,
    ) -> GameResult<()> {
        let (source, index) = runtime::access(character, true)?;
        if source != banker {
            return Err(GameError::new(
                GameErrorCode::NotOwned,
                "Banker does not own the opened session.",
            ));
        }
        let interaction = runtime::interaction(&self.content, source, index)?;
        if !matches!(
            interaction.action,
            InteractionAction::Bank | InteractionAction::OpenBank { .. }
        ) {
            return Err(invalid_state("Stored bank session changed kind."));
        }
        self.require_target(world, character, source, interaction)
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
        transfer_up_to(
            character,
            Quantity::new(quantity.get().min(available))?,
            |draft, quantity| bank::deposit(draft, &self.content, slot, quantity).map(|_| ()),
        )?;
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
        let (source, index) = runtime::access(character, false)?;
        let interaction = runtime::interaction(&self.content, source, index)?;
        if !matches!(&interaction.action, InteractionAction::Shop { shop: opened } | InteractionAction::OpenShop { shop: opened, .. } if opened == shop)
        {
            return Err(GameError::new(
                GameErrorCode::NotOwned,
                "Shop does not own the opened source session.",
            ));
        }
        self.require_target(world, character, source, interaction)
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
            .ok_or_else(|| unknown("Unknown shop."))?;
        let row = self.shop_row(world, definition, index)?;
        self.trade(world, character, definition, &row, quantity, None)
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
            .ok_or_else(|| unknown("Unknown shop."))?;
        let item = &inventory::stack_at(&character.inventory, slot)?.item;
        let item_definition = self
            .content
            .items
            .get(item)
            .ok_or_else(|| unknown("Unknown sale item."))?;
        if !item_definition.tradable
            || item == &definition.currency
            || item_definition.unnoted_variant.is_some()
        {
            return Err(GameError::new(
                GameErrorCode::RequirementNotMet,
                "Shop refuses this item form.",
            ));
        }
        let row = if let Some(row) = definition.stock.iter().find(|row| &row.item == item) {
            row.clone()
        } else {
            match &definition.unstocked {
                Some(UnstockedShopPolicy::Accept {
                    maximum_lines,
                    rule,
                    base_stock,
                }) => {
                    let state = world
                        .shops
                        .get(&definition.id)
                        .ok_or_else(|| unknown("Missing shop stock."))?;
                    let extras = state
                        .stock
                        .keys()
                        .filter(|id| !definition.stock.iter().any(|row| &row.item == *id))
                        .count();
                    if !state.stock.contains_key(item) && extras >= usize::from(*maximum_lines) {
                        return Err(GameError::new(
                            GameErrorCode::InventoryFull,
                            "Shop has no free unstocked lines.",
                        ));
                    }
                    if matches!(
                        rule.restock.phase.require()?,
                        RestockPhase::SinceLastStockChange
                    ) {
                        return Err(crate::unavailable(
                            "World runtime validation currently forbids persisted clocks for unstocked shop rows.",
                        ));
                    }
                    ShopItem {
                        item: item.clone(),
                        base_stock: *base_stock,
                        restock_ticks: rule.restock.interval_ticks,
                        buy_price: 1,
                        sell_price: 0,
                        mechanics: Some(rule.clone()),
                    }
                }
                Some(UnstockedShopPolicy::Reject) => {
                    return Err(GameError::new(
                        GameErrorCode::RequirementNotMet,
                        "Shop does not buy unstocked items.",
                    ));
                }
                None if definition.accepts_general_items => {
                    return Err(crate::unavailable(
                        "General-store unstocked policy is absent.",
                    ));
                }
                None => {
                    return Err(GameError::new(
                        GameErrorCode::RequirementNotMet,
                        "Shop does not buy this item.",
                    ));
                }
            }
        };
        self.trade(world, character, definition, &row, quantity, Some(slot))
    }

    fn trade(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        shop: &ShopDefinition,
        row: &ShopItem,
        quantity: Quantity,
        sale_slot: Option<usize>,
    ) -> GameResult<()> {
        if quantity.get() > 50 {
            return Err(invalid_state(
                "M1 shop operations support the source 1/5/10/50 quantities; submit a bounded batch.",
            ));
        }
        let mut stock = world
            .shops
            .get(&shop.id)
            .ok_or_else(|| unknown("Missing shop state."))?
            .stock
            .get(&row.item)
            .copied()
            .unwrap_or(row.base_stock);
        let original = stock;
        let mut moved = 0;
        let mut refusal = None;
        for _ in 0..quantity.get() {
            if sale_slot.is_none() && stock == 0 {
                refusal = Some(GameError::new(
                    GameErrorCode::InsufficientItems,
                    "Shop is out of stock.",
                ));
                break;
            }
            if sale_slot.is_some() && stock >= MAX_STACK_QUANTITY {
                refusal = Some(GameError::new(
                    GameErrorCode::StackOverflow,
                    "Shop stock is full.",
                ));
                break;
            }
            let price = self.shop_price(row, stock, sale_slot.is_some())?;
            let mut draft = character.clone();
            let operation = if let Some(slot) = sale_slot {
                let slot = if draft
                    .inventory
                    .slots
                    .get(slot)
                    .and_then(|slot| slot.as_ref())
                    .is_some_and(|stack| stack.item == row.item)
                {
                    Some(slot)
                } else {
                    draft.inventory.slots.iter().position(|stack| {
                        stack.as_ref().is_some_and(|stack| stack.item == row.item)
                    })
                };
                let result = slot
                    .ok_or_else(|| {
                        GameError::new(
                            GameErrorCode::InsufficientItems,
                            "No more owned items to sell.",
                        )
                    })
                    .and_then(|slot| {
                        inventory::remove_from_slot(
                            &mut draft.inventory,
                            &self.content.items,
                            slot,
                            Quantity::new(1)?,
                        )
                        .map(|_| ())
                    });
                result.and_then(|()| {
                    if price == 0 {
                        Ok(())
                    } else {
                        inventory::add(
                            &mut draft.inventory,
                            &self.content.items,
                            &ItemStack {
                                item: shop.currency.clone(),
                                quantity: Quantity::new(price)?,
                                instance: None,
                            },
                        )
                    }
                })
            } else {
                inventory::apply_operations(
                    &mut draft.inventory,
                    &self.content.items,
                    &[
                        inventory::InventoryOperation::Remove(ItemStack {
                            item: shop.currency.clone(),
                            quantity: Quantity::new(price)?,
                            instance: None,
                        }),
                        inventory::InventoryOperation::Add(ItemStack {
                            item: row.item.clone(),
                            quantity: Quantity::new(1)?,
                            instance: None,
                        }),
                    ],
                )
            };
            match operation {
                Ok(()) => {
                    *character = draft;
                    moved += 1;
                    if sale_slot.is_some() {
                        stock += 1;
                    } else {
                        stock -= 1;
                    }
                }
                Err(error)
                    if capacity_error(&error.code) || error.code == GameErrorCode::NotOwned =>
                {
                    refusal = Some(error);
                    break;
                }
                Err(error) => return Err(error),
            }
        }
        if moved == 0 {
            // Spending a whole currency stack can free a slot even when the first unit cannot.
            if sale_slot.is_none()
                && row
                    .mechanics
                    .as_ref()
                    .is_none_or(|m| matches!(m.pricing, ShopPricing::Fixed))
            {
                let currency =
                    inventory::count(&character.inventory, &self.content.items, &shop.currency)?;
                let amount = quantity.get().min(original).min(currency / row.buy_price);
                if amount > 0 && amount * row.buy_price == currency {
                    let mut draft = character.clone();
                    inventory::apply_operations(
                        &mut draft.inventory,
                        &self.content.items,
                        &[
                            inventory::InventoryOperation::Remove(ItemStack {
                                item: shop.currency.clone(),
                                quantity: Quantity::new(currency)?,
                                instance: None,
                            }),
                            inventory::InventoryOperation::Add(ItemStack {
                                item: row.item.clone(),
                                quantity: Quantity::new(amount)?,
                                instance: None,
                            }),
                        ],
                    )?;
                    *character = draft;
                    stock -= amount;
                    moved = amount;
                }
            }
            if sale_slot.is_some() {
                let owned = inventory::count(&character.inventory, &self.content.items, &row.item)?;
                if owned > 0 && owned <= quantity.get() && owned <= MAX_STACK_QUANTITY - original {
                    let mut proceeds = 0_u32;
                    for offset in 0..owned {
                        proceeds = proceeds
                            .checked_add(self.shop_price(row, original + offset, true)?)
                            .filter(|value| *value <= MAX_STACK_QUANTITY)
                            .ok_or_else(|| {
                                GameError::new(
                                    GameErrorCode::StackOverflow,
                                    "Sale proceeds overflow.",
                                )
                            })?;
                    }
                    let mut operations = vec![inventory::InventoryOperation::Remove(ItemStack {
                        item: row.item.clone(),
                        quantity: Quantity::new(owned)?,
                        instance: None,
                    })];
                    if proceeds > 0 {
                        operations.push(inventory::InventoryOperation::Add(ItemStack {
                            item: shop.currency.clone(),
                            quantity: Quantity::new(proceeds)?,
                            instance: None,
                        }));
                    }
                    inventory::apply_operations(
                        &mut character.inventory,
                        &self.content.items,
                        &operations,
                    )?;
                    stock += owned;
                    moved = owned;
                }
            }
        }
        if moved == 0 {
            return Err(refusal.unwrap_or_else(|| invalid_state("No shop transfer occurred.")));
        }
        world
            .shops
            .get_mut(&shop.id)
            .ok_or_else(|| unknown("Missing shop state."))?
            .stock
            .insert(row.item.clone(), stock);
        self.reset_stock_clock(world, shop, row)?;
        Ok(())
    }

    fn shop_row(
        &self,
        world: &WorldState,
        shop: &ShopDefinition,
        index: usize,
    ) -> GameResult<ShopItem> {
        if let Some(row) = shop.stock.get(index) {
            return Ok(row.clone());
        }
        let Some(UnstockedShopPolicy::Accept {
            rule, base_stock, ..
        }) = &shop.unstocked
        else {
            return Err(invalid_state("Shop index is out of range."));
        };
        let item = world
            .shops
            .get(&shop.id)
            .ok_or_else(|| unknown("Missing shop state."))?
            .stock
            .keys()
            .filter(|id| !shop.stock.iter().any(|row| &row.item == *id))
            .nth(index - shop.stock.len())
            .ok_or_else(|| invalid_state("Unstocked row index is out of range."))?;
        Ok(ShopItem {
            item: item.clone(),
            base_stock: *base_stock,
            restock_ticks: rule.restock.interval_ticks,
            buy_price: 1,
            sell_price: 0,
            mechanics: Some(rule.clone()),
        })
    }

    pub(crate) fn shop_price(&self, row: &ShopItem, stock: u32, selling: bool) -> GameResult<u32> {
        let pricing = row.mechanics.as_ref().map(|m| &m.pricing);
        let Some(ShopPricing::StockSensitive {
            buy,
            sell,
            overstock,
        }) = pricing
        else {
            return Ok(if selling {
                row.sell_price
            } else {
                row.buy_price
            });
        };
        let formula = if selling { sell } else { buy };
        let stock = if stock > row.base_stock
            && matches!(
                overstock.require()?,
                OverstockPricing::BasePriceAboveBaseStock
            ) {
            row.base_stock
        } else {
            stock
        };
        let rate = (i128::from(formula.base_per_mille)
            + i128::from(formula.change_per_stock)
                * (i128::from(row.base_stock) - i128::from(stock)))
        .clamp(
            i128::from(formula.minimum_per_mille),
            i128::from(formula.maximum_per_mille),
        ) as u128;
        let value = self
            .content
            .items
            .get(&row.item)
            .ok_or_else(|| unknown("Unknown priced item."))?
            .base_value;
        let price = source_math::rounded(u128::from(value) * rate, 1000, &formula.rounding)?
            .max(u64::from(formula.minimum_price));
        u32::try_from(price)
            .ok()
            .filter(|price| *price <= MAX_STACK_QUANTITY)
            .ok_or_else(|| invalid_content("Shop price overflow."))
    }

    pub(crate) fn next_restock(&self, shop: &ShopDefinition, tick: u64) -> GameResult<u64> {
        let mut next = i64::MAX as u64;
        for row in &shop.stock {
            let period = u64::from(
                row.mechanics
                    .as_ref()
                    .map_or(row.restock_ticks, |m| m.restock.interval_ticks),
            );
            if period == 0 {
                return Err(invalid_content("Restock interval is zero."));
            }
            next = next.min(runtime::deadline(tick, period - tick % period)?);
        }
        Ok(next)
    }

    fn stock_deadline(&self, row: &ShopItem, tick: u64) -> GameResult<u64> {
        let Some(mechanics) = &row.mechanics else {
            let period = u64::from(row.restock_ticks);
            return runtime::deadline(tick, period - tick % period);
        };
        let period = u64::from(mechanics.restock.interval_ticks);
        if period == 0 {
            return Err(invalid_content("Restock interval is zero."));
        }
        match mechanics.restock.phase.require()? {
            RestockPhase::WorldEpoch => runtime::deadline(tick, period - tick % period),
            RestockPhase::SinceLastStockChange => runtime::deadline(tick, period),
            RestockPhase::Explicit { first_tick } if *first_tick > tick => Ok(*first_tick),
            RestockPhase::Explicit { first_tick } => {
                runtime::deadline(tick, period - (tick - first_tick) % period)
            }
        }
    }

    fn reset_stock_clock(
        &self,
        world: &mut WorldState,
        shop: &ShopDefinition,
        row: &ShopItem,
    ) -> GameResult<()> {
        let deadline = self.stock_deadline(row, world.tick)?;
        if shop.stock.iter().any(|source| source.item == row.item) {
            let reset = row.mechanics.as_ref().is_some_and(|m| {
                matches!(
                    &m.restock.phase,
                    SourceBinding::Bound {
                        value: RestockPhase::SinceLastStockChange,
                        ..
                    }
                )
            });
            let clocks = world
                .runtime
                .stock_deadlines
                .entry(shop.id.clone())
                .or_default();
            if reset
                || clocks
                    .get(&row.item)
                    .is_none_or(|ready| *ready <= world.tick)
            {
                clocks.insert(row.item.clone(), deadline);
            }
        }
        Ok(())
    }

    pub(crate) fn restock(&self, world: &mut WorldState) -> GameResult<()> {
        for (id, definition) in &self.content.shops {
            let mut rows = definition.stock.clone();
            if let Some(UnstockedShopPolicy::Accept {
                rule, base_stock, ..
            }) = &definition.unstocked
            {
                let state = world
                    .shops
                    .get(id)
                    .ok_or_else(|| unknown("Missing shop state."))?;
                rows.extend(
                    state
                        .stock
                        .keys()
                        .filter(|item| !definition.stock.iter().any(|row| &row.item == *item))
                        .map(|item| ShopItem {
                            item: item.clone(),
                            base_stock: *base_stock,
                            restock_ticks: rule.restock.interval_ticks,
                            buy_price: 1,
                            sell_price: 0,
                            mechanics: Some(rule.clone()),
                        }),
                );
            }
            for row in rows {
                let stock = world
                    .shops
                    .get(id)
                    .and_then(|state| state.stock.get(&row.item))
                    .copied()
                    .ok_or_else(|| invalid_state("Shop row is missing."))?;
                if stock == row.base_stock {
                    continue;
                }
                let deadline = world
                    .runtime
                    .stock_deadlines
                    .get(id)
                    .and_then(|clocks| clocks.get(&row.item))
                    .copied()
                    .unwrap_or(self.stock_deadline(&row, world.tick.saturating_sub(1))?);
                if world.tick < deadline {
                    continue;
                }
                let amount = row.mechanics.as_ref().map_or(1, |m| m.restock.amount.get());
                let next = if stock < row.base_stock {
                    stock.saturating_add(amount).min(row.base_stock)
                } else {
                    stock.saturating_sub(amount).max(row.base_stock)
                };
                world
                    .shops
                    .get_mut(id)
                    .ok_or_else(|| unknown("Missing shop state."))?
                    .stock
                    .insert(row.item.clone(), next);
                if definition
                    .stock
                    .iter()
                    .any(|source| source.item == row.item)
                {
                    world
                        .runtime
                        .stock_deadlines
                        .entry(id.clone())
                        .or_default()
                        .insert(row.item.clone(), self.stock_deadline(&row, world.tick)?);
                }
            }
            world
                .shops
                .get_mut(id)
                .ok_or_else(|| unknown("Missing shop state."))?
                .next_restock_tick = self.next_restock(definition, world.tick)?;
        }
        Ok(())
    }
}

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
    let (mut low, mut high) = (1, quantity.get() - 1);
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
