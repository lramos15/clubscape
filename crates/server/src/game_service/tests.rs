#[path = "../../../content/tests/common/mod.rs"]
mod fixtures;

use std::{env, fs, path::PathBuf, str::FromStr, sync::Arc, time::Duration};

use clubscape_content::{ValidationMode, compile_content, encode_compiled, sha256};
use clubscape_game_types::*;
use clubscape_protocol::{
    ClientMessage, CurrentAccount, Hello, Login, Logout, MEDIA_TYPE, PROTOCOL_VERSION, Register,
    ServerMessage,
};
use reqwest::Client;
use sqlx::{
    ConnectOptions,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use tokio::{
    sync::{Mutex, MutexGuard, oneshot},
    time::{sleep, timeout},
};

use super::*;
use crate::Service;

const WAIT: Duration = Duration::from_secs(15);
static DATABASE_LOCK: Mutex<()> = Mutex::const_new(());

struct Database {
    url: String,
    pool: PgPool,
    _guard: MutexGuard<'static, ()>,
}

impl Database {
    async fn reset() -> Self {
        let guard = DATABASE_LOCK.lock().await;
        let url = env::var("CLUBSCAPE_TEST_DATABASE_URL")
            .expect("requires isolated PostgreSQL; run just test-integration");
        let options = PgConnectOptions::from_str(&url)
            .unwrap_or_else(|_| panic!("invalid isolated database configuration"));
        assert_eq!(
            options.get_database(),
            Some("clubscape_m1_test"),
            "refusing another database"
        );
        let ip: std::net::IpAddr = options
            .get_host()
            .parse()
            .expect("literal loopback PostgreSQL required");
        assert!(ip.is_loopback(), "refusing non-loopback PostgreSQL");
        let pool = timeout(
            WAIT,
            PgPoolOptions::new()
                .min_connections(0)
                .max_connections(8)
                .idle_timeout(None)
                .max_lifetime(None)
                .acquire_timeout(Duration::from_secs(5))
                .connect_with(
                    options
                        .disable_statement_logging()
                        .application_name("clubscape-live-world-tests"),
                ),
        )
        .await
        .unwrap()
        .unwrap_or_else(|_| panic!("isolated PostgreSQL connection failed"));
        let (name, schema): (String, String) =
            sqlx::query_as("SELECT current_database(), current_schema()")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(name, "clubscape_m1_test");
        assert_eq!(schema, "public");
        sqlx::raw_sql(
            "DROP TABLE IF EXISTS public.processed_game_commands;
             DROP TABLE IF EXISTS public.game_sessions;
             DROP TABLE IF EXISTS public.game_characters;
             DROP TABLE IF EXISTS public.game_worlds;
             DROP TABLE IF EXISTS public.account_sessions;
             DROP TABLE IF EXISTS public.accounts;
             DROP TABLE IF EXISTS public._sqlx_migrations;",
        )
        .execute(&pool)
        .await
        .expect("reset only the isolated owned service tables");
        Self {
            url,
            pool,
            _guard: guard,
        }
    }

    async fn world(&self, id: Uuid) -> WorldSnapshot {
        GameStore::new(self.pool.clone())
            .load_world(id)
            .await
            .unwrap()
    }

    async fn wait_world_lock(&self) {
        timeout(Duration::from_secs(3), async {
            loop {
                let waiting: bool = sqlx::query_scalar(
                    "SELECT EXISTS (SELECT 1 FROM pg_stat_activity
                     WHERE datname = current_database() AND application_name = 'clubscape-server'
                         AND wait_event_type = 'Lock' AND query LIKE 'SELECT world_id, content_revision,%')",
                ).fetch_one(&self.pool).await.unwrap();
                if waiting { break; }
                sleep(Duration::from_millis(10)).await;
            }
        }).await.expect("the live coordinator must actually wait on its owned world lock");
    }
}

struct Pack {
    root: PathBuf,
    world_id: Uuid,
    definition: GameContent,
}

