use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail, ensure};
use clubscape_protocol::{
    CurrentAccount, ErrorCode, Logout, client_message::Command, game,
    server_message::Result as Outcome,
};
use game::world_input::Action;
use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    Receipt, Runner, checkpoint,
    evidence::{self, bank_json, stable_player},
};
use crate::Connection;

impl Runner {
    pub(super) async fn verify_duplicate(&mut self, receipt: &Receipt, label: &str) -> Result<()> {
        let before = stable_player(self.player()?);
        let sequence = self.sequence;
        self.submit(receipt, true).await?;
        self.evidence
            .check(label, before, stable_player(self.player()?))?;
        self.evidence.check(
            "duplicate_did_not_consume_sequence",
            json!(sequence),
            json!(self.sequence),
        )?;
        self.recovery_record(label, "durable_duplicate_operation")
    }

    async fn reject_sequence(&mut self, sequence: u64, label: &str) -> Result<()> {
        self.bound()?;
        ensure!(
            !self.arguments.observe_dying,
            "Observation-only mode forbids sequence probes"
        );
        ensure!(
            self.input_count < self.arguments.max_inputs,
            "Journey input budget exhausted"
        );
        let before = stable_player(self.player()?);
        let expected_sequence = self.sequence;
        let operation_id = Uuid::new_v4().to_string();
        self.evidence.append(
            "intentional_sequence_probe",
            json!({
                "label": label, "operation_id": operation_id,
                "submitted_sequence": sequence, "authoritative_next_sequence": expected_sequence,
                "action": "close_interface", "expected_error": "CONFLICT"
            }),
        )?;
        self.input_count += 1;
        let input = game::WorldInput {
            world_session_id: self.world_session.clone(),
            sequence,
            expected_character_revision: Some(self.snapshot.character_revision),
            action: Some(Action::CloseInterface(game::Empty {})),
        };
        self.private_attempt = self
            .arguments
            .private_checkpoint_file
            .as_ref()
            .map(|_| checkpoint::Attempt::new(operation_id.clone(), input.clone()));
        let (status, result) = self
            .connection
            .request_with_id(Command::WorldInput(input), Some(&self.token), &operation_id)
            .await?;
        if let Some(attempt) = &mut self.private_attempt {
            match &result {
                Outcome::Error(error) => {
                    attempt.rejected(status.as_u16(), error.code, error.error_id.clone());
                }
                Outcome::ActionResult(reply)
                    if status.is_success()
                        && reply.operation_id == operation_id
                        && reply.sequence == sequence =>
                {
                    attempt.acknowledged(
                        reply.duplicate,
                        reply
                            .snapshot
                            .as_ref()
                            .map(|snapshot| snapshot.next_sequence),
                    );
                }
                _ => {}
            }
        }
        let Outcome::Error(error) = result else {
            bail!("Invalid sequence probe {label} was accepted; no rejection evidence");
        };
        self.evidence.append(
            "sequence_probe_reply",
            json!({
                "label": label, "http_status": status.as_u16(), "code": error.code,
                "error_id": error.error_id, "reason": error.message
            }),
        )?;
        ensure!(
            !status.is_success() && error.code == ErrorCode::Conflict as i32,
            "Sequence probe failed for a different reason: HTTP {status}, code={}, message={}",
            error.code,
            error.message
        );
        self.poll().await?;
        self.evidence
            .check(label, before, stable_player(self.player()?))?;
        self.evidence.check(
            "rejected_sequence_not_consumed",
            json!(expected_sequence),
            json!(self.sequence),
        )?;
        self.recovery_record(label, label)
    }

    fn recovery_record(&mut self, checkpoint: &str, check: &str) -> Result<()> {
        self.evidence.report["recovery_checks_passed"]
            .as_array_mut()
            .context("Missing recovery evidence ledger")?
            .push(json!({
                "checkpoint": checkpoint, "check": check, "tick": self.snapshot.tick,
                "next_sequence": self.sequence
            }));
        self.evidence.flush()
    }

