use serde::{Deserialize, Serialize};

use crate::{
    DeathId, Guard, Quantity, RecoveryItemId, RecoveryStorage, SourceRecord, UiItem, UiPermission,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryUiDefinition {
    pub grave_bank: RecoveryBankRule,
    pub office_bank: RecoveryBankRule,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RecoveryBankRule {
    Allowed {
        guard: Guard,
        source: Vec<SourceRecord>,
    },
    Unavailable {
        reason: String,
        source: Vec<SourceRecord>,
    },
}

/// Semantic source amount selection. All is never encoded as a guessed integer sentinel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UiAmount {
    Quantity { quantity: Quantity },
    All {},
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryItemAmount {
    pub id: RecoveryItemId,
    pub amount: UiAmount,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryRecordSelection {
    pub death: DeathId,
    pub items: Vec<RecoveryItemId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryEntryControlView {
    pub id: RecoveryItemId,
    pub item: UiItem,
    /// Quote for requesting one unit now, including the entry's already-paid credit.
    pub unit_fee: String,
    /// Quote for this entire remaining entry now; not a per-unit value to multiply in the UI.
    pub full_stack_fee: String,
    /// Maximum executable units for this entry alone, including funds and source auto-equip.
    pub inventory_capacity: u32,
    /// Maximum executable units for this entry alone under the declared banking permission.
    pub bank_capacity: u32,
    pub take: UiPermission,
    pub bank: UiPermission,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryPanelControlView {
    pub death: DeathId,
    pub storage: RecoveryStorage,
    pub entries: Vec<RecoveryEntryControlView>,
    pub full_selection_fee: String,
    pub take_all: UiPermission,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryManagementView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<crate::RecoveryContextView>,
    pub bank_revision: String,
    pub panels: Vec<RecoveryPanelControlView>,
    pub bank_all: UiPermission,
    /// Exact currently-owned entry identities to echo for a Bank-All request.
    pub bank_all_records: Vec<RecoveryRecordSelection>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GameplayUiRequest;

    #[test]
    fn all_is_semantic_and_partial_quantities_are_positive() {
        let all = serde_json::to_string(&UiAmount::All {}).unwrap();
        assert_eq!(all, r#"{"kind":"all"}"#);
        assert_eq!(
            serde_json::from_str::<UiAmount>(&all).unwrap(),
            UiAmount::All {}
        );
        assert!(serde_json::from_str::<UiAmount>(r#"{"kind":"quantity","quantity":0}"#).is_err());
        assert!(serde_json::from_str::<UiAmount>(r#"{"kind":"all","quantity":5}"#).is_err());
        assert_eq!(
            serde_json::from_str::<UiAmount>(r#"{"kind":"quantity","quantity":5}"#).unwrap(),
            UiAmount::Quantity {
                quantity: Quantity::new(5).unwrap()
            }
        );
    }

    #[test]
    fn bank_all_requires_the_actual_bank_revision_not_a_recovery_revision() {
        let request = GameplayUiRequest::RecoveryBankAll {
            records: vec![RecoveryRecordSelection {
                death: DeathId::new("death.1").unwrap(),
                items: vec![RecoveryItemId::new("recovery_item.1").unwrap()],
            }],
        };
        assert!(request.requires_bank_revision());
        let encoded = serde_json::to_string(&request).unwrap();
        assert_eq!(
            serde_json::from_str::<GameplayUiRequest>(&encoded).unwrap(),
            request
        );
        assert!(
            !GameplayUiRequest::RecoveryTake {
                death: DeathId::new("death.1").unwrap(),
                storage: RecoveryStorage::DeathOffice,
                items: vec![RecoveryItemAmount {
                    id: RecoveryItemId::new("recovery_item.1").unwrap(),
                    amount: UiAmount::All {},
                }],
            }
            .requires_bank_revision()
        );
    }
}
