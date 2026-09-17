use super::*;
use engine_fixtures::v2 as v;
use game::gameplay_ui_request::Request as Ui;

struct NoDraw;
impl clubscape_world_engine::RandomSource for NoDraw {
    fn draw_below(&mut self, _: u32) -> GameResult<u32> {
        panic!("UI amounts and recovery must not draw gameplay RNG")
    }
}

fn priced_pack(funds: u32) -> Pack {
    let mut pack = ui::recovery_pack();
    pack.definition.initial_state.inventory = Inventory::default();
    pack.definition.initial_state.inventory.slots[0] = Some(engine_fixtures::stack("arrow", 11));
    pack.definition.initial_state.bank.slots = vec![Some(engine_fixtures::stack("coins", funds))];
    pack.definition.initial_state.hitpoints = 1;
    for provider in pack.definition.mechanics.value_providers.values_mut() {
        let SourceBinding::Bound { value, .. } = &mut provider.values else {
            panic!()
        };
        value.insert(engine_fixtures::item("arrow"), 100_000);
    }
    pack.write();
    pack
}

fn native_office(pack: &Pack) -> (WorldEngine, WorldState, ActorId, DeathId) {
    let engine = WorldEngine::new(Arc::new(pack.definition.clone())).unwrap();
    let actor: ActorId = fixtures::id("actor.amounts.owner");
    let mut world = engine.initial_world().unwrap();
    world.characters.insert(
        actor.clone(),
        engine
            .character_from_initial(actor.clone(), "Owner", BTreeMap::new())
            .unwrap(),
    );
    engine
        .apply_lifecycle(&mut world, &actor, LifecycleTransition::Join)
        .unwrap();
    engine
        .apply_intent(
            &mut world,
            &actor,
            &engine_fixtures::interact("enemy"),
            &mut v::Hits(0),
        )
        .unwrap();
    for _ in 0..64 {
        if matches!(
            world.characters[&actor].runtime.life,
            LifeState::FirstDeathOffice { .. }
        ) {
            break;
        }
        let context = engine.tick_context(&world).unwrap();
        engine
            .tick_with_context(&mut world, &mut v::Hits(0), &context)
            .unwrap();
    }
    assert!(matches!(
        world.characters[&actor].runtime.life,
        LifeState::FirstDeathOffice { .. }
    ));
    while let Some(reward) = engine.ui_view(&world, &actor).unwrap().reward {
        engine
            .apply_intent(
                &mut world,
                &actor,
                &GameIntent::Ui {
                    request: reward.continuation,
                },
                &mut NoDraw,
            )
            .unwrap();
    }
    engine
        .apply_intent(
            &mut world,
            &actor,
            &GameIntent::OpenDeathOffice,
            &mut NoDraw,
        )
        .unwrap();
    let death = world.characters[&actor]
        .runtime
        .active_death
        .clone()
        .unwrap();
    (engine, world, actor, death)
}

fn management(engine: &WorldEngine, world: &WorldState, actor: &ActorId) -> RecoveryManagementView {
    engine
        .ui_view(world, actor)
        .unwrap()
        .recovery
        .unwrap()
        .management
        .unwrap()
}

fn next(engine: &WorldEngine, world: &mut WorldState) {
    let context = engine.tick_context(world).unwrap();
    engine
        .tick_with_context(world, &mut v::Hits(0), &context)
        .unwrap();
}

fn apply(
    engine: &WorldEngine,
    world: &mut WorldState,
    actor: &ActorId,
    request: GameplayUiRequest,
) -> GameResult<Vec<clubscape_world_engine::ActorEvent>> {
    engine.apply_intent(world, actor, &GameIntent::Ui { request }, &mut NoDraw)
}

fn arrow_row(view: &RecoveryManagementView) -> &RecoveryEntryControlView {
    view.panels
        .iter()
        .flat_map(|panel| &panel.entries)
        .find(|entry| entry.item.item == engine_fixtures::item("arrow"))
        .unwrap()
}

