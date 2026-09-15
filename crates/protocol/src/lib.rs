mod generated {
    include!(concat!(env!("OUT_DIR"), "/clubscape.rs"));
}

mod game_input;
mod ui_input;
pub use ui_input::{ui_bank_revision, ui_request};

pub use game_input::{
    ReadOnlyQuote, game_intent, quote_request, validate_character_options, validate_world_session,
};
pub use generated::clubscape::account::v1::*;
pub use generated::clubscape::game::v1 as game;

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_REQUEST_BYTES: usize = 16 * 1024;
pub const MAX_GAME_RESPONSE_BYTES: usize = 256 * 1024;
pub const GAME_CAPABILITY: &str = "game.v1";
pub const GAMEPLAY_UI_CAPABILITY: &str = "game.ui.v1";
pub const ACTOR_OBSERVER_CAPABILITY: &str = "game.observer.v1";
pub const MEDIA_TYPE: &str = "application/x-protobuf";
pub const CAPABILITIES: &[&str] = &["accounts.v1", "sessions.v1"];
pub const GAMEPLAY_UNAVAILABLE_REASON: &str =
    "The starter journey is not implemented. No character has been initialized.";
pub const FILE_DESCRIPTOR_SET: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/account-descriptor.bin"));

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct ValidationError {
    pub code: ErrorCode,
    pub message: &'static str,
}

fn invalid(message: &'static str) -> ValidationError {
    ValidationError {
        code: ErrorCode::InvalidArgument,
        message,
    }
}

pub fn normalize_login_name(value: &str) -> Result<String, ValidationError> {
    if !(3..=20).contains(&value.len())
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(invalid(
            "Login names must contain 3-20 ASCII letters, digits or underscores.",
        ));
    }
    Ok(value.to_ascii_lowercase())
}

pub fn validate_registration_password(value: &str) -> Result<(), ValidationError> {
    if !(15..=128).contains(&value.len()) {
        return Err(invalid("Passwords must contain 15-128 UTF-8 bytes."));
    }
    Ok(())
}

pub fn validate_client_message(message: &ClientMessage) -> Result<(), ValidationError> {
    if message.protocol_version != PROTOCOL_VERSION {
        return Err(ValidationError {
            code: ErrorCode::UnsupportedVersion,
            message: "Only protocol version 1 is supported.",
        });
    }
    let id = uuid::Uuid::parse_str(&message.request_id)
        .map_err(|_| invalid("A non-nil hyphenated UUID request ID is required."))?;
    if id.is_nil() || message.request_id.len() != 36 {
        return Err(invalid("A non-nil hyphenated UUID request ID is required."));
    }
    match message.command.as_ref() {
        Some(client_message::Command::Register(register)) => {
            normalize_login_name(&register.login_name)?;
            validate_registration_password(&register.password)?;
        }
        Some(client_message::Command::Login(login)) => {
            normalize_login_name(&login.login_name)?;
            if login.password.is_empty() || login.password.len() > 128 {
                return Err(invalid("Login passwords must contain 1-128 UTF-8 bytes."));
            }
        }
        Some(client_message::Command::CreateCharacter(options)) => {
            validate_character_options(options)?;
        }
        Some(client_message::Command::PollWorld(poll)) => {
            validate_world_session(&poll.world_session_id)?;
            if let Some(quote) = &poll.quote {
                quote_request(quote)?;
            }
        }
        Some(client_message::Command::WorldInput(input)) => {
            game_intent(input)?;
        }
        Some(client_message::Command::LeaveWorld(leave)) => {
            validate_world_session(&leave.world_session_id)?;
        }
        Some(_) => {}
        None => return Err(invalid("A supported command is required.")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;

    fn hello() -> ClientMessage {
        ClientMessage {
            protocol_version: PROTOCOL_VERSION,
            request_id: "00000000-0000-4000-8000-000000000001".to_owned(),
            command: Some(client_message::Command::Hello(Hello {})),
        }
    }

    #[test]
    fn version_one_wire_tags_are_stable() {
        let expected = [
            &[0x08, 0x01, 0x12, 0x24][..],
            b"00000000-0000-4000-8000-000000000001",
            &[0x52, 0x00],
        ]
        .concat();
        assert_eq!(hello().encode_to_vec(), expected);
        assert_eq!(ClientMessage::decode(expected.as_slice()).unwrap(), hello());
        assert!(!FILE_DESCRIPTOR_SET.is_empty());
    }

    #[test]
    fn unknown_fields_are_forward_decodable_but_unknown_commands_are_rejected() {
        let mut bytes = hello().encode_to_vec();
        bytes.extend_from_slice(&[0xf8, 0x07, 0x01]);
        assert!(validate_client_message(&ClientMessage::decode(bytes.as_slice()).unwrap()).is_ok());
        let message = ClientMessage {
            command: None,
            ..hello()
        };
        assert_eq!(
            validate_client_message(&message).unwrap_err().code,
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn unsupported_versions_and_invalid_ids_fail() {
        for version in [0, 2, u32::MAX] {
            let mut message = hello();
            message.protocol_version = version;
            assert_eq!(
                validate_client_message(&message).unwrap_err().code,
                ErrorCode::UnsupportedVersion
            );
        }
        for id in ["", "not-a-uuid", "00000000-0000-0000-0000-000000000000"] {
            let mut message = hello();
            message.request_id = id.to_owned();
            assert!(validate_client_message(&message).is_err());
        }
    }

    #[test]
    fn account_names_normalize_without_ambiguous_trimming_or_unicode() {
        assert_eq!(normalize_login_name("Penguin_42").unwrap(), "penguin_42");
        for name in ["ab", " penguin", "penguin ", "a-b", "p\u{e9}nguin", "a\nb"] {
            assert!(normalize_login_name(name).is_err(), "{name:?}");
        }
        assert!(normalize_login_name(&"a".repeat(20)).is_ok());
        assert!(normalize_login_name(&"a".repeat(21)).is_err());
    }

    #[test]
    fn passwords_are_bounded_by_utf8_bytes_and_not_trimmed() {
        assert!(validate_registration_password(&"a".repeat(14)).is_err());
        assert!(validate_registration_password(&"a".repeat(15)).is_ok());
        assert!(validate_registration_password(&"a".repeat(128)).is_ok());
        assert!(validate_registration_password(&"a".repeat(129)).is_err());
        assert!(validate_registration_password(&"\u{e9}".repeat(64)).is_ok());
        assert!(validate_registration_password(&"\u{e9}".repeat(65)).is_err());
        assert!(validate_registration_password("  real spaces stay  ").is_ok());
    }

    #[test]
    fn commands_apply_the_correct_credential_policy() {
        let register = ClientMessage {
            command: Some(client_message::Command::Register(Register {
                login_name: "penguin".to_owned(),
                password: "short".to_owned(),
            })),
            ..hello()
        };
        assert!(validate_client_message(&register).is_err());
        let login = ClientMessage {
            command: Some(client_message::Command::Login(Login {
                login_name: "penguin".to_owned(),
                password: "short".to_owned(),
            })),
            ..hello()
        };
        assert!(validate_client_message(&login).is_ok());
    }

    #[test]
    fn malformed_wire_input_is_not_a_valid_request() {
        assert!(ClientMessage::decode(&[0x12, 0xff][..]).is_err());
        assert!(validate_client_message(&ClientMessage::default()).is_err());
    }
}
