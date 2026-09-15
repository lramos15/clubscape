mod amounts;
mod anvil;
#[path = "../../../world-engine/tests/support/mod.rs"]
mod engine_fixtures;
#[path = "../../../content/tests/common/mod.rs"]
mod fixtures;
mod observer;
mod production;
mod ui;

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
            "DROP TABLE IF EXISTS public.game_content_migrations;
             DROP TABLE IF EXISTS public.game_lifecycle_commands;
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
    fn ui() -> Self {
        let mut pack = Self::new();
        fixtures::ui::enable(&mut pack.definition);
        pack.definition.initial_state.interfaces =
            pack.definition.interfaces.keys().cloned().collect();
        pack.definition.revision = "live-ui-synthetic-v4".into();
        pack.write();
        pack
    }
    fn new() -> Self {
        let mut definition = fixtures::fixture();
        definition.revision = "live-world-synthetic-v1".into();
        definition.initial_state.tile = fixtures::tile(1002, 1003);
        definition.mechanics.world_members = Some(false);
        definition.mechanics.appearance = Some(AppearanceDefinition {
            choices: BTreeMap::from([("body_type".into(), BTreeSet::from([0, 1]))]),
            confirmation_guard: Guard::Always,
            source: fixtures::sources(),
        });
        let experience = ExperienceId::new("experience.test.new").unwrap();
        definition.mechanics.experiences.insert(
            experience.clone(),
            ExperienceDefinition {
                id: experience,
                name: "Synthetic new-player branch".into(),
                selection_guard: Guard::Always,
                source: fixtures::sources(),
            },
        );
        let shop_interface = InterfaceId::new("interface.test.shop").unwrap();
        definition.interfaces.insert(
            shop_interface.clone(),
            InterfaceDefinition {
                id: shop_interface.clone(),
                name: "Synthetic shop".into(),
                access: InterfaceAccess::Contextual,
                source_ids: vec![9000],
                source: fixtures::sources(),
            },
        );
        for id in [
            InterfaceId::new("interface.test.bank").unwrap(),
            shop_interface.clone(),
        ] {
            if !definition.initial_state.interfaces.contains(&id) {
                definition.initial_state.interfaces.push(id);
            }
        }
        for stage in definition.tutorial.values_mut() {
            stage.allowed_actions = vec!["*".into()];
        }
        let guide = definition
            .spawns
            .get_mut(&fixtures::id("spawn.test.guide"))
            .unwrap();
        for interaction in &mut guide.interactions {
            interaction.reach = 4;
            interaction.action = match &interaction.action {
                InteractionAction::Bank => InteractionAction::OpenBank {
                    interface: InterfaceId::new("interface.test.bank").unwrap(),
                    before_open: Vec::new(),
                },
                InteractionAction::Shop { shop } => InteractionAction::OpenShop {
                    shop: shop.clone(),
                    interface: shop_interface.clone(),
                    before_open: Vec::new(),
                },
                other => other.clone(),
            };
        }
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
        self.write_manifest(entries, assets, &artifact);
    }

    fn combat() -> Self {
        use engine_fixtures::v2 as v;
        let mut definition = v::content();
        v::with_combat(&mut definition);
        v::with_death(&mut definition);
        v::armed(&mut definition, false);
        definition.mechanics.player_combat = Some(PlayerCombatPolicy {
            unarmed: v::bound(WeaponDefinition {
                styles: vec![v::style("accurate")],
                default_style: v::style("accurate"),
                ammunition: None,
            }),
            engagement: v::bound(PlayerEngagementPolicy {
                combat_state_ticks: 3,
                logout_lock_ticks: 3,
                travel_lock_ticks: 3,
            }),
            source: engine_fixtures::source(),
        });
        let npc = definition
            .npcs
            .get_mut(&v::npc())
            .unwrap()
            .combat
            .as_mut()
            .unwrap();
        npc.hitpoints = 200;
        npc.mechanics.as_mut().unwrap().retaliation = true;
        // Source-owned synthetic topics/exit stay executable without an unreachable legacy node.
        definition
            .dialogues
            .get_mut(&engine_fixtures::dialogue())
            .unwrap()
            .nodes
            .truncate(1);
        let base = fixtures::fixture();
        definition.items.extend(base.items);
        definition.skills.extend(base.skills);
        definition.regions.extend(base.regions);
        definition.spawns.extend(base.spawns);
        definition.objects.extend(base.objects);
        definition.npcs.extend(base.npcs);
        definition.recipes.extend(base.recipes);
        definition.dialogues.extend(base.dialogues);
        definition.quests.extend(base.quests);
        definition.shops.extend(base.shops);
        definition.interfaces.extend(base.interfaces);
        definition.equipment_slots.extend(base.equipment_slots);
        definition.equipment_slots.sort();
        definition.equipment_slots.dedup();
        definition
            .initial_state
            .flags
            .extend(base.initial_state.flags);
        for id in definition.skills.keys() {
            definition
                .initial_state
                .skills
                .entry(id.clone())
                .or_insert(SkillState {
                    xp_tenths: 0,
                    current_level: 1,
                });
        }
        for (id, quest) in &definition.quests {
            definition.initial_state.quests.insert(
                id.clone(),
                QuestState {
                    stage: quest.initial_stage.clone(),
                    flags: BTreeMap::new(),
                },
            );
        }
        definition
            .tutorial
            .retain(|id, _| id == &definition.initial_state.tutorial_stage);
        let stage = definition.tutorial.values_mut().next().unwrap();
        stage.transitions.clear();
        stage.nonfatal_combat = false;
        stage.allowed_actions = vec!["*".into()];
        let finish_id = StageId::new("stage.synthetic.integrated_finish").unwrap();
        let mut finish = stage.clone();
        finish.id = finish_id.clone();
        stage.transitions.push(ProgressTransition {
            event: "interacted".into(),
            target: Some(engine_fixtures::spawn("rock").to_string()),
            guard: Guard::Always,
            effects: vec![Effect::SetTutorialStage {
                stage: finish_id.clone(),
            }],
        });
        definition.tutorial.insert(finish_id, finish);
        let grave = InterfaceId::new("interface.synthetic.grave").unwrap();
        let office = InterfaceId::new("interface.synthetic.office").unwrap();
        for id in [&grave, &office] {
            definition.interfaces.insert(
                id.clone(),
                InterfaceDefinition {
                    id: id.clone(),
                    name: "Synthetic owned recovery".into(),
                    access: InterfaceAccess::Contextual,
                    source_ids: Vec::new(),
                    source: engine_fixtures::source(),
                },
            );
            definition.initial_state.interfaces.push(id.clone());
        }
        definition.mechanics.death.as_mut().unwrap().interfaces =
            Some(RecoveryInterfaces { grave, office });
        definition
            .mechanics
            .death
            .as_mut()
            .unwrap()
            .retained_unskulled = 0;
        if let SourceBinding::Bound {
            value: RecoveryFee::Bands { bands, .. },
            ..
        } = &mut definition.mechanics.death.as_mut().unwrap().grave_fee
        {
            bands.insert(
                0,
                FeeBand {
                    minimum_value: 0,
                    fee: 0,
                },
            );
        }
        for provider in definition.mechanics.value_providers.values_mut() {
            provider.values = v::bound(definition.items.keys().map(|id| (id.clone(), 1)).collect());
        }
        for (index, item) in definition.items.values_mut().enumerate() {
            item.source_id = Some(index as u32 + 1);
            if let Some(equipment) = &mut item.equipment
                && equipment.occupied_slots.is_empty()
            {
                equipment.occupied_slots.push(equipment.slot.clone());
            }
        }
        for (index, item) in definition.objects.values_mut().enumerate() {
            item.source_id = index as u32 + 1;
        }
        for (index, item) in definition.npcs.values_mut().enumerate() {
            item.source_id = index as u32 + 1;
        }
        for (index, skill) in definition.skills.values_mut().enumerate() {
            skill.source_id = index as u16;
            while skill.xp_thresholds_tenths.len() < 99 {
                skill
                    .xp_thresholds_tenths
                    .push(skill.xp_thresholds_tenths.last().unwrap() + 1000);
            }
            skill.maximum_xp_tenths = skill
                .maximum_xp_tenths
                .max(skill.xp_thresholds_tenths.last().unwrap() + 1000);
        }
        for (index, interface) in definition.interfaces.values_mut().enumerate() {
            interface.source_ids = vec![index as u32 + 1];
        }
        definition.revision = "live-combat-recovery-synthetic-v3".into();
        let mut serialized = serde_json::to_value(&definition).unwrap();
        normalize_fixture_sources(&mut serialized);
        let definition = serde_json::from_value(serialized).unwrap();
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

    fn write_manifest(
        &self,
        mut entries: Vec<serde_json::Value>,
        assets: BTreeMap<String, String>,
        artifact: &[u8],
    ) {
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
            "sha256":sha256(artifact), "content_manifest_path":"/content/manifest.json", "assets":assets,
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

fn normalize_fixture_sources(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(fields) => {
            if fields.contains_key("reference")
                && fields.contains_key("revision")
                && fields.get("status").and_then(serde_json::Value::as_str) == Some("test_fixture")
            {
                fields.insert(
                    "reference".into(),
                    serde_json::Value::String("fixture:clubscape-server/combat-recovery".into()),
                );
            }
            for child in fields.values_mut() {
                normalize_fixture_sources(child);
            }
        }
        serde_json::Value::Array(values) => {
            for child in values {
                normalize_fixture_sources(child);
            }
        }
        _ => {}
    }
}

#[test]
fn canonical_sized_game_descriptors_load_but_the_private_byte_limit_is_enforced() {
    let pack = Pack::new();
    let mut config = Config::new(
        "postgres://127.0.0.1/clubscape_m1_test",
        "127.0.0.1:0",
        None,
    )
    .unwrap()
    .with_game_root(&pack.root)
    .unwrap();
    config.game_test_fixture = true;
    assert!(config.web_root.is_none());
    let path = pack.root.join("clubscape-game.json");
    let original = fs::read(&path).unwrap();
    assert_eq!(content::MAX_GAME_DESCRIPTOR_BYTES, 512 * 1024);
    for size in [272_433, 406_574, 512 * 1024] {
        let mut bytes = original.clone();
        bytes.resize(size, b' ');
        fs::write(&path, &bytes).unwrap();
        assert!(content::load(&config).unwrap().is_some());
    }
    let mut oversized = original;
    oversized.resize(512 * 1024 + 1, b' ');
    fs::write(path, oversized).unwrap();
    let error = match content::load(&config) {
        Err(error) => error,
        Ok(_) => panic!("an oversized configured game descriptor cannot load or fall back"),
    };
    assert!(format!("{error:?}").contains("game_content"));
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

#[test]
fn source_readiness_proves_inactive_paths_without_rewriting_them() {
    let pack = Pack::new();
    let mut definition = pack.definition.clone();
    let alternative = ItemId::new("item.test.inactive_alternative").unwrap();
    let mut item = definition.items[&fixtures::id("item.test.coins")].clone();
    item.id = alternative.clone();
    item.source_id = Some(99991);
    item.stackable = Stackability::Conditional {
        source_mode: 2,
        rule: SourceBinding::Unresolved {
            reason: "Synthetic alternative-only instance rule.".into(),
            source: fixtures::sources(),
        },
    };
    definition.items.insert(alternative.clone(), item);
    let InteractionAction::Gather { rule } = &mut definition
        .spawns
        .get_mut(&fixtures::id("spawn.test.rock"))
        .unwrap()
        .interactions[0]
        .action
    else {
        panic!()
    };
    rule.attempt_ticks = None;
    rule.respawn_ticks = None;
    let bound = |value| SourceBinding::Bound {
        value,
        source: fixtures::sources(),
    };
    rule.mechanics = Some(GatherMechanics {
        method: ActionId::new("action.test.mining").unwrap(),
        levels: LevelDomain {
            minimum: 1,
            maximum: 3,
            basis: SkillLevelBasis::Current,
        },
        cadence: ActionCadence {
            single: bound(2),
            first: bound(2),
            repeat: bound(2),
            menu_delay: bound(0),
        },
        tool_cadences: Vec::new(),
        alternatives: Vec::new(),
        relocation: None,
        respawn: SourceBinding::Unresolved {
            reason: "No depletion implies no respawn draw.".into(),
            source: fixtures::sources(),
        },
    });
    let compiled = compile_content(definition.clone(), ValidationMode::TestFixture).unwrap();
    let profile = readiness::Profile {
        id: "ordinary_normal_f2p".into(),
        excluded_items: BTreeSet::from([alternative]),
    };
    assert!(readiness::Readiness::check(&compiled, None).is_err());
    let ready = readiness::Readiness::check(&compiled, Some(&profile)).unwrap();
    assert_eq!(ready.inactive.len(), 2);
    assert_eq!(
        compiled.report().unresolved_bindings.len(),
        2,
        "proofs do not manufacture missing values"
    );
    let invalid = readiness::Profile {
        id: profile.id.clone(),
        excluded_items: BTreeSet::from([fixtures::id("item.test.coins")]),
    };
    assert!(readiness::Readiness::check(&compiled, Some(&invalid)).is_err());
    let InteractionAction::Gather { rule } = &mut definition
        .spawns
        .get_mut(&fixtures::id("spawn.test.rock"))
        .unwrap()
        .interactions[0]
        .action
    else {
        panic!()
    };
    rule.depletion = ChanceRule::constant(1, 1);
    let now_required = compile_content(definition, ValidationMode::TestFixture).unwrap();
    assert!(readiness::Readiness::check(&now_required, Some(&profile)).is_err());
}

#[test]
fn source_combat_recovery_fixture_is_compiler_validated() {
    let pack = Pack::combat();
    let engine = WorldEngine::new(Arc::new(pack.definition.clone())).unwrap();
    engine
        .character_from_initial(
            ActorId::new("actor.test.source").unwrap(),
            "source",
            BTreeMap::new(),
        )
        .unwrap();
    let mut definition = pack.definition.clone();
    definition.mechanics.death.as_mut().unwrap().office_overflow = SourceBinding::Unresolved {
        reason: "Synthetic ordinary-key bound makes overflow inactive.".into(),
        source: fixtures::sources(),
    };
    let profile = readiness::Profile {
        id: "ordinary_normal_f2p".into(),
        excluded_items: BTreeSet::new(),
    };
    let compiled = compile_content(definition.clone(), ValidationMode::TestFixture).unwrap();
    let ready = readiness::Readiness::check(&compiled, Some(&profile)).unwrap();
    assert!(
        ready
            .inactive
            .contains_key("mechanics.death.office_overflow")
    );
    definition.mechanics.death.as_mut().unwrap().office_capacity = 1;
    let required = compile_content(definition, ValidationMode::TestFixture).unwrap();
    assert!(readiness::Readiness::check(&required, Some(&profile)).is_err());
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
    #[track_caller]
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
    name: String,
    password: String,
}

impl Endpoint {
    async fn ui(
        &self,
        account: &Account,
        joined: &game::WorldJoined,
        sequence: u64,
        operation: Uuid,
        bank_revision: Option<u64>,
        request: game::gameplay_ui_request::Request,
    ) -> Response {
        self.call(
            client_message::Command::WorldInput(game::WorldInput {
                world_session_id: joined.world_session_id.clone(),
                sequence,
                expected_character_revision: None,
                action: Some(game::world_input::Action::Ui(game::GameplayUiRequest {
                    expected_bank_revision: bank_revision.map(|revision| revision.to_string()),
                    request: Some(request),
                })),
            }),
            Some(&account.token),
            operation,
        )
        .await
    }
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
                    password: password.clone(),
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
            name: name.into(),
            password,
        }
    }

    async fn relogin(&self, account: &Account) -> Account {
        let response = self
            .call(
                client_message::Command::Login(Login {
                    login_name: account.name.clone(),
                    password: account.password.clone(),
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
            account_id: account.account_id,
            name: account.name.clone(),
            password: account.password.clone(),
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
                quote: None,
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
    let (join_operation, join_result): (Uuid, String) = sqlx::query_as(
        "SELECT operation_id, committed_result::text FROM game_lifecycle_commands
         WHERE account_id = $1 AND committed_result->'action'->>'kind' = 'join'
         ORDER BY committed_at DESC LIMIT 1",
    )
    .bind(alice.account_id)
    .fetch_one(&database.pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE game_lifecycle_commands SET committed_result =
             jsonb_set(committed_result, '{session,actor_id}', to_jsonb($3::text))
         WHERE account_id = $1 AND operation_id = $2",
    )
    .bind(alice.account_id)
    .bind(join_operation)
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
        .call(
            client_message::Command::JoinWorld(game::JoinWorld {}),
            Some(&alice.token),
            join_operation,
        )
        .await
        .error(StatusCode::INTERNAL_SERVER_ERROR);
    sqlx::query(
        "UPDATE game_lifecycle_commands SET committed_result = $3::jsonb WHERE account_id = $1 AND operation_id = $2",
    ).bind(alice.account_id).bind(join_operation).bind(join_result).execute(&database.pool).await.unwrap();
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
async fn lifecycle_logout_reconnect_revocation_and_idle_clocks_are_source_owned() {
    let database = Database::reset().await;
    let pack = Pack::new();
    let service = Live::start(pack.config(&database)).await;
    let account = service.endpoint.account("restart_live").await;
    let actor = ActorId::new(service.endpoint.create(&account).await).unwrap();
    let joined = service.endpoint.join(&account).await;
    let operation = Uuid::new_v4();
    service
        .endpoint
        .input(&account, &joined, 1, operation, practice())
        .await
        .action();
    let last_active = database.world(pack.world_id).await.state.characters[&actor]
        .runtime
        .last_active_tick;
    sleep(Duration::from_millis(1250)).await;
    service.endpoint.poll(&account, &joined, 0).await.snapshot();
    service.endpoint.join(&account).await;
    assert_eq!(
        database.world(pack.world_id).await.state.characters[&actor]
            .runtime
            .last_active_tick,
        last_active,
        "polls and repeated reconciliation/rejoin cannot claim user activity"
    );
    let leave_id = Uuid::new_v4();
    for _ in 0..2 {
        let response = service
            .endpoint
            .call(
                client_message::Command::LeaveWorld(game::LeaveWorld {
                    world_session_id: joined.world_session_id.clone(),
                }),
                Some(&account.token),
                leave_id,
            )
            .await;
        assert_eq!(response.status, StatusCode::OK);
        assert!(matches!(
            response.message.result,
            Some(server_message::Result::WorldLeft(_))
        ));
    }
    assert!(matches!(
        database.world(pack.world_id).await.state.characters[&actor]
            .runtime
            .presence,
        PresenceState::Offline { .. }
    ));
    let joined = service.endpoint.join(&account).await;
    assert_eq!(joined.next_sequence, 2);
    let confirmation = service
        .endpoint
        .input(
            &account,
            &joined,
            2,
            Uuid::new_v4(),
            game::world_input::Action::ConfirmAppearance(game::ConfirmAppearance {
                appearance: std::collections::HashMap::from([("body_type".into(), 1)]),
            }),
        )
        .await
        .action();
    assert!(
        confirmation
            .snapshot
            .unwrap()
            .player
            .unwrap()
            .appearance_confirmed
    );
    let selection = service
        .endpoint
        .input(
            &account,
            &joined,
            3,
            Uuid::new_v4(),
            game::world_input::Action::SelectExperience(game::SelectExperience {
                experience: "experience.test.new".into(),
            }),
        )
        .await
        .action();
    assert_eq!(
        selection
            .snapshot
            .unwrap()
            .player
            .unwrap()
            .experience
            .as_deref(),
        Some("experience.test.new")
    );
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
    service
        .endpoint
        .call(
            client_message::Command::CurrentAccount(CurrentAccount {}),
            Some(&account.token),
            Uuid::new_v4(),
        )
        .await
        .error(StatusCode::UNAUTHORIZED);
    let account = service.endpoint.relogin(&account).await;
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
    let restored = database.world(pack.world_id).await;
    assert_eq!(
        restored.state.characters[&actor].inventory,
        stored.state.characters[&actor].inventory
    );
    assert!(matches!(
        restored.state.characters[&actor].runtime.presence,
        PresenceState::Offline { .. }
    ));
    let rejoined = restarted.endpoint.join(&account).await;
    assert_eq!(rejoined.next_sequence, 4);
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
        .input(&account, &rejoined, 4, Uuid::new_v4(), mine())
        .await
        .action();
    let digest = crate::crypto::token_digest(&account.token).unwrap();
    crate::store::logout(&database.pool, &digest).await.unwrap();
    let after_revocation = database.world(pack.world_id).await;
    sleep(Duration::from_millis(800)).await;
    let after_wait = database.world(pack.world_id).await;
    assert!(
        after_wait.state.tick > after_revocation.state.tick,
        "one lost auth session must not freeze the world"
    );
    assert_eq!(
        after_wait.state.characters[&actor].skills,
        after_revocation.state.characters[&actor].skills
    );
    assert!(matches!(
        after_wait.state.characters[&actor].activity,
        Activity::Idle
    ));
    assert!(matches!(
        after_wait.state.characters[&actor].runtime.presence,
        PresenceState::Offline { .. }
    ));
    let health = restarted
        .endpoint
        .client
        .get(format!("{}/healthz", restarted.endpoint.origin))
        .send()
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::OK);
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
    assert!(hello.gameplay_available);
    restarted.stop().await.unwrap();
    database.pool.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn guarded_bank_shop_dialogue_and_quotes_are_real_immutable_public_views() {
    let database = Database::reset().await;
    let pack = Pack::new();
    let service = Live::start(pack.config(&database)).await;
    let account = service.endpoint.account("source_views").await;
    let actor = service.endpoint.create(&account).await;
    let joined = service.endpoint.join(&account).await;
    let other = service.endpoint.account("private_views").await;
    service.endpoint.create(&other).await;
    let other_joined = service.endpoint.join(&other).await;
    let bank = service
        .endpoint
        .input(
            &account,
            &joined,
            1,
            Uuid::new_v4(),
            game::world_input::Action::Interact(game::Interact {
                target: "spawn.test.guide".into(),
                action: "Bank".into(),
            }),
        )
        .await
        .action()
        .snapshot
        .unwrap();
    assert!(bank.player.as_ref().unwrap().bank_open);
    assert!(
        bank.bank_context
            .as_ref()
            .unwrap()
            .deposit
            .as_ref()
            .unwrap()
            .allowed
    );
    let revision = bank.character_revision;
    let quote = service
        .endpoint
        .call(
            client_message::Command::PollWorld(game::PollWorld {
                world_session_id: joined.world_session_id.clone(),
                after_revision: bank.revision,
                quote: Some(game::QuoteRequest {
                    request: Some(game::quote_request::Request::BankDeposit(
                        game::InventoryAmount {
                            inventory_slot: 2,
                            quantity: 2,
                        },
                    )),
                }),
            }),
            Some(&account.token),
            Uuid::new_v4(),
        )
        .await
        .snapshot();
    assert_eq!(
        quote.character_revision, revision,
        "a quote cannot mutate state or progression"
    );
    let Some(game::quote::Result::Bank(quote)) = quote.quote.unwrap().result else {
        panic!()
    };
    assert_eq!(quote.transferred.as_ref().unwrap().quantity, 2);
    let transferred = service
        .endpoint
        .input(
            &account,
            &joined,
            2,
            Uuid::new_v4(),
            game::world_input::Action::BankDeposit(game::BankDeposit {
                banker: "spawn.test.guide".into(),
                inventory_slot: 2,
                quantity: 2,
            }),
        )
        .await
        .action()
        .snapshot
        .unwrap();
    assert!(
        transferred
            .player
            .as_ref()
            .unwrap()
            .bank
            .iter()
            .any(|slot| slot.stack == quote.transferred)
    );
    let private = service
        .endpoint
        .poll(&other, &other_joined, 0)
        .await
        .snapshot();
    assert!(private.player.as_ref().unwrap().bank.is_empty());
    assert!(private.bank_context.is_none() && private.recovery.is_none());
    assert!(!private.events.iter().any(|event| event.actor_id == actor));
    service
        .endpoint
        .call(
            client_message::Command::PollWorld(game::PollWorld {
                world_session_id: other_joined.world_session_id.clone(),
                after_revision: private.revision,
                quote: Some(game::QuoteRequest {
                    request: Some(game::quote_request::Request::BankWithdraw(
                        game::BankWithdrawal {
                            bank_slot: 0,
                            quantity: 1,
                            noted: false,
                        },
                    )),
                }),
            }),
            Some(&other.token),
            Uuid::new_v4(),
        )
        .await
        .error(StatusCode::CONFLICT);
    let shop = service
        .endpoint
        .input(
            &account,
            &joined,
            3,
            Uuid::new_v4(),
            game::world_input::Action::Interact(game::Interact {
                target: "spawn.test.guide".into(),
                action: "Shop".into(),
            }),
        )
        .await
        .action()
        .snapshot
        .unwrap();
    assert!(!shop.player.as_ref().unwrap().bank_open);
    assert!(shop.player.as_ref().unwrap().bank.is_empty());
    let id = shop.shop.as_ref().unwrap().shop.clone();
    let quoted = service
        .endpoint
        .call(
            client_message::Command::PollWorld(game::PollWorld {
                world_session_id: joined.world_session_id.clone(),
                after_revision: shop.revision,
                quote: Some(game::QuoteRequest {
                    request: Some(game::quote_request::Request::ShopBuy(game::ShopBuy {
                        shop: id.clone(),
                        item_index: 0,
                        quantity: 1,
                        expected_item: None,
                    })),
                }),
            }),
            Some(&account.token),
            Uuid::new_v4(),
        )
        .await
        .snapshot();
    assert_eq!(quoted.character_revision, shop.character_revision);
    let Some(game::quote::Result::Shop(plan)) = quoted.quote.unwrap().result else {
        panic!()
    };
    let purchased = service
        .endpoint
        .input(
            &account,
            &joined,
            4,
            Uuid::new_v4(),
            game::world_input::Action::ShopBuy(game::ShopBuy {
                shop: id,
                item_index: 0,
                quantity: 1,
                expected_item: None,
            }),
        )
        .await
        .action()
        .snapshot
        .unwrap();
    assert_eq!(
        purchased.shop.as_ref().unwrap().lines[0].stock,
        plan.stock_after
    );
    let dialogue = service
        .endpoint
        .input(
            &account,
            &joined,
            5,
            Uuid::new_v4(),
            game::world_input::Action::Interact(game::Interact {
                target: "spawn.test.guide".into(),
                action: "Talk".into(),
            }),
        )
        .await
        .action()
        .snapshot
        .unwrap();
    assert!(dialogue.shop.is_none());
    let shown = dialogue.dialogue.as_ref().unwrap();
    assert_eq!(shown.speaker, "spawn.test.guide");
    assert!(!shown.choices.is_empty());
    service
        .endpoint
        .input(
            &account,
            &joined,
            6,
            Uuid::new_v4(),
            game::world_input::Action::DialogueChoice(game::DialogueChoice {
                speaker: shown.speaker.clone(),
                choice: "not_a_source_choice".into(),
            }),
        )
        .await
        .error(StatusCode::BAD_REQUEST);
    service
        .endpoint
        .input(
            &account,
            &joined,
            6,
            Uuid::new_v4(),
            game::world_input::Action::CloseInterface(game::Empty {}),
        )
        .await
        .action();
    service.stop().await.unwrap();
    database.pool.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn source_pending_disconnect_and_owned_grave_office_views_survive_restart() {
    use engine_fixtures::v2 as v;
    let database = Database::reset().await;
    let pack = Pack::combat();
    let service = Live::start(pack.config(&database)).await;
    let account = service.endpoint.account("vulnerable_actor").await;
    let actor = ActorId::new(service.endpoint.create(&account).await).unwrap();
    let joined = service.endpoint.join(&account).await;
    let other = service.endpoint.account("recovery_observer").await;
    service.endpoint.create(&other).await;
    let other_joined = service.endpoint.join(&other).await;
    let enemy = engine_fixtures::spawn("enemy");
    service
        .endpoint
        .input(
            &account,
            &joined,
            1,
            Uuid::new_v4(),
            game::world_input::Action::Interact(game::Interact {
                target: enemy.to_string(),
                action: "use".into(),
            }),
        )
        .await
        .action();
    service
        .endpoint
        .call(
            client_message::Command::Logout(Logout {}),
            Some(&account.token),
            Uuid::new_v4(),
        )
        .await
        .error(StatusCode::CONFLICT);
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
        .error(StatusCode::CONFLICT);
    assert_eq!(
        service
            .endpoint
            .call(
                client_message::Command::CurrentAccount(CurrentAccount {}),
                Some(&account.token),
                Uuid::new_v4()
            )
            .await
            .status,
        StatusCode::OK
    );
    crate::store::logout(
        &database.pool,
        &crate::crypto::token_digest(&account.token).unwrap(),
    )
    .await
    .unwrap();
    timeout(Duration::from_secs(3), async {
        loop {
            let world = database.world(pack.world_id).await;
            if matches!(
                world.state.characters[&actor].runtime.presence,
                PresenceState::Disconnecting { .. }
            ) {
                break;
            }
            sleep(Duration::from_millis(40)).await;
        }
    })
    .await
    .unwrap();
    let vulnerable = database.world(pack.world_id).await;
    assert_eq!(
        vulnerable.state.entities[&enemy]
            .runtime
            .retaliation_target
            .as_ref(),
        Some(&actor)
    );
    assert!(vulnerable.state.characters[&actor].hitpoints > 0);
    let visible = service
        .endpoint
        .poll(&other, &other_joined, 0)
        .await
        .snapshot();
    let body = visible
        .entities
        .iter()
        .find(|entity| entity.id == actor.as_str())
        .unwrap();
    assert!(!body.presence.as_ref().unwrap().connected);
    assert!(body.presence.as_ref().unwrap().present_in_world);
    service.stop().await.unwrap();

    // Deterministic server-only fixture advancement still uses real storage/fencing and the
    // actual source engine. No client can supply this random source or fabricate a death row.
    let store = GameStore::new(database.pool.clone());
    let lease = store
        .acquire_world_lease(pack.world_id, OWNER_LEASE)
        .await
        .unwrap();
    let engine = Arc::new(WorldEngine::new(Arc::new(pack.definition.clone())).unwrap());
    let mut world = store.load_world(pack.world_id).await.unwrap();
    for _ in 0..128 {
        if matches!(
            world.state.characters[&actor].runtime.life,
            LifeState::FirstDeathOffice { .. }
        ) {
            break;
        }
        let engine = engine.clone();
        world = store
            .commit_live_tick(&lease, world.state.tick, Vec::new(), move |world, _| {
                let context = engine.tick_context(world)?;
                engine.process_advanced_tick_with_context(world, &mut v::Hits(0), &context)
            })
            .await
            .unwrap()
            .snapshot;
    }
    assert!(matches!(
        world.state.characters[&actor].runtime.life,
        LifeState::FirstDeathOffice { .. }
    ));
    let death = world.state.characters[&actor]
        .runtime
        .active_death
        .clone()
        .unwrap();
    assert_eq!(world.state.runtime.deaths[&death].owner, actor);
    store.release_world_lease(&lease).await.unwrap();
    let restarted = Live::start(pack.config(&database)).await;
    let account = restarted.endpoint.relogin(&account).await;
    let joined = restarted.endpoint.join(&account).await;
    let other_joined = restarted.endpoint.join(&other).await;
    assert_eq!(joined.next_sequence, 2);
    restarted
        .endpoint
        .input(
            &account,
            &joined,
            2,
            Uuid::new_v4(),
            game::world_input::Action::OpenInterface(game::OpenInterface {
                interface: "interface.synthetic.grave".into(),
            }),
        )
        .await
        .error(StatusCode::SERVICE_UNAVAILABLE);
    let office = restarted
        .endpoint
        .input(
            &account,
            &joined,
            2,
            Uuid::new_v4(),
            game::world_input::Action::OpenDeathOffice(game::Empty {}),
        )
        .await
        .action()
        .snapshot
        .unwrap();
    let panel = &office.recovery.as_ref().unwrap().views[0];
    assert_eq!(panel.storage, game::RecoveryStorage::DeathOffice as i32);
    assert_eq!(panel.death, death.as_str());
    assert!(!panel.entries.is_empty());
    let selected = panel.entries[0].id.clone();
    let quote = restarted
        .endpoint
        .call(
            client_message::Command::PollWorld(game::PollWorld {
                world_session_id: joined.world_session_id.clone(),
                after_revision: office.revision,
                quote: Some(game::QuoteRequest {
                    request: Some(game::quote_request::Request::Recovery(game::Reclaim {
                        death: death.to_string(),
                        storage: game::RecoveryStorage::DeathOffice as i32,
                        items: vec![selected.clone()],
                    })),
                }),
            }),
            Some(&account.token),
            Uuid::new_v4(),
        )
        .await
        .snapshot();
    assert_eq!(quote.character_revision, office.character_revision);
    let Some(game::quote::Result::Recovery(quoted)) = quote.quote.unwrap().result else {
        panic!()
    };
    assert_eq!(quoted.full_selection_fee, 0);
    let private = restarted
        .endpoint
        .poll(&other, &other_joined, 0)
        .await
        .snapshot();
    assert!(private.recovery.is_none());
    restarted
        .endpoint
        .call(
            client_message::Command::PollWorld(game::PollWorld {
                world_session_id: other_joined.world_session_id.clone(),
                after_revision: private.revision,
                quote: Some(game::QuoteRequest {
                    request: Some(game::quote_request::Request::Recovery(game::Reclaim {
                        death: death.to_string(),
                        storage: game::RecoveryStorage::DeathOffice as i32,
                        items: vec![selected],
                    })),
                }),
            }),
            Some(&other.token),
            Uuid::new_v4(),
        )
        .await
        .error(StatusCode::CONFLICT);
    let speaker = engine_fixtures::spawn("cook").to_string();
    restarted
        .endpoint
        .input(
            &account,
            &joined,
            3,
            Uuid::new_v4(),
            game::world_input::Action::Interact(game::Interact {
                target: speaker.clone(),
                action: "use".into(),
            }),
        )
        .await
        .action();
    for (index, choice) in ["fees", "timer", "kept"].into_iter().enumerate() {
        restarted
            .endpoint
            .input(
                &account,
                &joined,
                4 + index as u64,
                Uuid::new_v4(),
                game::world_input::Action::DialogueChoice(game::DialogueChoice {
                    speaker: speaker.clone(),
                    choice: choice.into(),
                }),
            )
            .await
            .action();
    }
    let returned = restarted
        .endpoint
        .input(
            &account,
            &joined,
            7,
            Uuid::new_v4(),
            game::world_input::Action::Interact(game::Interact {
                target: engine_fixtures::spawn("portal").to_string(),
                action: "use".into(),
            }),
        )
        .await
        .action()
        .snapshot
        .unwrap();
    assert!(returned.player.as_ref().unwrap().instance.is_none());
    let grave = restarted
        .endpoint
        .input(
            &account,
            &joined,
            8,
            Uuid::new_v4(),
            game::world_input::Action::OpenGrave(game::OpenGrave {
                death: death.to_string(),
            }),
        )
        .await
        .action()
        .snapshot
        .unwrap();
    let grave_view = &grave.recovery.as_ref().unwrap().views[0];
    assert_eq!(grave_view.storage, game::RecoveryStorage::Grave as i32);
    let remaining = grave_view.active_ticks_remaining;
    let reclaim = grave_view.entries[0].id.clone();
    sleep(Duration::from_millis(1250)).await;
    let paused = restarted
        .endpoint
        .poll(&account, &joined, grave.revision)
        .await
        .snapshot();
    assert_eq!(
        paused.recovery.as_ref().unwrap().views[0].active_ticks_remaining,
        remaining,
        "the real owned grave context, not a client flag, pauses its source clock"
    );
    restarted
        .endpoint
        .input(
            &other,
            &other_joined,
            1,
            Uuid::new_v4(),
            game::world_input::Action::OpenGrave(game::OpenGrave {
                death: death.to_string(),
            }),
        )
        .await
        .error(StatusCode::CONFLICT);
    restarted
        .endpoint
        .input(
            &account,
            &joined,
            9,
            Uuid::new_v4(),
            game::world_input::Action::Reclaim(game::Reclaim {
                death: death.to_string(),
                storage: game::RecoveryStorage::Grave as i32,
                items: vec![reclaim.clone()],
            }),
        )
        .await
        .action();
    assert!(
        database.world(pack.world_id).await.state.runtime.deaths[&death]
            .reclaimed
            .contains(&RecoveryItemId::new(reclaim).unwrap())
    );
    restarted.stop().await.unwrap();
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
