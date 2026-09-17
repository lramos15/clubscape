use anyhow::{Context, Result, bail, ensure};
use clap::{Parser, Subcommand};
use clubscape_protocol::{
    Account, ClientMessage, CurrentAccount, ErrorCode, Hello, Login, Logout,
    MAX_GAME_RESPONSE_BYTES, MAX_REQUEST_BYTES, MEDIA_TYPE, PROTOCOL_VERSION, Register,
    ServerMessage, client_message::Command, server_message::Result as Outcome,
};
use prost::Message;
use reqwest::{Client, StatusCode, Url};
use std::{net::IpAddr, time::Duration};
use uuid::Uuid;

mod journey;

#[derive(Parser)]
#[command(about = "Protocol-level ClubScape checks; no game progress is fabricated")]
struct Arguments {
    #[command(subcommand)]
    command: Scenario,
}

#[derive(Subcommand)]
enum Scenario {
    AccountLifecycle {
        #[arg(long, default_value = "http://127.0.0.1:4010")]
        url: String,
    },
    /// Execute a source-backed named journey, never a fixture or seeded character.
    Scenario(Box<journey::Arguments>),
}

struct Connection {
    client: Client,
    endpoint: Url,
}

impl Connection {
    fn new(base: &str) -> Result<Self> {
        let mut endpoint = Url::parse(base).context("Invalid server URL")?;
        let host = endpoint.host_str().context("Server URL requires a host")?;
        let loopback = host == "localhost"
            || host
                .trim_matches(['[', ']'])
                .parse::<IpAddr>()
                .is_ok_and(|ip| ip.is_loopback());
        ensure!(
            loopback
                && matches!(endpoint.scheme(), "http" | "https")
                && endpoint.username().is_empty()
                && endpoint.password().is_none()
                && endpoint.query().is_none()
                && endpoint.fragment().is_none()
                && endpoint.path() == "/",
            "Synthetic checks require a loopback server origin without credentials or a path"
        );
        endpoint.set_path("/v1/rpc");
        Ok(Self {
            client: Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(20))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            endpoint,
        })
    }

    async fn request(
        &self,
        command: Command,
        token: Option<&str>,
    ) -> Result<(StatusCode, Outcome)> {
        let request_id = Uuid::new_v4().to_string();
        self.request_with_id(command, token, &request_id).await
    }

    async fn request_with_id(
        &self,
        command: Command,
        token: Option<&str>,
        request_id: &str,
    ) -> Result<(StatusCode, Outcome)> {
        let message = ClientMessage {
            protocol_version: PROTOCOL_VERSION,
            request_id: request_id.to_owned(),
            command: Some(command),
        };
        clubscape_protocol::validate_client_message(&message)?;
        let bytes = message.encode_to_vec();
        ensure!(
            bytes.len() <= MAX_REQUEST_BYTES,
            "Request exceeds protocol budget"
        );
        let mut request = self
            .client
            .post(self.endpoint.clone())
            .header(reqwest::header::CONTENT_TYPE, MEDIA_TYPE)
            .body(bytes);
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        let mut response = request.send().await.context(
            "RPC transport failed; a submitted write may have committed. No new operation was retried",
        )?;
        let status = response.status();
        ensure!(
            response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                == Some(MEDIA_TYPE),
            "Server did not return the Protobuf media type"
        );
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            ensure!(
                bytes.len() + chunk.len() <= MAX_GAME_RESPONSE_BYTES,
                "Server response exceeds the protocol budget"
            );
            bytes.extend_from_slice(&chunk);
        }
        let response = ServerMessage::decode(bytes.as_slice()).context("Invalid server message")?;
        ensure!(
            response.protocol_version == PROTOCOL_VERSION && response.request_id == request_id,
            "Response version or correlation ID mismatch"
        );
        Ok((
            status,
            response.result.context("Server response has no result")?,
        ))
    }
}

fn expect_error(
    response: (StatusCode, Outcome),
    status: StatusCode,
    code: ErrorCode,
) -> Result<()> {
    ensure!(response.0 == status, "Unexpected error HTTP status");
    match response.1 {
        Outcome::Error(error) => {
            ensure!(error.code == code as i32, "Unexpected protocol error");
            ensure!(
                Uuid::parse_str(&error.error_id).is_ok(),
                "Error correlation ID is missing"
            );
            Ok(())
        }
        _ => bail!("Expected an explicit protocol error"),
    }
}

async fn login(
    connection: &Connection,
    name: &str,
    password: &str,
) -> Result<clubscape_protocol::LoggedIn> {
    let (status, result) = connection
        .request(
            Command::Login(Login {
                login_name: name.to_owned(),
                password: password.to_owned(),
            }),
            None,
        )
        .await?;
    ensure!(status.is_success(), "Login failed: HTTP {status}");
    match result {
        Outcome::LoggedIn(logged_in) => {
            ensure!(
                logged_in.session_token.len() == 43 && logged_in.expires_at_unix_ms > 0,
                "Server returned an invalid session"
            );
            Ok(logged_in)
        }
        _ => bail!("Login did not return a session"),
    }
}

