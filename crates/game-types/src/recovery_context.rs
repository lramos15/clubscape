use serde::{Deserialize, Serialize};

use crate::{
    DeathId, InstanceId, InterfaceId, Quantity, RecoveryEntryControlView, RecoveryItemId,
    RecoveryStorage, UiPermission,
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