#[test]
fn native_recovery_one_five_x_all_uses_unit_fees_and_retains_partial_identity() {
    let pack = priced_pack(100_000);
    let (engine, mut world, actor, death) = native_office(&pack);
    let before = world.clone();
    let view = management(&engine, &world, &actor);
    assert_eq!(world, before);
    let row = arrow_row(&view);
    assert_eq!(row.unit_fee, "5000");
    assert_eq!(row.full_stack_fee, "55000");
    assert_eq!(row.inventory_capacity, 11);
    assert_eq!(row.bank_capacity, 11);
    assert!(row.take.allowed && row.bank.allowed);
    let id = row.id.clone();
    for (amount, total) in [
        (
            UiAmount::Quantity {
                quantity: Quantity::new(1).unwrap(),
            },
            1,
        ),
        (
            UiAmount::Quantity {
                quantity: Quantity::new(5).unwrap(),
            },
            6,
        ),
        (
            UiAmount::Quantity {
                quantity: Quantity::new(2).unwrap(),
            },
            8,
        ),
        (UiAmount::All {}, 11),
    ] {
        next(&engine, &mut world);
        apply(
            &engine,
            &mut world,
            &actor,
            GameplayUiRequest::RecoveryTake {
                death: death.clone(),
                storage: RecoveryStorage::DeathOffice,
                items: vec![RecoveryItemAmount {
                    id: id.clone(),
                    amount,
                }],
            },
        )
        .unwrap();
        let character = &world.characters[&actor];
        assert_eq!(
            clubscape_simulation::inventory::count(
                &character.inventory,
                &engine.content().items,
                &engine_fixtures::item("arrow")
            )
            .unwrap(),
            total
        );
        assert_eq!(
            clubscape_simulation::bank::count(
                &character.bank,
                &engine.content().items,
                &engine_fixtures::item("coins")
            )
            .unwrap(),
            100_000 - 5000 * total
        );
        if total < 11 {
            let view = management(&engine, &world, &actor);
            let row = arrow_row(&view);
            assert_eq!(row.id, id);
            assert_eq!(row.item.quantity, 11 - total);
            assert_eq!(row.full_stack_fee, (5000 * (11 - total)).to_string());
            assert!(!world.runtime.deaths[&death].reclaimed.contains(&id));
        } else {
            assert!(world.runtime.deaths[&death].reclaimed.contains(&id));
        }
        let encoded = serde_json::to_vec(&world).unwrap();
        world = serde_json::from_slice(&encoded).unwrap();
    }
    next(&engine, &mut world);
    let before = world.clone();
    assert_eq!(
        apply(
            &engine,
            &mut world,
            &actor,
            GameplayUiRequest::RecoveryTake {
                death,
                storage: RecoveryStorage::DeathOffice,
                items: vec![RecoveryItemAmount {
                    id,
                    amount: UiAmount::All {}
                }],
            }
        )
        .unwrap_err()
        .code,
        GameErrorCode::NotOwned
    );
    assert_eq!(world, before);
}

#[test]
fn native_recovery_funds_capacity_and_bad_identity_never_charge_a_failed_selection() {
    let pack = priced_pack(15_000);
    let (engine, mut world, actor, death) = native_office(&pack);
    let view = management(&engine, &world, &actor);
    let row = arrow_row(&view);
    assert_eq!(row.inventory_capacity, 3);
    let id = row.id.clone();
    next(&engine, &mut world);
    let before = world.clone();
    assert_eq!(
        apply(
            &engine,
            &mut world,
            &actor,
            GameplayUiRequest::RecoveryTake {
                death: death.clone(),
                storage: RecoveryStorage::DeathOffice,
                items: vec![
                    RecoveryItemAmount {
                        id: id.clone(),
                        amount: UiAmount::Quantity {
                            quantity: Quantity::new(1).unwrap()
                        }
                    },
                    RecoveryItemAmount {
                        id: fixtures::id("recovery_item.not_owned"),
                        amount: UiAmount::All {}
                    },
                ],
            }
        )
        .unwrap_err()
        .code,
        GameErrorCode::NotOwned
    );
    assert_eq!(world, before);
    let events = apply(
        &engine,
        &mut world,
        &actor,
        GameplayUiRequest::RecoveryTake {
            death: death.clone(),
            storage: RecoveryStorage::DeathOffice,
            items: vec![RecoveryItemAmount {
                id: id.clone(),
                amount: UiAmount::Quantity {
                    quantity: Quantity::new(5).unwrap(),
                },
            }],
        },
    )
    .unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(&event.event, GameEvent::Message { text }
        if text.contains("Some recovery items remain")))
    );
    let view = management(&engine, &world, &actor);
    let row = arrow_row(&view);
    assert_eq!(row.item.quantity, 8);
    assert_eq!(row.inventory_capacity, 0);
    assert_eq!(row.take.code, Some(GameErrorCode::InsufficientItems));
    next(&engine, &mut world);
    let before = world.clone();
    assert_eq!(
        apply(
            &engine,
            &mut world,
            &actor,
            GameplayUiRequest::RecoveryTake {
                death,
                storage: RecoveryStorage::DeathOffice,
                items: vec![RecoveryItemAmount {
                    id,
                    amount: UiAmount::All {}
                }],
            }
        )
        .unwrap_err()
        .code,
        GameErrorCode::InsufficientItems
    );
    assert_eq!(world, before);
}