async fn account_lifecycle(url: &str) -> Result<()> {
    let connection = Connection::new(url)?;
    let (status, hello) = connection.request(Command::Hello(Hello {}), None).await?;
    ensure!(status.is_success(), "Hello failed: HTTP {status}");
    match hello {
        Outcome::Hello(hello) => {
            ensure!(
                !hello.gameplay_available && !hello.gameplay_unavailable_reason.is_empty(),
                "This infrastructure check must not certify unimplemented gameplay"
            );
            ensure!(
                hello.capabilities.contains(&"accounts.v1".to_owned())
                    && hello.capabilities.contains(&"sessions.v1".to_owned()),
                "Required account capabilities are unavailable"
            );
        }
        _ => bail!("Hello did not return capability information"),
    }
    let name = format!("sim_{}", &Uuid::new_v4().simple().to_string()[..12]);
    let password = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let register = Register {
        login_name: name.clone(),
        password: password.clone(),
    };
    let (status, result) = connection
        .request(Command::Register(register.clone()), None)
        .await?;
    ensure!(status.is_success(), "Registration failed: HTTP {status}");
    let account: Account = match result {
        Outcome::Registered(registered) => registered.account.context("Account is missing")?,
        _ => bail!("Registration did not create an account"),
    };
    expect_error(
        connection
            .request(
                Command::Register(Register {
                    login_name: name.to_ascii_uppercase(),
                    ..register
                }),
                None,
            )
            .await?,
        StatusCode::CONFLICT,
        ErrorCode::Conflict,
    )?;
    let session = login(&connection, &name.to_ascii_uppercase(), &password).await?;
    ensure!(
        session.account.as_ref() == Some(&account),
        "Login did not recover the registered identity"
    );
    expect_error(
        connection
            .request(
                Command::CreateCharacter(clubscape_protocol::game::CreateCharacter {
                    appearance: Default::default(),
                    experience_choice: "new_to_runescape".to_owned(),
                }),
                Some(&session.session_token),
            )
            .await?,
        StatusCode::SERVICE_UNAVAILABLE,
        ErrorCode::Unavailable,
    )?;
    expect_error(
        connection
            .request(
                Command::JoinWorld(clubscape_protocol::game::JoinWorld {}),
                None,
            )
            .await?,
        StatusCode::UNAUTHORIZED,
        ErrorCode::Unauthenticated,
    )?;
    let (status, snapshot) = connection
        .request(
            Command::CurrentAccount(CurrentAccount {}),
            Some(&session.session_token),
        )
        .await?;
    ensure!(status.is_success(), "Account lookup failed: HTTP {status}");
    match snapshot {
        Outcome::Account(snapshot) => {
            ensure!(
                snapshot.account.as_ref() == Some(&account)
                    && !snapshot.character_initialized
                    && !snapshot.gameplay_unavailable_reason.is_empty(),
                "Account lookup changed identity or fabricated character progress"
            );
        }
        _ => bail!("Account lookup returned the wrong result"),
    }
    let (status, logout) = connection
        .request(Command::Logout(Logout {}), Some(&session.session_token))
        .await?;
    ensure!(
        status.is_success() && matches!(logout, Outcome::LoggedOut(_)),
        "Logout failed"
    );
    expect_error(
        connection
            .request(
                Command::CurrentAccount(CurrentAccount {}),
                Some(&session.session_token),
            )
            .await?,
        StatusCode::UNAUTHORIZED,
        ErrorCode::Unauthenticated,
    )?;
    let new_session = login(&connection, &name, &password).await?;
    ensure!(
        new_session.account.as_ref() == Some(&account)
            && new_session.session_token != session.session_token,
        "Relogin failed to preserve identity or rotate the token"
    );
    let (status, result) = connection
        .request(Command::Logout(Logout {}), Some(&new_session.session_token))
        .await?;
    ensure!(
        status.is_success() && matches!(result, Outcome::LoggedOut(_)),
        "Cleanup logout failed"
    );
    println!(
        "{}",
        serde_json::json!({
            "schema_version": 1,
            "check": "account_lifecycle",
            "result": "passed",
            "account_id": account.account_id,
            "checks": [
                "capability_negotiation", "registration", "case_insensitive_uniqueness",
                "login", "persistent_identity", "no_fabricated_character",
                "unavailable_game_commands", "game_authentication_required",
                "logout_revocation", "relogin", "token_rotation"
            ],
            "gameplay_verified": false,
            "browser_signup_verified": false,
            "milestone_accepted": false
        })
    );
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    match Arguments::parse().command {
        Scenario::AccountLifecycle { url } => account_lifecycle(&url).await,
        Scenario::Scenario(arguments) => journey::run(*arguments).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_accounts_are_restricted_to_local_origins() {
        for url in [
            "http://127.0.0.1:4010",
            "http://localhost:4010",
            "http://[::1]:4010",
        ] {
            assert!(Connection::new(url).is_ok(), "{url}");
        }
        for url in [
            "https://example.com",
            "http://127.0.0.1.evil.example",
            "http://user:secret@localhost",
            "http://localhost/path",
            "http://localhost/?token=secret",
            "ftp://localhost",
        ] {
            assert!(Connection::new(url).is_err(), "{url}");
        }
    }
}
