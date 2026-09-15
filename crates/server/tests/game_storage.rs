use std::{
    collections::BTreeMap,
    env, io,
    net::{IpAddr, SocketAddr},
    str::FromStr,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use clubscape_game_types::{
    ActorId, Bank, EntityState, EvidenceStatus, GAME_SCHEMA_VERSION, GameError, GameErrorCode,
    GameEvent, GameIntent, GameResult, InitialStateDefinition, Inventory, ItemId, ItemStack,
    MAX_RUN_ENERGY, Quantity, RegionId, SkillId, SkillState, SourceRecord, SpawnId, StageId, Tile,
    WorldState,
};
use clubscape_protocol::{
    ClientMessage, ErrorCode, LoggedIn, Login, Logout, MEDIA_TYPE, PROTOCOL_VERSION, Register,
    ServerMessage, client_message, server_message,
};
use clubscape_server::{
    Config, ServeError, Service,
    game_storage::{
        AuthTokenDigest, CharacterSnapshot, GameCommand, GameSession, GameStorageError, GameStore,
        MAX_SESSION_LEASE, MAX_WORLD_LEASE, SessionAccess, SourceCharacter, WorldLease,
    },
};
use futures_util::future::join_all;
use prost::Message;
use reqwest::{Client, StatusCode};
use sha2::{Digest, Sha256};
use sqlx::{
    ConnectOptions, PgPool,
    postgres::{PgConnectOptions, PgPoolOptions, PgSslMode},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{Mutex, MutexGuard, oneshot, watch},
    task::{JoinHandle, JoinSet},
    time::{Instant, sleep, timeout},
};
use uuid::Uuid;

#[path = "../../content/tests/common/mod.rs"]
mod engine_content;

const TEST_DATABASE: &str = "clubscape_m1_test";
const FIXTURE_REVISION: &str = "synthetic-storage-fixture-v2-not-gameplay-evidence";
const WAIT: Duration = Duration::from_secs(15);
static DATABASE_LOCK: Mutex<()> = Mutex::const_new(());

fn isolated_options(url: Option<&str>) -> Result<PgConnectOptions, &'static str> {
    let url = url.ok_or("CLUBSCAPE_TEST_DATABASE_URL is required; run just test-integration")?;
    let options =
        PgConnectOptions::from_str(url).map_err(|_| "invalid isolated test configuration")?;
    if options.get_database() != Some(TEST_DATABASE) {
        return Err("refusing to touch a database not named clubscape_m1_test");
    }
    let host = options.get_host();
    let host = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    let ip: IpAddr = host
        .parse()
        .map_err(|_| "a literal loopback IP is required")?;
    if !ip.is_loopback() {
        return Err("refusing a non-loopback test database");
    }
    Ok(options.host(&ip.to_string()))
}

#[test]
fn database_guard_rejects_absent_wrong_and_non_loopback_configuration() {
    for url in [
        None,
        Some("not a PostgreSQL URL"),
        Some("postgres://user:secret@127.0.0.1/production"),
        Some("postgres://user:secret@127.0.0.1/"),
        Some("postgres://user:secret@localhost/clubscape_m1_test"),
        Some("postgres://user:secret@192.0.2.1/clubscape_m1_test"),
        Some("postgres://user:secret@example.com/clubscape_m1_test"),
    ] {
        assert!(isolated_options(url).is_err());
    }
    for url in [
        "postgres://user:secret@127.0.0.1/clubscape_m1_test",
        "postgres://user:secret@[::1]/clubscape_m1_test",
    ] {
        assert!(isolated_options(Some(url)).is_ok());
    }
}

#[test]
fn authentication_digests_reuse_account_validation_and_are_redacted() {
    assert!(AuthTokenDigest::from_token("not a token").is_err());
    let auth = AuthTokenDigest::from_digest([0x42; 32]);
    assert_eq!(format!("{auth:?}"), "AuthTokenDigest([redacted])");
}

async fn open_pool(options: PgConnectOptions, maximum: u32) -> PgPool {
    timeout(
        WAIT,
        PgPoolOptions::new()
            .min_connections(0)
            .max_connections(maximum)
            .idle_timeout(None)
            .max_lifetime(None)
            .acquire_timeout(Duration::from_secs(5))
            .after_connect(|connection, _| {
                Box::pin(async move {
                    // Longer than the library's client deadline, to independently test that bound.
                    sqlx::query("SET statement_timeout = '20s'")
                        .execute(connection)
                        .await?;
                    Ok(())
                })
            })
            .connect_with(
                options
                    .disable_statement_logging()
                    .application_name("clubscape-game-storage-tests"),
            ),
    )
    .await
    .expect("bounded isolated PostgreSQL connection")
    .unwrap_or_else(|_| panic!("isolated PostgreSQL connection failed"))
}

struct TestDatabase {
    options: PgConnectOptions,
    pool: PgPool,
    store: GameStore,
    service: TestService,
    world_id: Uuid,
    lease: WorldLease,
    _guard: MutexGuard<'static, ()>,
}

impl TestDatabase {
    async fn reset() -> Self {
        let guard = DATABASE_LOCK.lock().await;
        let url = env::var("CLUBSCAPE_TEST_DATABASE_URL").ok();
        let options =
            isolated_options(url.as_deref()).expect("isolated PostgreSQL test safety guard");
        let pool = open_pool(options.clone(), 8).await;
        let (database, schema): (String, String) =
            sqlx::query_as("SELECT current_database(), current_schema()")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(database, TEST_DATABASE);
        assert_eq!(schema, "public");
        sqlx::raw_sql(
            "DROP TABLE IF EXISTS public.game_lifecycle_commands;
             DROP TABLE IF EXISTS public.processed_game_commands;
             DROP TABLE IF EXISTS public.game_sessions;
             DROP TABLE IF EXISTS public.game_characters;
             DROP TABLE IF EXISTS public.game_worlds;
             DROP TABLE IF EXISTS public.account_sessions;
             DROP TABLE IF EXISTS public.accounts;
             DROP TABLE IF EXISTS public._sqlx_migrations;",
        )
        .execute(&pool)
        .await
        .expect("reset only owned isolated service tables, in FK order");
        let store = GameStore::new(pool.clone());
        store.migrate().await.unwrap();
        let service = TestService::start(url.as_deref().unwrap()).await;
        let world_id = Uuid::new_v4();
        store
            .initialize_world(world_id, world_fixture())
            .await
            .unwrap();
        let lease = store
            .acquire_world_lease(world_id, MAX_WORLD_LEASE)
            .await
            .unwrap();
        Self {
            options,
            pool,
            store,
            service,
            world_id,
            lease,
            _guard: guard,
        }
    }

    async fn account(&self, name: &str) -> TestAccount {
        let password = format!("Synthetic-fixture-password-{}!", Uuid::new_v4());
        let registered = self
            .service
            .call(
                client_message::Command::Register(Register {
                    login_name: name.to_owned(),
                    password: password.clone(),
                }),
                None,
            )
            .await;
        assert_eq!(registered.0, StatusCode::OK);
        let Some(server_message::Result::Registered(result)) = registered.1.result else {
            panic!("registration must create an actual account");
        };
        let account_id = Uuid::parse_str(&result.account.unwrap().account_id).unwrap();
        let character_count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM game_characters WHERE account_id = $1")
                .bind(account_id)
                .fetch_one(&self.pool)
                .await
                .unwrap();
        assert_eq!(
            character_count, 0,
            "registration must not create a character"
        );
        let logged_in = self.service.login(name, &password).await;
        assert_eq!(
            logged_in.account.as_ref().unwrap().account_id,
            account_id.to_string()
        );
        TestAccount {
            name: name.to_owned(),
            password,
            account_id,
            logged_in,
        }
    }

    async fn player(&self, name: &str) -> Player {
        let account = self.account(name).await;
        let authentication = account.authentication();
        let character = self
            .store
            .create_character(&self.lease, authentication, character_fixture())
            .await
            .unwrap();
        let session = self
            .store
            .join_session(
                self.world_id,
                character.state.actor_id.clone(),
                authentication,
                MAX_SESSION_LEASE,
            )
            .await
            .unwrap();
        Player {
            account,
            character,
            session,
        }
    }

    async fn journal_count(&self) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM processed_game_commands")
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn expire_game_session(&self, account_id: Uuid) {
        let changed = sqlx::query(
            "UPDATE game_sessions SET created_at = clock_timestamp() - INTERVAL '3 minutes',
                 heartbeat_at = clock_timestamp() - INTERVAL '2 minutes',
                 expires_at = clock_timestamp() - INTERVAL '1 minute' WHERE account_id = $1",
        )
        .bind(account_id)
        .execute(&self.pool)
        .await
        .unwrap();
        assert_eq!(changed.rows_affected(), 1);
    }

    async fn expire_authentication(&self, token: &str) {
        let changed = sqlx::query(
            "UPDATE account_sessions SET created_at = clock_timestamp() - INTERVAL '31 minutes',
                 expires_at = clock_timestamp() - INTERVAL '1 minute' WHERE token_digest = $1",
        )
        .bind(Sha256::digest(token.as_bytes()).as_slice())
        .execute(&self.pool)
        .await
        .unwrap();
        assert_eq!(changed.rows_affected(), 1);
    }

