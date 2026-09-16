use anyhow::{Context, Result, ensure};
use clubscape_protocol::{
    ClientMessage, ErrorCode, PROTOCOL_VERSION, client_message::Command, game,
};
use prost::Message;
use serde_json::{Value, json};

use super::{Runner, checkpoint, evidence};

pub(super) const AUTHORITY: &str = "bd33c742bad7be4ab244e1402cc840d89d3e427b";
pub(super) const SOURCE: &str = "5e0aa8a28851752ae0b8b0a08c635c6c8d2979f509f3a3e564ee74e2abfc8e6f";
pub(super) const QUERY_ERROR: &str = "688aab5a-a58b-4a57-bb48-ec9dcdd1391a";
pub(super) const QUERY_REQUEST: &str = "6ec49ce1-3987-43d4-9bc9-37afc6c6f2a3";

pub(super) fn allowed(command: &Command) -> bool {
    matches!(
        command,
        Command::Hello(_) | Command::Login(_) | Command::CurrentAccount(_) | Command::JoinWorld(_)
    ) || matches!(command, Command::PollWorld(query) if query.quote.is_none())
}

pub(super) fn command_name(command: &Command) -> &'static str {
    match command {
        Command::Hello(_) => "hello",
        Command::Login(_) => "login",
        Command::CurrentAccount(_) => "current_account",
        Command::JoinWorld(_) => "join_world",
        Command::PollWorld(_) => "poll_world",
        _ => "forbidden",
    }
}

pub(super) fn validate_control(
    value: &Value,
    permit_demonstrated_query_error: bool,
) -> Result<Value> {
    if value.is_null() {
        return Ok(json!({"recorded_control": "absent", "failed_control_exception_used": false}));
    }
    let bytes: Vec<u8> = serde_json::from_value(value["client_message_protobuf"].clone())
        .map_err(|_| anyhow::anyhow!("Invalid private control encoding"))?;
    ensure!(
        bytes.len() <= 16384,
        "Private control exceeds the protocol bound"
    );
    let message = ClientMessage::decode(bytes.as_slice())
        .map_err(|_| anyhow::anyhow!("Original private control cannot be decoded"))?;
    let operation = value["operation_id"]
        .as_str()
        .context("Original control identity missing")?;
    ensure!(
        message.protocol_version == PROTOCOL_VERSION && message.request_id == operation,
        "Original control protocol/correlation identity mismatch"
    );
    let command = message
        .command
        .context("Original control command missing")?;
    let status = value["observed_http_status"]
        .as_u64()
        .context("Original control outcome is unknown")?;
    let success = (200..300).contains(&status) && value["observed_error"].is_null();
    let query_error = permit_demonstrated_query_error
        && status == 409
        && operation == QUERY_REQUEST
        && matches!(&command, Command::PollWorld(query) if query.quote.is_none())
        && value["observed_error"] == json!([ErrorCode::Conflict as i32, QUERY_ERROR]);
    ensure!(
        success || query_error,
        "Unknown or failed mutating control is not authorized for observation"
    );
    Ok(json!({
        "operation_id": operation,
        "decoded_command": command_name(&command),
        "observed_http_status": status,
        "observed_error": value["observed_error"],
        "failed_control_exception_used": query_error,
        "request_replayed": false
    }))
}

pub(super) fn saved_boundary(capsule: &Value, report: &Value) -> Result<Value> {
    ensure!(
        capsule["source_identity"]["content_artifact"]["uncompressed_sha256"] == SOURCE
            && report["tutorial_edges_passed"]
                .as_array()
                .is_some_and(|edges| edges.len() == 70)
            && report["checks_passed"] == 321
            && report["current_action"] == "lumbridge.source_item_losing_death"
            && report["last_snapshot"]["tick"] == 1613
            && report["last_snapshot"]["player"]["vitals"]["hitpoints"] == 1
            && report["last_snapshot"]["next_sequence"] == 302
            && report["first_failure"]["reason"]
                .as_str()
                .is_some_and(|reason| reason.contains(QUERY_ERROR) && reason.contains("HTTP 409")),
        "Checkpoint is not the explicitly authorized historical dying-query boundary"
    );
    let control = validate_control(&capsule["latest_control_request"], true)?;
    Ok(json!({
        "authority": AUTHORITY,
        "original_recorded_control": control,
        "historical_query_error": report["first_failure"],
        "historical_public_tick": 1613,
        "historical_public_hitpoints": 1,
        "private_saved_tick": 1616,
        "private_saved_hitpoints": 0,
        "old_observation_rewritten": false
    }))
}

pub(super) fn public_facts(snapshot: &game::WorldSnapshot) -> Result<Value> {
    let player = snapshot
        .player
        .as_ref()
        .context("Public observation omitted player")?;
    let ui = snapshot
        .ui
        .as_ref()
        .context("Public observation omitted UI")?;
    ensure!(
        ui.version == 1
            && player.active_death.is_some()
            && player.tutorial_stage == "stage.tutorial.mainland"
            && snapshot.next_sequence == 302,
        "Public observation identity/source/death/sequence requirements failed"
    );
    ensure!(
        player.combat_style.is_some() && ui.combat_style == player.combat_style,
        "Public repaired style is missing or inconsistent"
    );
    Ok(json!({
        "tick": snapshot.tick, "revision": snapshot.revision,
        "character_revision": snapshot.character_revision, "next_sequence": snapshot.next_sequence,
        "actor_id": player.actor_id, "hitpoints": player.hitpoints,
        "region": player.region, "tile": player.tile.as_ref().map(super::source::Tile::from),
        "instance": player.instance, "active_death": player.active_death,
        "combat_style": player.combat_style, "activity": player.activity,
        "actor_action": player.action.as_ref().map(|action| json!({
            "activity": action.activity, "action_id": action.action_id,
            "animation": action.animation, "started_at_tick": action.started_at_tick,
            "observed_at_tick": action.observed_at_tick
        })),
        "ui_version": ui.version, "active_interface": ui.active_interface,
        "production_present": ui.production.is_some(), "document_present": ui.document.is_some(),
        "public_life_enum_exists": false,
        "private_phase_not_substituted_for_public_fields": true
    }))
}

