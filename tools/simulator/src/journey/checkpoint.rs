use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};

use anyhow::{Context, Result, ensure};
use clubscape_protocol::{ClientMessage, PROTOCOL_VERSION, client_message::Command, game};
use prost::Message;
use serde::Serialize;
use serde_json::{Value, json};

use super::{Receipt, Runner, evidence, source};

const MAX_CAPSULE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Response {
    Unresolved,
    AcknowledgmentReceived {
        duplicate: bool,
        next_sequence: Option<u64>,
    },
    ErrorReceived {
        http_status: u16,
        code: i32,
        error_id: String,
    },
}

pub(super) struct Attempt {
    operation_id: String,
    input: game::WorldInput,
    response: Response,
}

pub(super) struct ControlAttempt {
    message: ClientMessage,
    bearer_token: Option<String>,
    http_status: Option<u16>,
    error: Option<(i32, String)>,
}

impl ControlAttempt {
    pub(super) fn new(
        operation_id: String,
        command: Command,
        bearer_token: Option<String>,
    ) -> Self {
        Self {
            message: ClientMessage {
                protocol_version: PROTOCOL_VERSION,
                request_id: operation_id,
                command: Some(command),
            },
            bearer_token,
            http_status: None,
            error: None,
        }
    }

    pub(super) fn received(&mut self, status: u16, error: Option<(i32, String)>) {
        self.http_status = Some(status);
        self.error = error;
    }

    fn value(&self) -> Value {
        json!({
            "operation_id": self.message.request_id,
            "client_message_protobuf": self.message.encode_to_vec(),
            "private_bearer_token": self.bearer_token,
            "observed_http_status": self.http_status,
            "observed_error": self.error,
            "receipt_absence_is_not_rollback_proof": true,
            "retry_authorized": false
        })
    }
}

impl Attempt {
    pub(super) fn new(operation_id: String, input: game::WorldInput) -> Self {
        Self {
            operation_id,
            input,
            response: Response::Unresolved,
        }
    }

    pub(super) fn acknowledged(&mut self, duplicate: bool, next_sequence: Option<u64>) {
        self.response = Response::AcknowledgmentReceived {
            duplicate,
            next_sequence,
        };
    }

    pub(super) fn rejected(&mut self, http_status: u16, code: i32, error_id: String) {
        self.response = Response::ErrorReceived {
            http_status,
            code,
            error_id,
        };
    }

    fn value(&self) -> Value {
        json!({
            "operation_id": self.operation_id,
            "sequence": self.input.sequence,
            "world_input_protobuf": self.input.encode_to_vec(),
            "observed_response": self.response,
            "receipt_state_reconciliation_required": true,
            "retry_authorized": false
        })
    }
}

fn receipt_value(receipt: &Receipt) -> Value {
    let action = game::WorldInput {
        action: Some(receipt.action.clone()),
        ..Default::default()
    };
    json!({
        "operation_id": receipt.operation_id,
        "sequence": receipt.sequence,
        "observed_character_revision": receipt.observed_revision,
        "action_as_world_input_protobuf": action.encode_to_vec()
    })
}

pub(super) fn capture(runner: &Runner, path: &Path) -> Result<Value> {
    ensure!(
        !runner.account_id.is_empty()
            && !runner.actor_id.is_empty()
            && runner.snapshot.player.is_some(),
        "Private checkpoint has no actually observed source character"
    );
    let value = json!({
        "schema_version": 1,
        "kind": "private_m1_client_checkpoint",
        "scenario": "m1_fresh_account",
        "private_authentication_do_not_publish": {
            "account_id": runner.account_id,
            "login_name": runner.login_name,
            "password": runner.password,
            "token": runner.token,
            "world_session_id": runner.world_session
        },
        "actor_id": runner.actor_id,
        "source_identity": runner.source.identity,
        "server_build_revision": runner.evidence.report["server_build_revision"],
        "report_path": runner.arguments.report,
        "trace_path": runner.evidence.report["trace_path"],
        "current_action": runner.evidence.report["current_action"],
        "expected_stage": runner.expected_stage,
        "last_observed_state": evidence::public_snapshot(&runner.snapshot)?,
        "latest_attempt": runner.private_attempt.as_ref().map(Attempt::value),
        "latest_control_request": runner.private_control.as_ref().map(ControlAttempt::value),
        "original_receipts": {
            "onboarding": runner.onboarding_receipt.as_ref().map(receipt_value),
            "reward": runner.reward_receipt.as_ref().map(receipt_value)
        },
        "private_rng_source": "Only the actual server's PostgreSQL snapshot; the client never supplies or reconstructs RNG.",
        "automatic_restore": false,
        "resume_authorized": false,
        "receipt_state_reconciliation_required": true
    });
    write_capsule(path, &value)
}