#[test]
fn native_bank_all_is_source_gated_and_uses_the_same_bank_without_inventory_staging() {
    let mut pack = priced_pack(100_000);
    let note = pack
        .definition
        .items
        .values()
        .find(|item| item.unnoted_variant.is_some())
        .unwrap()
        .clone();
    pack.definition.initial_state.inventory.slots[1] = Some(ItemStack {
        item: note.id.clone(),
        quantity: Quantity::new(7).unwrap(),
        instance: None,
    });
    pack.write();
    let (engine, mut world, actor, _) = native_office(&pack);
    let view = management(&engine, &world, &actor);
    assert!(view.bank_all.allowed);
    let mut denied = engine.content().clone();
    denied
        .ui
        .as_mut()
        .unwrap()
        .recovery
        .as_mut()
        .unwrap()
        .office_bank = RecoveryBankRule::Unavailable {
        reason: "Synthetic source profile has no Office banking action.".into(),
        source: fixtures::sources(),
    };
    let denied = WorldEngine::new(Arc::new(denied)).unwrap();
    let denied_view = management(&denied, &world, &actor);
    assert!(!denied_view.bank_all.allowed);
    assert_eq!(arrow_row(&denied_view).bank_capacity, 0);
    next(&engine, &mut world);
    let before = world.clone();
    assert_eq!(
        apply(
            &denied,
            &mut world,
            &actor,
            GameplayUiRequest::RecoveryBankAll {
                records: view.bank_all_records.clone(),
            }
        )
        .unwrap_err()
        .code,
        GameErrorCode::Unavailable
    );
    assert_eq!(world, before);
    let mut stale = view.bank_all_records.clone();
    stale[0].items.pop();
    assert_eq!(
        apply(
            &engine,
            &mut world,
            &actor,
            GameplayUiRequest::RecoveryBankAll { records: stale }
        )
        .unwrap_err()
        .code,
        GameErrorCode::StaleCommand
    );
    assert_eq!(world, before);
    let inventory = world.characters[&actor].inventory.clone();
    apply(
        &engine,
        &mut world,
        &actor,
        GameplayUiRequest::RecoveryBankAll {
            records: view.bank_all_records,
        },
    )
    .unwrap();
    let character = &world.characters[&actor];
    assert_eq!(character.inventory, inventory);
    assert_eq!(
        clubscape_simulation::bank::count(
            &character.bank,
            &engine.content().items,
            &engine_fixtures::item("arrow")
        )
        .unwrap(),
        11
    );
    assert_eq!(
        clubscape_simulation::bank::count(
            &character.bank,
            &engine.content().items,
            &note.unnoted_variant.unwrap()
        )
        .unwrap(),
        7
    );
    assert_eq!(
        clubscape_simulation::bank::count(
            &character.bank,
            &engine.content().items,
            &engine_fixtures::item("coins")
        )
        .unwrap(),
        45_000
    );
    assert!(character.bank.slots.iter().flatten().all(|stack| {
        engine.content().items[&stack.item]
            .unnoted_variant
            .is_none()
    }));
    assert!(!management(&engine, &world, &actor).bank_all.allowed);
}

