use std::{
    env,
    io::{BufRead, BufReader},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    process::{Child, Command, Stdio},
    str::FromStr,
    sync::mpsc,
    thread,
    time::Duration,
};

use argon2::{Argon2, PasswordHash, PasswordVerifier};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use clubscape_protocol::{
    Account, CAPABILITIES, ClientMessage, CurrentAccount, ErrorCode, GAMEPLAY_UNAVAILABLE_REASON,
    Hello, LoggedIn, Login, Logout, MAX_REQUEST_BYTES, MEDIA_TYPE, PROTOCOL_VERSION, Register,
    ServerMessage, client_message, server_message,
};
use clubscape_server::{Config, ServeError, Service, UNVERSIONED_BUILD};
use prost::Message;
use reqwest::{
    Client, StatusCode,
    header::{self, HeaderMap, HeaderValue},
};
use sha2::{Digest, Sha256};
use sqlx::{ConnectOptions, PgPool, postgres::PgConnectOptions, postgres::PgPoolOptions};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{Mutex, MutexGuard, oneshot},
    task::{JoinHandle, JoinSet},
    time::{sleep, timeout},
};
use uuid::Uuid;

const TEST_DATABASE: &str = "clubscape_m1_test";
const PROCESS_REVISION: &str = "integration-process-revision";
const WAIT: Duration = Duration::from_secs(15);
static DATABASE_LOCK: Mutex<()> = Mutex::const_new(());

struct TestDatabase {
    url: String,
    pool: PgPool,
    address: SocketAddr,
    _guard: MutexGuard<'static, ()>,
}

impl TestDatabase {
    async fn reset() -> Self {
        let guard = DATABASE_LOCK.lock().await;
        let url = env::var("CLUBSCAPE_TEST_DATABASE_URL").unwrap_or_else(|_| {
            panic!("CLUBSCAPE_TEST_DATABASE_URL is required; use the isolated integration runner")
        });
        let options = PgConnectOptions::from_str(&url)
            .unwrap_or_else(|_| panic!("invalid isolated PostgreSQL test configuration"));
        assert_eq!(
            options.get_database(),
            Some(TEST_DATABASE),
            "refusing to touch a database not named clubscape_m1_test"
        );
        let ip = options.get_host().parse::<IpAddr>().unwrap_or_else(|_| {
            panic!("the isolated test database must use a literal loopback IP")
        });
        assert!(ip.is_loopback(), "refusing a non-loopback test database");
        let address = SocketAddr::new(ip, options.get_port());
        let pool = timeout(
            WAIT,
            PgPoolOptions::new()
                .max_connections(5)
                .acquire_timeout(Duration::from_secs(5))
                .after_connect(|connection, _| {
                    Box::pin(async move {
                        sqlx::query("SET statement_timeout = '5s'")
                            .execute(connection)
                            .await?;
                        Ok(())
                    })
                })
                .connect_with(
                    options
                        .disable_statement_logging()
                        .application_name("clubscape-m1-tests"),
                ),
        )
        .await
        .unwrap_or_else(|_| panic!("isolated PostgreSQL connection timed out"))
        .unwrap_or_else(|_| panic!("isolated PostgreSQL connection failed"));
        let (database, schema): (String, String) =
            sqlx::query_as("SELECT current_database(), current_schema()")
                .fetch_one(&pool)
                .await
                .expect("check test database identity");
        assert_eq!(database, TEST_DATABASE, "unexpected connected database");
        assert_eq!(schema, "public", "unexpected test schema");
        sqlx::raw_sql(
            "DROP TABLE IF EXISTS public.account_sessions;
             DROP TABLE IF EXISTS public.accounts;
             DROP TABLE IF EXISTS public._sqlx_migrations;",
        )
        .execute(&pool)
        .await
        .expect("reset only the isolated account service tables");
        Self {
            url,
            pool,
            address,
            _guard: guard,
        }
    }

    async fn account_count(&self) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM accounts")
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn session_count(&self) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM account_sessions")
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn expire(&self, token: &str) {
        let changed = sqlx::query(
            "UPDATE account_sessions
             SET created_at = clock_timestamp() - INTERVAL '31 minutes',
                 expires_at = clock_timestamp() - INTERVAL '1 minute'
             WHERE token_digest = $1",
        )
        .bind(digest(token))
        .execute(&self.pool)
        .await
        .unwrap();
        assert_eq!(changed.rows_affected(), 1);
    }

    async fn has_session(&self, token: &str) -> bool {
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM account_sessions WHERE token_digest = $1)")
            .bind(digest(token))
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }
}

#[derive(Clone)]
struct Endpoint {
    client: Client,
    base_url: String,
}

impl Endpoint {
    fn new(address: SocketAddr) -> Self {
        assert!(address.ip().is_loopback());
        Self {
            client: Client::builder().no_proxy().timeout(WAIT).build().unwrap(),
            base_url: format!("http://{address}"),
        }
    }

    fn request(&self) -> reqwest::RequestBuilder {
        self.client.post(format!("{}/v1/rpc", self.base_url))
    }

    async fn call(&self, command: client_message::Command, token: Option<&str>) -> Reply {
        self.send_message(&message(command), token).await
    }

