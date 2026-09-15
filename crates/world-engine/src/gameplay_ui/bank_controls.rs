use super::*;
use clubscape_simulation::{bank, bank_layout};

impl WorldEngine {
    pub(super) fn bank_ui_action(
        &self,
        world: &WorldState,
        character: &mut CharacterState,
        request: &GameplayUiRequest,
    ) -> GameResult<Vec<GameEvent>> {
        let banker = runtime::access(character, true)?.0.clone();
        self.bank_access(world, character, &banker)?;
        self.authorize(character, &["bank".into()])?;
        let maximum = self
            .content
            .ui
            .as_ref()
            .ok_or_else(|| unavailable("UI profile is absent."))?
            .bank
            .maximum_tabs;
        let mut events = Vec::new();
        let inventory_before = character.inventory.clone();
        match request {
            GameplayUiRequest::BankSelectTab { tab } => {
                let layout = &mut ui_mut(character)?.bank;
                if *tab != 0 && !layout.entries.iter().any(|entry| entry.tab == *tab) {
                    return Err(GameError::new(
                        GameErrorCode::StaleCommand,
                        "Source bank tab is no longer present.",
                    ));
                }
                layout.selected_tab = *tab;
                bank_layout::bump(layout)?;
            }
            GameplayUiRequest::BankSetInsert { enabled } => {
                let layout = &mut ui_mut(character)?.bank;
                layout.insert = *enabled;
                bank_layout::bump(layout)?;
            }
            GameplayUiRequest::BankSetPlaceholders { enabled } => {
                let layout = &mut ui_mut(character)?.bank;
                layout.placeholders = *enabled;
                bank_layout::bump(layout)?;
            }
            GameplayUiRequest::BankSetOptions { amount, noted } => {
                Quantity::new(*amount)?;
                let layout = &mut ui_mut(character)?.bank;
                layout.amount = *amount;
                layout.noted = *noted;
                bank_layout::bump(layout)?;
            }
            GameplayUiRequest::BankCreateTab { entry_id } => {
                let ui = character.runtime.ui.as_mut().unwrap();
                bank_layout::create_tab(&mut character.bank, &mut ui.bank, entry_id, maximum)?;
            }
            GameplayUiRequest::BankMove {
                entry_id,
                before_entry_id,
                tab,
            } => {
                let ui = character.runtime.ui.as_mut().unwrap();
                bank_layout::move_entry(
                    &mut character.bank,
                    &mut ui.bank,
                    entry_id,
                    before_entry_id.as_deref(),
                    *tab,
                )?;
            }
            GameplayUiRequest::BankCollapseTab { tab } => {
                let ui = character.runtime.ui.as_mut().unwrap();
                bank_layout::collapse_tab(&mut character.bank, &mut ui.bank, *tab)?;
            }
            GameplayUiRequest::BankReleasePlaceholder { entry_id } => {
                let ui = character.runtime.ui.as_mut().unwrap();
                bank_layout::release_placeholder(&character.bank, &mut ui.bank, entry_id)?;
            }
            GameplayUiRequest::BankPlaceholder { entry_id } => {
                let entry = bank_layout::entry(&ui_mut(character)?.bank, entry_id)?;
                if entry.placeholder {
                    return Err(GameError::new(
                        GameErrorCode::NotOwned,
                        "A placeholder has no withdrawable quantity.",
                    ));
                }
                let stored = character.bank.slots[usize::from(entry.slot)]
                    .as_ref()
                    .ok_or_else(|| invalid_state("Missing bank stack."))?;
                let quantity = stored.quantity;
                let noted = ui_mut(character)?.bank.noted;
                let enabled = ui_mut(character)?.bank.placeholders;
                ui_mut(character)?.bank.placeholders = true;
                bank::withdraw(
                    character,
                    &self.content,
                    usize::from(entry.slot),
                    quantity,
                    noted,
                )?;
                ui_mut(character)?.bank.placeholders = enabled;
            }
            GameplayUiRequest::BankWithdrawEntry {
                entry_id,
                quantity,
                noted,
            } => {
                let entry = bank_layout::entry(&ui_mut(character)?.bank, entry_id)?;
                if entry.placeholder {
                    return Err(GameError::new(
                        GameErrorCode::NotOwned,
                        "A placeholder cannot be spent or withdrawn.",
                    ));
                }
                self.withdraw(
                    world,
                    character,
                    &banker,
                    usize::from(entry.slot),
                    Quantity::new(*quantity)?,
                    *noted,
                )?;
            }
            GameplayUiRequest::BankDepositEquipment => {
                self.authorize(character, &["bank".into(), "bank_deposit".into()])?;
                let items = bank::deposit_equipment(character, &self.content)?;
                if !items.is_empty() {
                    events.push(GameEvent::ItemTransferred {
                        from: ContainerKind::Equipment,
                        to: ContainerKind::Bank,
                        items,
                    });
                }
            }
            _ => return Err(invalid_state("Not a bank control request.")),
        }
        if matches!(
            request,
            GameplayUiRequest::BankPlaceholder { .. } | GameplayUiRequest::BankWithdrawEntry { .. }
        ) {
            events.push(GameEvent::ItemTransferred {
                from: ContainerKind::Bank,
                to: ContainerKind::Inventory,
                items: crate::actions::removed_items(&character.inventory, &inventory_before)?,
            });
        }
        bank_layout::validate(
            &character.bank,
            &character.runtime.ui.as_ref().unwrap().bank,
        )?;
        Ok(events)
    }
}