fn write_capsule(path: &Path, value: &Value) -> Result<Value> {
    let parts = path
        .iter()
        .map(|part| part.to_str())
        .collect::<Option<Vec<_>>>()
        .context("Private checkpoint path must use the owned ASCII run layout")?;
    ensure!(
        parts.len() == 5
            && parts[0] == ".local"
            && parts[1] == "journey-runs"
            && parts[2].len() == 16
            && parts[2]
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
            && parts[3] == "control"
            && parts[4] == "private-client-checkpoint.json",
        "Private checkpoint must remain in the owned ignored run/control directory"
    );
    let resolved = evidence::local_path(path)?;
    let parent = resolved
        .parent()
        .context("Missing private checkpoint parent")?;
    let parent_metadata = fs::metadata(parent)?;
    ensure!(
        parent_metadata.is_dir() && parent_metadata.mode() & 0o777 == 0o700,
        "Private checkpoint parent must already have mode 0700"
    );
    let bytes = serde_json::to_vec(value).context("Private checkpoint serialization failed")?;
    ensure!(
        bytes.len() <= MAX_CAPSULE_BYTES,
        "Private checkpoint exceeds its two-MiB bound"
    );
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&resolved)?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.uid() == parent_metadata.uid() && metadata.mode() & 0o777 == 0o600,
        "Private checkpoint file must have the private directory's owner and mode 0600"
    );
    file.write_all(&bytes)?;
    file.sync_all()?;
    File::open(parent)?.sync_all()?;
    Ok(json!({
        "status": "captured",
        "bytes": bytes.len(),
        "sha256": source::hash(&bytes),
        "private_payload_published": false,
        "database_snapshot_still_required": true,
        "resume_authorized": false
    }))
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

    use super::*;

    struct OwnedDirectory(std::path::PathBuf);

    impl OwnedDirectory {
        fn new() -> Self {
            let path = std::path::PathBuf::from(".local/journey-runs")
                .join(&uuid::Uuid::new_v4().simple().to_string()[..16]);
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(path.join("control"))
                .unwrap();
            Self(path)
        }

        fn capsule(&self) -> std::path::PathBuf {
            self.0.join("control/private-client-checkpoint.json")
        }
    }

    impl Drop for OwnedDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn private_capsule_is_exclusive_0600_and_only_hashes_are_public() {
        let directory = OwnedDirectory::new();
        let private = json!({"password": "synthetic-not-public", "rng": "not-a-real-rng"});
        let status = write_capsule(&directory.capsule(), &private).unwrap();
        assert_eq!(status["status"], "captured");
        assert!(!status.to_string().contains("synthetic-not-public"));
        assert!(!status.to_string().contains("not-a-real-rng"));
        assert_eq!(
            fs::metadata(directory.capsule())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert!(write_capsule(&directory.capsule(), &json!({})).is_err());
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(directory.capsule()).unwrap()).unwrap(),
            private
        );
    }

    #[test]
    fn private_capsule_rejects_public_paths_and_permissive_parents() {
        for path in [
            "tools/simulator/private.json",
            ".local/evidence/private-client-checkpoint.json",
            "../private-client-checkpoint.json",
        ] {
            assert!(write_capsule(Path::new(path), &json!({})).is_err());
        }
        let directory = OwnedDirectory::new();
        fs::set_permissions(
            directory.0.join("control"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        assert!(write_capsule(&directory.capsule(), &json!({})).is_err());
        assert!(!directory.capsule().exists());
    }

    #[test]
    fn private_capsule_refuses_symlinks_and_oversized_payloads() {
        let directory = OwnedDirectory::new();
        let outside = directory.0.join("unchanged");
        fs::write(&outside, "original").unwrap();
        std::os::unix::fs::symlink(&outside, directory.capsule()).unwrap();
        assert!(write_capsule(&directory.capsule(), &json!({})).is_err());
        assert_eq!(fs::read_to_string(outside).unwrap(), "original");
        fs::remove_file(directory.capsule()).unwrap();
        assert!(
            write_capsule(
                &directory.capsule(),
                &json!({"payload": "x".repeat(MAX_CAPSULE_BYTES)})
            )
            .is_err()
        );
        assert!(!directory.capsule().exists());
    }

    #[test]
    fn private_attempt_preserves_wire_identity_and_never_authorizes_unknown_retry() {
        let input = game::WorldInput {
            world_session_id: "original-session".into(),
            sequence: 245,
            expected_character_revision: Some(1234),
            action: Some(game::world_input::Action::ShopBuy(game::ShopBuy {
                shop: "shop.lumbridge.general_store".into(),
                item_index: 2,
                quantity: 1,
                expected_item: Some("item.bucket".into()),
            })),
        };
        let mut attempt = Attempt::new("original-operation".into(), input.clone());
        let value = attempt.value();
        let bytes: Vec<u8> = serde_json::from_value(value["world_input_protobuf"].clone()).unwrap();
        assert_eq!(game::WorldInput::decode(bytes.as_slice()).unwrap(), input);
        assert_eq!(value["observed_response"]["kind"], "unresolved");
        attempt.rejected(500, 9, "server-error".into());
        assert_eq!(attempt.value()["retry_authorized"], false);
        attempt.acknowledged(false, Some(246));
        assert_eq!(
            attempt.value()["receipt_state_reconciliation_required"],
            true
        );
        assert_eq!(attempt.value()["operation_id"], "original-operation");
        assert_eq!(attempt.value()["sequence"], 245);
    }

    #[test]
    fn control_capsule_preserves_lifecycle_uuid_and_original_private_authorization() {
        let operation = uuid::Uuid::new_v4().to_string();
        let mut attempt = ControlAttempt::new(
            operation.clone(),
            Command::Logout(clubscape_protocol::Logout {}),
            Some("synthetic-original-token".into()),
        );
        attempt.received(503, Some((9, "source-error".into())));
        let value = attempt.value();
        let bytes: Vec<u8> =
            serde_json::from_value(value["client_message_protobuf"].clone()).unwrap();
        let decoded = ClientMessage::decode(bytes.as_slice()).unwrap();
        assert_eq!(decoded.request_id, operation);
        assert!(matches!(decoded.command, Some(Command::Logout(_))));
        assert_eq!(value["private_bearer_token"], "synthetic-original-token");
        assert_eq!(value["retry_authorized"], false);
        assert_eq!(value["receipt_absence_is_not_rollback_proof"], true);
    }
}