    async fn send_message(&self, message: &ClientMessage, token: Option<&str>) -> Reply {
        let mut request = self
            .request()
            .header(header::CONTENT_TYPE, MEDIA_TYPE)
            .body(message.encode_to_vec());
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        let reply = read_reply(request.send().await.unwrap()).await;
        let valid_id = message.request_id.len() == 36
            && Uuid::parse_str(&message.request_id).is_ok_and(|id| !id.is_nil());
        assert_eq!(
            reply.message.request_id,
            if valid_id { &message.request_id } else { "" }
        );
        reply
    }

    async fn register(&self, name: &str, password: &str) -> Reply {
        self.call(register(name, password), None).await
    }

    async fn login(&self, name: &str, password: &str) -> Reply {
        self.call(login(name, password), None).await
    }

    async fn snapshot(&self, token: &str) -> Reply {
        self.call(current(), Some(token)).await
    }

    async fn logout(&self, token: &str) -> Reply {
        self.call(client_message::Command::Logout(Logout {}), Some(token))
            .await
    }

    async fn health(&self) -> reqwest::Response {
        self.client
            .get(format!("{}/healthz", self.base_url))
            .send()
            .await
            .unwrap()
    }
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    message: ServerMessage,
}

impl Reply {
    fn error(&self, status: StatusCode, code: ErrorCode) -> &clubscape_protocol::Error {
        assert_eq!(self.status, status);
        let Some(server_message::Result::Error(error)) = &self.message.result else {
            panic!("failure must have an explicit protocol error, never a success result");
        };
        assert_eq!(error.code(), code);
        assert!(!error.message.is_empty());
        assert!(!Uuid::parse_str(&error.error_id).unwrap().is_nil());
        if status == StatusCode::UNAUTHORIZED {
            assert_eq!(self.headers[header::WWW_AUTHENTICATE], "Bearer");
        }
        error
    }

    fn registered(self) -> Account {
        assert_eq!(self.status, StatusCode::OK);
        match self.message.result {
            Some(server_message::Result::Registered(registered)) => {
                let account = registered
                    .account
                    .expect("registration must include an identity");
                assert!(!Uuid::parse_str(&account.account_id).unwrap().is_nil());
                account
            }
            _ => panic!("expected registration, not another success-shaped result"),
        }
    }

    fn logged_in(self) -> LoggedIn {
        assert_eq!(self.status, StatusCode::OK);
        match self.message.result {
            Some(server_message::Result::LoggedIn(login)) => {
                assert!(login.account.is_some());
                assert_eq!(login.session_token.len(), 43);
                let decoded = URL_SAFE_NO_PAD.decode(&login.session_token).unwrap();
                assert_eq!(decoded.len(), 32);
                assert_eq!(URL_SAFE_NO_PAD.encode(decoded), login.session_token);
                assert!(login.expires_at_unix_ms > 0);
                login
            }
            _ => panic!("expected a real login result"),
        }
    }

    fn account(self) -> Account {
        assert_eq!(self.status, StatusCode::OK);
        match self.message.result {
            Some(server_message::Result::Account(snapshot)) => {
                assert!(!snapshot.character_initialized);
                assert_eq!(
                    snapshot.gameplay_unavailable_reason,
                    GAMEPLAY_UNAVAILABLE_REASON
                );
                snapshot.account.expect("snapshot must include an identity")
            }
            _ => panic!("expected an account snapshot without fabricated character state"),
        }
    }

    fn logged_out(self) {
        assert_eq!(self.status, StatusCode::OK);
        assert!(matches!(
            self.message.result,
            Some(server_message::Result::LoggedOut(_))
        ));
    }

    fn hello(self, revision: &str) {
        assert_eq!(self.status, StatusCode::OK);
        match self.message.result {
            Some(server_message::Result::Hello(hello)) => {
                assert_eq!(hello.capabilities, CAPABILITIES);
                assert_eq!(hello.max_request_bytes, MAX_REQUEST_BYTES as u32);
                assert!(!hello.gameplay_available);
                assert_eq!(
                    hello.gameplay_unavailable_reason,
                    GAMEPLAY_UNAVAILABLE_REASON
                );
                assert_eq!(hello.build_revision, revision);
            }
            _ => panic!("expected the frozen account-only hello"),
        }
    }
}

async fn read_reply(response: reqwest::Response) -> Reply {
    let status = response.status();
    let headers = response.headers().clone();
    assert_eq!(headers[header::CONTENT_TYPE], MEDIA_TYPE);
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert!(!headers.contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));
    let bytes = response.bytes().await.unwrap();
    assert!(bytes.len() <= MAX_REQUEST_BYTES);
    let message = ServerMessage::decode(bytes).expect("structured Protobuf response");
    assert_eq!(message.protocol_version, PROTOCOL_VERSION);
    assert!(message.result.is_some());
    Reply {
        status,
        headers,
        message,
    }
}

fn message(command: client_message::Command) -> ClientMessage {
    ClientMessage {
        protocol_version: PROTOCOL_VERSION,
        request_id: Uuid::new_v4().to_string(),
        command: Some(command),
    }
}

fn register(name: &str, password: &str) -> client_message::Command {
    client_message::Command::Register(Register {
        login_name: name.to_owned(),
        password: password.to_owned(),
    })
}

fn login(name: &str, password: &str) -> client_message::Command {
    client_message::Command::Login(Login {
        login_name: name.to_owned(),
        password: password.to_owned(),
    })
}

fn current() -> client_message::Command {
    client_message::Command::CurrentAccount(CurrentAccount {})
}

fn password() -> String {
    format!("  Pénguin-{}!  ", Uuid::new_v4())
}

