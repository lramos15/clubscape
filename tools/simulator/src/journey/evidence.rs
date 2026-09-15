use std::{
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, ensure};
use clubscape_protocol::game;
use prost::Message;
use prost_reflect::{DescriptorPool, DynamicMessage};
use serde_json::{Value, json};

use super::source::Tile;

pub const SEGMENTS: &[&str] = &[
    "registration_login",
    "source_initial_character",
    "full_tutorial_learning_the_ropes",
    "onboarding_recovery",
    "lumbridge_copper",
    "inventory_equipment_bank_shop",
    "goblin_combat",
    "source_death_office_grave_recovery",
    "cooks_legitimate_acquisition_partial_delivery",
    "cooks_reward_and_range",
    "after_quest_recovery",
];

pub fn timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

pub fn local_path(path: &Path) -> Result<PathBuf> {
    ensure!(
        !path.as_os_str().is_empty()
            && path
                .components()
                .all(|part| matches!(part, Component::Normal(_))),
        "Evidence/control paths must be relative project paths, without traversal"
    );
    let mut current = std::env::current_dir()?;
    for part in path.components() {
        current.push(part);
        if let Ok(metadata) = fs::symlink_metadata(&current) {
            ensure!(
                !metadata.file_type().is_symlink(),
                "Refusing symlinked evidence/control path"
            );
        }
    }
    Ok(current)
}

pub fn private_directory(path: &Path) -> Result<PathBuf> {
    let resolved = local_path(path)?;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&resolved)?;
    Ok(resolved)
}

pub fn write_json(path: &Path, value: &Value) -> Result<()> {
    let resolved = local_path(path)?;
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        private_directory(parent)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&resolved)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}

pub fn message_json<M: Message>(descriptor: &str, message: &M) -> Result<Value> {
    let pool = DescriptorPool::decode(clubscape_protocol::FILE_DESCRIPTOR_SET)?;
    let descriptor = pool
        .get_message_by_name(descriptor)
        .context("Missing generated message descriptor")?;
    let reflected = DynamicMessage::decode(descriptor, message.encode_to_vec().as_slice())?;
    Ok(serde_json::to_value(reflected)?)
}

pub fn action_json(action: &game::world_input::Action) -> Result<Value> {
    message_json(
        "clubscape.game.v1.WorldInput",
        &game::WorldInput {
            action: Some(action.clone()),
            ..Default::default()
        },
    )
}

pub fn message_json_with_defaults<M: Message>(descriptor: &str, message: &M) -> Result<Value> {
    let pool = DescriptorPool::decode(clubscape_protocol::FILE_DESCRIPTOR_SET)?;
    let descriptor = pool
        .get_message_by_name(descriptor)
        .context("Missing generated message descriptor")?;
    let reflected = DynamicMessage::decode(descriptor, message.encode_to_vec().as_slice())?;
    let mut bytes = Vec::new();
    reflected.serialize_with_options(
        &mut serde_json::Serializer::new(&mut bytes),
        &prost_reflect::SerializeOptions::new().skip_default_fields(false),
    )?;
    Ok(serde_json::from_slice(&bytes)?)
}

// Additive requests are resolved from the actual generated schema, not guessed tags or hand-built wire.
pub fn generated_action(field: &str, arguments: Value) -> Result<game::world_input::Action> {
    ensure!(
        matches!(field, "open_grave" | "open_death_office"),
        "Not an allowed recovery input"
    );
    let pool = DescriptorPool::decode(clubscape_protocol::FILE_DESCRIPTOR_SET)?;
    let descriptor = pool
        .get_message_by_name("clubscape.game.v1.WorldInput")
        .context("Missing generated WorldInput")?;
    ensure!(
        descriptor.get_field_by_name(field).is_some(),
        "Generated game.proto has no WorldInput.{field}. Integrate the real OpenGrave/OpenDeathOffice request and guarded recovery projection; generic OpenInterface cannot authorize recovery"
    );
    let encoded = serde_json::to_string(&json!({field: arguments}))?;
    let mut deserializer = serde_json::Deserializer::from_str(&encoded);
    let dynamic = DynamicMessage::deserialize(descriptor, &mut deserializer)
        .with_context(|| format!("Generated schema rejected public {field} request"))?;
    deserializer.end()?;
    let input = game::WorldInput::decode(dynamic.encode_to_vec().as_slice())?;
    input
        .action
        .context("Generated schema did not produce a public action")
}