    async fn stop(self) {
        timeout(Duration::from_secs(3), self.store.close())
            .await
            .unwrap()
            .unwrap();
        self.service.stop().await;
    }
}

struct TestService {
    client: Client,
    origin: String,
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), ServeError>>>,
}

impl TestService {
    async fn start(url: &str) -> Self {
        let service = timeout(
            WAIT,
            Service::bind(Config::new(url, "127.0.0.1:0", None).unwrap()),
        )
        .await
        .unwrap()
        .unwrap();
        let origin = format!("http://{}", service.local_addr());
        let client = Client::builder().no_proxy().timeout(WAIT).build().unwrap();
        let (shutdown, receive) = oneshot::channel();
        let task = tokio::spawn(service.serve(async move {
            let _ = receive.await;
        }));
        assert_eq!(
            client
                .get(format!("{origin}/healthz"))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        Self {
            client,
            origin,
            shutdown: Some(shutdown),
            task: Some(task),
        }
    }

    async fn call(
        &self,
        command: client_message::Command,
        token: Option<&str>,
    ) -> (StatusCode, ServerMessage) {
        let message = ClientMessage {
            protocol_version: PROTOCOL_VERSION,
            request_id: Uuid::new_v4().to_string(),
            command: Some(command),
        };
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
        let status = response.status();
        let reply = ServerMessage::decode(response.bytes().await.unwrap()).unwrap();
        assert_eq!(reply.request_id, message.request_id);
        assert_eq!(reply.protocol_version, PROTOCOL_VERSION);
        (status, reply)
    }

    async fn login(&self, name: &str, password: &str) -> LoggedIn {
        let (status, reply) = self
            .call(
                client_message::Command::Login(Login {
                    login_name: name.to_owned(),
                    password: password.to_owned(),
                }),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        let Some(server_message::Result::LoggedIn(logged_in)) = reply.result else {
            panic!("an actual account-service session is required");
        };
        logged_in
    }

    async fn logout(&self, token: &str) {
        let (status, reply) = self
            .call(client_message::Command::Logout(Logout {}), Some(token))
            .await;
        assert_eq!(status, StatusCode::OK);
        assert!(matches!(
            reply.result,
            Some(server_message::Result::LoggedOut(_))
        ));
    }

    async fn stop(mut self) {
        self.shutdown.take().unwrap().send(()).unwrap();
        timeout(WAIT, self.task.as_mut().unwrap())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
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

struct TestAccount {
    name: String,
    password: String,
    account_id: Uuid,
    logged_in: LoggedIn,
}

impl TestAccount {
    fn authentication(&self) -> AuthTokenDigest {
        AuthTokenDigest::from_token(&self.logged_in.session_token).unwrap()
    }
}

struct Player {
    account: TestAccount,
    character: CharacterSnapshot,
    session: GameSession,
}

impl Player {
    fn access(&self) -> SessionAccess {
        self.session.access(self.account.authentication())
    }
}

fn world_fixture() -> WorldState {
    WorldState {
        schema_version: GAME_SCHEMA_VERSION,
        content_revision: FIXTURE_REVISION.to_owned(),
        tick: 0,
        revision: 0,
        characters: BTreeMap::new(),
        entities: BTreeMap::from([(
            SpawnId::new("spawn.fixture.rock").unwrap(),
            EntityState {
                tile: Tile::new(1, 1, 0).unwrap(),
                hitpoints: 1,
                available_at_tick: 0,
                flags: BTreeMap::new(),
                runtime: clubscape_game_types::EntityRuntime::default(),
            },
        )]),
        shops: BTreeMap::new(),
        ground_items: Vec::new(),
        runtime: clubscape_game_types::WorldRuntime::default(),
    }
}

fn character_fixture() -> SourceCharacter {
    let mut inventory = Inventory::default();
    inventory.slots[0] = Some(ItemStack {
        item: ItemId::new("item.fixture.token").unwrap(),
        quantity: Quantity::new(3).unwrap(),
        instance: None,
    });
    SourceCharacter {
        content_revision: FIXTURE_REVISION.to_owned(),
        appearance: BTreeMap::from([("synthetic_colour".to_owned(), 7)]),
        initial_state: InitialStateDefinition {
            region: RegionId::new("region.fixture").unwrap(),
            tile: Tile::new(1, 2, 0).unwrap(),
            inventory,
            equipment: BTreeMap::new(),
            bank: Bank {
                capacity: 8,
                slots: vec![None; 8],
            },
            skills: BTreeMap::from([(
                SkillId::new("skill.fixture.gathering").unwrap(),
                SkillState {
                    xp_tenths: 123,
                    current_level: 1,
                },
            )]),
            hitpoints: 5,
            prayer_points: 2,
            run_energy: 9_877,
            tutorial_stage: StageId::new("stage.fixture.initial").unwrap(),
            quest_points: 0,
            quests: BTreeMap::new(),
            flags: BTreeMap::from([("synthetic_source_marker".to_owned(), 9)]),
            interfaces: Vec::new(),
            runtime: clubscape_game_types::InitialRuntimeDefinition::default(),
            source: vec![SourceRecord {
                reference: "crates/server/tests/game_storage.rs".to_owned(),
                revision: FIXTURE_REVISION.to_owned(),
                status: EvidenceStatus::TestFixture,
                notes: "Synthetic storage fixture only; not source content or gameplay acceptance."
                    .to_owned(),
            }],
        },
    }
}

fn command(sequence: u64) -> GameCommand {
    GameCommand {
        operation_id: Uuid::new_v4(),
        sequence,
        intent: GameIntent::Interact {
            target: SpawnId::new("spawn.fixture.rock").unwrap(),
            action: "synthetic_gain".to_owned(),
        },
    }
}

fn synthetic_gain(
    world: &mut WorldState,
    actor: &ActorId,
    _intent: &GameIntent,
) -> GameResult<Vec<GameEvent>> {
    let character = world.characters.get_mut(actor).unwrap();
    let stack = character.inventory.slots[0].as_mut().unwrap();
    stack.quantity = Quantity::new(stack.quantity.get() + 1)?;
    let skill = SkillId::new("skill.fixture.gathering")?;
    character.skills.get_mut(&skill).unwrap().xp_tenths += 10;
    *character
        .flags
        .entry("synthetic_commits".to_owned())
        .or_default() += 1;
    character.last_action_tick = world.tick;
    let target = SpawnId::new("spawn.fixture.rock")?;
    world.entities.get_mut(&target).unwrap().available_at_tick += 1;
    Ok(vec![
        GameEvent::Gathered {
            target,
            stack: ItemStack {
                item: stack.item.clone(),
                quantity: Quantity::new(1)?,
                instance: None,
            },
        },
        GameEvent::XpGained {
            skill,
            amount_tenths: 10,
        },
    ])
}

fn assert_error(error: GameStorageError, status: StatusCode, code: ErrorCode) {
    assert_eq!(error.status, status, "{error}");
    assert_eq!(error.code, code);
    assert!(!error.error_id.is_nil());
    assert!(!error.message.is_empty());
}

fn assert_conflict(error: GameStorageError) {
    assert_error(error, StatusCode::CONFLICT, ErrorCode::Conflict);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn engine_processes_store_advanced_ticks_once_with_rollback_and_receipt_replay() {
    use clubscape_world_engine::{LifecycleTransition, RandomSource, WorldEngine};

    struct Draw(u32);
    impl RandomSource for Draw {
        fn draw_below(&mut self, _: u32) -> GameResult<u32> {
            Ok(self.0)
        }
    }

    let database = TestDatabase::reset().await;
    let mut content = engine_content::fixture();
    content.initial_state.tile = engine_content::tile(1002, 1003);
    for stage in content.tutorial.values_mut() {
        stage.allowed_actions = vec!["*".into()];
    }
    let source = SourceCharacter {
        content_revision: content.revision.clone(),
        initial_state: content.initial_state.clone(),
        appearance: BTreeMap::new(),
    };
    let engine = Arc::new(WorldEngine::new(Arc::new(content)).unwrap());
    let world_id = Uuid::new_v4();
    database
        .store
        .initialize_world(world_id, engine.initial_world().unwrap())
        .await
        .unwrap();
    let lease = database
        .store
        .acquire_world_lease(world_id, MAX_WORLD_LEASE)
        .await
        .unwrap();
    let account = database.account("tick_engine").await;
    let character = database
        .store
        .create_character(&lease, account.authentication(), source)
        .await
        .unwrap();
    let actor = character.state.actor_id.clone();
    let session = database
        .store
        .join_session(
            world_id,
            actor.clone(),
            account.authentication(),
            MAX_SESSION_LEASE,
        )
        .await
        .unwrap();
    let action_engine = engine.clone();
    database
        .store
        .commit_command(
            &lease,
            &session.access(account.authentication()),
            GameCommand {
                operation_id: Uuid::new_v4(),
                sequence: 1,
                intent: GameIntent::Interact {
                    target: engine_content::id("spawn.test.rock"),
                    action: "Mine".into(),
                },
            },
            move |world, actor, intent| {
                let mut events =
                    action_engine.apply_lifecycle(world, actor, LifecycleTransition::Join)?;
                events.extend(action_engine.apply_intent(world, actor, intent, &mut Draw(0))?);
                Ok(events.into_iter().map(|event| event.event).collect())
            },
        )
        .await
        .unwrap();
    let started = database.store.load_world(world_id).await.unwrap();
    assert_eq!(started.state.tick, 0);
    assert!(matches!(
        started.state.characters[&actor].activity,
        clubscape_game_types::Activity::Gathering { next_tick: 2, .. }
    ));
    let tick_engine = engine.clone();
    let first = database
        .store
        .commit_tick(&lease, 0, move |world| {
            assert_eq!(world.tick, 1);
            tick_engine
                .process_advanced_tick(world, &mut Draw(0))
                .map(|events| events.into_iter().map(|event| event.event).collect())
        })
        .await
        .unwrap();
    assert_eq!(first.receipt.tick, 1);
    assert_eq!(first.receipt.world_revision, started.state.revision + 1);
    assert!(!first.duplicate);
    assert!(first.receipt.events.is_empty());
    let before_attempt = database.store.load_world(world_id).await.unwrap();
    assert_eq!(
        before_attempt.state.characters[&actor].skills[&engine_content::id("skill.test.mining")]
            .xp_tenths,
        0
    );
    let invalid_engine = engine.clone();
    let failure = database
        .store
        .commit_tick(&lease, 1, move |world| {
            assert_eq!(world.tick, 2);
            invalid_engine
                .process_advanced_tick(world, &mut Draw(u32::MAX))
                .map(|events| events.into_iter().map(|event| event.event).collect())
        })
        .await
        .unwrap_err();
    assert_error(failure, StatusCode::BAD_REQUEST, ErrorCode::InvalidArgument);
    assert_eq!(
        database.store.load_world(world_id).await.unwrap(),
        before_attempt
    );
    let second = database
        .store
        .commit_tick(&lease, 1, move |world| {
            assert_eq!(world.tick, 2);
            engine
                .process_advanced_tick(world, &mut Draw(0))
                .map(|events| events.into_iter().map(|event| event.event).collect())
        })
        .await
        .unwrap();
    assert_eq!(second.receipt.tick, 2);
    assert_eq!(
        second.receipt.world_revision,
        first.receipt.world_revision + 1
    );
    assert_eq!(
        second
            .receipt
            .events
            .iter()
            .filter(|event| matches!(event, GameEvent::Gathered { .. }))
            .count(),
        1
    );
    let completed = database.store.load_world(world_id).await.unwrap();
    assert_eq!(completed.state.tick, 2);
    assert_eq!(completed.state.characters[&actor].last_command_sequence, 1);
    assert_eq!(
        completed.state.characters[&actor]
            .runtime
            .played_time
            .as_ref()
            .unwrap()
            .ticks,
        2,
    );
    assert_eq!(
        completed.state.characters[&actor].skills[&engine_content::id("skill.test.mining")]
            .xp_tenths,
        100
    );
    assert_eq!(
        completed.state.entities[&engine_content::id("spawn.test.rock")].available_at_tick,
        6
    );
    let duplicate = database
        .store
        .commit_tick(&lease, 1, |_| {
            panic!("stored tick replay must not process engine work again")
        })
        .await
        .unwrap();
    assert!(duplicate.duplicate);
    assert_eq!(duplicate.receipt, second.receipt);
    assert_eq!(
        database.store.load_world(world_id).await.unwrap(),
        completed
    );
    database.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn typed_runtime_ledgers_and_deadlines_persist_without_replay_or_acknowledged_state_loss() {
    use clubscape_game_types::{
        CounterId, CounterValue, EntitlementId, EntitlementState, FractionalAccumulator,
        GroundClock, GroundItem, GroundPolicyId, GroundProducer, GroundProvenance, PlayedTime,
    };

    let database = TestDatabase::reset().await;
    let player = database.player("typed_runtime").await;
    let operation = command(1);
    let committed = database
        .store
        .commit_command(
            &database.lease,
            &player.access(),
            operation.clone(),
            |world, actor, _| {
                // Synthetic persistence data, not execution of mill/prayer/grant mechanics.
                let character = world.characters.get_mut(actor).unwrap();
                character.runtime.counters.insert(
                    CounterId::new("counter.fixture.flour")?,
                    CounterValue::Integer(30),
                );
                character.runtime.food_ready = 17;
                character.runtime.combat.attack_ready = 19;
                character.runtime.combat.prayer_drain = Some(FractionalAccumulator {
                    numerator: 1,
                    denominator: 60,
                });
                character.runtime.played_time = Some(PlayedTime {
                    ticks: 119999,
                    through_world_tick: Some(world.tick),
                });
                character.runtime.entitlements.insert(
                    EntitlementId::new("entitlement.fixture.reward")?,
                    EntitlementState::Claimed {
                        at_tick: world.tick,
                    },
                );
                let dropped_tile = character.tile;
                world.runtime.next_ground_id = 1;
                world.runtime.ground_provenance.insert(
                    "ground.engine.1".into(),
                    GroundProvenance {
                        policy: GroundPolicyId::new("ground_policy.fixture.owner")?,
                        producer: GroundProducer::PlayerDrop {
                            actor: actor.clone(),
                            at_tick: world.tick,
                        },
                        clock: Some(GroundClock::OwnerOnlineTicks),
                    },
                );
                world.ground_items.push(GroundItem {
                    id: "ground.engine.1".into(),
                    tile: dropped_tile,
                    stack: ItemStack {
                        item: ItemId::new("item.fixture.dropped_coins")?,
                        quantity: Quantity::new(25)?,
                        instance: None,
                    },
                    owner: Some(actor.clone()),
                    public_at_tick: u64::MAX,
                    expires_at_tick: 301,
                    instance: None,
                });
                Ok(vec![GameEvent::CounterChanged {
                    counter: CounterId::new("counter.fixture.flour")?,
                    value: CounterValue::Integer(30),
                }])
            },
        )
        .await
        .unwrap();
    assert_eq!(
        committed.receipt.character.inventory,
        player.character.state.inventory
    );
    assert_eq!(
        committed.receipt.character.skills,
        player.character.state.skills
    );
    assert_eq!(
        committed.receipt.character.quests,
        player.character.state.quests
    );
    let reader = GameStore::new(database.pool.clone());
    let restored = reader
        .load_character(database.world_id, player.account.authentication())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(restored.state, committed.receipt.character);
    assert_eq!(restored.state.runtime.food_ready, 17);
    assert_eq!(restored.state.runtime.combat.attack_ready, 19);
    assert_eq!(
        restored.state.runtime.played_time.as_ref().unwrap().ticks,
        119999
    );
    let persisted_world = reader.load_world(database.world_id).await.unwrap().state;
    assert_eq!(
        persisted_world.runtime.ground_provenance["ground.engine.1"].clock,
        Some(GroundClock::OwnerOnlineTicks),
    );
    assert_eq!(persisted_world.ground_items[0].expires_at_tick, 301);
    assert_eq!(persisted_world.ground_items[0].stack.quantity.get(), 25);
    assert_eq!(
        persisted_world.ground_items[0].owner,
        Some(restored.state.actor_id.clone())
    );
    let duplicate = database
        .store
        .commit_command(&database.lease, &player.access(), operation, |_, _, _| {
            panic!("typed runtime replay must return its durable receipt")
        })
        .await
        .unwrap();
    assert!(duplicate.duplicate);
    assert_eq!(duplicate.receipt, committed.receipt);
    let before = database.store.load_world(database.world_id).await.unwrap();
    for invalid_change in ["entitlements", "prayer", "playtime"] {
        let failure = database
            .store
            .commit_command(
                &database.lease,
                &player.access(),
                command(2),
                move |world, actor, _| {
                    let runtime = &mut world.characters.get_mut(actor).unwrap().runtime;
                    match invalid_change {
                        "entitlements" => runtime.entitlements.clear(),
                        "prayer" => {
                            runtime.combat.prayer_drain = Some(FractionalAccumulator {
                                numerator: 1,
                                denominator: 0,
                            })
                        }
                        "playtime" => runtime.played_time = None,
                        _ => unreachable!(),
                    }
                    Ok(Vec::new())
                },
            )
            .await
            .unwrap_err();
        assert_error(
            failure,
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::Internal,
        );
        assert_eq!(
            database.store.load_world(database.world_id).await.unwrap(),
            before
        );
    }
    assert_eq!(database.journal_count().await, 1);
    database.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn legacy_jsonb_runtime_defaults_preserve_pending_activity_inventory_xp_and_source_flags() {
    let database = TestDatabase::reset().await;
    let player = database.player("legacy_runtime").await;
    let committed = database
        .store
        .commit_command(
            &database.lease,
            &player.access(),
            command(1),
            |world, actor, intent| {
                let events = synthetic_gain(world, actor, intent)?;
                let character = world.characters.get_mut(actor).unwrap();
                character
                    .flags
                    .insert("__world_engine.command_seen".into(), 1);
                character
                    .flags
                    .insert("__world_engine.food_ready".into(), 11);
                character
                    .flags
                    .insert("__world_engine.attack_ready".into(), 13);
                character.activity = clubscape_game_types::Activity::Producing {
                    recipe: clubscape_game_types::RecipeId::new("recipe.fixture.pending")?,
                    target: None,
                    remaining: 3,
                    next_tick: 40,
                };
                Ok(events)
            },
        )
        .await
        .unwrap();
    let path = vec![
        "characters".to_owned(),
        player.character.state.actor_id.to_string(),
        "runtime".to_owned(),
    ];
    sqlx::query(
        "UPDATE game_worlds SET state = (state - 'runtime') #- $2::text[] WHERE world_id = $1",
    )
    .bind(database.world_id)
    .bind(path)
    .execute(&database.pool)
    .await
    .unwrap();
    let restored = database
        .store
        .load_character(database.world_id, player.account.authentication())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        restored.state.inventory,
        committed.receipt.character.inventory
    );
    assert_eq!(restored.state.skills, committed.receipt.character.skills);
    assert_eq!(restored.state.flags, committed.receipt.character.flags);
    assert_eq!(
        restored.state.activity,
        committed.receipt.character.activity
    );
    assert_eq!(restored.state.last_command_sequence, 1);
    assert!(matches!(
        restored.state.runtime.engine,
        clubscape_game_types::EngineMetadata::Legacy
    ));
    assert!(matches!(
        restored.state.runtime.life,
        clubscape_game_types::LifeState::Legacy
    ));
    assert_eq!(restored.state.runtime.played_time, None);
    assert_eq!(database.journal_count().await, 1);
    database.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn world_initialization_and_source_character_creation_are_atomic_and_never_overwrite() {
    let database = TestDatabase::reset().await;
    let migration_count: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
        .fetch_one(&database.pool)
        .await
        .unwrap();
    assert_eq!(migration_count, 4);
    let initial = database.store.load_world(database.world_id).await.unwrap();
    let copies = join_all((0..8).map(|_| {
        database
            .store
            .initialize_world(database.world_id, world_fixture())
    }))
    .await;
    assert!(copies.into_iter().all(|copy| copy.unwrap() == initial));
    let mut wrong_revision = world_fixture();
    wrong_revision.content_revision = "different-pack".to_owned();
    assert_conflict(
        database
            .store
            .initialize_world(database.world_id, wrong_revision)
            .await
            .unwrap_err(),
    );

    let account = database.account("create_race").await;
    assert!(
        database
            .store
            .load_character(database.world_id, account.authentication())
            .await
            .unwrap()
            .is_none()
    );
    let mut no_source = character_fixture();
    no_source.initial_state.source.clear();
    assert_error(
        database
            .store
            .create_character(&database.lease, account.authentication(), no_source)
            .await
            .unwrap_err(),
        StatusCode::BAD_REQUEST,
        ErrorCode::InvalidArgument,
    );
    let created = join_all((0..8).map(|_| {
        database.store.create_character(
            &database.lease,
            account.authentication(),
            character_fixture(),
        )
    }))
    .await;
    let first = created[0].as_ref().unwrap();
    assert!(
        created
            .iter()
            .all(|created| created.as_ref().unwrap() == first)
    );
    assert_eq!(first.revision, 1);
    assert_eq!(first.last_sequence, 0);
    let definition = character_fixture().initial_state;
    assert_eq!(first.state.inventory, definition.inventory);
    assert_eq!(first.state.skills, definition.skills);
    assert_eq!(first.state.bank, definition.bank);
    assert_eq!(first.state.tile, definition.tile);
    assert_eq!(first.state.hitpoints, definition.hitpoints);
    assert_eq!(first.state.prayer_points, definition.prayer_points);
    assert_eq!(first.state.run_energy, definition.run_energy);
    assert_eq!(first.state.tutorial_stage, definition.tutorial_stage);
    assert_eq!(first.state.flags, definition.flags);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM game_characters")
        .fetch_one(&database.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let mut changed_initial = character_fixture();
    changed_initial.initial_state.inventory = Inventory::default();
    changed_initial.initial_state.skills.clear();
    assert_eq!(
        database
            .store
            .create_character(&database.lease, account.authentication(), changed_initial)
            .await
            .unwrap(),
        *first,
    );
    let reinitialized = database
        .store
        .initialize_world(database.world_id, world_fixture())
        .await
        .unwrap();
    assert_eq!(reinitialized.state.revision, 1);
    assert_eq!(reinitialized.state.characters.len(), 1);

    let other_world = Uuid::new_v4();
    database
        .store
        .initialize_world(other_world, world_fixture())
        .await
        .unwrap();
    let other_lease = database
        .store
        .acquire_world_lease(other_world, MAX_WORLD_LEASE)
        .await
        .unwrap();
    assert_conflict(
        database
            .store
            .create_character(&other_lease, account.authentication(), character_fixture())
            .await
            .unwrap_err(),
    );
    let second = database.account("cross_world_race").await;
    let (left, right) = tokio::join!(
        database.store.create_character(
            &database.lease,
            second.authentication(),
            character_fixture()
        ),
        database
            .store
            .create_character(&other_lease, second.authentication(), character_fixture()),
    );
    assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
    assert_conflict(left.err().or(right.err()).unwrap());
    database.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn exclusive_session_reconnect_heartbeat_leave_and_expiry_takeover_use_real_tokens() {
    let database = TestDatabase::reset().await;
    let player = database.player("session_owner").await;
    let auth = player.account.authentication();
    let other_login = database
        .service
        .login(&player.account.name, &player.account.password)
        .await;
    let other_auth = AuthTokenDigest::from_token(&other_login.session_token).unwrap();
    let actor = player.character.state.actor_id.clone();
    let reconnect = database
        .store
        .join_session(database.world_id, actor.clone(), auth, MAX_SESSION_LEASE)
        .await
        .unwrap();
    assert_eq!(reconnect.session_id, player.session.session_id);
    assert!(reconnect.heartbeat_at_unix_ms >= player.session.heartbeat_at_unix_ms);
    assert_conflict(
        database
            .store
            .join_session(
                database.world_id,
                actor.clone(),
                other_auth,
                MAX_SESSION_LEASE,
            )
            .await
            .unwrap_err(),
    );
    let heartbeat = database
        .store
        .heartbeat_session(&player.access(), MAX_SESSION_LEASE)
        .await
        .unwrap();
    assert_eq!(heartbeat.session_id, player.session.session_id);
    assert_eq!(
        database.store.read_session(&player.access()).await.unwrap(),
        heartbeat
    );
    assert!(
        database
            .store
            .leave_session(&player.access())
            .await
            .unwrap()
    );
    assert!(
        !database
            .store
            .leave_session(&player.access())
            .await
            .unwrap()
    );
    let replacement = database
        .store
        .join_session(
            database.world_id,
            actor.clone(),
            other_auth,
            MAX_SESSION_LEASE,
        )
        .await
        .unwrap();
    assert_ne!(replacement.session_id, player.session.session_id);
    assert_conflict(
        database
            .store
            .read_session(&player.access())
            .await
            .unwrap_err(),
    );
    assert_conflict(
        database
            .store
            .leave_session(&player.access())
            .await
            .unwrap_err(),
    );

    database
        .expire_game_session(player.account.account_id)
        .await;
    assert_conflict(
        database
            .store
            .heartbeat_session(&replacement.access(other_auth), MAX_SESSION_LEASE)
            .await
            .unwrap_err(),
    );
    let takeover = database
        .store
        .join_session(database.world_id, actor, auth, MAX_SESSION_LEASE)
        .await
        .unwrap();
    assert_ne!(takeover.session_id, replacement.session_id);
    let shortened_auth_expiry: i64 = sqlx::query_scalar(
        "UPDATE account_sessions SET expires_at = clock_timestamp() + INTERVAL '20 seconds'
         WHERE token_digest = $1 RETURNING floor(extract(epoch FROM expires_at) * 1000)::bigint",
    )
    .bind(Sha256::digest(player.account.logged_in.session_token.as_bytes()).as_slice())
    .fetch_one(&database.pool)
    .await
    .unwrap();
    let heartbeat = database
        .store
        .heartbeat_session(&takeover.access(auth), MAX_SESSION_LEASE)
        .await
        .unwrap();
    assert_eq!(heartbeat.expires_at_unix_ms, shortened_auth_expiry);
    let digest: Vec<u8> =
        sqlx::query_scalar("SELECT token_digest FROM game_sessions WHERE session_id = $1")
            .bind(takeover.session_id)
            .fetch_one(&database.pool)
            .await
            .unwrap();
    assert_eq!(
        digest,
        Sha256::digest(player.account.logged_in.session_token.as_bytes()).to_vec()
    );
    assert_eq!(database.journal_count().await, 0);
    database.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn account_logout_expiry_and_pruning_revoke_game_access_and_allow_new_owners() {
    let database = TestDatabase::reset().await;
    let player = database.player("revoked_owner").await;
    let login = database
        .service
        .login(&player.account.name, &player.account.password)
        .await;
    let auth = AuthTokenDigest::from_token(&login.session_token).unwrap();
    database
        .service
        .logout(&player.account.logged_in.session_token)
        .await;
    for error in [
        database
            .store
            .read_session(&player.access())
            .await
            .unwrap_err(),
        database
            .store
            .leave_session(&player.access())
            .await
            .unwrap_err(),
        database
            .store
            .load_character(database.world_id, player.account.authentication())
            .await
            .unwrap_err(),
        database
            .store
            .commit_command(
                &database.lease,
                &player.access(),
                command(1),
                synthetic_gain,
            )
            .await
            .unwrap_err(),
    ] {
        assert_error(error, StatusCode::UNAUTHORIZED, ErrorCode::Unauthenticated);
    }
    assert_eq!(database.journal_count().await, 0);
    let session = database
        .store
        .join_session(
            database.world_id,
            player.character.state.actor_id.clone(),
            auth,
            MAX_SESSION_LEASE,
        )
        .await
        .unwrap();
    assert_ne!(session.session_id, player.session.session_id);
    let live = database
        .service
        .login(&player.account.name, &player.account.password)
        .await;
    let live_auth = AuthTokenDigest::from_token(&live.session_token).unwrap();
    database.expire_authentication(&login.session_token).await;
    assert_error(
        database
            .store
            .read_session(&session.access(auth))
            .await
            .unwrap_err(),
        StatusCode::UNAUTHORIZED,
        ErrorCode::Unauthenticated,
    );
    let active = database
        .store
        .join_session(
            database.world_id,
            player.character.state.actor_id.clone(),
            live_auth,
            MAX_SESSION_LEASE,
        )
        .await
        .unwrap();
    let mut newest = live;
    for _ in 0..5 {
        newest = database
            .service
            .login(&player.account.name, &player.account.password)
            .await;
    }
    assert_error(
        database
            .store
            .read_session(&active.access(live_auth))
            .await
            .unwrap_err(),
        StatusCode::UNAUTHORIZED,
        ErrorCode::Unauthenticated,
    );
    let newest_auth = AuthTokenDigest::from_token(&newest.session_token).unwrap();
    database
        .store
        .join_session(
            database.world_id,
            player.character.state.actor_id,
            newest_auth,
            MAX_SESSION_LEASE,
        )
        .await
        .unwrap();
    database.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn authentication_revoked_while_waiting_for_a_world_lock_is_rechecked_inside_the_transaction()
{
    let database = TestDatabase::reset().await;
    let player = database.player("waiting_auth").await;
    let before = database.store.load_world(database.world_id).await.unwrap();
    let mut blocker = database.pool.begin().await.unwrap();
    sqlx::query("SELECT world_id FROM game_worlds WHERE world_id = $1 FOR UPDATE")
        .bind(database.world_id)
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    let store = database.store.clone();
    let lease = database.lease.clone();
    let access = player.access();
    let operation = command(1);
    let retry = operation.clone();
    let request = tokio::spawn(async move {
        store
            .commit_command(&lease, &access, operation, |_, _, _| {
                panic!("revocation during the world lock wait must prevent mechanics")
            })
            .await
    });
    timeout(Duration::from_secs(2), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM pg_stat_activity
                 WHERE datname = current_database()
                     AND application_name = 'clubscape-game-storage-tests'
                     AND wait_event_type = 'Lock'
                     AND query LIKE 'SELECT world_id, content_revision,%')",
            )
            .fetch_one(&database.pool)
            .await
            .unwrap();
            if waiting {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the command must really wait for the owned world-row lock");
    database
        .service
        .logout(&player.account.logged_in.session_token)
        .await;
    blocker.rollback().await.unwrap();
    assert_error(
        timeout(WAIT, request).await.unwrap().unwrap().unwrap_err(),
        StatusCode::UNAUTHORIZED,
        ErrorCode::Unauthenticated,
    );
    assert_eq!(
        database.store.load_world(database.world_id).await.unwrap(),
        before
    );
    assert_eq!(database.journal_count().await, 0);
    let login = database
        .service
        .login(&player.account.name, &player.account.password)
        .await;
    let auth = AuthTokenDigest::from_token(&login.session_token).unwrap();
    let session = database
        .store
        .join_session(
            database.world_id,
            player.character.state.actor_id,
            auth,
            MAX_SESSION_LEASE,
        )
        .await
        .unwrap();
    let accepted = database
        .store
        .commit_command(
            &database.lease,
            &session.access(auth),
            retry,
            synthetic_gain,
        )
        .await
        .unwrap();
    assert!(!accepted.duplicate);
    assert_eq!(accepted.receipt.sequence, 1);
    database.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn wrong_session_actor_world_and_token_cannot_mutate_or_release_another_character() {
    let database = TestDatabase::reset().await;
    let player = database.player("owned_character").await;
    let other = database.player("other_character").await;
    let before = database.store.load_world(database.world_id).await.unwrap();
    let mut bad_session = player.access();
    bad_session.session_id = Uuid::new_v4();
    let mut bad_actor = player.access();
    bad_actor.actor_id = other.character.state.actor_id.clone();
    let mut bad_owner = player.access();
    bad_owner.authentication = other.account.authentication();
    for access in [bad_session, bad_actor, bad_owner] {
        assert_conflict(database.store.read_session(&access).await.unwrap_err());
        assert_conflict(database.store.leave_session(&access).await.unwrap_err());
        assert_conflict(
            database
                .store
                .commit_command(&database.lease, &access, command(1), synthetic_gain)
                .await
                .unwrap_err(),
        );
    }
    assert_conflict(
        database
            .store
            .join_session(
                database.world_id,
                player.character.state.actor_id.clone(),
                other.account.authentication(),
                MAX_SESSION_LEASE,
            )
            .await
            .unwrap_err(),
    );
    let mut bad_token = player.access();
    bad_token.authentication = AuthTokenDigest::from_digest([0x19; 32]);
    assert_error(
        database
            .store
            .commit_command(&database.lease, &bad_token, command(1), synthetic_gain)
            .await
            .unwrap_err(),
        StatusCode::UNAUTHORIZED,
        ErrorCode::Unauthenticated,
    );
    let mut bad_world = player.access();
    bad_world.world_id = Uuid::new_v4();
    assert_error(
        database
            .store
            .commit_command(&database.lease, &bad_world, command(1), synthetic_gain)
            .await
            .unwrap_err(),
        StatusCode::BAD_REQUEST,
        ErrorCode::InvalidArgument,
    );
    assert_eq!(
        database.store.load_world(database.world_id).await.unwrap(),
        before
    );
    assert_eq!(database.journal_count().await, 0);
    database.store.read_session(&other.access()).await.unwrap();
    database.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn command_state_revisions_and_events_commit_once_with_a_canonical_intent_journal() {
    let database = TestDatabase::reset().await;
    let player = database.player("journal_player").await;
    let operation = command(1);
    let committed = database
        .store
        .commit_command(
            &database.lease,
            &player.access(),
            operation.clone(),
            synthetic_gain,
        )
        .await
        .unwrap();
    assert!(!committed.duplicate);
    assert_eq!(committed.receipt.world_revision, 2);
    assert_eq!(committed.receipt.world_tick, 0);
    assert_eq!(committed.receipt.character_revision, 2);
    assert_eq!(committed.receipt.sequence, 1);
    assert_eq!(
        committed.receipt.character.inventory.slots[0]
            .as_ref()
            .unwrap()
            .quantity
            .get(),
        4
    );
    assert_eq!(
        committed.receipt.character.skills[&SkillId::new("skill.fixture.gathering").unwrap()]
            .xp_tenths,
        133
    );
    assert_eq!(committed.receipt.events.len(), 2);
    let duplicate = database
        .store
        .commit_command(
            &database.lease,
            &player.access(),
            operation.clone(),
            |_, _, _| panic!("a committed command must not rerun mechanics"),
        )
        .await
        .unwrap();
    assert!(duplicate.duplicate);
    assert_eq!(duplicate.receipt, committed.receipt);
    let loaded = database
        .store
        .load_character(database.world_id, player.account.authentication())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.state, committed.receipt.character);
    assert_eq!(loaded.revision, committed.receipt.character_revision);
    assert_eq!(loaded.last_sequence, committed.receipt.sequence);
    let retried_create = database
        .store
        .create_character(
            &database.lease,
            player.account.authentication(),
            character_fixture(),
        )
        .await
        .unwrap();
    assert_eq!(
        retried_create, loaded,
        "retrying creation cannot reset inventory or XP"
    );
    let (hash, result, revision, character_revision): (Vec<u8>, String, i64, i64) = sqlx::query_as(
        "SELECT intent_hash, committed_result::text, world_revision, character_revision
         FROM processed_game_commands WHERE account_id = $1 AND operation_id = $2",
    )
    .bind(player.account.account_id)
    .bind(operation.operation_id)
    .fetch_one(&database.pool)
    .await
    .unwrap();
    let mut canonical = serde_json::to_value(&operation.intent).unwrap();
    canonical.sort_all_objects();
    let mut expected_hash = Sha256::new();
    expected_hash.update(b"clubscape.game-intent.v1\0");
    expected_hash.update(serde_json::to_vec(&canonical).unwrap());
    assert_eq!(hash, expected_hash.finalize().to_vec());
    assert_eq!(
        serde_json::from_str::<clubscape_server::game_storage::CommandReceipt>(&result).unwrap(),
        committed.receipt
    );
    assert_eq!(revision, 2);
    assert_eq!(character_revision, 2);
    assert!(!result.contains(&player.account.logged_in.session_token));
    assert!(!result.contains(&player.session.session_id.to_string()));

    let mut different_intent = operation.clone();
    different_intent.intent = GameIntent::CancelActivity;
    let mut different_sequence = operation.clone();
    different_sequence.sequence = 2;
    for rejected in [different_intent, different_sequence, command(1), command(3)] {
        assert_conflict(
            database
                .store
                .commit_command(&database.lease, &player.access(), rejected, |_, _, _| {
                    panic!("mismatched, stale and gapped commands cannot run")
                })
                .await
                .unwrap_err(),
        );
    }
    assert_eq!(database.journal_count().await, 1);
    database.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn canonical_run_energy_and_event_identities_survive_storage_without_clamping_or_rebinding() {
    let database = TestDatabase::reset().await;
    let account = database.account("canonical_state").await;
    let auth = account.authentication();
    let mut definition = character_fixture();
    definition.initial_state.run_energy = MAX_RUN_ENERGY + 1;
    assert_error(
        database
            .store
            .create_character(&database.lease, auth, definition.clone())
            .await
            .unwrap_err(),
        StatusCode::BAD_REQUEST,
        ErrorCode::InvalidArgument,
    );
    assert!(
        database
            .store
            .load_character(database.world_id, auth)
            .await
            .unwrap()
            .is_none()
    );
    definition.initial_state.run_energy = MAX_RUN_ENERGY;
    let character = database
        .store
        .create_character(&database.lease, auth, definition)
        .await
        .unwrap();
    assert_eq!(character.state.run_energy, MAX_RUN_ENERGY);
    let actor = character.state.actor_id;
    let session = database
        .store
        .join_session(database.world_id, actor.clone(), auth, MAX_SESSION_LEASE)
        .await
        .unwrap();
    let access = session.access(auth);
    let first = command(1);
    let committed = database
        .store
        .commit_command(
            &database.lease,
            &access,
            first.clone(),
            |world, actor, _| {
                let character = world.characters.get_mut(actor).unwrap();
                character.run_energy = 0;
                Ok(vec![
                    GameEvent::Sound {
                        asset: "asset.fixture.sound".to_owned(),
                    },
                    GameEvent::Animation {
                        target: actor.to_string(),
                        animation: "synthetic_animation".to_owned(),
                    },
                    GameEvent::Moved {
                        tile: character.tile,
                    },
                    GameEvent::Died,
                    GameEvent::Recovered,
                    GameEvent::Message {
                        text: "Synthetic serialization fixture, not gameplay evidence.".to_owned(),
                    },
                ])
            },
        )
        .await
        .unwrap();
    let duplicate = database
        .store
        .commit_command(&database.lease, &access, first, |_, _, _| {
            panic!("reading canonical event identities must not replay a committed command")
        })
        .await
        .unwrap();
    assert!(duplicate.duplicate);
    assert_eq!(duplicate.receipt, committed.receipt);
    assert_eq!(duplicate.receipt.character.run_energy, 0);
    let identities: Vec<_> = duplicate
        .receipt
        .events
        .iter()
        .map(|event| (event.kind(), event.primary_target()))
        .collect();
    assert_eq!(
        identities,
        [
            ("sound", Some("asset.fixture.sound")),
            ("animation", Some(actor.as_str())),
            ("moved", None),
            ("died", None),
            ("recovered", None),
            ("message", None),
        ]
    );

    let before = database.store.load_world(database.world_id).await.unwrap();
    let second = command(2);
    assert_error(
        database
            .store
            .commit_command(
                &database.lease,
                &access,
                second.clone(),
                |world, actor, _| {
                    world.characters.get_mut(actor).unwrap().run_energy = MAX_RUN_ENERGY + 1;
                    Ok(Vec::new())
                },
            )
            .await
            .unwrap_err(),
        StatusCode::INTERNAL_SERVER_ERROR,
        ErrorCode::Internal,
    );
    assert_eq!(
        database.store.load_world(database.world_id).await.unwrap(),
        before
    );
    assert_eq!(database.journal_count().await, 1);
    let fractional = database
        .store
        .commit_command(
            &database.lease,
            &access,
            second.clone(),
            |world, actor, _| {
                world.characters.get_mut(actor).unwrap().run_energy = 9_876;
                Ok(Vec::new())
            },
        )
        .await
        .unwrap();
    assert_eq!(fractional.receipt.character.run_energy, 9_876);
    assert_eq!(
        database
            .store
            .load_character(database.world_id, auth)
            .await
            .unwrap()
            .unwrap()
            .state
            .run_energy,
        9_876
    );
    let energy_path = vec![
        "characters".to_owned(),
        actor.to_string(),
        "run_energy".to_owned(),
    ];
    sqlx::query(
        "UPDATE game_worlds SET state = jsonb_set(state, $2::text[], to_jsonb($3::int))
         WHERE world_id = $1",
    )
    .bind(database.world_id)
    .bind(&energy_path)
    .bind(i32::from(MAX_RUN_ENERGY) + 1)
    .execute(&database.pool)
    .await
    .unwrap();
    assert_error(
        database
            .store
            .load_world(database.world_id)
            .await
            .unwrap_err(),
        StatusCode::INTERNAL_SERVER_ERROR,
        ErrorCode::Internal,
    );
    sqlx::query(
        "UPDATE game_worlds SET state = jsonb_set(state, $2::text[], to_jsonb($3::int))
         WHERE world_id = $1",
    )
    .bind(database.world_id)
    .bind(energy_path)
    .bind(9_876_i32)
    .execute(&database.pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE processed_game_commands SET committed_result =
             jsonb_set(committed_result, '{character,run_energy}', to_jsonb($3::int))
         WHERE account_id = $1 AND operation_id = $2",
    )
    .bind(account.account_id)
    .bind(second.operation_id)
    .bind(i32::from(MAX_RUN_ENERGY) + 1)
    .execute(&database.pool)
    .await
    .unwrap();
    assert_error(
        database
            .store
            .commit_command(&database.lease, &access, second, |_, _, _| {
                panic!("invalid stored energy cannot be repaired by replaying a callback")
            })
            .await
            .unwrap_err(),
        StatusCode::INTERNAL_SERVER_ERROR,
        ErrorCode::Internal,
    );
    assert_eq!(database.journal_count().await, 2);
    database.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn validation_callback_failure_and_invalid_metadata_roll_back_without_consuming_sequences() {
    let database = TestDatabase::reset().await;
    let player = database.player("rollback_player").await;
    let before = database.store.load_world(database.world_id).await.unwrap();
    let operation = command(1);
    let failure = database
        .store
        .commit_command(
            &database.lease,
            &player.access(),
            operation.clone(),
            |world, actor, intent| {
                synthetic_gain(world, actor, intent)?;
                Err(GameError::new(
                    GameErrorCode::RequirementNotMet,
                    "secret callback payload and SQL",
                ))
            },
        )
        .await
        .unwrap_err();
    assert!(!format!("{failure:?}").contains("secret"));
    assert_conflict(failure);
    let bad_sequence = database
        .store
        .commit_command(
            &database.lease,
            &player.access(),
            operation.clone(),
            |world, actor, _| {
                world
                    .characters
                    .get_mut(actor)
                    .unwrap()
                    .last_command_sequence = 10;
                Ok(Vec::new())
            },
        )
        .await
        .unwrap_err();
    assert_error(
        bad_sequence,
        StatusCode::INTERNAL_SERVER_ERROR,
        ErrorCode::Internal,
    );
    let bad_tick = database
        .store
        .commit_command(
            &database.lease,
            &player.access(),
            operation.clone(),
            |world, _, _| {
                world.tick += 1;
                Ok(Vec::new())
            },
        )
        .await
        .unwrap_err();
    assert_error(
        bad_tick,
        StatusCode::INTERNAL_SERVER_ERROR,
        ErrorCode::Internal,
    );
    let too_large = database
        .store
        .commit_command(
            &database.lease,
            &player.access(),
            operation.clone(),
            |world, actor, intent| {
                synthetic_gain(world, actor, intent)?;
                Ok(vec![GameEvent::Message {
                    text: "x".repeat(256 * 1024),
                }])
            },
        )
        .await
        .unwrap_err();
    assert_error(
        too_large,
        StatusCode::BAD_REQUEST,
        ErrorCode::InvalidArgument,
    );
    assert_eq!(database.journal_count().await, 0);
    assert_eq!(
        database.store.load_world(database.world_id).await.unwrap(),
        before
    );
    let mut changed_intent = operation;
    changed_intent.intent = GameIntent::CancelActivity;
    let successful = database
        .store
        .commit_command(
            &database.lease,
            &player.access(),
            changed_intent,
            synthetic_gain,
        )
        .await
        .unwrap();
    assert!(
        !successful.duplicate,
        "failed validation did not reserve an operation ID"
    );
    assert_eq!(successful.receipt.sequence, 1);
    assert_eq!(database.journal_count().await, 1);
    database.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn concurrent_operation_retries_and_different_sequence_contenders_are_exactly_once() {
    let database = TestDatabase::reset().await;
    let player = database.player("concurrent_player").await;
    let operation = command(1);
    let calls = Arc::new(AtomicUsize::new(0));
    let access = player.access();
    let outcomes = join_all((0..8).map(|_| {
        let calls = calls.clone();
        database.store.commit_command(
            &database.lease,
            &access,
            operation.clone(),
            move |world, actor, intent| {
                calls.fetch_add(1, Ordering::SeqCst);
                synthetic_gain(world, actor, intent)
            },
        )
    }))
    .await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| !outcome.as_ref().unwrap().duplicate)
            .count(),
        1
    );
    assert!(
        outcomes
            .iter()
            .all(|outcome| outcome.as_ref().unwrap().receipt
                == outcomes[0].as_ref().unwrap().receipt)
    );
    let contenders = join_all((0..8).map(|_| {
        let calls = calls.clone();
        database.store.commit_command(
            &database.lease,
            &access,
            command(2),
            move |world, actor, intent| {
                calls.fetch_add(1, Ordering::SeqCst);
                synthetic_gain(world, actor, intent)
            },
        )
    }))
    .await;
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(contenders.iter().filter(|result| result.is_ok()).count(), 1);
    for error in contenders.into_iter().filter_map(Result::err) {
        assert_conflict(error);
    }
    let character = database
        .store
        .load_character(database.world_id, player.account.authentication())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(character.last_sequence, 2);
    assert_eq!(character.revision, 3);
    assert_eq!(character.state.flags["synthetic_commits"], 2);
    assert_eq!(database.journal_count().await, 2);
    database.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn pool_repository_and_auth_session_renewal_preserve_historical_committed_results() {
    let mut database = TestDatabase::reset().await;
    let player = database.player("restart_player").await;
    let operation = command(1);
    let first = database
        .store
        .commit_command(
            &database.lease,
            &player.access(),
            operation.clone(),
            synthetic_gain,
        )
        .await
        .unwrap();
    database
        .store
        .commit_command(
            &database.lease,
            &player.access(),
            command(2),
            synthetic_gain,
        )
        .await
        .unwrap();
    let world = database.store.load_world(database.world_id).await.unwrap();
    database
        .store
        .leave_session(&player.access())
        .await
        .unwrap();
    database
        .service
        .logout(&player.account.logged_in.session_token)
        .await;
    let login = database
        .service
        .login(&player.account.name, &player.account.password)
        .await;
    let auth = AuthTokenDigest::from_token(&login.session_token).unwrap();
    database
        .store
        .release_world_lease(&database.lease)
        .await
        .unwrap();
    database.store.clone().close().await.unwrap();
    assert!(database.pool.is_closed());
    database.pool = open_pool(database.options.clone(), 8).await;
    database.store = GameStore::new(database.pool.clone());
    database.store.migrate().await.unwrap();
    let lease = database
        .store
        .acquire_world_lease(database.world_id, MAX_WORLD_LEASE)
        .await
        .unwrap();
    assert!(lease.fence > database.lease.fence);
    database.lease = lease;
    assert_eq!(
        database.store.load_world(database.world_id).await.unwrap(),
        world
    );
    let session = database
        .store
        .join_session(
            database.world_id,
            player.character.state.actor_id,
            auth,
            MAX_SESSION_LEASE,
        )
        .await
        .unwrap();
    assert_ne!(session.session_id, player.session.session_id);
    let retried = database
        .store
        .commit_command(
            &database.lease,
            &session.access(auth),
            operation,
            |_, _, _| {
                panic!("a repository restart and token/session renewal cannot replay a reward")
            },
        )
        .await
        .unwrap();
    assert!(retried.duplicate);
    assert_eq!(retried.receipt, first.receipt);
    let latest = database
        .store
        .load_character(database.world_id, auth)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(latest.last_sequence, 2);
    assert_ne!(
        latest.state, retried.receipt.character,
        "the journal returns the historical committed result"
    );
    assert_eq!(database.journal_count().await, 2);
    database.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn fenced_world_ownership_prevents_stale_writers_and_duplicate_simulation_ticks() {
    let database = TestDatabase::reset().await;
    let player = database.player("fenced_player").await;
    let other_player = database.player("tick_other_player").await;
    let other = GameStore::new(database.pool.clone());
    assert_conflict(
        other
            .acquire_world_lease(database.world_id, MAX_WORLD_LEASE)
            .await
            .unwrap_err(),
    );
    let renewed = database
        .store
        .renew_world_lease(&database.lease, MAX_WORLD_LEASE)
        .await
        .unwrap();
    assert_eq!(renewed.fence, database.lease.fence);
    assert_eq!(
        database
            .store
            .acquire_world_lease(database.world_id, MAX_WORLD_LEASE)
            .await
            .unwrap()
            .fence,
        renewed.fence
    );
    database
        .store
        .release_world_lease(&database.lease)
        .await
        .unwrap();
    let replacement = other
        .acquire_world_lease(database.world_id, MAX_WORLD_LEASE)
        .await
        .unwrap();
    assert!(replacement.fence > database.lease.fence);
    assert_conflict(
        database
            .store
            .commit_command(
                &database.lease,
                &player.access(),
                command(1),
                synthetic_gain,
            )
            .await
            .unwrap_err(),
    );
    assert_conflict(
        database
            .store
            .create_character(
                &database.lease,
                player.account.authentication(),
                character_fixture(),
            )
            .await
            .unwrap_err(),
    );
    assert_conflict(
        database
            .store
            .commit_tick(&database.lease, 0, |_| panic!("stale ticker"))
            .await
            .unwrap_err(),
    );
    assert_conflict(
        other
            .commit_command(
                &database.lease,
                &player.access(),
                command(1),
                synthetic_gain,
            )
            .await
            .unwrap_err(),
    );
    sqlx::query("UPDATE game_worlds SET lease_expires_at = clock_timestamp() - INTERVAL '1 second' WHERE world_id = $1")
        .bind(database.world_id).execute(&database.pool).await.unwrap();
    assert_conflict(
        other
            .renew_world_lease(&replacement, MAX_WORLD_LEASE)
            .await
            .unwrap_err(),
    );
    let takeover = database
        .store
        .acquire_world_lease(database.world_id, MAX_WORLD_LEASE)
        .await
        .unwrap();
    assert!(takeover.fence > replacement.fence);
    assert_conflict(
        other
            .commit_tick(&replacement, 0, |_| panic!("expired ticker"))
            .await
            .unwrap_err(),
    );
    let calls = Arc::new(AtomicUsize::new(0));
    let ticks = join_all((0..4).map(|_| {
        let calls = calls.clone();
        database.store.commit_tick(&takeover, 0, move |world| {
            calls.fetch_add(1, Ordering::SeqCst);
            assert_eq!(world.tick, 1);
            for character in world.characters.values_mut() {
                character.hitpoints += 1;
            }
            Ok(vec![GameEvent::Message {
                text: "synthetic tick committed".to_owned(),
            }])
        })
    }))
    .await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        ticks
            .iter()
            .filter(|tick| !tick.as_ref().unwrap().duplicate)
            .count(),
        1
    );
    let receipt = ticks[0].as_ref().unwrap().receipt.clone();
    for authentication in [
        player.account.authentication(),
        other_player.account.authentication(),
    ] {
        let character = database
            .store
            .load_character(database.world_id, authentication)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(character.revision, 2);
        assert_eq!(character.last_sequence, 0);
        assert_eq!(character.state.hitpoints, 6);
    }
    database
        .store
        .commit_command(&takeover, &player.access(), command(1), synthetic_gain)
        .await
        .unwrap();
    let duplicate = database
        .store
        .commit_tick(&takeover, 0, |_| {
            panic!("a command between ticks does not rerun the tick")
        })
        .await
        .unwrap();
    assert!(duplicate.duplicate);
    assert_eq!(duplicate.receipt, receipt);
    assert_conflict(
        database
            .store
            .commit_tick(&takeover, 5, |_| panic!("gapped ticker"))
            .await
            .unwrap_err(),
    );
    database
        .store
        .commit_tick(&takeover, 1, |_| Ok(Vec::new()))
        .await
        .unwrap();
    assert_conflict(
        database
            .store
            .commit_tick(&takeover, 0, |_| panic!("old ticker"))
            .await
            .unwrap_err(),
    );
    database.store.read_session(&player.access()).await.unwrap();
    database.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn corrupt_persisted_world_character_metadata_and_results_fail_closed_without_initial_fallback()
 {
    let database = TestDatabase::reset().await;
    let player = database.player("corrupt_player").await;
    let operation = command(1);
    database
        .store
        .commit_command(
            &database.lease,
            &player.access(),
            operation.clone(),
            synthetic_gain,
        )
        .await
        .unwrap();
    let snapshot = database.store.load_world(database.world_id).await.unwrap();
    sqlx::query("UPDATE game_characters SET last_sequence = 20 WHERE account_id = $1")
        .bind(player.account.account_id)
        .execute(&database.pool)
        .await
        .unwrap();
    assert_error(
        database
            .store
            .load_world(database.world_id)
            .await
            .unwrap_err(),
        StatusCode::INTERNAL_SERVER_ERROR,
        ErrorCode::Internal,
    );
    sqlx::query("UPDATE game_characters SET last_sequence = 1 WHERE account_id = $1")
        .bind(player.account.account_id)
        .execute(&database.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE game_characters SET revision = 20 WHERE account_id = $1")
        .bind(player.account.account_id)
        .execute(&database.pool)
        .await
        .unwrap();
    assert_error(
        database
            .store
            .load_world(database.world_id)
            .await
            .unwrap_err(),
        StatusCode::INTERNAL_SERVER_ERROR,
        ErrorCode::Internal,
    );
    sqlx::query("UPDATE game_characters SET revision = 2 WHERE account_id = $1")
        .bind(player.account.account_id)
        .execute(&database.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE game_worlds SET state = jsonb_set(state, '{characters}', '\"corrupt\"'::jsonb) WHERE world_id = $1")
        .bind(database.world_id).execute(&database.pool).await.unwrap();
    for error in [
        database
            .store
            .load_world(database.world_id)
            .await
            .unwrap_err(),
        database
            .store
            .initialize_world(database.world_id, world_fixture())
            .await
            .unwrap_err(),
        database
            .store
            .create_character(
                &database.lease,
                player.account.authentication(),
                character_fixture(),
            )
            .await
            .unwrap_err(),
    ] {
        assert_error(
            error,
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::Internal,
        );
    }
    sqlx::query("UPDATE game_worlds SET state = $2::jsonb WHERE world_id = $1")
        .bind(database.world_id)
        .bind(serde_json::to_string(&snapshot.state).unwrap())
        .execute(&database.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE processed_game_commands SET committed_result = jsonb_set(committed_result, '{sequence}', '999'::jsonb)")
        .execute(&database.pool).await.unwrap();
    assert_error(
        database
            .store
            .commit_command(&database.lease, &player.access(), operation, |_, _, _| {
                panic!("corrupt journal cannot replay mechanics")
            })
            .await
            .unwrap_err(),
        StatusCode::INTERNAL_SERVER_ERROR,
        ErrorCode::Internal,
    );
    assert_eq!(
        database.store.load_world(database.world_id).await.unwrap(),
        snapshot
    );
    assert_eq!(database.journal_count().await, 1);
    database.stop().await;
}

struct ProxyControl {
    armed: AtomicBool,
    held: watch::Sender<bool>,
    active: AtomicUsize,
}

struct ProxyConnection(Arc<ProxyControl>);

impl Drop for ProxyConnection {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
    }
}

struct CommitProxy {
    address: SocketAddr,
    control: Arc<ProxyControl>,
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}

impl CommitProxy {
    async fn start(target: SocketAddr) -> Self {
        assert!(target.ip().is_loopback());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let control = Arc::new(ProxyControl {
            armed: AtomicBool::new(false),
            held: watch::channel(false).0,
            active: AtomicUsize::new(0),
        });
        let (shutdown, mut receive) = oneshot::channel();
        let task_control = control.clone();
        let task = tokio::spawn(async move {
            let mut connections = JoinSet::new();
            loop {
                tokio::select! {
                    _ = &mut receive => break,
                    accepted = listener.accept() => {
                        let Ok((client, _)) = accepted else { break };
                        if connections.len() >= 4 { continue; }
                        let control = task_control.clone();
                        connections.spawn(async move {
                            if let Ok(server) = TcpStream::connect(target).await {
                                control.active.fetch_add(1, Ordering::SeqCst);
                                let _active = ProxyConnection(control.clone());
                                let (client_read, client_write) = client.into_split();
                                let (server_read, server_write) = server.into_split();
                                tokio::select! {
                                    _ = proxy_requests(client_read, server_write, control.clone()) => {},
                                    _ = proxy_responses(server_read, client_write, control.held.subscribe()) => {},
                                }
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
            control,
            shutdown: Some(shutdown),
            task: Some(task),
        }
    }

    async fn wait_held(&self) {
        let mut held = self.control.held.subscribe();
        timeout(Duration::from_secs(4), held.wait_for(|held| *held))
            .await
            .unwrap()
            .unwrap();
        assert!(self.control.active.load(Ordering::SeqCst) > 0);
    }

    async fn wait_disconnected(&self) {
        timeout(Duration::from_secs(2), async {
            while self.control.active.load(Ordering::SeqCst) > 0 {
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("cancelled transactions must discard the physical connection");
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

impl Drop for CommitProxy {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

async fn proxy_frame(reader: &mut (impl AsyncRead + Unpin), length: u32) -> io::Result<Vec<u8>> {
    if !(4..=1024 * 1024).contains(&length) {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let mut body = vec![0; length as usize - 4];
    reader.read_exact(&mut body).await?;
    Ok(body)
}

async fn proxy_requests(
    mut client: impl AsyncRead + Unpin,
    mut server: impl AsyncWrite + Unpin,
    control: Arc<ProxyControl>,
) -> io::Result<()> {
    let startup = client.read_u32().await?;
    let body = proxy_frame(&mut client, startup).await?;
    server.write_all(&startup.to_be_bytes()).await?;
    server.write_all(&body).await?;
    loop {
        let kind = client.read_u8().await?;
        let length = client.read_u32().await?;
        let body = proxy_frame(&mut client, length).await?;
        if kind == b'Q' && body == b"COMMIT\0" && control.armed.swap(false, Ordering::SeqCst) {
            control.held.send_replace(true);
        }
        server.write_all(&[kind]).await?;
        server.write_all(&length.to_be_bytes()).await?;
        server.write_all(&body).await?;
    }
}

async fn proxy_responses(
    mut server: impl AsyncRead + Unpin,
    mut client: impl AsyncWrite + Unpin,
    mut held: watch::Receiver<bool>,
) -> io::Result<()> {
    let mut buffer = [0; 8192];
    loop {
        let length = server.read(&mut buffer).await?;
        if length == 0 {
            return Ok(());
        }
        while *held.borrow_and_update() {
            held.changed()
                .await
                .map_err(|_| io::ErrorKind::BrokenPipe)?;
        }
        client.write_all(&buffer[..length]).await?;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn a_lost_commit_acknowledgment_times_out_but_a_fenced_retry_recovers_the_durable_result() {
    let database = TestDatabase::reset().await;
    let player = database.player("lost_ack_player").await;
    database
        .store
        .release_world_lease(&database.lease)
        .await
        .unwrap();
    let target = SocketAddr::new(
        database.options.get_host().parse().unwrap(),
        database.options.get_port(),
    );
    let proxy = CommitProxy::start(target).await;
    let pool = open_pool(
        database
            .options
            .clone()
            .host("127.0.0.1")
            .port(proxy.address.port())
            .ssl_mode(PgSslMode::Disable),
        1,
    )
    .await;
    let store = GameStore::new(pool.clone());
    let lease = store
        .acquire_world_lease(database.world_id, MAX_WORLD_LEASE)
        .await
        .unwrap();
    let operation = command(1);
    let request_store = store.clone();
    let request_lease = lease.clone();
    let request_access = player.access();
    let request_command = operation.clone();
    let calls = Arc::new(AtomicUsize::new(0));
    let callback_calls = calls.clone();
    proxy.control.armed.store(true, Ordering::SeqCst);
    let started = Instant::now();
    let request = tokio::spawn(async move {
        request_store
            .commit_command(
                &request_lease,
                &request_access,
                request_command,
                move |world, actor, intent| {
                    callback_calls.fetch_add(1, Ordering::SeqCst);
                    synthetic_gain(world, actor, intent)
                },
            )
            .await
    });
    proxy.wait_held().await;
    timeout(Duration::from_secs(2), async {
        while database.journal_count().await != 1 {
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("COMMIT must reach PostgreSQL even though its response is withheld");
    let committed_world = database.store.load_world(database.world_id).await.unwrap();
    assert_eq!(
        committed_world.state.characters[&player.character.state.actor_id].last_command_sequence,
        1
    );
    let error = timeout(Duration::from_secs(8), request)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    assert!(started.elapsed() >= Duration::from_secs(5));
    assert!(started.elapsed() < Duration::from_secs(8));
    assert!(error.message.contains("unknown"));
    assert_eq!(error.retry_after_seconds, 0);
    assert_error(
        error,
        StatusCode::SERVICE_UNAVAILABLE,
        ErrorCode::Unavailable,
    );
    proxy.wait_disconnected().await;
    proxy.control.held.send_replace(false);
    store.release_world_lease(&lease).await.unwrap();
    store.close().await.unwrap();
    assert!(pool.is_closed());
    proxy.stop().await;
    let replacement = database
        .store
        .acquire_world_lease(database.world_id, MAX_WORLD_LEASE)
        .await
        .unwrap();
    let retry = database
        .store
        .commit_command(&replacement, &player.access(), operation, |_, _, _| {
            panic!("unknown outcome retries must recover the stored result without replay")
        })
        .await
        .unwrap();
    assert!(retry.duplicate);
    assert_eq!(retry.receipt.world_revision, committed_world.state.revision);
    assert_eq!(
        retry.receipt.character,
        committed_world.state.characters[&player.character.state.actor_id]
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(database.journal_count().await, 1);
    database.stop().await;
}
