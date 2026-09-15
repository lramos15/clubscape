//! Non-spendable native bank layout metadata over the one authoritative Bank.
use clubscape_game_types::*;
use std::collections::BTreeSet;

fn error(code: GameErrorCode, message: &'static str) -> GameError {
    GameError::new(code, message)
}

pub fn reconcile(bank: &Bank, layout: &mut BankLayout, contents_changed: bool) -> GameResult<()> {
    let mut draft = layout.clone();
    reconcile_inner(bank, &mut draft, contents_changed)?;
    *layout = draft;
    Ok(())
}

fn reconcile_inner(bank: &Bank, layout: &mut BankLayout, contents_changed: bool) -> GameResult<()> {
    if bank.slots.len() > usize::from(bank.capacity) {
        return Err(error(GameErrorCode::InvalidInput, "Bank exceeds capacity."));
    }
    let before = layout.entries.clone();
    let mut entries = Vec::new();
    for (slot, value) in bank.slots.iter().enumerate() {
        let Some(stack) = value else { continue };
        let instance = stack.instance.as_ref().map(|instance| instance.id.clone());
        let previous = before
            .iter()
            .find(|entry| entry.item == stack.item && entry.instance == instance);
        let mut entry = if let Some(entry) = previous {
            entry.clone()
        } else {
            let id = layout.next_entry;
            layout.next_entry = layout
                .next_entry
                .checked_add(1)
                .filter(|id| *id <= i64::MAX as u64)
                .ok_or_else(|| {
                    error(
                        GameErrorCode::InvalidInput,
                        "Bank entry identities exhausted.",
                    )
                })?;
            BankLayoutEntry {
                id,
                slot: slot as u16,
                tab: layout.selected_tab,
                item: stack.item.clone(),
                instance,
                placeholder: false,
            }
        };
        entry.slot = slot as u16;
        entry.placeholder = false;
        entries.push(entry);
    }
    for entry in &before {
        if entries.iter().any(|current| current.id == entry.id) {
            continue;
        }
        if (entry.placeholder || layout.placeholders)
            && entry.instance.is_none()
            && usize::from(entry.slot) < usize::from(bank.capacity)
            && bank
                .slots
                .get(usize::from(entry.slot))
                .is_none_or(Option::is_none)
            && !entries.iter().any(|current| current.slot == entry.slot)
        {
            let mut entry = entry.clone();
            entry.placeholder = true;
            entries.push(entry);
        }
    }
    entries.sort_by_key(|entry| entry.slot);
    if layout.selected_tab != 0 && !entries.iter().any(|entry| entry.tab == layout.selected_tab) {
        layout.selected_tab = 0;
    }
    layout.entries = entries;
    normalize_tabs(layout);
    if layout.entries != before || contents_changed {
        bump(layout)?;
    }
    validate(bank, layout)
}

