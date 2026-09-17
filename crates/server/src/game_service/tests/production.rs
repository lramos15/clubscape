use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn inventory_production_menu_has_no_world_target_and_commits_selected_inputs_once() {
    let database = Database::reset().await;
    let mut pack = Pack::ui();
    fixtures::ui::inventory_production(&mut pack.definition);
    for (slot, item) in [(5, "flour"), (6, "water"), (8, "flour"), (9, "water")] {
        pack.definition.initial_state.inventory.slots[slot] =
            Some(fixtures::stack(&format!("item.test.{item}"), 1));
    }
    pack.write();
    let service = Live::start(pack.config(&database)).await;
    let account = service.endpoint.account("inventory_dough").await;
    let actor = ActorId::new(service.endpoint.create(&account).await).unwrap();
    let joined = service.endpoint.join(&account).await;
    let open = game::world_input::Action::UseItem(game::UseItem {
        inventory_slot: 8,
        target: Some(game::use_item::Target::OtherInventorySlot(9)),
    });
    let operation = Uuid::new_v4();
    let result = service
        .endpoint
        .input(&account, &joined, 1, operation, open.clone())
        .await
        .action()
        .snapshot
        .unwrap();
    let menu = result.ui.unwrap().production.unwrap();
    assert!(menu.target.is_none());
    assert_eq!(menu.recipes[0].recipe, "recipe.test.dough");
    let duplicate = service
        .endpoint
        .input(&account, &joined, 1, operation, open)
        .await
        .action();
    assert!(duplicate.duplicate);
    assert_eq!(
        duplicate
            .snapshot
            .unwrap()
            .ui
            .unwrap()
            .production
            .unwrap()
            .id,
        menu.id
    );
    let stored = database.world(pack.world_id).await;
    let selection = stored.state.characters[&actor]
        .runtime
        .ui
        .as_ref()
        .unwrap()
        .production
        .as_ref()
        .unwrap()
        .inventory_selection
        .as_ref()
        .unwrap();
    assert_eq!((selection.used_slot, selection.target_slot), (8, 9));
    let select = game::gameplay_ui_request::Request::Production(game::UiProductionSelection {
        menu_id: menu.id,
        recipe: "recipe.test.dough".into(),
        quantity: 1,
        mode: game::ProductionMode::MakeX as i32,
    });
    let operation = Uuid::new_v4();
    service
        .endpoint
        .ui(&account, &joined, 2, operation, None, select.clone())
        .await
        .action();
    let completed = timeout(WAIT, async {
        loop {
            let snapshot = service.endpoint.poll(&account, &joined, 0).await.snapshot();
            if snapshot
                .player
                .as_ref()
                .unwrap()
                .inventory
                .iter()
                .any(|slot| {
                    slot.stack
                        .as_ref()
                        .is_some_and(|stack| stack.item == "item.test.dough")
                })
            {
                break snapshot;
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    let inventory = &completed.player.as_ref().unwrap().inventory;
    for (index, item) in [(5, "item.test.flour"), (6, "item.test.water")] {
        assert_eq!(
            inventory
                .iter()
                .find(|slot| slot.index == index)
                .unwrap()
                .stack
                .as_ref()
                .unwrap()
                .item,
            item
        );
    }
    let stored = database.world(pack.world_id).await;
    assert!(
        stored.state.characters[&actor]
            .runtime
            .ui
            .as_ref()
            .unwrap()
            .production_input
            .is_none()
    );
    let expected = stored.state.characters[&actor].inventory.clone();
    service.stop().await.unwrap();
    let restarted = Live::start(pack.config(&database)).await;
    let rejoined = restarted.endpoint.join(&account).await;
    assert_eq!(rejoined.next_sequence, 3);
    assert!(
        restarted
            .endpoint
            .ui(&account, &rejoined, 2, operation, None, select)
            .await
            .action()
            .duplicate
    );
    assert_eq!(
        database.world(pack.world_id).await.state.characters[&actor].inventory,
        expected
    );
    restarted.stop().await.unwrap();
    database.pool.close().await;
}