pub fn stable_player(player: &game::Player) -> Value {
    let mut inventory: Vec<_> = player
        .inventory
        .iter()
        .map(|slot| {
            json!({
                "index": slot.index, "stack": slot.stack.as_ref().map(stack_json)
            })
        })
        .collect();
    inventory.sort_by_key(|slot| slot["index"].as_u64());
    let equipment: std::collections::BTreeMap<_, _> = player
        .equipment
        .iter()
        .map(|slot| (slot.slot.clone(), slot.stack.as_ref().map(stack_json)))
        .collect();
    let skills: std::collections::BTreeMap<_, _> = player
        .skills
        .iter()
        .map(|skill| {
            (
                skill.id.clone(),
                json!({"xp_tenths": skill.xp_tenths, "base_level": skill.base_level}),
            )
        })
        .collect();
    let quests: std::collections::BTreeMap<_, _> = player
        .quests
        .iter()
        .map(|quest| (quest.id.clone(), quest.stage.clone()))
        .collect();
    let settings: std::collections::BTreeMap<_, _> = player
        .settings
        .iter()
        .map(|setting| (setting.setting, setting.enabled))
        .collect();
    let mut interfaces = player.unlocked_interfaces.clone();
    interfaces.sort();
    json!({
        "actor_id": player.actor_id,
        "appearance": player.appearance,
        "appearance_confirmed": player.appearance_confirmed,
        "experience": player.experience,
        "region": player.region,
        "tile": player.tile.as_ref().map(Tile::from),
        "instance": player.instance,
        "inventory": inventory,
        "equipment": equipment,
        "skills": skills,
        "quests": quests,
        "quest_points": player.quest_points,
        "tutorial_stage": player.tutorial_stage,
        "interfaces": interfaces,
        "settings": settings,
        "combat_style": player.combat_style,
        "active_death": player.active_death
    })
}

pub fn stack_json(stack: &game::Stack) -> Value {
    json!({"item": stack.item, "quantity": stack.quantity, "instance_id": stack.instance_id, "charges": stack.charges})
}

pub fn bank_json(player: &game::Player) -> Value {
    let slots: std::collections::BTreeMap<_, _> = player
        .bank
        .iter()
        .map(|slot| (slot.index, slot.stack.as_ref().map(stack_json)))
        .collect();
    json!({"capacity": player.bank_capacity, "slots": slots})
}

pub fn public_snapshot(snapshot: &game::WorldSnapshot) -> Result<Value> {
    let player = snapshot
        .player
        .as_ref()
        .context("Snapshot has no local player")?;
    let mut state = stable_player(player);
    state["bank"] = if player.bank_open {
        bank_json(player)
    } else {
        Value::Null
    };
    state["bank_visibility"] = json!(if player.bank_open {
        "authorized_open_context"
    } else {
        "not_observable"
    });
    state["vitals"] = json!({
        "hitpoints": player.hitpoints, "prayer_points": player.prayer_points,
        "run_energy": player.run_energy, "activity": player.activity
    });
    Ok(json!({
        "revision": snapshot.revision, "tick": snapshot.tick,
        "character_revision": snapshot.character_revision,
        "next_sequence": snapshot.next_sequence, "player": state,
        "dialogue": snapshot.dialogue.as_ref().map(|dialogue| json!({
            "id": dialogue.id, "speaker": dialogue.speaker,
            "choices": dialogue.choices.iter().map(|choice| choice.id.clone()).collect::<Vec<_>>()
        })),
        "unavailable_views": snapshot.unavailable_views.iter().map(|view| json!({
            "view": view.view, "reason": view.reason
        })).collect::<Vec<_>>(),
        "event_history_gap": snapshot.event_history_gap
    }))
}

pub struct Evidence {
    pub report: Value,
    path: PathBuf,
    trace: BufWriter<File>,
}

