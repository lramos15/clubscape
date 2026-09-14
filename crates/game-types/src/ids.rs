use serde::{Deserialize, Serialize};

use crate::{GameError, GameErrorCode};

fn validate_id(value: &str, kind: &str) -> Result<(), GameError> {
    let valid_segments = value.split('.').all(|segment| {
        !segment.is_empty()
            && segment.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
            })
    });
    if value.len() > 160
        || !value.starts_with(kind)
        || value.as_bytes().get(kind.len()) != Some(&b'.')
        || !valid_segments
    {
        return Err(GameError::new(
            GameErrorCode::InvalidInput,
            format!("Expected a nonempty kind-first {kind} ID."),
        ));
    }
    Ok(())
}

macro_rules! content_id {
    ($name:ident, $kind:literal) => {
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, GameError> {
                let value = value.into();
                validate_id(&value, $kind)?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = GameError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

content_id!(ItemId, "item");
content_id!(SkillId, "skill");
content_id!(SlotId, "slot");
content_id!(RegionId, "region");
content_id!(SpawnId, "spawn");
content_id!(ActorId, "actor");
content_id!(NpcId, "npc");
content_id!(ObjectId, "object");
content_id!(QuestId, "quest");
content_id!(StageId, "stage");
content_id!(InterfaceId, "interface");
content_id!(AssetId, "asset");
content_id!(RecipeId, "recipe");
content_id!(ActionId, "action");
content_id!(DialogueId, "dialogue");
content_id!(ShopId, "shop");
content_id!(CounterId, "counter");
content_id!(EntitlementId, "entitlement");
content_id!(GrantId, "grant");
content_id!(CombatStyleId, "style");
content_id!(SpellId, "spell");
content_id!(ProjectileId, "projectile");
content_id!(PrayerId, "prayer");
content_id!(TravelId, "travel");
content_id!(InstanceTemplateId, "instance_template");
content_id!(InstanceId, "instance");
content_id!(ObjectTransformId, "transform");
content_id!(ObjectStateId, "object_state");
content_id!(TemporaryObjectId, "temporary_object");
content_id!(DynamicObjectId, "dynamic_object");
content_id!(GroundPolicyId, "ground_policy");
content_id!(ExperienceId, "experience");
content_id!(ReconciliationId, "reconciliation");
content_id!(ValueProviderId, "value_provider");
content_id!(ItemInstanceId, "item_instance");
content_id!(ChargeKindId, "charge");
content_id!(DeathId, "death");
content_id!(RecoveryItemId, "recovery_item");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_ids_validate_kind_segments_and_deserialization() {
        let item = ItemId::new("item.pickaxe.bronze").unwrap();
        assert_eq!(
            serde_json::from_str::<ItemId>(&serde_json::to_string(&item).unwrap()).unwrap(),
            item
        );
        for invalid in [
            "npc.goblin",
            "item.",
            "item..bronze",
            "item.Bronze",
            "item/bronze",
            "item.bronze\n",
            "item.\u{e9}",
            "",
        ] {
            assert!(ItemId::new(invalid).is_err(), "{invalid:?}");
            assert!(serde_json::from_str::<ItemId>(&format!("{invalid:?}")).is_err());
        }
        assert!(ItemId::new(format!("item.{}", "a".repeat(160))).is_err());
    }
}