pub fn validate(bank: &Bank, layout: &BankLayout) -> GameResult<()> {
    if layout.version != 1
        || layout.next_entry == 0
        || layout.next_entry > i64::MAX as u64
        || layout.revision > i64::MAX as u64
        || bank.slots.len() > usize::from(bank.capacity)
        || layout.entries.len() > usize::from(bank.capacity)
        || layout
            .entries
            .windows(2)
            .any(|entries| entries[0].slot >= entries[1].slot)
        || layout.entries.iter().any(|entry| entry.tab > 9)
        || (layout.selected_tab != 0
            && !layout
                .entries
                .iter()
                .any(|entry| entry.tab == layout.selected_tab))
    {
        return Err(error(
            GameErrorCode::InvalidInput,
            "Invalid versioned source bank layout.",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut slots = BTreeSet::new();
    let mut keys = BTreeSet::new();
    for entry in &layout.entries {
        if entry.id == 0
            || entry.id >= layout.next_entry
            || entry.slot >= bank.capacity
            || !ids.insert(entry.id)
            || !slots.insert(entry.slot)
            || !keys.insert((&entry.item, &entry.instance))
        {
            return Err(error(
                GameErrorCode::InvalidInput,
                "Invalid bank entry identity.",
            ));
        }
        let stored = bank
            .slots
            .get(usize::from(entry.slot))
            .and_then(Option::as_ref);
        match (entry.placeholder, stored) {
            (true, None) if entry.instance.is_none() => {}
            (false, Some(stack))
                if stack.item == entry.item
                    && stack.instance.as_ref().map(|instance| &instance.id)
                        == entry.instance.as_ref() => {}
            _ => {
                return Err(error(
                    GameErrorCode::InvalidInput,
                    "Bank metadata disagrees with its contents.",
                ));
            }
        }
    }
    if bank
        .slots
        .iter()
        .enumerate()
        .any(|(index, stack)| stack.is_some() && !slots.contains(&(index as u16)))
    {
        return Err(error(
            GameErrorCode::InvalidInput,
            "Bank item lacks an entry identity.",
        ));
    }
    Ok(())
}

pub fn entry(layout: &BankLayout, id: &str) -> GameResult<BankLayoutEntry> {
    let parsed = id
        .parse::<u64>()
        .ok()
        .filter(|value| value.to_string() == id)
        .ok_or_else(|| error(GameErrorCode::InvalidInput, "Invalid bank entry identity."))?;
    layout
        .entries
        .iter()
        .find(|entry| entry.id == parsed)
        .cloned()
        .ok_or_else(|| error(GameErrorCode::StaleCommand, "Bank entry no longer exists."))
}

pub fn move_entry(
    bank: &mut Bank,
    layout: &mut BankLayout,
    id: &str,
    before: Option<&str>,
    tab: u8,
) -> GameResult<()> {
    change(bank, layout, |bank, layout| {
        move_inner(bank, layout, id, before, tab)
    })
}

fn move_inner(
    bank: &mut Bank,
    layout: &mut BankLayout,
    id: &str,
    before: Option<&str>,
    tab: u8,
) -> GameResult<()> {
    let moving = entry(layout, id)?;
    if tab != 0 && !layout.entries.iter().any(|entry| entry.tab == tab) {
        return Err(error(
            GameErrorCode::StaleCommand,
            "Bank tab no longer exists.",
        ));
    }
    let destination = before.map(|id| entry(layout, id)).transpose()?;
    if destination.as_ref().is_some_and(|entry| entry.tab != tab) {
        return Err(error(
            GameErrorCode::StaleCommand,
            "Destination entry is not in the selected source tab.",
        ));
    }
    if destination
        .as_ref()
        .is_some_and(|entry| entry.id == moving.id)
    {
        return Ok(());
    }
    let mut ordered: Vec<_> = layout
        .entries
        .iter()
        .cloned()
        .map(|entry| {
            let stack = bank.slots.get(usize::from(entry.slot)).cloned().flatten();
            (entry, stack)
        })
        .collect();
    let from = ordered
        .iter()
        .position(|(entry, _)| entry.id == moving.id)
        .unwrap();
    let to = destination.as_ref().map(|destination| {
        ordered
            .iter()
            .position(|(entry, _)| entry.id == destination.id)
            .unwrap()
    });
    if !layout.insert
        && moving.tab == tab
        && let Some(to) = to
    {
        ordered.swap(from, to);
    } else {
        let mut selected = ordered.remove(from);
        selected.0.tab = tab;
        let to = destination
            .as_ref()
            .and_then(|destination| {
                ordered
                    .iter()
                    .position(|(entry, _)| entry.id == destination.id)
            })
            .unwrap_or_else(|| {
                ordered
                    .iter()
                    .rposition(|(entry, _)| entry.tab == tab)
                    .map_or(0, |index| index + 1)
            });
        ordered.insert(to, selected);
    }
    install(bank, layout, ordered)
}

pub fn create_tab(
    bank: &mut Bank,
    layout: &mut BankLayout,
    id: &str,
    maximum: u8,
) -> GameResult<()> {
    change(bank, layout, |bank, layout| {
        create_tab_inner(bank, layout, id, maximum)
    })
}

fn create_tab_inner(
    bank: &mut Bank,
    layout: &mut BankLayout,
    id: &str,
    maximum: u8,
) -> GameResult<()> {
    if maximum == 0 || maximum > 9 {
        return Err(error(
            GameErrorCode::InvalidInput,
            "Invalid source tab capacity.",
        ));
    }
    let selected = entry(layout, id)?;
    let used: BTreeSet<_> = layout.entries.iter().map(|entry| entry.tab).collect();
    let tab = (1..=maximum)
        .find(|tab| !used.contains(tab))
        .ok_or_else(|| {
            error(
                GameErrorCode::InventoryFull,
                "All source bank tabs are in use.",
            )
        })?;
    layout
        .entries
        .iter_mut()
        .find(|entry| entry.id == selected.id)
        .unwrap()
        .tab = tab;
    layout.selected_tab = tab;
    compact_inner(bank, layout)
}

pub fn collapse_tab(bank: &mut Bank, layout: &mut BankLayout, tab: u8) -> GameResult<()> {
    change(bank, layout, |bank, layout| {
        collapse_inner(bank, layout, tab)
    })
}

fn collapse_inner(bank: &mut Bank, layout: &mut BankLayout, tab: u8) -> GameResult<()> {
    if tab == 0 || !layout.entries.iter().any(|entry| entry.tab == tab) {
        return Err(error(
            GameErrorCode::StaleCommand,
            "Bank tab does not exist.",
        ));
    }
    for entry in &mut layout.entries {
        if entry.tab == tab {
            entry.tab = 0;
        }
    }
    if layout.selected_tab == tab {
        layout.selected_tab = 0;
    }
    compact_inner(bank, layout)
}

pub fn release_placeholder(bank: &Bank, layout: &mut BankLayout, id: &str) -> GameResult<()> {
    validate(bank, layout)?;
    let mut draft = layout.clone();
    release_inner(bank, &mut draft, id)?;
    *layout = draft;
    Ok(())
}

fn release_inner(bank: &Bank, layout: &mut BankLayout, id: &str) -> GameResult<()> {
    let selected = entry(layout, id)?;
    if !selected.placeholder {
        return Err(error(
            GameErrorCode::InvalidInput,
            "Only a non-spendable placeholder can be released.",
        ));
    }
    layout.entries.retain(|entry| entry.id != selected.id);
    if layout.selected_tab != 0
        && !layout
            .entries
            .iter()
            .any(|entry| entry.tab == layout.selected_tab)
    {
        layout.selected_tab = 0;
    }
    normalize_tabs(layout);
    bump(layout)?;
    validate(bank, layout)
}

pub fn compact(bank: &mut Bank, layout: &mut BankLayout) -> GameResult<()> {
    change(bank, layout, compact_inner)
}

fn compact_inner(bank: &mut Bank, layout: &mut BankLayout) -> GameResult<()> {
    let mut ordered: Vec<_> = layout
        .entries
        .iter()
        .cloned()
        .map(|entry| {
            let stack = bank.slots.get(usize::from(entry.slot)).cloned().flatten();
            (entry, stack)
        })
        .collect();
    ordered.sort_by_key(|(entry, _)| (entry.tab, entry.slot));
    install(bank, layout, ordered)
}

fn change(
    bank: &mut Bank,
    layout: &mut BankLayout,
    edit: impl FnOnce(&mut Bank, &mut BankLayout) -> GameResult<()>,
) -> GameResult<()> {
    validate(bank, layout)?;
    let mut next_bank = bank.clone();
    let mut next_layout = layout.clone();
    edit(&mut next_bank, &mut next_layout)?;
    validate(&next_bank, &next_layout)?;
    *bank = next_bank;
    *layout = next_layout;
    Ok(())
}

fn install(
    bank: &mut Bank,
    layout: &mut BankLayout,
    ordered: Vec<(BankLayoutEntry, Option<ItemStack>)>,
) -> GameResult<()> {
    if ordered.len() > usize::from(bank.capacity) {
        return Err(error(
            GameErrorCode::InventoryFull,
            "Bank layout exceeds capacity.",
        ));
    }
    bank.slots.clear();
    layout.entries.clear();
    for (slot, (mut entry, stack)) in ordered.into_iter().enumerate() {
        entry.slot = slot as u16;
        layout.entries.push(entry);
        bank.slots.push(stack);
    }
    normalize_tabs(layout);
    if layout.selected_tab != 0
        && !layout
            .entries
            .iter()
            .any(|entry| entry.tab == layout.selected_tab)
    {
        layout.selected_tab = 0;
    }
    bump(layout)?;
    validate(bank, layout)
}

fn normalize_tabs(layout: &mut BankLayout) {
    let used: Vec<_> = layout
        .entries
        .iter()
        .filter_map(|entry| (entry.tab != 0).then_some(entry.tab))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let normalized = |tab| {
        used.iter()
            .position(|current| *current == tab)
            .map_or(0, |index| index as u8 + 1)
    };
    layout.selected_tab = normalized(layout.selected_tab);
    for entry in &mut layout.entries {
        entry.tab = normalized(entry.tab);
    }
}

pub fn bump(layout: &mut BankLayout) -> GameResult<()> {
    layout.revision = layout
        .revision
        .checked_add(1)
        .filter(|value| *value <= i64::MAX as u64)
        .ok_or_else(|| error(GameErrorCode::InvalidInput, "Bank revision exhausted."))?;
    Ok(())
}
