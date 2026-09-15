use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DisplayDefinition {
    pub name: String,
    pub source_id: Option<u32>,
    pub asset: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct QuestDefinition {
    pub name: String,
    pub completed_stage: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ShopDefinition {
    pub name: String,
    pub currency: String,
}

/// Display-only projection of compiler-validated content, never an authority for actions.
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DisplayCatalog {
    pub content_revision: String,
    pub items: BTreeMap<String, DisplayDefinition>,
    pub skills: BTreeMap<String, DisplayDefinition>,
    pub entities: BTreeMap<String, DisplayDefinition>,
    pub quests: BTreeMap<String, QuestDefinition>,
    pub equipment_slots: Vec<String>,
    #[serde(default)]
    pub icons: BTreeMap<String, String>,
    #[serde(default)]
    pub shops: BTreeMap<String, ShopDefinition>,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn from_compiled(content: &clubscape_content::CompiledContent) -> DisplayCatalog {
    let source = content.definition();
    let mut entities: BTreeMap<_, _> = source
        .npcs
        .values()
        .map(|value| {
            (
                value.id.to_string(),
                DisplayDefinition {
                    name: value.name.clone(),
                    source_id: Some(value.source_id),
                    asset: value.asset.as_ref().map(ToString::to_string),
                },
            )
        })
        .collect();
    entities.extend(source.objects.values().map(|value| {
        (
            value.id.to_string(),
            DisplayDefinition {
                name: value.name.clone(),
                source_id: Some(value.source_id),
                asset: value.asset.as_ref().map(ToString::to_string),
            },
        )
    }));
    DisplayCatalog {
        content_revision: source.revision.clone(),
        items: source
            .items
            .values()
            .map(|value| {
                (
                    value.id.to_string(),
                    DisplayDefinition {
                        name: value.name.clone(),
                        source_id: value.source_id,
                        asset: value.asset.as_ref().map(ToString::to_string),
                    },
                )
            })
            .collect(),
        skills: source
            .skills
            .values()
            .map(|value| {
                (
                    value.id.to_string(),
                    DisplayDefinition {
                        name: value.name.clone(),
                        source_id: Some(u32::from(value.source_id)),
                        asset: None,
                    },
                )
            })
            .collect(),
        entities,
        quests: source
            .quests
            .values()
            .map(|value| {
                (
                    value.id.to_string(),
                    QuestDefinition {
                        name: value.name.clone(),
                        completed_stage: value.completed_stage.to_string(),
                    },
                )
            })
            .collect(),
        equipment_slots: source
            .equipment_slots
            .iter()
            .map(ToString::to_string)
            .collect(),
        icons: BTreeMap::new(),
        shops: source
            .shops
            .values()
            .map(|shop| {
                (
                    shop.id.to_string(),
                    ShopDefinition {
                        name: shop.name.clone(),
                        currency: shop.currency.to_string(),
                    },
                )
            })
            .collect(),
    }
}