#[test]
fn native_recovery_reports_real_full_container_limits_without_charging_failed_transfers() {
    let mut pack = priced_pack(100_000);
    for slot in &mut pack.definition.initial_state.inventory.slots[1..] {
        *slot = Some(engine_fixtures::stack("egg", 1));
    }
    pack.definition
        .initial_state
        .runtime
        .settings
        .death_auto_equip = Some(false);
    pack.definition.initial_state.bank.capacity = 1;
    pack.write();
    let (engine, mut world, actor, death) = native_office(&pack);
    let view = management(&engine, &world, &actor);
    assert_eq!(arrow_row(&view).bank_capacity, 0);
    assert_eq!(
        arrow_row(&view).bank.code,
        Some(GameErrorCode::InventoryFull)
    );
    next(&engine, &mut world);
    let before = world.clone();
    assert_eq!(
        apply(
            &engine,
            &mut world,
            &actor,
            GameplayUiRequest::RecoveryBankAll {
                records: view.bank_all_records,
            }
        )
        .unwrap_err()
        .code,
        GameErrorCode::InventoryFull
    );
    assert_eq!(world, before);
    let view = management(&engine, &world, &actor);
    let selected = arrow_row(&view).id.clone();
    let items = view
        .panels
        .iter()
        .flat_map(|panel| &panel.entries)
        .filter(|entry| entry.id != selected)
        .map(|entry| RecoveryItemAmount {
            id: entry.id.clone(),
            amount: UiAmount::All {},
        })
        .collect();
    apply(
        &engine,
        &mut world,
        &actor,
        GameplayUiRequest::RecoveryTake {
            death: death.clone(),
            storage: RecoveryStorage::DeathOffice,
            items,
        },
    )
    .unwrap();
    assert!(
        world.characters[&actor]
            .inventory
            .slots
            .iter()
            .all(Option::is_some)
    );
    let view = management(&engine, &world, &actor);
    assert_eq!(arrow_row(&view).inventory_capacity, 0);
    assert_eq!(
        arrow_row(&view).take.code,
        Some(GameErrorCode::InventoryFull)
    );
    next(&engine, &mut world);
    let before = world.clone();
    assert_eq!(
        apply(
            &engine,
            &mut world,
            &actor,
            GameplayUiRequest::RecoveryTake {
                death,
                storage: RecoveryStorage::DeathOffice,
                items: vec![RecoveryItemAmount {
                    id: selected,
                    amount: UiAmount::All {}
                }],
            }
        )
        .unwrap_err()
        .code,
        GameErrorCode::InventoryFull
    );
    assert_eq!(world, before);
}

#[test]
fn native_recovery_paid_credit_is_not_divided_or_recharged_after_partial_serialization() {
    let pack = priced_pack(100_000);
    let (engine, mut world, actor, death) = native_office(&pack);
    let id = arrow_row(&management(&engine, &world, &actor)).id.clone();
    let record = world.runtime.deaths.get_mut(&death).unwrap();
    let entry = record
        .grave
        .as_mut()
        .unwrap()
        .items
        .iter_mut()
        .find(|entry| entry.id == id)
        .unwrap();
    entry.fee_paid = 6500; // A previously paid source-credit persistence boundary, not a new grant.
    let encoded = serde_json::to_vec(&world).unwrap();
    world = serde_json::from_slice(&encoded).unwrap();
    let view = management(&engine, &world, &actor);
    assert_eq!(arrow_row(&view).unit_fee, "0");
    assert_eq!(arrow_row(&view).full_stack_fee, "48500");
    for quantity in [1, 5, 5] {
        next(&engine, &mut world);
        apply(
            &engine,
            &mut world,
            &actor,
            GameplayUiRequest::RecoveryTake {
                death: death.clone(),
                storage: RecoveryStorage::DeathOffice,
                items: vec![RecoveryItemAmount {
                    id: id.clone(),
                    amount: UiAmount::Quantity {
                        quantity: Quantity::new(quantity).unwrap(),
                    },
                }],
            },
        )
        .unwrap();
        world = serde_json::from_slice(&serde_json::to_vec(&world).unwrap()).unwrap();
    }
    assert_eq!(
        clubscape_simulation::bank::count(
            &world.characters[&actor].bank,
            &engine.content().items,
            &engine_fixtures::item("coins")
        )
        .unwrap(),
        51_500
    );
}

