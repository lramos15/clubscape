use serde::{Deserialize, Serialize};

use crate::{
    DeathId, GameError, GameErrorCode, GameResult, InstanceId, InterfaceId, Quantity,
    RecoveryEntryControlView, RecoveryItemId, RecoveryStorage, UiPermission,
};

pub const RECOVERY_CONTEXT_VERSION: u32 = 1;
pub const MAX_RECOVERY_SELECTION_ENTRIES: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RecoveryContextIdentity {
    Grave {
        interface: InterfaceId,
        death: DeathId,
    },
    DeathOffice {
        interface: InterfaceId,
        instance: Option<InstanceId>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryContextEntrySelection {
    pub id: RecoveryItemId,
    pub quantity: Quantity,
    pub current_storage: RecoveryStorage,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryContextRecordSelection {
    pub death: DeathId,
    pub entries: Vec<RecoveryContextEntrySelection>,
}

/// An observation precondition, never client-supplied items or a transfer plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryContextSelection {
    pub context: RecoveryContextIdentity,
    pub records: Vec<RecoveryContextRecordSelection>,
}

impl RecoveryContextSelection {
    pub fn validate_shape(&self) -> GameResult<()> {
        use std::collections::BTreeSet;
        let mut deaths = BTreeSet::new();
        let mut entries = BTreeSet::new();
        if self.records.len() > MAX_RECOVERY_SELECTION_ENTRIES {
            return Err(GameError::new(
                GameErrorCode::InvalidInput,
                "Recovery context selection exceeds its record bound.",
            ));
        }
        for record in &self.records {
            if record.entries.is_empty() || !deaths.insert(&record.death) {
                return Err(GameError::new(
                    GameErrorCode::InvalidInput,
                    "Recovery context records must be distinct and nonempty.",
                ));
            }
            for entry in &record.entries {
                if !entries.insert(&entry.id) || entries.len() > MAX_RECOVERY_SELECTION_ENTRIES {
                    return Err(GameError::new(
                        GameErrorCode::InvalidInput,
                        "Recovery context entries must be distinct and bounded.",
                    ));
                }
                if let RecoveryContextIdentity::Grave { death, .. } = &self.context
                    && (death != &record.death || entry.current_storage != RecoveryStorage::Grave)
                {
                    return Err(GameError::new(
                        GameErrorCode::InvalidInput,
                        "A grave context can select only its own grave entries.",
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryCapacityUnit {
    Entries,
    ItemTypesOrInstances,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryContextCounts {
    pub entries: u32,
    /// Distinct original item IDs in the visible container; absent if any ID is unbound.
    pub native_item_types: Option<u32>,
    pub capacity: u16,
    pub capacity_unit: RecoveryCapacityUnit,
    /// Physically stored entries/keys, using the same rule as storage enforcement.
    pub stored: u32,
    /// Offered entries/keys, including still-grave items visible from the Office.
    pub offered: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RecoveryTypeCaption {
    Source {
        source_id: u32,
        quantity: String,
        unit_fee: String,
        /// Native display multiplication, not a combined executable fee quote.
        total_fee: String,
    },
    Unavailable {
        reason: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryContextSlotView {
    pub slot: u32,
    pub death: DeathId,
    pub current_storage: RecoveryStorage,
    pub entry: RecoveryEntryControlView,
    pub selected_type_caption: RecoveryTypeCaption,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryPlannedItemView {
    pub death: DeathId,
    pub id: RecoveryItemId,
    pub quantity: Quantity,
    pub fee: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryExecutionView {
    pub total_fee: String,
    pub transfers: Vec<RecoveryPlannedItemView>,
    pub partial: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryTakeAllControlView {
    pub permission: UiPermission,
    /// Echo unchanged in one recovery_take_all operation, not a loop of recovery_take requests.
    pub selection: RecoveryContextSelection,
    /// The shared planner's result for the current state, absent when permission is denied.
    pub plan: Option<RecoveryExecutionView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryContextView {
    pub version: u32,
    pub identity: RecoveryContextIdentity,
    pub counts: RecoveryContextCounts,
    /// Authoritative contiguous native slot order, independent of per-death panel layout.
    pub slots: Vec<RecoveryContextSlotView>,
    pub take_all: RecoveryTakeAllControlView,
}

fn invalid_context() -> GameError {
    GameError::new(
        GameErrorCode::InvalidInput,
        "Recovery context identities, counts, captions or plan are inconsistent.",
    )
}

fn decimal(value: &str) -> GameResult<u64> {
    value
        .parse::<u64>()
        .ok()
        .filter(|number| number.to_string() == value)
        .ok_or_else(invalid_context)
}

fn permission_shape(value: &UiPermission) -> GameResult<()> {
    if value.allowed {
        if value.code.is_some() || value.reason.is_some() {
            return Err(invalid_context());
        }
    } else if value.code.is_none() || value.reason.as_ref().is_none_or(String::is_empty) {
        return Err(invalid_context());
    }
    Ok(())
}

impl RecoveryContextView {
    /// Validates lossless projection relationships without repricing or executing an action.
    pub fn validate_shape(&self) -> GameResult<()> {
        use std::collections::{BTreeMap, BTreeSet};
        if self.version != RECOVERY_CONTEXT_VERSION
            || self.slots.len() > MAX_RECOVERY_SELECTION_ENTRIES
            || self.counts.entries as usize != self.slots.len()
            || self.counts.capacity == 0
            || self.counts.stored > u32::from(self.counts.capacity)
            || self.take_all.selection.context != self.identity
        {
            return Err(invalid_context());
        }
        self.take_all.selection.validate_shape()?;
        permission_shape(&self.take_all.permission)?;
        if self.take_all.permission.allowed != self.take_all.plan.is_some() {
            return Err(invalid_context());
        }
        let office = matches!(self.identity, RecoveryContextIdentity::DeathOffice { .. });
        let mut records: Vec<RecoveryContextRecordSelection> = Vec::new();
        let mut stored = BTreeSet::new();
        let mut offered = BTreeSet::new();
        let mut native = BTreeMap::<u32, u64>::new();
        let mut all_source_ids = true;
        for (index, slot) in self.slots.iter().enumerate() {
            let item = &slot.entry.item;
            let quantity = Quantity::new(item.quantity)?;
            if slot.slot as usize != index
                || slot.entry.inventory_capacity > item.quantity
                || slot.entry.bank_capacity > item.quantity
            {
                return Err(invalid_context());
            }
            permission_shape(&slot.entry.take)?;
            permission_shape(&slot.entry.bank)?;
            decimal(&slot.entry.unit_fee)?;
            decimal(&slot.entry.full_stack_fee)?;
            let key = item.instance_id.as_ref().map_or_else(
                || crate::RecoverySlotKey::Ordinary(item.item.clone()),
                |id| crate::RecoverySlotKey::Instance(id.clone()),
            );
            offered.insert(key.clone());
            if slot.current_storage == RecoveryStorage::DeathOffice {
                stored.insert(key);
            }
            if let Some(id) = item.source_id {
                let total = native.entry(id).or_default();
                *total = total
                    .checked_add(u64::from(item.quantity))
                    .ok_or_else(invalid_context)?;
            } else {
                all_source_ids = false;
            }
            if records
                .last()
                .is_none_or(|record| record.death != slot.death)
            {
                records.push(RecoveryContextRecordSelection {
                    death: slot.death.clone(),
                    entries: Vec::new(),
                });
            }
            records
                .last_mut()
                .ok_or_else(invalid_context)?
                .entries
                .push(RecoveryContextEntrySelection {
                    id: slot.entry.id.clone(),
                    quantity,
                    current_storage: slot.current_storage,
                });
        }
        if records != self.take_all.selection.records {
            return Err(invalid_context());
        }
        let count = |value: usize| u32::try_from(value).map_err(|_| invalid_context());
        if office {
            if self.counts.capacity_unit != RecoveryCapacityUnit::ItemTypesOrInstances
                || self.counts.stored != count(stored.len())?
                || self.counts.offered != count(offered.len())?
            {
                return Err(invalid_context());
            }
        } else if self.counts.capacity_unit != RecoveryCapacityUnit::Entries
            || self.counts.stored != self.counts.entries
            || self.counts.offered != self.counts.entries
        {
            return Err(invalid_context());
        }
        if self.counts.native_item_types
            != all_source_ids.then(|| count(native.len())).transpose()?
        {
            return Err(invalid_context());
        }
        for slot in &self.slots {
            match &slot.selected_type_caption {
                RecoveryTypeCaption::Source {
                    source_id,
                    quantity,
                    unit_fee,
                    total_fee,
                } if all_source_ids && slot.entry.item.source_id == Some(*source_id) => {
                    let quantity = decimal(quantity)?;
                    let unit = decimal(unit_fee)?;
                    if Some(&quantity) != native.get(source_id)
                        || unit_fee != &slot.entry.unit_fee
                        || quantity.checked_mul(unit) != Some(decimal(total_fee)?)
                    {
                        return Err(invalid_context());
                    }
                }
                RecoveryTypeCaption::Unavailable { reason }
                    if !all_source_ids && !reason.is_empty() => {}
                _ => return Err(invalid_context()),
            }
        }
        if let Some(plan) = &self.take_all.plan {
            if plan.transfers.is_empty() || plan.transfers.len() > self.slots.len() {
                return Err(invalid_context());
            }
            let expected: BTreeMap<_, _> = self
                .slots
                .iter()
                .map(|slot| ((&slot.death, &slot.entry.id), slot.entry.item.quantity))
                .collect();
            let mut seen = BTreeSet::new();
            let mut total = 0_u64;
            let mut partial = plan.transfers.len() != self.slots.len();
            for transfer in &plan.transfers {
                let key = (&transfer.death, &transfer.id);
                let owned = expected.get(&key).ok_or_else(invalid_context)?;
                if !seen.insert(key) || transfer.quantity.get() > *owned {
                    return Err(invalid_context());
                }
                partial |= transfer.quantity.get() != *owned;
                total = total
                    .checked_add(decimal(&transfer.fee)?)
                    .ok_or_else(invalid_context)?;
            }
            if decimal(&plan.total_fee)? != total || partial != plan.partial {
                return Err(invalid_context());
            }
        }
        Ok(())
    }
}
