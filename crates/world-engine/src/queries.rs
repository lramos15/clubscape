//! Source-owned read-only projections. These do not execute intents, effects, ticks or RNG.
use clubscape_game_types::*;
use clubscape_simulation::{bank, inventory};
use serde::{Deserialize, Serialize};

use crate::{WorldEngine, commerce::transfer_up_to, invalid_state, runtime, unavailable, unknown};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Permission {
    pub allowed: bool,
    pub denial: Option<GameError>,
}

impl Permission {
    pub(crate) fn evaluate(result: GameResult<()>) -> GameResult<Self> {
        match result {
            Ok(()) => Ok(Self {
                allowed: true,
                denial: None,
            }),
            Err(error)
                if matches!(
                    error.code,
                    GameErrorCode::NotOwned
                        | GameErrorCode::OutOfReach
                        | GameErrorCode::RequirementNotMet
                        | GameErrorCode::Busy
                        | GameErrorCode::Blocked
                        | GameErrorCode::InventoryFull
                        | GameErrorCode::InsufficientItems
                        | GameErrorCode::Unavailable
                        | GameErrorCode::StackOverflow
                        | GameErrorCode::SessionConflict
                ) =>
            {
                Ok(Self {
                    allowed: false,
                    denial: Some(error),
                })
            }
            Err(error) => Err(error),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionView {
    pub name: String,
    pub permission: Permission,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetView {
    pub target: WorldTarget,
    pub name: String,
    pub tile: Tile,
    pub width: u8,
    pub height: u8,
    pub object: Option<ObjectId>,
    pub npc: Option<NpcId>,
    pub asset: Option<AssetId>,
    pub available: bool,
    pub interactions: Vec<InteractionView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroundItemView {
    pub id: String,
    pub tile: Tile,
    pub stack: ItemStack,
    pub can_take: Permission,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialogueOptionView {
    pub id: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialogueView {
    pub dialogue: DialogueId,
    pub speaker: SpawnId,
    pub node: String,
    pub text: String,
    pub choices: Vec<DialogueOptionView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BankView {
    pub banker: SpawnId,
    pub interface: Option<InterfaceId>,
    pub bank: Bank,
    pub deposit: Permission,
    pub withdraw: Permission,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BankQuote {
    pub requested: Quantity,
    pub transferred: ItemStack,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShopLineView {
    pub index: u16,
    pub item: ItemId,
    pub stock: u32,
    pub buy_price: u32,
    pub sell_price: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShopView {
    pub shop: ShopId,
    pub interface: Option<InterfaceId>,
    pub currency: ItemId,
    pub lines: Vec<ShopLineView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShopQuote {
    pub shop: ShopId,
    pub item: ItemId,
    pub requested: Quantity,
    pub quantity: u32,
    pub currency: ItemId,
    pub total_price: u32,
    pub stock_after: u32,
    pub partial_reason: Option<GameError>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryEntryView {
    pub id: RecoveryItemId,
    pub stack: ItemStack,
    pub layout: ItemLayout,
    pub full_entry_fee: u64,
    pub current_storage: RecoveryStorage,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryView {
    pub death: DeathId,
    pub storage: RecoveryStorage,
    pub interface: Option<InterfaceId>,
    pub entries: Vec<RecoveryEntryView>,
    pub active_ticks_remaining: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryQuote {
    pub death: DeathId,
    pub storage: RecoveryStorage,
    /// Full selected quantities, not a promise that a future partial transfer will fit.
    pub selected: Vec<RecoveryItemId>,
    pub full_selection_fee: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ContextView {
    None,
    Dialogue { view: DialogueView },
    Bank { view: BankView },
    Shop { view: ShopView },
    Recovery { views: Vec<RecoveryView> },
}

impl WorldEngine {
    fn query_actor<'a>(
        &self,
        world: &'a WorldState,
        actor: &ActorId,
    ) -> GameResult<&'a CharacterState> {
        self.check_world(world)?;
        let character = world.characters.get(actor).ok_or_else(|| {
            GameError::new(GameErrorCode::NotOwned, "Query actor is not in this world.")
        })?;
        crate::validation::character(character, &self.content)?;
        Ok(character)
    }

    pub fn dialogue_view(
        &self,
        world: &WorldState,
        actor: &ActorId,
    ) -> GameResult<Option<DialogueView>> {
        let character = self.query_actor(world, actor)?;
        let Some(open) = &character.dialogue else {
            return Ok(None);
        };
        self.validate_open_dialogue(world, character)?;
        let dialogue = self
            .content
            .dialogues
            .get(&open.id)
            .ok_or_else(|| unknown("Unknown opened dialogue."))?;
        let node = self.dialogue_node(dialogue, &open.node)?;
        let mut choices = Vec::new();
        for choice in &node.choices {
            if self.guard(world, character, &choice.guard, None)? {
                choices.push(DialogueOptionView {
                    id: choice.id.clone(),
                    text: choice.text.clone(),
                });
            }
        }
        Ok(Some(DialogueView {
            dialogue: open.id.clone(),
            speaker: open.speaker.clone(),
            node: node.id.clone(),
            text: node.text.clone(),
            choices,
        }))
    }

    pub fn bank_view(&self, world: &WorldState, actor: &ActorId) -> GameResult<BankView> {
        let character = self.query_actor(world, actor)?;
        let (banker, _) = runtime::access(character, true)?;
        self.bank_access(world, character, banker)?;
        Ok(BankView {
            banker: banker.clone(),
            interface: self.opened_interface(character)?,
            bank: character.bank.clone(),
            deposit: Permission::evaluate(
                self.authorize(character, &["bank".into(), "bank_deposit".into()]),
            )?,
            withdraw: Permission::evaluate(
                self.authorize(character, &["bank".into(), "bank_withdraw".into()]),
            )?,
        })
    }

    pub fn bank_deposit_quote(
        &self,
        world: &WorldState,
        actor: &ActorId,
        slot: u8,
        quantity: Quantity,
    ) -> GameResult<BankQuote> {
        let character = self.query_actor(world, actor)?;
        self.input_permission(character)?;
        let (banker, _) = runtime::access(character, true)?;
        self.bank_access(world, character, banker)?;
        self.authorize(character, &["bank".into(), "bank_deposit".into()])?;
        let item = &inventory::stack_at(&character.inventory, usize::from(slot))?.item;
        let available = inventory::count(&character.inventory, &self.content.items, item)?;
        let mut plan = character.clone();
        let (_, transferred) = transfer_up_to(
            &mut plan,
            Quantity::new(quantity.get().min(available))?,
            |draft, amount| bank::deposit(draft, &self.content, usize::from(slot), amount),
        )?;
        Ok(BankQuote {
            requested: quantity,
            transferred,
        })
    }

    pub fn bank_withdraw_quote(
        &self,
        world: &WorldState,
        actor: &ActorId,
        slot: u16,
        quantity: Quantity,
        noted: bool,
    ) -> GameResult<BankQuote> {
        let character = self.query_actor(world, actor)?;
        self.input_permission(character)?;
        let (banker, _) = runtime::access(character, true)?;
        self.bank_access(world, character, banker)?;
        self.authorize(character, &["bank".into(), "bank_withdraw".into()])?;
        let mut plan = character.clone();
        let (_, transferred) = transfer_up_to(&mut plan, quantity, |draft, amount| {
            bank::withdraw(draft, &self.content, usize::from(slot), amount, noted)
        })?;
        Ok(BankQuote {
            requested: quantity,
            transferred,
        })
    }

    pub fn shop_view(&self, world: &WorldState, actor: &ActorId) -> GameResult<ShopView> {
        let character = self.query_actor(world, actor)?;
        let (spawn, index) = runtime::access(character, false)?;
        let action = &runtime::interaction(&self.content, spawn, index)?.action;
        let shop = match action {
            InteractionAction::Shop { shop } | InteractionAction::OpenShop { shop, .. } => shop,
            _ => return Err(invalid_state("Opened session is not a source shop.")),
        };
        self.shop_access(world, character, shop)?;
        let definition = self
            .content
            .shops
            .get(shop)
            .ok_or_else(|| unknown("Unknown opened shop."))?;
        let state = world
            .shops
            .get(shop)
            .ok_or_else(|| invalid_state("Missing shop stock."))?;
        let stock = &state.stock;
        let count =
            definition.stock.len() + crate::commerce::live_shop_extras(definition, state).count();
        let mut lines = Vec::new();
        for index in 0..count {
            let row = self.shop_row(world, definition, index)?;
            let current = *stock
                .get(&row.item)
                .ok_or_else(|| invalid_state("Missing displayed stock row."))?;
            lines.push(ShopLineView {
                index: u16::try_from(index)
                    .map_err(|_| invalid_state("Shop view index overflow."))?,
                item: row.item.clone(),
                stock: current,
                buy_price: self.shop_price(&row, current, false)?,
                sell_price: self.shop_price(&row, current, true)?,
            });
        }
        Ok(ShopView {
            shop: shop.clone(),
            interface: self.opened_interface(character)?,
            currency: definition.currency.clone(),
            lines,
        })
    }

    pub fn shop_buy_quote(
        &self,
        world: &WorldState,
        actor: &ActorId,
        shop: &ShopId,
        index: u16,
        quantity: Quantity,
        expected_item: Option<&ItemId>,
    ) -> GameResult<ShopQuote> {
        let character = self.query_actor(world, actor)?;
        self.input_permission(character)?;
        self.shop_access(world, character, shop)?;
        self.authorize_intent(
            character,
            &GameIntent::ShopBuy {
                shop: shop.clone(),
                item_index: index,
                quantity,
                expected_item: expected_item.cloned(),
            },
        )?;
        let definition = self
            .content
            .shops
            .get(shop)
            .ok_or_else(|| unknown("Unknown shop."))?;
        let (row, plan) = self.buy_plan(
            world,
            character,
            definition,
            usize::from(index),
            quantity,
            expected_item,
        )?;
        Ok(ShopQuote {
            shop: shop.clone(),
            item: row.item,
            requested: quantity,
            quantity: plan.quantity,
            currency: definition.currency.clone(),
            total_price: plan.currency,
            stock_after: plan.stock,
            partial_reason: plan.refusal,
        })
    }

    pub fn shop_sell_quote(
        &self,
        world: &WorldState,
        actor: &ActorId,
        shop: &ShopId,
        slot: u8,
        quantity: Quantity,
    ) -> GameResult<ShopQuote> {
        let character = self.query_actor(world, actor)?;
        self.input_permission(character)?;
        self.shop_access(world, character, shop)?;
        self.authorize_intent(
            character,
            &GameIntent::ShopSell {
                shop: shop.clone(),
                inventory_slot: slot,
                quantity,
            },
        )?;
        let definition = self
            .content
            .shops
            .get(shop)
            .ok_or_else(|| unknown("Unknown shop."))?;
        let item = &inventory::stack_at(&character.inventory, usize::from(slot))?.item;
        let row = self.sale_row(world, definition, item)?;
        let plan = self.trade_plan(
            world,
            character,
            definition,
            &row,
            quantity,
            Some(usize::from(slot)),
        )?;
        Ok(ShopQuote {
            shop: shop.clone(),
            item: row.item,
            requested: quantity,
            quantity: plan.quantity,
            currency: definition.currency.clone(),
            total_price: plan.currency,
            stock_after: plan.stock,
            partial_reason: plan.refusal,
        })
    }

    pub fn target_view(
        &self,
        world: &WorldState,
        actor: &ActorId,
        target: &WorldTarget,
    ) -> GameResult<Option<TargetView>> {
        let character = self.query_actor(world, actor)?;
        if let WorldTarget::Spawn { spawn } = target {
            if !self.content.spawns.contains_key(spawn) {
                return Err(unknown("Unknown source target."));
            }
            let entities = match &character.runtime.instance {
                Some(id) => {
                    &world
                        .runtime
                        .instances
                        .get(id)
                        .ok_or_else(|| invalid_state("Unknown instance."))?
                        .entities
                }
                None => &world.entities,
            };
            if !entities.contains_key(spawn) {
                return Ok(None);
            }
        }
        let shape = match self.resolve_shape(world, character, target, false) {
            Ok(shape) => shape,
            Err(error)
                if matches!(
                    error.code,
                    GameErrorCode::NotOwned | GameErrorCode::OutOfReach
                ) =>
            {
                return Ok(None);
            }
            Err(error) => return Err(error),
        };
        if shape.tile.plane() != character.tile.plane() {
            return Ok(None);
        }
        let (name, asset) = if let Some(id) = &shape.npc {
            let definition = self
                .content
                .npcs
                .get(id)
                .ok_or_else(|| unknown("Unknown resolved NPC."))?;
            (definition.name.clone(), definition.asset.clone())
        } else if let Some(id) = &shape.object {
            let definition = self
                .content
                .objects
                .get(id)
                .ok_or_else(|| unknown("Unknown resolved object."))?;
            (definition.name.clone(), definition.asset.clone())
        } else {
            let WorldTarget::Spawn { spawn } = target else {
                return Err(invalid_state("Temporary target has no object."));
            };
            let SpawnKind::Item { stack, .. } = &self.content.spawns[spawn].kind else {
                return Err(invalid_state("Unclassified target."));
            };
            let item = self
                .content
                .items
                .get(&stack.item)
                .ok_or_else(|| unknown("Unknown source item."))?;
            (item.name.clone(), item.asset.clone())
        };
        let available = match target {
            WorldTarget::Spawn { spawn } => {
                let entity = runtime::entity(world, character.runtime.instance.as_ref(), spawn)?;
                entity.available_at_tick <= world.tick
                    && shape.npc.as_ref().is_none_or(|id| {
                        self.content.npcs[id].combat.is_none() || entity.hitpoints > 0
                    })
            }
            WorldTarget::TemporaryObject { .. } => true,
        };
        Ok(Some(TargetView {
            target: target.clone(),
            name,
            tile: shape.tile,
            width: shape.width,
            height: shape.height,
            object: shape.object,
            npc: shape.npc,
            asset,
            available,
            interactions: self.interaction_options(world, actor, target)?,
        }))
    }

    pub fn interaction_options(
        &self,
        world: &WorldState,
        actor: &ActorId,
        target: &WorldTarget,
    ) -> GameResult<Vec<InteractionView>> {
        let character = self.query_actor(world, actor)?;
        let mut options = Vec::new();
        for interaction in self.target_interactions(world, target)? {
            let intent = match target {
                WorldTarget::Spawn { spawn } => GameIntent::Interact {
                    target: spawn.clone(),
                    action: interaction.name.clone(),
                },
                _ => GameIntent::InteractWith {
                    target: target.clone(),
                    action: interaction.name.clone(),
                },
            };
            let check = self
                .input_permission(character)
                .and_then(|()| self.authorize_intent(character, &intent))
                .and_then(|()| {
                    if matches!(interaction.action, InteractionAction::Attack) {
                        let WorldTarget::Spawn { spawn } = target else {
                            return Err(invalid_state("Temporary object cannot be attacked."));
                        };
                        return self.attack_permission(world, character, spawn);
                    }
                    self.require_world_target(world, character, target, interaction)?;
                    match &interaction.action {
                        InteractionAction::Unavailable { reason } => {
                            Err(unavailable(reason.clone()))
                        }
                        InteractionAction::Gather { rule } => {
                            let WorldTarget::Spawn { spawn } = target else {
                                return Err(unavailable("Gathering requires a source spawn."));
                            };
                            self.authorize(
                                character,
                                &["gather".into(), format!("gather:{spawn}")],
                            )?;
                            self.check_gather(character, rule)
                        }
                        InteractionAction::OpenBank { interface, .. } => {
                            self.authorize(character, &["bank".into()])?;
                            self.require_context_interface(character, interface)
                        }
                        InteractionAction::OpenShop {
                            interface, shop, ..
                        } => {
                            self.authorize(character, &["shop".into(), format!("shop:{shop}")])?;
                            self.require_context_interface(character, interface)
                        }
                        InteractionAction::Bank => self.authorize(character, &["bank".into()]),
                        InteractionAction::Shop { shop } => {
                            self.authorize(character, &["shop".into(), format!("shop:{shop}")])
                        }
                        InteractionAction::Dialogue { dialogue } => {
                            self.dialogue_entry(world, character, dialogue).map(|_| ())
                        }
                        InteractionAction::TravelVia { travel } => {
                            self.travel_permission(world, character, travel)
                        }
                        _ => Ok(()),
                    }
                });
            options.push(InteractionView {
                name: interaction.name.clone(),
                permission: Permission::evaluate(check)?,
            });
        }
        Ok(options)
    }

    pub fn ground_item_views(
        &self,
        world: &WorldState,
        actor: &ActorId,
    ) -> GameResult<Vec<GroundItemView>> {
        let character = self.query_actor(world, actor)?;
        let mut items = Vec::new();
        for item in &world.ground_items {
            if item.instance != character.runtime.instance
                || item.tile.plane() != character.tile.plane()
                || item.expires_at_tick <= world.tick
                || item.owner.as_ref().is_some_and(|owner| owner != actor)
                    && item.public_at_tick > world.tick
            {
                continue;
            }
            let permission = self
                .input_permission(character)
                .and_then(|()| self.ground_access(world, character, &item.id))
                .and_then(|(_, item)| {
                    inventory::add(
                        &mut character.inventory.clone(),
                        &self.content.items,
                        &item.stack,
                    )
                });
            items.push(GroundItemView {
                id: item.id.clone(),
                tile: item.tile,
                stack: item.stack.clone(),
                can_take: Permission::evaluate(permission)?,
            });
        }
        Ok(items)
    }

    pub fn recovery_view(
        &self,
        world: &WorldState,
        actor: &ActorId,
        death: &DeathId,
        storage: RecoveryStorage,
    ) -> GameResult<RecoveryView> {
        let character = self.query_actor(world, actor)?;
        let record =
            world.runtime.deaths.get(death).ok_or_else(|| {
                GameError::new(GameErrorCode::NotOwned, "Unknown recovery record.")
            })?;
        if &record.owner != actor {
            return Err(GameError::new(
                GameErrorCode::NotOwned,
                "Recovery belongs to another actor.",
            ));
        }
        let policy = self
            .content
            .mechanics
            .death
            .as_ref()
            .ok_or_else(|| unavailable("Death policy is not configured."))?;
        let (entries, ticks, interface, rule) = match storage {
            RecoveryStorage::Grave => {
                self.grave_access(world, character, death)?;
                let grave = record
                    .grave
                    .as_ref()
                    .ok_or_else(|| GameError::new(GameErrorCode::NotOwned, "No active grave."))?;
                (
                    grave
                        .items
                        .iter()
                        .map(|item| (item, RecoveryStorage::Grave))
                        .collect::<Vec<_>>(),
                    Some(grave.active_ticks_remaining),
                    policy.interfaces.as_ref().map(|ids| ids.grave.clone()),
                    policy.grave_fee.require()?,
                )
            }
            RecoveryStorage::DeathOffice => {
                self.office_access(world, character)?;
                (
                    record
                        .office
                        .iter()
                        .map(|item| (item, RecoveryStorage::DeathOffice))
                        .chain(record.grave.iter().flat_map(|grave| {
                            grave
                                .items
                                .iter()
                                .map(|item| (item, RecoveryStorage::Grave))
                        }))
                        .collect::<Vec<_>>(),
                    None,
                    policy.interfaces.as_ref().map(|ids| ids.office.clone()),
                    policy.office_fee.require()?,
                )
            }
        };
        let entries = entries
            .iter()
            .map(|(item, current_storage)| {
                Ok(RecoveryEntryView {
                    id: item.id.clone(),
                    stack: item.stack.clone(),
                    layout: item.layout.clone(),
                    full_entry_fee: crate::death::recovery_fee(
                        rule,
                        item,
                        item.stack.quantity.get(),
                    )?,
                    current_storage: *current_storage,
                })
            })
            .collect::<GameResult<Vec<_>>>()?;
        Ok(RecoveryView {
            death: death.clone(),
            storage,
            interface,
            entries,
            active_ticks_remaining: ticks,
        })
    }

    pub fn recovery_quote(
        &self,
        world: &WorldState,
        actor: &ActorId,
        death: &DeathId,
        storage: RecoveryStorage,
        selected: &[RecoveryItemId],
    ) -> GameResult<RecoveryQuote> {
        let view = self.recovery_view(world, actor, death, storage)?;
        if selected.is_empty()
            || selected
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != selected.len()
        {
            return Err(invalid_state("Select distinct recovery entries."));
        }
        let mut fee = 0_u64;
        for id in selected {
            let entry = view
                .entries
                .iter()
                .find(|entry| &entry.id == id)
                .ok_or_else(|| {
                    GameError::new(
                        GameErrorCode::NotOwned,
                        "Recovery entry is not in this panel.",
                    )
                })?;
            fee = fee
                .checked_add(entry.full_entry_fee)
                .ok_or_else(|| invalid_state("Recovery quote overflow."))?;
        }
        let policy = self
            .content
            .mechanics
            .death
            .as_ref()
            .ok_or_else(|| unknown("Missing death policy."))?;
        let rule = if storage == RecoveryStorage::Grave {
            policy.grave_fee.require()?
        } else {
            policy.office_fee.require()?
        };
        if let RecoveryFee::Bands { maximum_total, .. } = rule {
            fee = fee.min(*maximum_total);
        }
        Ok(RecoveryQuote {
            death: death.clone(),
            storage,
            selected: selected.to_vec(),
            full_selection_fee: fee,
        })
    }

    pub fn context_view(&self, world: &WorldState, actor: &ActorId) -> GameResult<ContextView> {
        let character = self.query_actor(world, actor)?;
        if let Some(view) = self.dialogue_view(world, actor)? {
            return Ok(ContextView::Dialogue { view });
        }
        match &runtime::schedule(character)?.access {
            None => Ok(ContextView::None),
            Some(ContainerSession::Bank { .. }) => Ok(ContextView::Bank {
                view: self.bank_view(world, actor)?,
            }),
            Some(ContainerSession::Shop { .. }) => Ok(ContextView::Shop {
                view: self.shop_view(world, actor)?,
            }),
            Some(ContainerSession::Grave { death, .. }) => Ok(ContextView::Recovery {
                views: vec![self.recovery_view(world, actor, death, RecoveryStorage::Grave)?],
            }),
            Some(ContainerSession::DeathOffice { .. }) => {
                let views = world
                    .runtime
                    .deaths
                    .iter()
                    .filter(|(_, record)| &record.owner == actor)
                    .map(|(death, _)| {
                        self.recovery_view(world, actor, death, RecoveryStorage::DeathOffice)
                    })
                    .collect::<GameResult<Vec<_>>>()?;
                Ok(ContextView::Recovery { views })
            }
        }
    }
}