#[test]
fn native_grave_combined_quote_and_all_transfer_share_the_source_fee_cap() {
    let mut pack = priced_pack(1_000_000);
    pack.definition.initial_state.inventory.slots[0] = Some(engine_fixtures::stack("arrow", 6));
    for provider in pack.definition.mechanics.value_providers.values_mut() {
        let SourceBinding::Bound { value, .. } = &mut provider.values else {
            panic!()
        };
        value.insert(engine_fixtures::item("arrow"), 10_000_000);
    }
    pack.write();
    let (engine, mut world, actor, death) = native_office(&pack);
    next(&engine, &mut world);
    engine
        .apply_intent(
            &mut world,
            &actor,
            &engine_fixtures::interact("cook"),
            &mut NoDraw,
        )
        .unwrap();
    for topic in ["fees", "timer", "kept"] {
        next(&engine, &mut world);
        engine
            .apply_intent(
                &mut world,
                &actor,
                &engine_fixtures::select(topic),
                &mut NoDraw,
            )
            .unwrap();
    }
    next(&engine, &mut world);
    engine
        .apply_intent(
            &mut world,
            &actor,
            &engine_fixtures::interact("portal"),
            &mut NoDraw,
        )
        .unwrap();
    for _ in 0..16 {
        next(&engine, &mut world);
        if world.characters[&actor].runtime.instance.is_none() {
            break;
        }
    }
    assert!(world.characters[&actor].runtime.instance.is_none());
    engine
        .apply_intent(
            &mut world,
            &actor,
            &GameIntent::OpenGrave {
                death: death.clone(),
            },
            &mut NoDraw,
        )
        .unwrap();
    let view = management(&engine, &world, &actor);
    let row = arrow_row(&view);
    assert_eq!(row.unit_fee, "100000");
    assert_eq!(row.full_stack_fee, "500000");
    assert_eq!(view.panels[0].full_selection_fee, "500000");
    let id = row.id.clone();
    next(&engine, &mut world);
    apply(
        &engine,
        &mut world,
        &actor,
        GameplayUiRequest::RecoveryTake {
            death,
            storage: RecoveryStorage::Grave,
            items: vec![RecoveryItemAmount {
                id,
                amount: UiAmount::All {},
            }],
        },
    )
    .unwrap();
    assert_eq!(
        clubscape_simulation::bank::count(
            &world.characters[&actor].bank,
            &engine.content().items,
            &engine_fixtures::item("coins")
        )
        .unwrap(),
        500_000
    );
}