    pub(super) async fn logout(&mut self) -> Result<()> {
        self.cancel().await?;
        let response = self
            .rpc(
                Command::LeaveWorld(game::LeaveWorld {
                    world_session_id: self.world_session.clone(),
                }),
                true,
            )
            .await?;
        ensure!(
            matches!(response, Outcome::WorldLeft(_)),
            "LeaveWorld did not acknowledge the real source logout transition"
        );
        let result = self.rpc(Command::Logout(Logout {}), true).await?;
        ensure!(
            matches!(result, Outcome::LoggedOut(_)),
            "Account Logout was not acknowledged"
        );
        let (status, result) = self
            .connection
            .request(
                Command::CurrentAccount(CurrentAccount {}),
                Some(&self.token),
            )
            .await?;
        ensure!(
            status == reqwest::StatusCode::UNAUTHORIZED
                && matches!(result, Outcome::Error(ref error) if error.code == ErrorCode::Unauthenticated as i32),
            "The old account token still works after logout"
        );
        self.token.clear();
        self.world_session.clear();
        Ok(())
    }

    async fn compare_recovered(
        &mut self,
        label: &str,
        before: &Value,
        sequence: u64,
        bank: Option<&Value>,
    ) -> Result<()> {
        self.evidence
            .check(label, before.clone(), stable_player(self.player()?))?;
        self.evidence.check(
            "recovered_next_sequence",
            json!(sequence),
            json!(self.sequence),
        )?;
        if let Some(bank) = bank {
            self.open_mainland_bank().await?;
            self.evidence.check(
                "bank_recovered_only_through_authorized_view",
                bank.clone(),
                bank_json(self.player()?),
            )?;
            self.input(Action::CloseInterface(game::Empty {})).await?;
        }
        Ok(())
    }

    pub(super) async fn recovery_checkpoint(
        &mut self,
        label: &str,
        receipt: &Receipt,
    ) -> Result<()> {
        self.label(&format!("recovery.{label}"))?;
        let bank = self
            .player()?
            .bank_open
            .then(|| bank_json(self.player().expect("joined player")));
        self.input(Action::CloseInterface(game::Empty {})).await?;
        self.cancel().await?;
        self.evidence.append(
            "durable_checkpoint",
            json!({
                "label": label, "state": evidence::public_snapshot(&self.snapshot)?,
                "bank_when_authorized": bank, "replayed_operation_id": receipt.operation_id,
                "source_rule": "rule.persistence.acknowledged_state",
                "private_flags_or_reward_ledgers_read": false
            }),
        )?;
        self.evidence.flush()?;
        self.verify_duplicate(receipt, &format!("{label}.before_reconnect"))
            .await?;
        self.reject_sequence(receipt.sequence, "reused_sequence_rejected")
            .await?;
        self.reject_sequence(self.sequence + 1, "future_sequence_rejected")
            .await?;

        let before = stable_player(self.player()?);
        let sequence = self.sequence;
        self.connection = Connection::new(&self.arguments.url)?;
        self.hello().await?;
        self.join().await?;
        self.compare_recovered(
            "transport_reconnect_preserves_source_state",
            &before,
            sequence,
            bank.as_ref(),
        )
        .await?;
        self.recovery_record(label, "transport_reconnect")?;
        self.verify_duplicate(receipt, &format!("{label}.after_transport_reconnect"))
            .await?;

        let before = stable_player(self.player()?);
        let sequence = self.sequence;
        self.logout().await?;
        self.login().await?;
        self.hello().await?;
        self.join().await?;
        self.compare_recovered(
            "logout_login_preserves_source_state",
            &before,
            sequence,
            bank.as_ref(),
        )
        .await?;
        self.recovery_record(label, "logout_login")?;
        self.verify_duplicate(receipt, &format!("{label}.after_logout_login"))
            .await?;

        let before = stable_player(self.player()?);
        let sequence = self.sequence;
        self.request_owned_restart(label).await?;
        self.hello().await?;
        self.join().await?;
        self.compare_recovered(
            "owned_restart_preserves_source_state",
            &before,
            sequence,
            bank.as_ref(),
        )
        .await?;
        self.recovery_record(label, "owned_server_restart")?;
        self.verify_duplicate(receipt, &format!("{label}.after_owned_restart"))
            .await?;
        self.evidence.passed(match label {
            "onboarding" => "onboarding_recovery",
            "after_quest" => "after_quest_recovery",
            _ => bail!("Unknown required recovery checkpoint"),
        })
    }