impl Pack {
    fn new() -> Self {
        let mut definition = fixtures::fixture();
        definition.revision = "live-world-synthetic-v1".into();
        definition.initial_state.tile = fixtures::tile(1002, 1003);
        for stage in definition.tutorial.values_mut() {
            stage.allowed_actions = vec!["*".into()];
        }
        let guide = definition
            .spawns
            .get_mut(&fixtures::id("spawn.test.guide"))
            .unwrap();
        guide.interactions.push(InteractionDefinition {
            name: "Practice".into(),
            reach: 4,
            guard: Guard::Always,
            action: InteractionAction::Effects {
                effects: vec![
                    Effect::GiveItems {
                        items: vec![fixtures::stack("item.test.coins", 13)],
                    },
                    Effect::AwardXp {
                        rewards: vec![XpReward {
                            skill: fixtures::id("skill.test.mining"),
                            amount_tenths: 50,
                        }],
                    },
                    Effect::Message {
                        text: "Only the acting player receives this synthetic notification.".into(),
                    },
                ],
            },
        });
        let InteractionAction::Gather { rule } = &mut definition
            .spawns
            .get_mut(&fixtures::id("spawn.test.rock"))
            .unwrap()
            .interactions[0]
            .action
        else {
            panic!()
        };
        rule.success = ChanceRule::constant(1, 1);
        rule.depletion = ChanceRule::constant(0, 1);
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../.local/live-server-tests")
            .join(Uuid::new_v4().to_string());
        fs::create_dir_all(&root).unwrap();
        let pack = Self {
            root,
            world_id: Uuid::new_v4(),
            definition,
        };
        pack.write();
        pack
    }

    fn write(&self) {
        let compiled =
            compile_content(self.definition.clone(), ValidationMode::TestFixture).unwrap();
        let artifact = encode_compiled(&compiled).unwrap();
        fs::write(self.root.join("world.csc"), &artifact).unwrap();
        let mut entries = Vec::new();
        let mut assets = BTreeMap::new();
        for (index, id) in compiled.referenced_assets().iter().enumerate() {
            let path = format!("asset-{index}.bin");
            let url = format!("/assets/fixture-{index}.bin");
            let bytes =
                format!("Synthetic integration bytes for {id}; not a real presentation asset.");
            fs::write(self.root.join(&path), &bytes).unwrap();
            entries.push(serde_json::json!({
                "url":url, "path":path, "sha256":sha256(bytes.as_bytes()), "content_type":"application/octet-stream"
            }));
            assets.insert(id.to_string(), url);
        }
        let manifest = serde_json::to_vec(&serde_json::json!({
            "schema_version":1, "content_revision":self.definition.revision,
            "synthetic_fixture":true, "milestone_accepted":false,
        }))
        .unwrap();
        fs::write(self.root.join("public.json"), &manifest).unwrap();
        entries.push(serde_json::json!({
            "url":"/content/manifest.json", "path":"public.json",
            "sha256":sha256(&manifest), "content_type":"application/json"
        }));
        fs::write(
            self.root.join("clubscape-game-assets.json"),
            serde_json::to_vec(&serde_json::json!({"schema_version":1,"files":entries})).unwrap(),
        )
        .unwrap();
        fs::write(self.root.join("clubscape-game.json"), serde_json::to_vec(&serde_json::json!({
            "schema_version":1, "world_id":self.world_id, "artifact":"world.csc",
            "sha256":sha256(&artifact), "content_manifest_path":"/content/manifest.json", "assets":assets,
        })).unwrap()).unwrap();
    }

    fn config(&self, database: &Database) -> Config {
        let mut config = Config::new(&database.url, "127.0.0.1:0", None)
            .unwrap()
            .with_game_root(&self.root)
            .unwrap();
        // No environment/production flag or public constructor can select this test-only path.
        config.game_test_fixture = true;
        config
    }
}

impl Drop for Pack {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).expect("remove only this UUID-owned test pack");
    }
}

#[test]
fn compiler_fixture_loads_through_the_explicit_test_only_adapter() {
    let pack = Pack::new();
    clubscape_world_engine::WorldEngine::new(Arc::new(pack.definition.clone())).unwrap();
    let mut config = Config::new(
        "postgres://127.0.0.1/clubscape_m1_test",
        "127.0.0.1:0",
        None,
    )
    .unwrap()
    .with_game_root(&pack.root)
    .unwrap();
    config.game_test_fixture = true;
    let subscriber = tracing_subscriber::fmt().with_test_writer().finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    assert!(content::load(&config).unwrap().is_some());
}

struct Live {
    endpoint: Endpoint,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), ServeError>>>,
}

#[derive(Clone)]
struct Endpoint {
    client: Client,
    origin: String,
}

impl Live {
    async fn start(config: Config) -> Self {
        let service = timeout(WAIT, Service::bind(config)).await.unwrap().unwrap();
        let endpoint = Endpoint {
            client: Client::builder().no_proxy().timeout(WAIT).build().unwrap(),
            origin: format!("http://{}", service.local_addr()),
        };
        let (stop, receive) = oneshot::channel();
        let task = tokio::spawn(service.serve(async move {
            let _ = receive.await;
        }));
        let health = endpoint
            .client
            .get(format!("{}/healthz", endpoint.origin))
            .send()
            .await
            .unwrap();
        assert!(matches!(
            health.status(),
            StatusCode::OK | StatusCode::SERVICE_UNAVAILABLE
        ));
        Self {
            endpoint,
            stop: Some(stop),
            task: Some(task),
        }
    }