fn digest(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

struct TestService {
    endpoint: Endpoint,
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), ServeError>>>,
}

impl TestService {
    async fn start(url: &str) -> Self {
        let config = Config::new(url, "127.0.0.1:0", None).unwrap();
        let service = timeout(WAIT, Service::bind(config))
            .await
            .expect("service startup must be bounded")
            .expect("isolated account service startup");
        let endpoint = Endpoint::new(service.local_addr());
        let (shutdown, receive) = oneshot::channel();
        let task = tokio::spawn(service.serve(async move {
            let _ = receive.await;
        }));
        assert_eq!(endpoint.health().await.status(), StatusCode::OK);
        Self {
            endpoint,
            shutdown: Some(shutdown),
            task: Some(task),
        }
    }

    async fn stop(mut self) {
        self.shutdown.take().unwrap().send(()).unwrap();
        timeout(WAIT, self.task.as_mut().unwrap())
            .await
            .expect("bounded graceful service shutdown")
            .expect("service task")
            .expect("service shutdown");
        self.task.take();
    }
}

impl Drop for TestService {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

struct ChildCapture {
    child: Option<Child>,
    reader: Option<thread::JoinHandle<String>>,
}

impl ChildCapture {
    fn spawn(database_url: Option<&str>, bind: &str) -> (Self, mpsc::Receiver<SocketAddr>) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_clubscape-server"));
        command
            .env_clear()
            .env("CLUBSCAPE_BIND", bind)
            .env("CLUBSCAPE_BUILD_REVISION", PROCESS_REVISION)
            .env("RUST_LOG", "trace")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if let Some(url) = database_url {
            command.env("DATABASE_URL", url);
        }
        let mut child = command.spawn().expect("launch the actual server binary");
        let stdout = child.stdout.take().unwrap();
        let (sender, receive) = mpsc::sync_channel(1);
        let reader = thread::spawn(move || {
            let mut captured = String::new();
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&line)
                    && value["fields"]["event"] == "listening"
                    && let Some(address) = value["fields"]["address"].as_str()
                    && let Ok(address) = address.parse::<SocketAddr>()
                {
                    let _ = sender.try_send(address);
                }
                if captured.len() + line.len() < 128 * 1024 {
                    captured.push_str(&line);
                    captured.push('\n');
                }
            }
            captured
        });
        (
            Self {
                child: Some(child),
                reader: Some(reader),
            },
            receive,
        )
    }

    fn start(database_url: &str) -> (Self, Endpoint) {
        let (process, receive) = Self::spawn(Some(database_url), "127.0.0.1:0");
        let address = receive
            .recv_timeout(WAIT)
            .expect("the real process must report its loopback listener");
        (process, Endpoint::new(address))
    }

    fn signal(&self, signal: &str) {
        let status = Command::new("kill")
            .arg(signal)
            .arg(self.child.as_ref().unwrap().id().to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("send a signal to this test's exact child PID");
        assert!(status.success());
    }

    async fn finish(mut self, expected_success: bool) -> String {
        let mut exit = None;
        for _ in 0..300 {
            exit = self.child.as_mut().unwrap().try_wait().unwrap();
            if exit.is_some() {
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }
        let status = exit.expect("the server process must exit within fifteen seconds");
        self.child.take();
        let logs = self.reader.take().unwrap().join().unwrap();
        assert_eq!(status.success(), expected_success);
        for line in logs.lines() {
            let value: serde_json::Value =
                serde_json::from_str(line).expect("server output must be structured JSON");
            assert_eq!(value["target"], "clubscape_server");
        }
        logs
    }
}

impl Drop for ChildCapture {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

struct DatabaseProxy {
    address: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}

impl DatabaseProxy {
    async fn start(target: SocketAddr) -> Self {
        assert!(target.ip().is_loopback());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (shutdown, mut receive) = oneshot::channel();
        let task = tokio::spawn(async move {
            let mut connections = JoinSet::new();
            loop {
                tokio::select! {
                    _ = &mut receive => break,
                    accepted = listener.accept() => {
                        let Ok((mut downstream, _)) = accepted else { break };
                        if connections.len() >= 16 {
                            continue;
                        }
                        connections.spawn(async move {
                            if let Ok(mut upstream) = TcpStream::connect(target).await {
                                let _ = tokio::io::copy_bidirectional(
                                    &mut downstream,
                                    &mut upstream,
                                ).await;
                            }
                        });
                    },
                    _ = connections.join_next(), if !connections.is_empty() => {},
                }
            }
            connections.abort_all();
            while connections.join_next().await.is_some() {}
        });
        Self {
            address,
            shutdown: Some(shutdown),
            task: Some(task),
        }
    }

    async fn stop(mut self) {
        self.shutdown.take().unwrap().send(()).unwrap();
        timeout(WAIT, self.task.as_mut().unwrap())
            .await
            .unwrap()
            .unwrap();
        self.task.take();
    }
}

impl Drop for DatabaseProxy {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

fn replace_port(url: &str, port: u16) -> String {
    let mut url = reqwest::Url::parse(url).unwrap();
    url.set_host(Some("127.0.0.1")).unwrap();
    url.set_port(Some(port)).unwrap();
    url.to_string()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn accounts_and_sessions_survive_actual_process_restart() {
    let database = TestDatabase::reset().await;
    let (process, endpoint) = ChildCapture::start(&database.url);
    assert_eq!(endpoint.health().await.status(), StatusCode::OK);
    endpoint
        .call(client_message::Command::Hello(Hello {}), None)
        .await
        .hello(PROCESS_REVISION);

    let secret = password();
    let account = endpoint.register("Keeper_One", &secret).await.registered();
    assert_eq!(account.login_name, "keeper_one");
    assert_eq!(database.account_count().await, 1);
    assert_eq!(database.session_count().await, 0, "signup must not log in");
    endpoint
        .register("KEEPER_ONE", &secret)
        .await
        .error(StatusCode::CONFLICT, ErrorCode::Conflict);
    let account_id = Uuid::parse_str(&account.account_id).unwrap();
    let stored_hash: String =
        sqlx::query_scalar("SELECT password_hash FROM accounts WHERE account_id = $1")
            .bind(account_id)
            .fetch_one(&database.pool)
            .await
            .unwrap();
    assert_ne!(stored_hash, secret);
    assert!(!stored_hash.contains(&secret));
    let hash = PasswordHash::new(&stored_hash).unwrap();
    assert_eq!(hash.algorithm.as_str(), "argon2id");
    assert_eq!(hash.params.get_decimal("m"), Some(19_456));
    assert_eq!(hash.params.get_decimal("t"), Some(2));
    assert_eq!(hash.params.get_decimal("p"), Some(1));
    Argon2::default()
        .verify_password(secret.as_bytes(), &hash)
        .unwrap();

    let wrong = endpoint.login("keeper_one", "incorrect password").await;
    let unknown = endpoint.login("unknown_keeper", "incorrect password").await;
    let wrong_error = wrong.error(StatusCode::UNAUTHORIZED, ErrorCode::Unauthenticated);
    let unknown_error = unknown.error(StatusCode::UNAUTHORIZED, ErrorCode::Unauthenticated);
    assert_eq!(wrong_error.code, unknown_error.code);
    assert_eq!(wrong_error.message, unknown_error.message);
    assert_eq!(
        wrong_error.retry_after_seconds,
        unknown_error.retry_after_seconds
    );
    assert_ne!(wrong_error.error_id, unknown_error.error_id);
    endpoint
        .login("keeper_one", secret.trim())
        .await
        .error(StatusCode::UNAUTHORIZED, ErrorCode::Unauthenticated);
    endpoint
        .login("keeper_one", &secret.to_uppercase())
        .await
        .error(StatusCode::UNAUTHORIZED, ErrorCode::Unauthenticated);
    assert_eq!(database.session_count().await, 0);

    let first = endpoint.login("KEEPER_ONE", &secret).await.logged_in();
    assert_eq!(first.account.as_ref(), Some(&account));
    let (stored_digest, created, expires): (Vec<u8>, i64, i64) = sqlx::query_as(
        "SELECT token_digest,
                floor(extract(epoch FROM created_at) * 1000)::bigint,
                floor(extract(epoch FROM expires_at) * 1000)::bigint
         FROM account_sessions WHERE account_id = $1",
    )
    .bind(account_id)
    .fetch_one(&database.pool)
    .await
    .unwrap();
    assert_eq!(stored_digest, digest(&first.session_token));
    assert_ne!(stored_digest, first.session_token.as_bytes());
    assert_eq!(expires - created, 30 * 60 * 1000);
    assert_eq!(expires, first.expires_at_unix_ms);
    let now: i64 =
        sqlx::query_scalar("SELECT floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint")
            .fetch_one(&database.pool)
            .await
            .unwrap();
    assert!((29 * 60 * 1000..=30 * 60 * 1000).contains(&(expires - now)));
    assert_eq!(
        endpoint.snapshot(&first.session_token).await.account(),
        account
    );

    let other_secret = password();
    let other = endpoint
        .register("another_keeper", &other_secret)
        .await
        .registered();
    let other_login = endpoint
        .login("ANOTHER_KEEPER", &other_secret)
        .await
        .logged_in();
    let mut forged_identity = message(current());
    forged_identity.request_id.clone_from(&account.account_id);
    assert_eq!(
        endpoint
            .send_message(&forged_identity, Some(&other_login.session_token))
            .await
            .account(),
        other,
        "a client-provided account UUID is correlation, not authorization"
    );

    endpoint.logout(&first.session_token).await.logged_out();
    assert!(!database.has_session(&first.session_token).await);
    endpoint
        .snapshot(&first.session_token)
        .await
        .error(StatusCode::UNAUTHORIZED, ErrorCode::Unauthenticated);
    endpoint
        .logout(&first.session_token)
        .await
        .error(StatusCode::UNAUTHORIZED, ErrorCode::Unauthenticated);
    let rotated = endpoint.login("keeper_one", &secret).await.logged_in();
    assert_ne!(rotated.session_token, first.session_token);
    assert_eq!(rotated.account.as_ref(), Some(&account));
    process.signal("-TERM");
    let first_logs = process.finish(true).await;
    assert!(first_logs.contains("shutdown_complete"));

    let (restarted, fresh_endpoint) = ChildCapture::start(&database.url);
    assert_eq!(fresh_endpoint.health().await.status(), StatusCode::OK);
    assert_eq!(
        fresh_endpoint
            .snapshot(&rotated.session_token)
            .await
            .account(),
        account
    );
    assert_eq!(
        fresh_endpoint
            .snapshot(&other_login.session_token)
            .await
            .account(),
        other
    );
    fresh_endpoint
        .snapshot(&first.session_token)
        .await
        .error(StatusCode::UNAUTHORIZED, ErrorCode::Unauthenticated);
    let resumed = fresh_endpoint
        .login("keeper_one", &secret)
        .await
        .logged_in();
    assert_eq!(resumed.account.as_ref(), Some(&account));
    assert_ne!(resumed.session_token, rotated.session_token);
    assert_eq!(database.account_count().await, 2);
    restarted.signal("-INT");
    let second_logs = restarted.finish(true).await;
    assert!(second_logs.contains("shutdown_complete"));
    for logs in [&first_logs, &second_logs] {
        for sensitive in [
            &secret,
            &other_secret,
            &database.url,
            &first.session_token,
            &other_login.session_token,
            &rotated.session_token,
            &resumed.session_token,
        ] {
            assert!(
                !logs.contains(sensitive),
                "a sensitive value reached process logs"
            );
        }
        assert!(!logs.contains("Authorization"));
        assert!(!logs.contains("Bearer "));
    }
    database.pool.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn concurrent_registration_and_session_pruning_are_atomic_and_account_scoped() {
    let database = TestDatabase::reset().await;
    let service = TestService::start(&database.url).await;
    let endpoint = &service.endpoint;
    let secret = password();
    let mut registrations = JoinSet::new();
    for name in [
        "Race_Penguin",
        "race_penguin",
        "RACE_PENGUIN",
        "rAcE_pEnGuIn",
    ] {
        let endpoint = endpoint.clone();
        let secret = secret.clone();
        registrations.spawn(async move { endpoint.register(name, &secret).await });
    }
    let mut winner = None;
    let mut conflicts = 0;
    while let Some(reply) = registrations.join_next().await {
        let reply = reply.unwrap();
        if reply.status == StatusCode::OK {
            assert!(winner.replace(reply.registered()).is_none());
        } else {
            reply.error(StatusCode::CONFLICT, ErrorCode::Conflict);
            conflicts += 1;
        }
    }
    assert_eq!(conflicts, 3);
    assert_eq!(database.account_count().await, 1);
    assert_eq!(database.session_count().await, 0);
    let account = winner.unwrap();
    let account_id = Uuid::parse_str(&account.account_id).unwrap();

    let other = endpoint
        .register("pruning_control", &secret)
        .await
        .registered();
    let other_session = endpoint.login("pruning_control", &secret).await.logged_in();
    database.expire(&other_session.session_token).await;
    let mut earlier = Vec::new();
    for _ in 0..4 {
        earlier.push(endpoint.login("race_penguin", &secret).await.logged_in());
    }
    let mut logins = JoinSet::new();
    for _ in 0..4 {
        let endpoint = endpoint.clone();
        let secret = secret.clone();
        let pool = database.pool.clone();
        logins.spawn(async move {
            let logged_in = endpoint.login("race_penguin", &secret).await.logged_in();
            let active: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM account_sessions
                 WHERE account_id = $1 AND expires_at > clock_timestamp()",
            )
            .bind(account_id)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert!(
                active <= 5,
                "an acknowledged issuance must not exceed the cap"
            );
            logged_in
        });
    }
    let mut recent = Vec::new();
    while let Some(logged_in) = logins.join_next().await {
        recent.push(logged_in.unwrap());
    }
    let retained: i64 =
        sqlx::query_scalar("SELECT count(*) FROM account_sessions WHERE account_id = $1")
            .bind(account_id)
            .fetch_one(&database.pool)
            .await
            .unwrap();
    assert_eq!(retained, 5);
    for old in earlier.iter().take(3) {
        assert!(!database.has_session(&old.session_token).await);
        endpoint
            .snapshot(&old.session_token)
            .await
            .error(StatusCode::UNAUTHORIZED, ErrorCode::Unauthenticated);
    }
    for kept in recent.iter().chain(earlier.last()) {
        assert_eq!(
            endpoint.snapshot(&kept.session_token).await.account(),
            account
        );
    }
    for expired in recent.iter().take(2) {
        database.expire(&expired.session_token).await;
        endpoint
            .snapshot(&expired.session_token)
            .await
            .error(StatusCode::UNAUTHORIZED, ErrorCode::Unauthenticated);
        endpoint
            .logout(&expired.session_token)
            .await
            .error(StatusCode::UNAUTHORIZED, ErrorCode::Unauthenticated);
    }
    let next = endpoint.login("race_penguin", &secret).await.logged_in();
    assert_eq!(
        endpoint.snapshot(&next.session_token).await.account(),
        account
    );
    for expired in recent.iter().take(2) {
        assert!(!database.has_session(&expired.session_token).await);
    }
    assert!(
        database.has_session(&other_session.session_token).await,
        "expiry cleanup must not perform a global delete"
    );
    let other_id = Uuid::parse_str(&other.account_id).unwrap();
    let other_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM account_sessions WHERE account_id = $1")
            .bind(other_id)
            .fetch_one(&database.pool)
            .await
            .unwrap();
    assert_eq!(other_count, 1);
    let normalized_names: Vec<String> = sqlx::query_scalar("SELECT login_name FROM accounts")
        .fetch_all(&database.pool)
        .await
        .unwrap();
    assert!(
        normalized_names
            .iter()
            .all(|name| *name == name.to_ascii_lowercase())
    );
    service.stop().await;
    database.pool.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn invalid_protocol_media_bodies_and_authentication_cannot_mutate_state() {
    let database = TestDatabase::reset().await;
    let service = TestService::start(&database.url).await;
    let endpoint = &service.endpoint;
    endpoint
        .call(client_message::Command::Hello(Hello {}), None)
        .await
        .hello(UNVERSIONED_BUILD);
    let secret = password();
    for version in [0, 2, u32::MAX] {
        let mut request = message(register("must_not_exist", &secret));
        request.protocol_version = version;
        endpoint
            .send_message(&request, None)
            .await
            .error(StatusCode::UPGRADE_REQUIRED, ErrorCode::UnsupportedVersion);
    }
    let mut absent_command = message(current());
    absent_command.command = None;
    endpoint
        .send_message(&absent_command, None)
        .await
        .error(StatusCode::BAD_REQUEST, ErrorCode::InvalidArgument);
    for id in ["", "not-an-id", "00000000-0000-0000-0000-000000000000"] {
        let mut request = message(register("must_not_exist", &secret));
        request.request_id = id.to_owned();
        endpoint
            .send_message(&request, None)
            .await
            .error(StatusCode::BAD_REQUEST, ErrorCode::InvalidArgument);
    }
    for name in ["ab", "a-b", " a_name", "pénguin", &"a".repeat(21)] {
        endpoint
            .register(name, &secret)
            .await
            .error(StatusCode::BAD_REQUEST, ErrorCode::InvalidArgument);
    }
    for invalid_password in ["a".repeat(14), "a".repeat(129), "é".repeat(65)] {
        endpoint
            .register("must_not_exist", &invalid_password)
            .await
            .error(StatusCode::BAD_REQUEST, ErrorCode::InvalidArgument);
    }
    for invalid_password in [String::new(), "a".repeat(129)] {
        endpoint
            .login("must_not_exist", &invalid_password)
            .await
            .error(StatusCode::BAD_REQUEST, ErrorCode::InvalidArgument);
    }
    let valid_wire = message(register("must_not_exist", &secret)).encode_to_vec();
    let mut valid_oversized = valid_wire.clone();
    valid_oversized.extend_from_slice(&[0xfa, 0x07, 0x80, 0x80, 0x01]);
    valid_oversized.extend_from_slice(&vec![0; MAX_REQUEST_BYTES]);
    assert!(ClientMessage::decode(valid_oversized.as_slice()).is_ok());
    read_reply(
        endpoint
            .request()
            .header(header::CONTENT_TYPE, MEDIA_TYPE)
            .body(valid_oversized)
            .send()
            .await
            .unwrap(),
    )
    .await
    .error(StatusCode::PAYLOAD_TOO_LARGE, ErrorCode::InvalidArgument);
    let mut exact = message(client_message::Command::Hello(Hello {})).encode_to_vec();
    let padding = MAX_REQUEST_BYTES - exact.len() - 4;
    assert!((128..16_384).contains(&padding));
    exact.extend_from_slice(&[
        0xfa,
        0x07,
        (padding as u8 & 0x7f) | 0x80,
        (padding >> 7) as u8,
    ]);
    exact.extend_from_slice(&vec![0; padding]);
    assert_eq!(exact.len(), MAX_REQUEST_BYTES);
    read_reply(
        endpoint
            .request()
            .header(header::CONTENT_TYPE, MEDIA_TYPE)
            .body(exact)
            .send()
            .await
            .unwrap(),
    )
    .await
    .hello(UNVERSIONED_BUILD);
    for media in [None, Some("application/json"), Some("text/plain")] {
        let mut request = endpoint.request().body(valid_wire.clone());
        if let Some(media) = media {
            request = request.header(header::CONTENT_TYPE, media);
        }
        read_reply(request.send().await.unwrap()).await.error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ErrorCode::InvalidArgument,
        );
    }
    let mut duplicate_types = HeaderMap::new();
    duplicate_types.append(header::CONTENT_TYPE, HeaderValue::from_static(MEDIA_TYPE));
    duplicate_types.append(header::CONTENT_TYPE, HeaderValue::from_static(MEDIA_TYPE));
    read_reply(
        endpoint
            .request()
            .headers(duplicate_types)
            .body(valid_wire.clone())
            .send()
            .await
            .unwrap(),
    )
    .await
    .error(
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ErrorCode::InvalidArgument,
    );
    for malformed in [
        vec![0x12, 0xff],
        vec![0xff; 20],
        vec![0x08, 0x01, 0x12, 0x02, 0xff, 0xff],
    ] {
        read_reply(
            endpoint
                .request()
                .header(header::CONTENT_TYPE, MEDIA_TYPE)
                .body(malformed)
                .send()
                .await
                .unwrap(),
        )
        .await
        .error(StatusCode::BAD_REQUEST, ErrorCode::InvalidArgument);
    }
    read_reply(
        endpoint
            .request()
            .header(header::CONTENT_TYPE, MEDIA_TYPE)
            .body(Vec::new())
            .send()
            .await
            .unwrap(),
    )
    .await
    .error(StatusCode::UPGRADE_REQUIRED, ErrorCode::UnsupportedVersion);
    for length in [MAX_REQUEST_BYTES + 1, MAX_REQUEST_BYTES * 2] {
        read_reply(
            endpoint
                .request()
                .header(header::CONTENT_TYPE, MEDIA_TYPE)
                .body(vec![0xff; length])
                .send()
                .await
                .unwrap(),
        )
        .await
        .error(StatusCode::PAYLOAD_TOO_LARGE, ErrorCode::InvalidArgument);
    }
    let chunks = futures_util::stream::iter([
        Ok::<_, std::io::Error>(vec![0xff; MAX_REQUEST_BYTES]),
        Ok(vec![0xff; 1]),
    ]);
    read_reply(
        endpoint
            .request()
            .header(header::CONTENT_TYPE, MEDIA_TYPE)
            .body(reqwest::Body::wrap_stream(chunks))
            .send()
            .await
            .unwrap(),
    )
    .await
    .error(StatusCode::PAYLOAD_TOO_LARGE, ErrorCode::InvalidArgument);
    let token = URL_SAFE_NO_PAD.encode([0x17; 32]);
    let current_wire = message(current()).encode_to_vec();
    for invalid in [
        "".to_owned(),
        "Basic invalid".to_owned(),
        "Bearer".to_owned(),
        "Bearer ".to_owned(),
        format!("Bearer {}", "a".repeat(42)),
        format!("Bearer {}", "a".repeat(44)),
        format!("Bearer {}", "!".repeat(43)),
        format!("Bearer {token}="),
        format!("Bearer  {token}"),
        format!("Bearer {}B", "A".repeat(42)),
        format!("Bearer {token}"),
    ] {
        read_reply(
            endpoint
                .request()
                .header(header::CONTENT_TYPE, MEDIA_TYPE)
                .header(header::AUTHORIZATION, invalid)
                .body(current_wire.clone())
                .send()
                .await
                .unwrap(),
        )
        .await
        .error(StatusCode::UNAUTHORIZED, ErrorCode::Unauthenticated);
    }
    endpoint
        .call(current(), None)
        .await
        .error(StatusCode::UNAUTHORIZED, ErrorCode::Unauthenticated);
    let mut duplicate_auth = HeaderMap::new();
    for _ in 0..2 {
        duplicate_auth.append(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
        );
    }
    read_reply(
        endpoint
            .request()
            .header(header::CONTENT_TYPE, MEDIA_TYPE)
            .headers(duplicate_auth)
            .body(current_wire.clone())
            .send()
            .await
            .unwrap(),
    )
    .await
    .error(StatusCode::UNAUTHORIZED, ErrorCode::Unauthenticated);
    read_reply(
        endpoint
            .request()
            .header(header::CONTENT_TYPE, MEDIA_TYPE)
            .header(
                header::AUTHORIZATION,
                HeaderValue::from_bytes(b"Bearer \xff").unwrap(),
            )
            .body(current_wire)
            .send()
            .await
            .unwrap(),
    )
    .await
    .error(StatusCode::UNAUTHORIZED, ErrorCode::Unauthenticated);
    assert_eq!(database.account_count().await, 0);
    assert_eq!(database.session_count().await, 0);

    let minimum = Uuid::new_v4().simple().to_string()[..15].to_owned();
    endpoint.register("abc", &minimum).await.registered();
    let maximum = format!("{}{}", "é".repeat(48), Uuid::new_v4().simple());
    assert_eq!(maximum.len(), 128);
    let boundary = endpoint
        .register(&"a".repeat(20), &maximum)
        .await
        .registered();
    assert_eq!(
        endpoint
            .login(&"A".repeat(20), &maximum)
            .await
            .logged_in()
            .account
            .as_ref(),
        Some(&boundary)
    );
    assert_eq!(database.account_count().await, 2);
    service.stop().await;
    database.pool.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn rate_limits_use_actual_peers_not_spoofed_forwarding_headers() {
    let database = TestDatabase::reset().await;
    let service = TestService::start(&database.url).await;
    let endpoint = &service.endpoint;
    let secret = password();
    for index in 1..=20 {
        let request = message(login("unknown_penguin", &secret));
        let reply = read_reply(
            endpoint
                .request()
                .header(header::CONTENT_TYPE, MEDIA_TYPE)
                .header("X-Forwarded-For", format!("192.0.2.{index}"))
                .header("Forwarded", format!("for=192.0.2.{index}"))
                .body(request.encode_to_vec())
                .send()
                .await
                .unwrap(),
        )
        .await;
        reply.error(StatusCode::UNAUTHORIZED, ErrorCode::Unauthenticated);
        assert_eq!(reply.message.request_id, request.request_id);
    }
    let limited = endpoint.login("unknown_penguin", &secret).await;
    let error = limited.error(StatusCode::TOO_MANY_REQUESTS, ErrorCode::ResourceExhausted);
    assert!((1..=60).contains(&error.retry_after_seconds));
    assert_eq!(
        limited.headers[header::RETRY_AFTER].to_str().unwrap(),
        error.retry_after_seconds.to_string()
    );
    endpoint
        .register("still_limited", &secret)
        .await
        .error(StatusCode::TOO_MANY_REQUESTS, ErrorCode::ResourceExhausted);
    assert_eq!(database.account_count().await, 0);
    assert_eq!(database.session_count().await, 0);
    assert_eq!(endpoint.health().await.status(), StatusCode::OK);
    endpoint
        .call(client_message::Command::Hello(Hello {}), None)
        .await
        .hello(UNVERSIONED_BUILD);
    let other_peer = Endpoint {
        client: Client::builder()
            .no_proxy()
            .timeout(WAIT)
            .local_address(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2)))
            .build()
            .unwrap(),
        base_url: endpoint.base_url.clone(),
    };
    other_peer
        .register("independent_peer", &secret)
        .await
        .registered();
    assert_eq!(database.account_count().await, 1);
    service.stop().await;
    database.pool.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn lost_database_transport_fails_readiness_and_mutations_without_fallback() {
    let database = TestDatabase::reset().await;
    let proxy = DatabaseProxy::start(database.address).await;
    let url = replace_port(&database.url, proxy.address.port());
    let service = TestService::start(&url).await;
    let endpoint = &service.endpoint;
    let secret = password();
    let account = endpoint
        .register("outage_penguin", &secret)
        .await
        .registered();
    let session = endpoint.login("outage_penguin", &secret).await.logged_in();
    assert_eq!(
        endpoint.snapshot(&session.session_token).await.account(),
        account
    );
    proxy.stop().await;

    read_reply(endpoint.health().await)
        .await
        .error(StatusCode::SERVICE_UNAVAILABLE, ErrorCode::Unavailable);
    endpoint
        .snapshot(&session.session_token)
        .await
        .error(StatusCode::SERVICE_UNAVAILABLE, ErrorCode::Unavailable);
    endpoint
        .logout(&session.session_token)
        .await
        .error(StatusCode::SERVICE_UNAVAILABLE, ErrorCode::Unavailable);
    endpoint
        .login("outage_penguin", &secret)
        .await
        .error(StatusCode::SERVICE_UNAVAILABLE, ErrorCode::Unavailable);
    endpoint
        .register("no_fallback_account", &secret)
        .await
        .error(StatusCode::SERVICE_UNAVAILABLE, ErrorCode::Unavailable);
    endpoint
        .snapshot("malformed")
        .await
        .error(StatusCode::UNAUTHORIZED, ErrorCode::Unauthenticated);
    assert_eq!(database.account_count().await, 1);
    assert_eq!(database.session_count().await, 1);
    assert!(database.has_session(&session.session_token).await);
    service.stop().await;

    let fresh = TestService::start(&database.url).await;
    assert_eq!(
        fresh
            .endpoint
            .snapshot(&session.session_token)
            .await
            .account(),
        account
    );
    fresh.stop().await;
    database.pool.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn startup_and_unexpected_database_failures_are_explicit_and_sanitized() {
    let database = TestDatabase::reset().await;
    for (url, bind) in [
        (None, "127.0.0.1:0"),
        (Some(database.url.as_str()), "0.0.0.0:4010"),
        (Some("not-a-postgres-url-with-secret"), "127.0.0.1:0"),
        (Some(database.url.as_str()), "not-a-socket"),
    ] {
        let (process, _) = ChildCapture::spawn(url, bind);
        let logs = process.finish(false).await;
        assert!(logs.contains("configuration_failure"));
        assert!(!logs.contains(&database.url));
        assert!(!logs.contains("not-a-postgres-url-with-secret"));
        assert!(!logs.contains("listening"));
    }

    let unresponsive = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let invalid_url = replace_port(&database.url, unresponsive.local_addr().unwrap().port());
    let config = Config::new(&invalid_url, "127.0.0.1:0", None).unwrap();
    let failure = timeout(WAIT, Service::bind(config))
        .await
        .expect("database startup timeout must be bounded")
        .err()
        .expect("an unresponsive database must prevent startup");
    assert_eq!(failure.stage(), "database");
    assert!(!failure.error_id().is_nil());
    assert!(!format!("{failure:?} {failure}").contains(&invalid_url));
    drop(unresponsive);

    sqlx::query("CREATE TABLE accounts (incompatible INTEGER)")
        .execute(&database.pool)
        .await
        .unwrap();
    let config = Config::new(&database.url, "127.0.0.1:0", None).unwrap();
    let failure = timeout(WAIT, Service::bind(config))
        .await
        .unwrap()
        .err()
        .expect("a failed migration must prevent startup");
    assert_eq!(failure.stage(), "migrations");
    assert!(!failure.error_id().is_nil());
    sqlx::query("DROP TABLE accounts")
        .execute(&database.pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE _sqlx_migrations")
        .execute(&database.pool)
        .await
        .unwrap();

    let occupied = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let config = Config::new(
        &database.url,
        &occupied.local_addr().unwrap().to_string(),
        None,
    )
    .unwrap();
    let failure = Service::bind(config)
        .await
        .err()
        .expect("an occupied listener must prevent startup");
    assert_eq!(failure.stage(), "listener");
    drop(occupied);

    let service = TestService::start(&database.url).await;
    let secret = password();
    let account = service
        .endpoint
        .register("corrupt_hash", &secret)
        .await
        .registered();
    sqlx::query(
        "UPDATE accounts SET password_hash = replace(password_hash, 'm=19456', 'm=4294967295')
         WHERE account_id = $1",
    )
    .bind(Uuid::parse_str(&account.account_id).unwrap())
    .execute(&database.pool)
    .await
    .unwrap();
    service
        .endpoint
        .login("corrupt_hash", &secret)
        .await
        .error(StatusCode::INTERNAL_SERVER_ERROR, ErrorCode::Internal);
    assert_eq!(database.session_count().await, 0);
    sqlx::query("DROP TABLE account_sessions")
        .execute(&database.pool)
        .await
        .unwrap();
    let raw = URL_SAFE_NO_PAD.encode([0x42; 32]);
    service
        .endpoint
        .snapshot(&raw)
        .await
        .error(StatusCode::INTERNAL_SERVER_ERROR, ErrorCode::Internal);
    service.stop().await;
    database.pool.close().await;
}