    async fn request_owned_restart(&mut self, label: &str) -> Result<()> {
        let directory = self.arguments.recovery_control_dir.as_ref().context(
            "Required server-restart checkpoint has no owned orchestrator. Run tools/journey-tests/run.py; the client never kills an arbitrary server or silently omits restart evidence",
        )?.clone();
        evidence::private_directory(&directory)?;
        let request_path = directory.join(format!("restart-{label}.request.json"));
        let acknowledgment_path = directory.join(format!("restart-{label}.ack.json"));
        ensure!(
            !evidence::local_path(&request_path)?.exists()
                && !evidence::local_path(&acknowledgment_path)?.exists(),
            "Restart handshake would reuse stale checkpoint files"
        );
        let request_id = Uuid::new_v4().to_string();
        atomic_json(
            &request_path,
            &json!({
                "schema_version": 1, "request_id": request_id, "checkpoint": label,
                "origin": self.arguments.url, "world_revision": self.snapshot.revision,
                "character_revision": self.snapshot.character_revision,
                "next_sequence": self.sequence,
                "public_source_state": stable_player(self.player()?),
                "require_same_isolated_database": true,
                "require_same_build_and_content": true
            }),
        )?;
        self.evidence.append(
            "owned_restart_requested",
            json!({"checkpoint":label, "request_id":request_id}),
        )?;
        self.evidence.flush()?;
        let deadline = Instant::now() + Duration::from_secs(180);
        loop {
            self.bound()?;
            ensure!(
                Instant::now() < deadline,
                "Owned orchestrator did not acknowledge restart within 180 seconds"
            );
            if let Ok(bytes) = fs::read(evidence::local_path(&acknowledgment_path)?) {
                ensure!(
                    bytes.len() <= 32 * 1024,
                    "Restart acknowledgment exceeds bound"
                );
                let ack: Value = serde_json::from_slice(&bytes)?;
                ensure!(
                    ack["request_id"] == request_id && ack["checkpoint"] == label,
                    "Restart acknowledgment identity mismatch"
                );
                ensure!(
                    ack["status"] == "restarted",
                    "Owned restart failed: {}",
                    ack["reason"]
                );
                ensure!(
                    ack["same_isolated_database"] == true,
                    "Restart did not retain the isolated database"
                );
                let old_pid = ack["old_server_pid"]
                    .as_u64()
                    .context("Restart omitted the old owned PID")?;
                let new_pid = ack["new_server_pid"]
                    .as_u64()
                    .context("Restart omitted the new owned PID")?;
                ensure!(
                    old_pid > 0 && new_pid > 0 && old_pid != new_pid,
                    "Restart did not replace the owned server process"
                );
                let origin = ack["origin"]
                    .as_str()
                    .context("Restart omitted loopback origin")?
                    .to_owned();
                self.connection = Connection::new(&origin)?;
                self.arguments.url = origin;
                self.evidence.append("owned_restart_acknowledgment", ack)?;
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }
}

fn atomic_json(path: &Path, value: &Value) -> Result<()> {
    let pending = path.with_extension("pending.json");
    evidence::write_json(&pending, value)?;
    fs::rename(evidence::local_path(&pending)?, evidence::local_path(path)?)?;
    Ok(())
}