    async fn stop(mut self) -> Result<(), ServeError> {
        self.stop.take().unwrap().send(()).unwrap();
        let result = timeout(WAIT, self.task.as_mut().unwrap())
            .await
            .expect("bounded owned game/HTTP/pool shutdown")
            .unwrap();
        self.task.take();
        result
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

struct Response {
    status: StatusCode,
    message: ServerMessage,
}

impl Response {
    fn error(self, expected: StatusCode) -> clubscape_protocol::Error {
        assert_eq!(self.status, expected, "{:?}", self.message.result);
        let Some(server_message::Result::Error(error)) = self.message.result else {
            panic!("explicit protocol error required")
        };
        assert!(!Uuid::parse_str(&error.error_id).unwrap().is_nil());
        error
    }

    fn snapshot(self) -> game::WorldSnapshot {
        assert_eq!(self.status, StatusCode::OK, "{:?}", self.message.result);
        match self.message.result {
            Some(server_message::Result::WorldSnapshot(value)) => value,
            _ => panic!("authoritative snapshot required"),
        }
    }

    fn action(self) -> game::ActionResult {
        assert_eq!(self.status, StatusCode::OK, "{:?}", self.message.result);
        match self.message.result {
            Some(server_message::Result::ActionResult(value)) => value,
            _ => panic!("committed action result required"),
        }
    }
}

struct Account {
    token: String,
    account_id: Uuid,
}

impl Endpoint {
    async fn call(
        &self,
        command: client_message::Command,
        token: Option<&str>,
        operation: Uuid,
    ) -> Response {
        let message = ClientMessage {
            protocol_version: PROTOCOL_VERSION,
            request_id: operation.to_string(),
            command: Some(command),
        };
        self.send(message, token).await
    }

    async fn send(&self, message: ClientMessage, token: Option<&str>) -> Response {
        let mut request = self
            .client
            .post(format!("{}/v1/rpc", self.origin))
            .header("content-type", MEDIA_TYPE)
            .body(message.encode_to_vec());
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        let response = request.send().await.unwrap();
        assert_eq!(response.headers()["content-type"], MEDIA_TYPE);
        assert_eq!(response.headers()["cache-control"], "no-store");
        let status = response.status();
        let bytes = response.bytes().await.unwrap();
        assert!(bytes.len() <= clubscape_protocol::MAX_GAME_RESPONSE_BYTES);
        let response = ServerMessage::decode(bytes).unwrap();
        assert_eq!(response.request_id, message.request_id);
        Response {
            status,
            message: response,
        }
    }

    async fn account(&self, name: &str) -> Account {
        let password = format!("Synthetic account password {}!", Uuid::new_v4());
        let response = self
            .call(
                client_message::Command::Register(Register {
                    login_name: name.into(),
                    password: password.clone(),
                }),
                None,
                Uuid::new_v4(),
            )
            .await;
        assert_eq!(response.status, StatusCode::OK);
        let response = self
            .call(
                client_message::Command::Login(Login {
                    login_name: name.into(),
                    password,
                }),
                None,
                Uuid::new_v4(),
            )
            .await;
        assert_eq!(response.status, StatusCode::OK);
        let Some(server_message::Result::LoggedIn(login)) = response.message.result else {
            panic!()
        };
        Account {
            token: login.session_token,
            account_id: Uuid::parse_str(&login.account.unwrap().account_id).unwrap(),
        }
    }

    async fn create(&self, account: &Account) -> String {
        let response = self
            .call(
                client_message::Command::CreateCharacter(game::CreateCharacter::default()),
                Some(&account.token),
                Uuid::new_v4(),
            )
            .await;
        assert_eq!(
            response.status,
            StatusCode::OK,
            "{:?}",
            response.message.result
        );
        let Some(server_message::Result::CharacterCreated(character)) = response.message.result
        else {
            panic!()
        };
        character.actor_id
    }

    async fn join(&self, account: &Account) -> game::WorldJoined {
        let response = self
            .call(
                client_message::Command::JoinWorld(game::JoinWorld {}),
                Some(&account.token),
                Uuid::new_v4(),
            )
            .await;
        assert_eq!(
            response.status,
            StatusCode::OK,
            "{:?}",
            response.message.result
        );
        let Some(server_message::Result::WorldJoined(joined)) = response.message.result else {
            panic!()
        };
        joined
    }

    async fn poll(&self, account: &Account, joined: &game::WorldJoined, revision: u64) -> Response {
        self.call(
            client_message::Command::PollWorld(game::PollWorld {
                world_session_id: joined.world_session_id.clone(),
                after_revision: revision,
            }),
            Some(&account.token),
            Uuid::new_v4(),
        )
        .await
    }

    async fn input(
        &self,
        account: &Account,
        joined: &game::WorldJoined,
        sequence: u64,
        operation: Uuid,
        action: game::world_input::Action,
    ) -> Response {
        self.call(
            client_message::Command::WorldInput(game::WorldInput {
                world_session_id: joined.world_session_id.clone(),
                sequence,
                expected_character_revision: None,
                action: Some(action),
            }),
            Some(&account.token),
            operation,
        )
        .await
    }
}

fn practice() -> game::world_input::Action {
    game::world_input::Action::Interact(game::Interact {
        target: "spawn.test.guide".into(),
        action: "Practice".into(),
    })
}

fn mine() -> game::world_input::Action {
    game::world_input::Action::Interact(game::Interact {
        target: "spawn.test.rock".into(),
        action: "Mine".into(),
    })
}

fn xp(snapshot: &game::WorldSnapshot) -> u64 {
    snapshot
        .player
        .as_ref()
        .unwrap()
        .skills
        .iter()
        .find(|skill| skill.id == "skill.test.mining")
        .unwrap()
        .xp_tenths
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn configured_artifacts_are_strict_pinned_and_never_fall_back_to_an_account_seed() {
    let database = Database::reset().await;
    let mut pack = Pack::new();
    let runtime = Config::new(&database.url, "127.0.0.1:0", None)
        .unwrap()
        .with_game_root(&pack.root)
        .unwrap();
    let error = Service::bind(runtime)
        .await
        .err()
        .expect("runtime cannot load a TestFixture artifact");
    assert_eq!(error.stage(), "game_content");
    fs::write(pack.root.join("world.csc"), b"corrupt").unwrap();
    assert_eq!(
        Service::bind(pack.config(&database))
            .await
            .err()
            .unwrap()
            .stage(),
        "game_content"
    );
    pack.write();
    fs::remove_file(pack.root.join("public.json")).unwrap();
    assert_eq!(
        Service::bind(pack.config(&database))
            .await
            .err()
            .unwrap()
            .stage(),
        "game_content"
    );
    pack.write();
    pack.definition
        .items
        .get_mut(&fixtures::id("item.test.coins"))
        .unwrap()
        .weight = Some(SourceBinding::Unresolved {
        reason: "Synthetic unresolved input must not receive a server default.".into(),
        source: fixtures::sources(),
    });
    pack.write();
    assert_eq!(
        Service::bind(pack.config(&database))
            .await
            .err()
            .unwrap()
            .stage(),
        "game_content"
    );
    let service = Live::start(Config::new(&database.url, "127.0.0.1:0", None).unwrap()).await;
    let account = service.endpoint.account("accounts_only").await;
    service
        .endpoint
        .call(
            client_message::Command::CreateCharacter(game::CreateCharacter::default()),
            Some(&account.token),
            Uuid::new_v4(),
        )
        .await
        .error(StatusCode::SERVICE_UNAVAILABLE);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM game_characters")
        .fetch_one(&database.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert_eq!(
        service
            .endpoint
            .call(
                client_message::Command::Logout(Logout {}),
                Some(&account.token),
                Uuid::new_v4()
            )
            .await
            .status,
        StatusCode::OK
    );
    service.stop().await.unwrap();
    database.pool.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn live_http_clones_source_characters_and_routes_only_committed_own_effects() {
    let database = Database::reset().await;
    let pack = Pack::new();
    let service = Live::start(pack.config(&database)).await;
    let endpoint = &service.endpoint;
    let hello = endpoint
        .call(
            client_message::Command::Hello(Hello {}),
            None,
            Uuid::new_v4(),
        )
        .await;
    let Some(server_message::Result::Hello(hello)) = hello.message.result else {
        panic!()
    };
    assert!(hello.gameplay_available);
    assert!(
        hello
            .capabilities
            .iter()
            .any(|value| value == clubscape_protocol::GAME_CAPABILITY)
    );
    let alice = endpoint.account("live_alice").await;
    let current = endpoint
        .call(
            client_message::Command::CurrentAccount(CurrentAccount {}),
            Some(&alice.token),
            Uuid::new_v4(),
        )
        .await;
    let Some(server_message::Result::Account(current)) = current.message.result else {
        panic!()
    };
    assert!(!current.character_initialized);
    endpoint
        .call(
            client_message::Command::CreateCharacter(game::CreateCharacter {
                experience_choice: "new".into(),
                ..Default::default()
            }),
            Some(&alice.token),
            Uuid::new_v4(),
        )
        .await
        .error(StatusCode::BAD_REQUEST);
    let alice_id = endpoint.create(&alice).await;
    let a = endpoint.join(&alice).await;
    let initial = a.snapshot.as_ref().unwrap();
    assert_eq!(initial.player.as_ref().unwrap().actor_id, alice_id);
    assert!(initial.events.is_empty());
    assert!(initial.player.as_ref().unwrap().bank.is_empty());
    assert!(!initial.player.as_ref().unwrap().bank_open);
    assert!(!initial.player.as_ref().unwrap().appearance_confirmed);
    let stored = database.world(pack.world_id).await;
    let actor = ActorId::new(&alice_id).unwrap();
    assert_eq!(
        stored.state.characters[&actor].inventory,
        pack.definition.initial_state.inventory
    );
    assert_eq!(
        stored.state.characters[&actor].bank,
        pack.definition.initial_state.bank
    );
    assert_eq!(
        stored.state.characters[&actor].skills,
        pack.definition.initial_state.skills
    );
    assert!(
        !stored.state.characters[&actor]
            .runtime
            .settings
            .appearance_confirmed
    );
    assert_eq!(
        stored.state.characters[&actor].runtime.settings.experience,
        None
    );
    assert_eq!(endpoint.create(&alice).await, alice_id);
    let current = endpoint
        .call(
            client_message::Command::CurrentAccount(CurrentAccount {}),
            Some(&alice.token),
            Uuid::new_v4(),
        )
        .await;
    let Some(server_message::Result::Account(current)) = current.message.result else {
        panic!()
    };
    assert!(current.character_initialized);
    let asset = endpoint
        .client
        .get(format!("{}{}", endpoint.origin, a.content_manifest_path))
        .send()
        .await
        .unwrap();
    assert_eq!(asset.status(), StatusCode::OK);
    assert_eq!(asset.headers()["x-content-type-options"], "nosniff");
    assert!(asset.headers().contains_key("content-security-policy"));
    let bob = endpoint.account("live_bob").await;
    endpoint.create(&bob).await;
    let b = endpoint.join(&bob).await;
    let operation = Uuid::new_v4();
    let action = endpoint
        .input(&alice, &a, 1, operation, practice())
        .await
        .action();
    let first = action.snapshot.unwrap();
    assert!(!action.duplicate);
    assert_eq!(xp(&first), 50);
    assert!(first.events.iter().any(|event| event.kind == "xp_gained"));
    assert!(
        first
            .events
            .iter()
            .all(|event| event.actor_id == alice_id && !event.event_id.is_empty())
    );
    let other = endpoint
        .poll(&bob, &b, b.snapshot.as_ref().unwrap().revision)
        .await
        .snapshot();
    assert_eq!(xp(&other), 0);
    assert!(
        other.events.is_empty(),
        "another actor must not receive XP or sound notifications"
    );
    assert!(
        b.snapshot
            .as_ref()
            .unwrap()
            .entities
            .iter()
            .any(|entity| entity.id == alice_id)
    );
    assert!(!other.full_snapshot);
    assert!(!other.removed_entities.contains(&alice_id));
    assert!(
        !other.entities.iter().any(|entity| entity.id == alice_id),
        "private inventory/XP changes must not appear as another player's entity delta"
    );
    assert!(
        other
            .entities
            .iter()
            .all(|entity| entity.equipment.is_empty())
    );
    let duplicate = endpoint
        .input(&alice, &a, 1, operation, practice())
        .await
        .action();
    assert!(duplicate.duplicate);
    assert_eq!(xp(duplicate.snapshot.as_ref().unwrap()), 50);
    assert_eq!(
        first
            .events
            .iter()
            .map(|event| &event.event_id)
            .collect::<Vec<_>>(),
        duplicate
            .snapshot
            .as_ref()
            .unwrap()
            .events
            .iter()
            .map(|event| &event.event_id)
            .collect::<Vec<_>>(),
    );
    endpoint
        .input(
            &alice,
            &a,
            1,
            operation,
            game::world_input::Action::MoveInventory(game::MoveInventory { from: 0, to: 1 }),
        )
        .await
        .error(StatusCode::CONFLICT);
    endpoint
        .input(&alice, &a, 3, Uuid::new_v4(), practice())
        .await
        .error(StatusCode::CONFLICT);
    endpoint.poll(&bob, &a, 0).await.error(StatusCode::CONFLICT);
    let uppercase_id = Uuid::new_v4().to_string().to_uppercase();
    let uppercase = endpoint
        .send(
            ClientMessage {
                protocol_version: PROTOCOL_VERSION,
                request_id: uppercase_id.clone(),
                command: Some(client_message::Command::WorldInput(game::WorldInput {
                    world_session_id: a.world_session_id.clone(),
                    sequence: 2,
                    expected_character_revision: None,
                    action: Some(game::world_input::Action::MoveInventory(
                        game::MoveInventory { from: 0, to: 1 },
                    )),
                })),
            },
            Some(&alice.token),
        )
        .await
        .action();
    assert_eq!(
        uppercase.operation_id, uppercase_id,
        "operation and envelope correlation IDs must agree exactly"
    );
    let before = database.world(pack.world_id).await.state.characters[&actor].clone();
    endpoint.create(&alice).await;
    assert_eq!(
        database.world(pack.world_id).await.state.characters[&actor],
        before
    );
    let result: String = sqlx::query_scalar(
        "SELECT committed_result::text FROM processed_game_commands WHERE account_id = $1 AND operation_id = $2",
    ).bind(alice.account_id).bind(operation).fetch_one(&database.pool).await.unwrap();
    let receipt: crate::game_storage::CommandReceipt = serde_json::from_str(&result).unwrap();
    assert!(
        receipt.events.is_empty(),
        "routing must not be flattened into the legacy receipt"
    );
    assert!(
        receipt
            .routed_events
            .iter()
            .all(|event| event.actor_id == actor)
    );
    assert!(!result.contains(&alice.token));
    assert!(!result.contains(&a.world_session_id));
    sqlx::query(
        "UPDATE processed_game_commands SET committed_result =
             jsonb_set(committed_result, '{routed_events,0,actor_id}', to_jsonb($3::text))
         WHERE account_id = $1 AND operation_id = $2",
    )
    .bind(alice.account_id)
    .bind(operation)
    .bind(
        &b.snapshot
            .as_ref()
            .unwrap()
            .player
            .as_ref()
            .unwrap()
            .actor_id,
    )
    .execute(&database.pool)
    .await
    .unwrap();
    endpoint
        .input(&alice, &a, 1, operation, practice())
        .await
        .error(StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        database.world(pack.world_id).await.state.characters[&actor],
        before,
        "corrupt routing cannot rerun the mutation or redirect an old result"
    );
    sqlx::query(
        "UPDATE processed_game_commands SET committed_result = $3::jsonb
         WHERE account_id = $1 AND operation_id = $2",
    )
    .bind(alice.account_id)
    .bind(operation)
    .bind(result)
    .execute(&database.pool)
    .await
    .unwrap();
    service.stop().await.unwrap();
    database.pool.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn fixed_cadence_runs_real_engine_work_once_for_concurrent_players() {
    let database = Database::reset().await;
    let pack = Pack::new();
    let service = Live::start(pack.config(&database)).await;
    let a = service.endpoint.account("cadence_a").await;
    let actor_a = service.endpoint.create(&a).await;
    let joined_a = service.endpoint.join(&a).await;
    let b = service.endpoint.account("cadence_b").await;
    let actor_b = service.endpoint.create(&b).await;
    let joined_b = service.endpoint.join(&b).await;
    let previous_tick = database.world(pack.world_id).await.state.tick;
    timeout(Duration::from_secs(3), async {
        loop {
            sleep(Duration::from_millis(30)).await;
            if database.world(pack.world_id).await.state.tick > previous_tick {
                break;
            }
        }
    })
    .await
    .unwrap();
    let (first, second) = tokio::join!(
        service
            .endpoint
            .input(&a, &joined_a, 1, Uuid::new_v4(), mine()),
        service
            .endpoint
            .input(&b, &joined_b, 1, Uuid::new_v4(), mine()),
    );
    let first = first.action().snapshot.unwrap();
    let second = second.action().snapshot.unwrap();
    assert_eq!(
        first.tick, second.tick,
        "a bounded batch uses one authoritative tick"
    );
    assert_eq!(xp(&first), 0);
    assert_eq!(xp(&second), 0);
    let started = Instant::now();
    let mut latest = first.clone();
    while latest.tick < first.tick + 2 {
        if latest.tick < first.tick + 2 {
            assert_eq!(xp(&latest), 0);
        }
        sleep(Duration::from_millis(60)).await;
        latest = service
            .endpoint
            .poll(&a, &joined_a, latest.revision)
            .await
            .snapshot();
        assert!(started.elapsed() < Duration::from_secs(4));
    }
    assert!(
        started.elapsed() >= Duration::from_millis(1000),
        "two 600ms source ticks cannot run on network arrivals"
    );
    assert_eq!(xp(&latest), 100);
    assert!(latest.events.iter().all(|event| event.actor_id == actor_a));
    let other = service
        .endpoint
        .poll(&b, &joined_b, second.revision)
        .await
        .snapshot();
    assert_eq!(xp(&other), 100);
    assert!(other.events.iter().all(|event| event.actor_id == actor_b));
    let tick = database.world(pack.world_id).await.last_tick.unwrap();
    assert!(tick.events.is_empty());
    let actors: BTreeSet<_> = tick
        .routed_events
        .iter()
        .map(|event| event.actor_id.to_string())
        .collect();
    assert!(actors.contains(&actor_a) && actors.contains(&actor_b));
    let conflict = Service::bind(pack.config(&database))
        .await
        .err()
        .expect("a second ticker cannot own the same live world");
    assert_eq!(conflict.stage(), "game_world");
    let before = database.world(pack.world_id).await.state.tick;
    let started = Instant::now();
    for _ in 0..10 {
        service.endpoint.poll(&a, &joined_a, 0).await.snapshot();
    }
    let after = database.world(pack.world_id).await.state.tick;
    assert!(after - before <= started.elapsed().as_millis() as u64 / TICK_MILLISECONDS + 1);
    service.stop().await.unwrap();
    let stopped = database.world(pack.world_id).await.state.tick;
    sleep(Duration::from_millis(700)).await;
    assert_eq!(
        database.world(pack.world_id).await.state.tick,
        stopped,
        "no ticker may survive service shutdown"
    );
    database.pool.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn reconnect_restart_and_missing_presence_or_context_helpers_are_explicit() {
    let database = Database::reset().await;
    let pack = Pack::new();
    let service = Live::start(pack.config(&database)).await;
    let account = service.endpoint.account("restart_live").await;
    let actor = service.endpoint.create(&account).await;
    let joined = service.endpoint.join(&account).await;
    let operation = Uuid::new_v4();
    service
        .endpoint
        .input(&account, &joined, 1, operation, practice())
        .await
        .action();
    for action in [
        game::world_input::Action::RequestLogout(game::Empty {}),
        game::world_input::Action::Interact(game::Interact {
            target: "spawn.test.guide".into(),
            action: "Bank".into(),
        }),
        game::world_input::Action::Interact(game::Interact {
            target: "spawn.test.guide".into(),
            action: "Shop".into(),
        }),
        game::world_input::Action::Interact(game::Interact {
            target: "spawn.test.guide".into(),
            action: "Talk".into(),
        }),
        game::world_input::Action::ConfirmAppearance(game::ConfirmAppearance::default()),
        game::world_input::Action::SelectExperience(game::SelectExperience {
            experience: "experience.test.new".into(),
        }),
    ] {
        service
            .endpoint
            .input(&account, &joined, 2, Uuid::new_v4(), action)
            .await
            .error(StatusCode::SERVICE_UNAVAILABLE);
    }
    service
        .endpoint
        .call(
            client_message::Command::LeaveWorld(game::LeaveWorld {
                world_session_id: joined.world_session_id.clone(),
            }),
            Some(&account.token),
            Uuid::new_v4(),
        )
        .await
        .error(StatusCode::SERVICE_UNAVAILABLE);
    service
        .endpoint
        .call(
            client_message::Command::Logout(Logout {}),
            Some(&account.token),
            Uuid::new_v4(),
        )
        .await
        .error(StatusCode::SERVICE_UNAVAILABLE);
    let private_key: Vec<u8> =
        sqlx::query_scalar("SELECT runtime_random_key FROM game_worlds WHERE world_id = $1")
            .bind(pack.world_id)
            .fetch_one(&database.pool)
            .await
            .unwrap();
    service.stop().await.unwrap();
    let stored = database.world(pack.world_id).await;
    let account_only = Live::start(Config::new(&database.url, "127.0.0.1:0", None).unwrap()).await;
    let account_view = account_only
        .endpoint
        .call(
            client_message::Command::CurrentAccount(CurrentAccount {}),
            Some(&account.token),
            Uuid::new_v4(),
        )
        .await;
    let Some(server_message::Result::Account(account_view)) = account_view.message.result else {
        panic!()
    };
    assert!(
        account_view.character_initialized,
        "account-only mode must not hide a persisted character"
    );
    account_only.stop().await.unwrap();
    let restarted = Live::start(pack.config(&database)).await;
    assert_eq!(
        database.world(pack.world_id).await,
        stored,
        "restart must not tick disconnected stored actors"
    );
    let rejoined = restarted.endpoint.join(&account).await;
    assert_eq!(rejoined.next_sequence, 2);
    assert!(
        rejoined.snapshot.as_ref().unwrap().events.is_empty(),
        "reconnect cannot replay obsolete notifications/audio"
    );
    let retry = restarted
        .endpoint
        .input(&account, &rejoined, 1, operation, practice())
        .await
        .action();
    assert!(retry.duplicate);
    assert_eq!(xp(retry.snapshot.as_ref().unwrap()), 50);
    assert!(retry.snapshot.as_ref().unwrap().events.is_empty());
    let gap = restarted
        .endpoint
        .poll(&account, &rejoined, 0)
        .await
        .snapshot();
    assert!(gap.full_snapshot);
    assert!(gap.event_history_gap);
    assert!(gap.event_history_floor_revision >= stored.state.revision);
    let restored_key: Vec<u8> =
        sqlx::query_scalar("SELECT runtime_random_key FROM game_worlds WHERE world_id = $1")
            .bind(pack.world_id)
            .fetch_one(&database.pool)
            .await
            .unwrap();
    assert_eq!(restored_key, private_key);
    restarted
        .endpoint
        .input(&account, &rejoined, 2, Uuid::new_v4(), mine())
        .await
        .action();
    let digest = crate::crypto::token_digest(&account.token).unwrap();
    crate::store::logout(&database.pool, &digest).await.unwrap();
    let after_revocation = database.world(pack.world_id).await;
    sleep(Duration::from_millis(800)).await;
    let after_wait = database.world(pack.world_id).await;
    assert_eq!(
        after_wait.state, after_revocation.state,
        "unbound presence cannot permit offline gathering or clear activity"
    );
    assert!(matches!(
        after_wait.state.characters[&ActorId::new(actor).unwrap()].activity,
        Activity::Gathering { .. }
    ));
    let health = restarted
        .endpoint
        .client
        .get(format!("{}/healthz", restarted.endpoint.origin))
        .send()
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::SERVICE_UNAVAILABLE);
    let hello = restarted
        .endpoint
        .call(
            client_message::Command::Hello(Hello {}),
            None,
            Uuid::new_v4(),
        )
        .await;
    let Some(server_message::Result::Hello(hello)) = hello.message.result else {
        panic!()
    };
    assert!(!hello.gameplay_available);
    assert!(hello.gameplay_unavailable_reason.contains("presence"));
    assert_eq!(
        restarted.stop().await.unwrap_err().kind(),
        "game_loop_failure"
    );
    database.pool.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn stalled_world_database_backpressures_the_queue_and_owned_shutdown_is_bounded() {
    let database = Database::reset().await;
    let pack = Pack::new();
    let service = Live::start(pack.config(&database)).await;
    let account = Arc::new(service.endpoint.account("bounded_queue").await);
    service.endpoint.create(&account).await;
    let joined = Arc::new(service.endpoint.join(&account).await);
    let mut blocker = database.pool.begin().await.unwrap();
    let before: i64 =
        sqlx::query_scalar("SELECT tick FROM game_worlds WHERE world_id = $1 FOR UPDATE")
            .bind(pack.world_id)
            .fetch_one(&mut *blocker)
            .await
            .unwrap();
    database.wait_world_lock().await;
    let started = Instant::now();
    let mut requests = JoinSet::new();
    for _ in 0..80 {
        let endpoint = service.endpoint.clone();
        let account = account.clone();
        let joined = joined.clone();
        requests.spawn(async move {
            endpoint
                .input(&account, &joined, 1, Uuid::new_v4(), practice())
                .await
        });
    }
    let mut overloaded = 0;
    while let Some(response) = requests.join_next().await {
        let response = response.unwrap();
        if response.status == StatusCode::TOO_MANY_REQUESTS {
            overloaded += 1;
            response.error(StatusCode::TOO_MANY_REQUESTS);
        } else {
            response.error(StatusCode::SERVICE_UNAVAILABLE);
        }
    }
    assert!(
        overloaded > 0,
        "the bounded channel must surface backpressure"
    );
    assert!(started.elapsed() < Duration::from_secs(11));
    blocker.rollback().await.unwrap();
    assert_eq!(
        database.world(pack.world_id).await.state.tick,
        before as u64
    );
    let commands: i64 = sqlx::query_scalar("SELECT count(*) FROM processed_game_commands")
        .fetch_one(&database.pool)
        .await
        .unwrap();
    assert_eq!(commands, 0);
    assert_eq!(
        service.stop().await.unwrap_err().kind(),
        "game_loop_failure"
    );
    let stopped = database.world(pack.world_id).await.state.tick;
    sleep(Duration::from_millis(700)).await;
    assert_eq!(database.world(pack.world_id).await.state.tick, stopped);
    database.pool.close().await;
}