fn wire_amount(amount: UiAmount) -> game::UiAmount {
    game::UiAmount {
        selection: Some(match amount {
            UiAmount::Quantity { quantity } => game::ui_amount::Selection::Quantity(quantity.get()),
            UiAmount::All {} => game::ui_amount::Selection::All(game::Empty {}),
        }),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn real_bank_all_default_survives_auth_renewal_and_checks_revision_after_deduplication() {
    let database = Database::reset().await;
    let pack = ui::pack();
    let service = Live::start(pack.config(&database)).await;
    let account = service.endpoint.account("amount_bank").await;
    let actor = ActorId::new(service.endpoint.create(&account).await).unwrap();
    let joined = service.endpoint.join(&account).await;
    let open = game::world_input::Action::Interact(game::Interact {
        target: "spawn.test.guide".into(),
        action: "Bank".into(),
    });
    let opened = service
        .endpoint
        .input(&account, &joined, 1, Uuid::new_v4(), open.clone())
        .await
        .action()
        .snapshot
        .unwrap();
    assert_eq!(
        opened.bank_context.as_ref().unwrap().banker,
        "spawn.test.guide"
    );
    let revision = opened
        .ui
        .as_ref()
        .unwrap()
        .bank
        .as_ref()
        .unwrap()
        .revision
        .parse()
        .unwrap();
    let before = database.world(pack.world_id).await.state.characters[&actor].clone();
    let operation = Uuid::new_v4();
    let request = Ui::BankAmount(game::UiBankAmount {
        amount: Some(wire_amount(UiAmount::All {})),
        noted: true,
    });
    let saved = service
        .endpoint
        .ui(
            &account,
            &joined,
            2,
            operation,
            Some(revision),
            request.clone(),
        )
        .await
        .action()
        .snapshot
        .unwrap();
    let bank = saved.ui.as_ref().unwrap().bank.as_ref().unwrap();
    assert_eq!(bank.amount_selection, Some(wire_amount(UiAmount::All {})));
    assert!(bank.noted);
    service
        .endpoint
        .ui(
            &account,
            &joined,
            3,
            Uuid::new_v4(),
            Some(bank.revision.parse().unwrap()),
            Ui::InsertMode(game::UiToggle { enabled: true }),
        )
        .await
        .action();
    assert!(
        service
            .endpoint
            .ui(
                &account,
                &joined,
                2,
                operation,
                Some(revision),
                request.clone()
            )
            .await
            .action()
            .duplicate
    );
    service
        .endpoint
        .ui(
            &account,
            &joined,
            4,
            Uuid::new_v4(),
            Some(revision),
            Ui::BankOptions(game::UiBankOptions {
                amount: 5,
                noted: false,
            }),
        )
        .await
        .error(StatusCode::CONFLICT);
    crate::store::logout(
        &database.pool,
        &crate::crypto::token_digest(&account.token).unwrap(),
    )
    .await
    .unwrap();
    service.stop().await.unwrap();
    let restarted = Live::start(pack.config(&database)).await;
    let account = restarted.endpoint.relogin(&account).await;
    let rejoined = restarted.endpoint.join(&account).await;
    assert_eq!(rejoined.next_sequence, 4);
    assert!(
        restarted
            .endpoint
            .ui(&account, &rejoined, 2, operation, Some(revision), request)
            .await
            .action()
            .duplicate
    );
    let opened = restarted
        .endpoint
        .input(&account, &rejoined, 4, Uuid::new_v4(), open)
        .await
        .action()
        .snapshot
        .unwrap();
    let bank = opened.ui.as_ref().unwrap().bank.as_ref().unwrap();
    assert_eq!(
        opened.bank_context.as_ref().unwrap().banker,
        "spawn.test.guide"
    );
    assert_eq!(bank.amount_selection, Some(wire_amount(UiAmount::All {})));
    let after = database.world(pack.world_id).await.state.characters[&actor].clone();
    assert_eq!(before.inventory, after.inventory);
    assert_eq!(before.bank, after.bank);
    assert_eq!(before.skills, after.skills);
    assert_eq!(before.runtime.entitlements, after.runtime.entitlements);
    let literal = restarted
        .endpoint
        .ui(
            &account,
            &rejoined,
            5,
            Uuid::new_v4(),
            Some(bank.revision.parse().unwrap()),
            Ui::BankOptions(game::UiBankOptions {
                amount: 5,
                noted: false,
            }),
        )
        .await
        .action()
        .snapshot
        .unwrap()
        .ui
        .unwrap()
        .bank
        .unwrap();
    assert_eq!(
        literal.amount_selection,
        Some(wire_amount(UiAmount::Quantity {
            quantity: Quantity::new(5).unwrap()
        }))
    );
    restarted.stop().await.unwrap();
    database.pool.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn real_partial_recovery_and_bank_all_preserve_fees_identity_and_restart_receipts() {
    let database = Database::reset().await;
    let pack = priced_pack(100_000);
    let service = Live::start(pack.config(&database)).await;
    let account = service.endpoint.account("amount_recovery").await;
    let actor = ActorId::new(service.endpoint.create(&account).await).unwrap();
    let joined = service.endpoint.join(&account).await;
    let other = service.endpoint.account("amount_other").await;
    service.endpoint.create(&other).await;
    service.endpoint.join(&other).await;
    service
        .endpoint
        .input(
            &account,
            &joined,
            1,
            Uuid::new_v4(),
            game::world_input::Action::Interact(game::Interact {
                target: engine_fixtures::spawn("enemy").to_string(),
                action: "use".into(),
            }),
        )
        .await
        .action();
    crate::store::logout(
        &database.pool,
        &crate::crypto::token_digest(&account.token).unwrap(),
    )
    .await
    .unwrap();
    service.stop().await.unwrap();
    let death = ui::finish_disconnected_death(&database, &pack, &actor).await;
    let restarted = Live::start(pack.config(&database)).await;
    let account = restarted.endpoint.relogin(&account).await;
    let joined = restarted.endpoint.join(&account).await;
    let other_joined = restarted.endpoint.join(&other).await;
    let mut sequence = joined.next_sequence;
    let mut snapshot = joined.snapshot.clone().unwrap();
    while let Some(reward) = &snapshot.ui.as_ref().unwrap().reward {
        snapshot = restarted
            .endpoint
            .ui(
                &account,
                &joined,
                sequence,
                Uuid::new_v4(),
                None,
                Ui::Dismiss(game::UiIdentity {
                    id: reward.id.clone(),
                }),
            )
            .await
            .action()
            .snapshot
            .unwrap();
        sequence += 1;
    }
    snapshot = restarted
        .endpoint
        .input(
            &account,
            &joined,
            sequence,
            Uuid::new_v4(),
            game::world_input::Action::OpenDeathOffice(game::Empty {}),
        )
        .await
        .action()
        .snapshot
        .unwrap();
    sequence += 1;
    let scene = snapshot.scene.as_ref().unwrap();
    let template = pack
        .definition
        .mechanics
        .death
        .as_ref()
        .unwrap()
        .first_office
        .require()
        .unwrap()
        .instance
        .as_ref()
        .unwrap()
        .to_string();
    assert_eq!(scene.instance_template.as_deref(), Some(template.as_str()));
    let instance = InstanceId::new(scene.instance.as_ref().unwrap()).unwrap();
    assert_eq!(
        database.world(pack.world_id).await.state.runtime.instances[&instance]
            .owner
            .as_ref(),
        Some(&actor)
    );
    let controls = snapshot
        .ui
        .as_ref()
        .unwrap()
        .recovery
        .as_ref()
        .unwrap()
        .management
        .as_ref()
        .unwrap();
    let row = controls
        .panels
        .iter()
        .flat_map(|panel| &panel.entries)
        .find(|row| {
            row.item.as_ref().unwrap().stack.as_ref().unwrap().item
                == engine_fixtures::item("arrow").to_string()
        })
        .unwrap();
    assert_eq!(
        (row.unit_fee.as_str(), row.full_stack_fee.as_str()),
        ("5000", "55000")
    );
    assert_eq!(row.inventory_capacity, 11);
    let selected = row.id.clone();
    let take = |amount| {
        Ui::RecoveryTake(game::UiRecoveryTake {
            death: death.to_string(),
            storage: game::RecoveryStorage::DeathOffice as i32,
            items: vec![game::UiRecoveryItemAmount {
                id: selected.clone(),
                amount: Some(wire_amount(amount)),
            }],
        })
    };
    restarted
        .endpoint
        .ui(
            &other,
            &other_joined,
            other_joined.next_sequence,
            Uuid::new_v4(),
            None,
            take(UiAmount::All {}),
        )
        .await
        .error(StatusCode::CONFLICT);
    let first_sequence = sequence;
    let first_operation = Uuid::new_v4();
    let first_request = take(UiAmount::Quantity {
        quantity: Quantity::new(1).unwrap(),
    });
    snapshot = restarted
        .endpoint
        .ui(
            &account,
            &joined,
            sequence,
            first_operation,
            None,
            first_request.clone(),
        )
        .await
        .action()
        .snapshot
        .unwrap();
    sequence += 1;
    for (quantity, remaining) in [(5, 5), (2, 3)] {
        snapshot = restarted
            .endpoint
            .ui(
                &account,
                &joined,
                sequence,
                Uuid::new_v4(),
                None,
                take(UiAmount::Quantity {
                    quantity: Quantity::new(quantity).unwrap(),
                }),
            )
            .await
            .action()
            .snapshot
            .unwrap();
        sequence += 1;
        let entry = snapshot
            .recovery
            .as_ref()
            .unwrap()
            .views
            .iter()
            .flat_map(|view| &view.entries)
            .find(|entry| entry.id == selected)
            .unwrap();
        assert_eq!(entry.stack.as_ref().unwrap().quantity, remaining);
        assert_eq!(entry.full_entry_fee, u64::from(remaining) * 5000);
    }
    assert!(
        restarted
            .endpoint
            .ui(
                &account,
                &joined,
                first_sequence,
                first_operation,
                None,
                first_request.clone()
            )
            .await
            .action()
            .duplicate
    );
    let management = snapshot
        .ui
        .as_ref()
        .unwrap()
        .recovery
        .as_ref()
        .unwrap()
        .management
        .as_ref()
        .unwrap();
    let revision: u64 = management.bank_revision.parse().unwrap();
    let bank_request = Ui::RecoveryBankAll(game::UiRecoveryBankAll {
        records: management.bank_all_records.clone(),
    });
    restarted
        .endpoint
        .ui(
            &account,
            &joined,
            sequence,
            Uuid::new_v4(),
            Some(0),
            bank_request.clone(),
        )
        .await
        .error(StatusCode::CONFLICT);
    let bank_operation = Uuid::new_v4();
    restarted
        .endpoint
        .ui(
            &account,
            &joined,
            sequence,
            bank_operation,
            Some(revision),
            bank_request.clone(),
        )
        .await
        .action();
    let saved = database.world(pack.world_id).await;
    let character = &saved.state.characters[&actor];
    assert_eq!(
        clubscape_simulation::inventory::count(
            &character.inventory,
            &pack.definition.items,
            &engine_fixtures::item("arrow")
        )
        .unwrap(),
        8
    );
    assert_eq!(
        clubscape_simulation::bank::count(
            &character.bank,
            &pack.definition.items,
            &engine_fixtures::item("arrow")
        )
        .unwrap(),
        3
    );
    assert_eq!(
        clubscape_simulation::bank::count(
            &character.bank,
            &pack.definition.items,
            &engine_fixtures::item("coins")
        )
        .unwrap(),
        45_000
    );
    assert!(
        saved.state.runtime.deaths[&death]
            .reclaimed
            .contains(&RecoveryItemId::new(&selected).unwrap())
    );
    let private = restarted
        .endpoint
        .poll(&other, &other_joined, 0)
        .await
        .snapshot();
    assert!(private.recovery.is_none() && private.ui.unwrap().recovery.is_none());
    assert!(private.scene.unwrap().instance.is_none());
    crate::store::logout(
        &database.pool,
        &crate::crypto::token_digest(&account.token).unwrap(),
    )
    .await
    .unwrap();
    restarted.stop().await.unwrap();
    let again = Live::start(pack.config(&database)).await;
    let account = again.endpoint.relogin(&account).await;
    let rejoined = again.endpoint.join(&account).await;
    assert_eq!(rejoined.next_sequence, sequence + 1);
    assert!(
        again
            .endpoint
            .ui(
                &account,
                &rejoined,
                first_sequence,
                first_operation,
                None,
                first_request
            )
            .await
            .action()
            .duplicate
    );
    assert!(
        again
            .endpoint
            .ui(
                &account,
                &rejoined,
                sequence,
                bank_operation,
                Some(revision),
                bank_request
            )
            .await
            .action()
            .duplicate
    );
    let loaded = database.world(pack.world_id).await;
    assert_eq!(
        loaded.state.characters[&actor].inventory,
        character.inventory
    );
    assert_eq!(loaded.state.characters[&actor].bank, character.bank);
    assert_eq!(loaded.state.runtime.deaths, saved.state.runtime.deaths);
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM processed_game_commands WHERE operation_id = ANY($1)",
    )
    .bind(vec![first_operation, bank_operation])
    .fetch_one(&database.pool)
    .await
    .unwrap();
    assert_eq!(count, 2);
    again.stop().await.unwrap();
    database.pool.close().await;
}