impl Runner {
    pub(super) async fn observe_dying(&mut self, resume: checkpoint::Resume) -> Result<()> {
        ensure!(
            self.arguments.max_seconds <= 180,
            "Observation exceeds the authorized deadline"
        );
        self.account_id = resume.string("/private_authentication_do_not_publish/account_id")?;
        self.login_name = resume.string("/private_authentication_do_not_publish/login_name")?;
        self.password = resume.string("/private_authentication_do_not_publish/password")?;
        self.actor_id = resume.string("/actor_id")?;
        self.onboarding_receipt = resume.receipt("onboarding")?;
        self.reward_receipt = resume.receipt("reward")?;
        self.private_attempt = Some(checkpoint::Attempt::from_saved(
            &resume.capsule["latest_attempt"],
        )?);
        self.historical_observation = Some(resume.capsule["last_observed_state"].clone());
        self.evidence.report["dying_observation"] = json!({
            "authority": AUTHORITY,
            "original_boundary": resume.observation_boundary,
            "world_inputs_emitted": 0, "public_poll_succeeded": false,
            "successful_stop": false, "historical_input_count": self.input_count
        });
        self.hello().await?;
        self.login().await?;
        let account = self
            .rpc(
                Command::CurrentAccount(clubscape_protocol::CurrentAccount {}),
                true,
            )
            .await?;
        let clubscape_protocol::server_message::Result::Account(account) = account else {
            anyhow::bail!("CurrentAccount did not return the restored account");
        };
        ensure!(
            account
                .account
                .as_ref()
                .is_some_and(|value| value.account_id == self.account_id)
                && account.character_initialized,
            "Restored account/character identity changed"
        );
        self.join().await?;
        let joined = public_facts(&self.snapshot)?;
        self.evidence
            .append("dying_public_join_observation", joined.clone())?;
        self.evidence.report["dying_observation"]["join"] = joined;
        self.poll().await?;
        let polled = public_facts(&self.snapshot)?;
        self.evidence
            .append("dying_public_poll_observation", polled.clone())?;
        self.evidence.report["dying_observation"]["poll"] = polled;
        self.evidence.report["dying_observation"]["public_poll_succeeded"] = json!(true);
        self.evidence.report["dying_observation"]["successful_stop"] = json!(true);
        self.evidence.report["current_action"] = json!("observation.readable_public_state_stop");
        self.evidence.report["last_snapshot"] = evidence::public_snapshot(&self.snapshot)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn control(command: Command, status: u64, error: Value) -> Value {
        let message = ClientMessage {
            protocol_version: PROTOCOL_VERSION,
            request_id: QUERY_REQUEST.into(),
            command: Some(command),
        };
        json!({"operation_id": QUERY_REQUEST, "client_message_protobuf": message.encode_to_vec(),
            "observed_http_status": status, "observed_error": error})
    }

    #[test]
    fn failed_control_exception_requires_the_exact_typed_read_only_query() {
        let error = json!([ErrorCode::Conflict as i32, QUERY_ERROR]);
        let query = control(
            Command::PollWorld(game::PollWorld::default()),
            409,
            error.clone(),
        );
        assert!(validate_control(&query, true).is_ok());
        assert!(validate_control(&query, false).is_err());
        assert!(
            validate_control(
                &control(Command::Logout(clubscape_protocol::Logout {}), 409, error),
                true
            )
            .is_err()
        );
        assert!(
            validate_control(
                &control(
                    Command::PollWorld(game::PollWorld::default()),
                    503,
                    Value::Null
                ),
                true
            )
            .is_err()
        );
    }

    #[test]
    fn observation_rpc_whitelist_cannot_emit_gameplay_or_create_an_account() {
        assert!(allowed(&Command::Hello(clubscape_protocol::Hello {})));
        assert!(allowed(&Command::JoinWorld(game::JoinWorld {})));
        assert!(allowed(&Command::PollWorld(game::PollWorld::default())));
        assert!(!allowed(&Command::WorldInput(game::WorldInput::default())));
        assert!(!allowed(&Command::CreateCharacter(
            game::CreateCharacter::default()
        )));
        assert!(!allowed(&Command::Register(
            clubscape_protocol::Register::default()
        )));
        assert!(!allowed(&Command::LeaveWorld(game::LeaveWorld::default())));
    }

    #[test]
    fn actual_zero_hp_observation_is_valid_without_a_healthy_substitute() {
        let snapshot = game::WorldSnapshot {
            next_sequence: 302,
            player: Some(game::Player {
                hitpoints: 0,
                active_death: Some("death.original".into()),
                tutorial_stage: "stage.tutorial.mainland".into(),
                combat_style: Some("style.unarmed.punch.accurate".into()),
                ..Default::default()
            }),
            ui: Some(game::GameplayUiView {
                version: 1,
                combat_style: Some("style.unarmed.punch.accurate".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let facts = public_facts(&snapshot).unwrap();
        assert_eq!(facts["hitpoints"], 0);
        assert_eq!(facts["public_life_enum_exists"], false);
        assert_eq!(snapshot.player.unwrap().hitpoints, 0);
    }
}