impl Evidence {
    pub fn new(path: &Path) -> Result<Self> {
        local_path(path)?;
        if let Some(parent) = path.parent() {
            private_directory(parent)?;
        }
        let trace_path = path.with_extension("trace.jsonl");
        let trace = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(local_path(&trace_path)?)?;
        let mut evidence = Self {
            report: json!({
                "schema_version": 1, "scenario": "m1_fresh_account",
                "status": "running", "full_journey_passed": false,
                "milestone_accepted": false, "browser_signup_verified": false,
                "presentation_verified": false, "audio_verified": false,
                "performance_verified": false, "runelite_verified": false,
                "fixture_gameplay_evidence": false,
                "recorded_at_unix_ms": timestamp_ms(),
                "trace_path": trace_path,
                "trace_records": 0, "checks_passed": 0,
                "segments": SEGMENTS.iter().map(|name| ((*name).to_owned(), json!({"status": "unchecked"})))
                    .collect::<serde_json::Map<String, Value>>(),
                "tutorial_edges_passed": [], "recovery_checks_passed": []
            }),
            path: path.to_owned(),
            trace: BufWriter::new(trace),
        };
        evidence.flush()?;
        Ok(evidence)
    }

    pub fn append(&mut self, kind: &str, value: Value) -> Result<()> {
        let index = self.report["trace_records"].as_u64().unwrap_or(0) + 1;
        ensure!(index <= 30_000, "Trace record budget exhausted");
        serde_json::to_writer(
            &mut self.trace,
            &json!({
                "index": index, "at_unix_ms": timestamp_ms(), "kind": kind, "data": value
            }),
        )?;
        self.trace.write_all(b"\n")?;
        self.trace.flush()?;
        self.report["trace_records"] = json!(index);
        Ok(())
    }

    pub fn check(&mut self, label: &str, expected: Value, actual: Value) -> Result<()> {
        let passed = expected == actual;
        self.append(
            "source_checkpoint",
            json!({
                "label": label, "expected": expected, "actual": actual, "passed": passed
            }),
        )?;
        ensure!(
            passed,
            "Checkpoint {label}: expected {expected}, actual {actual}"
        );
        self.report["checks_passed"] =
            json!(self.report["checks_passed"].as_u64().unwrap_or(0) + 1);
        Ok(())
    }

    pub fn passed(&mut self, name: &str) -> Result<()> {
        ensure!(SEGMENTS.contains(&name), "Unknown segment {name}");
        self.report["segments"][name] = json!({"status": "passed", "at_unix_ms": timestamp_ms()});
        self.flush()
    }

    pub fn flush(&mut self) -> Result<()> {
        self.trace.flush()?;
        self.trace.get_ref().sync_data()?;
        write_json(&self.path, &self.report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_core_action_roundtrips_without_secrets() {
        let action = game::world_input::Action::SetSetting(game::SetSetting {
            setting: game::SettingKind::Run as i32,
            enabled: true,
        });
        let json = action_json(&action).unwrap();
        assert_eq!(json["setSetting"]["enabled"], true);
        assert!(json.get("worldSessionId").is_none());
    }

    #[test]
    fn generated_zero_fields_are_preserved_for_authorized_fee_views() {
        let value = message_json_with_defaults(
            "clubscape.game.v1.WorldSnapshot",
            &game::WorldSnapshot::default(),
        )
        .unwrap();
        assert_eq!(value["nextSequence"], "0");
        assert_eq!(value["characterRevision"], "0");
    }

    #[test]
    fn undeclared_recovery_request_is_an_error_not_a_forged_interface_open() {
        let pool = DescriptorPool::decode(clubscape_protocol::FILE_DESCRIPTOR_SET).unwrap();
        let has = pool
            .get_message_by_name("clubscape.game.v1.WorldInput")
            .unwrap()
            .get_field_by_name("open_grave")
            .is_some();
        let result = generated_action("open_grave", json!({"death":"death.synthetic"}));
        if !has {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("Generated game.proto has no")
            );
        }
        assert!(generated_action("advance_stage", json!({})).is_err());
    }

    #[test]
    fn evidence_paths_cannot_escape_the_project() {
        for path in [
            "/tmp/evidence",
            "../evidence",
            ".local/../evidence",
            "/var/tmp/evidence",
        ] {
            assert!(local_path(Path::new(path)).is_err());
        }
        assert!(local_path(Path::new(".local/evidence/journey.json")).is_ok());
    }

    #[test]
    fn recovery_fingerprint_ignores_regeneration_but_not_items_or_xp() {
        let mut player = game::Player::default();
        let initial = stable_player(&player);
        player.hitpoints = 10;
        player.run_energy = 10000;
        assert_eq!(stable_player(&player), initial);
        player.skills.push(game::Skill {
            id: "skill.mining".into(),
            xp_tenths: 175,
            ..Default::default()
        });
        assert_ne!(stable_player(&player), initial);
    }
}
