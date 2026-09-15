//! Shared DTO/projection fixtures only. No server capability or gameplay implementation is implied.
use clubscape_game_types::GameplayUiView;
use clubscape_wasm::gameplay_ui;
use serde_json::{Value, json};

fn permission() -> Value {
    json!({"allowed":true,"code":null,"reason":null})
}
fn item() -> Value {
    json!({"item":"item.fixture","name":"Fixture item","quantity":1,"source_id":17,
        "asset":"asset.fixture.icon","instance_id":"item_instance.fixture","charges":4})
}
fn view() -> GameplayUiView {
    let action = json!({"slot":3,"item":"item.fixture","instance":"item_instance.fixture",
        "actions":[{"id":"action.fixture","label":"Use","permission":permission()}]});
    serde_json::from_value(json!({
        "version":1,"active_tab":"interface.inventory","active_interface":"interface.fixture","document":null,
        "production":{"id":"menu.fixture","interface":"interface.production",
            "target":{"kind":"spawn","spawn":"spawn.fixture"},
            "recipes":[{"recipe":"recipe.fixture","name":"Fixture recipe","outputs":[item()],
                "single":permission(),"make_x":permission()}]},
        "reward":{"id":"presentation.fixture","kind":"quest","interface":"interface.reward",
            "title":"Fixture reward","lines":["Original source line"],"items":[item()],
            "xp":[{"skill":"skill.fixture","amount_tenths":"18446744073709551615"}],"quest_points":1,
            "quest":"quest.fixture","skill":null,"level":null,
            "continuation":{"kind":"ui_dismiss","presentation_id":"presentation.fixture"}},
        "confirmation":{"id":"confirmation.fixture","kind":"coffer_offer","title":"Confirm","lines":["Keep the exact source credit"],
            "items":[item()],"credit":"9007199254740993"},
        "interfaces":[{"interface":"interface.fixture","visibility":"locked","highlighted":true,
            "permission":{"allowed":false,"code":"requirement_not_met","reason":"Exact source denial"}}],
        "combat_style":"style.fixture","combat_styles":[{"id":"style.fixture","name":"Fixture","selected":true,"visible":true,"permission":permission()}],
        "prayers":[],"spells":[],
        "equipment":{"bonuses":{"attack":{"stab":-1},"defence":{},"melee_strength":2,"ranged_strength":3,
            "magic_damage_percent":4,"prayer":5},"weight_grams":"18446744073709551615","slots":["slot.weapon"]},
        "inventory_actions":[action.clone()],
        "bank":{"revision":"9007199254740993","capacity":8,"selected_tab":2,"insert_mode":true,"placeholders":true,"amount":5,"noted":false,
            "tabs":[{"tab":2,"first_entry":"entry.placeholder","entries":2}],
            "entries":[{"id":"entry.placeholder","slot":0,"tab":2,"item":"item.fixture","value":null,"placeholder":true},
                {"id":"entry.value","slot":1,"tab":2,"item":"item.fixture","value":item(),"placeholder":false}],
            "deposit_equipment":permission(),"unavailable_containers":[{"id":"container.fixture","label":"Source unavailable",
                "permission":{"allowed":false,"code":"unavailable","reason":"Not enabled by this source scope"}}]},
        "kept_on_death":{"scope":"normal_unsafe_non_pvp","kept":[item()],"lost":[],
            "full_grave_fee":"18446744073709551615","full_office_fee":"9007199254740993","value_revision":"9007199254740995"},
        "recovery":{"coffer_balance":"18446744073709551615","discard":permission(),"coffer_offer":permission(),"coffer_items":[action]},
        "appearance":{"choices":{"body_type":[{"value":0,"label":null,"permission":permission()}]},
            "base":{"asset":"asset.fixture.penguin","source_npc":2063,"adaptation":"adaptation.fixture"},"confirmed":false},
        "public_chat":{"permission":permission(),"maximum_bytes":120,"channel":"public",
            "messages":[{"id":"chat.fixture","actor":"actor.fixture","sender":"Fixture","channel":"public",
                "text":"Exact public text","colour":2,"effect":1}]}
    })).unwrap()
}

#[test]
fn published_ui_projects_exact_camel_case_fields_and_preserves_all_stable_selection_identities() {
    let source = view();
    let result = gameplay_ui::project(&source).unwrap();
    assert_eq!(result["version"], 1);
    assert_eq!(result["production"]["id"], "menu.fixture");
    assert_eq!(
        result["production"]["target"],
        json!({"kind":"spawn","spawn":"spawn.fixture"})
    );
    assert_eq!(
        result["production"]["recipes"][0]["outputs"][0]["id"],
        "item.fixture"
    );
    assert_eq!(
        result["production"]["recipes"][0]["outputs"][0]["instanceId"],
        "item_instance.fixture"
    );
    assert_eq!(result["production"]["recipes"][0]["makeX"]["allowed"], true);
    assert_eq!(result["reward"]["id"], "presentation.fixture");
    assert_eq!(
        result["reward"]["continuation"],
        json!({"kind":"ui_dismiss","presentation_id":"presentation.fixture"})
    );
    assert_eq!(result["confirmation"]["id"], "confirmation.fixture");
    assert_eq!(
        result["interfaces"][0]["permission"]["reason"],
        "Exact source denial"
    );
    assert_eq!(
        result["inventoryActions"][0]["actions"][0]["id"],
        "action.fixture"
    );
    assert_eq!(result["bank"]["tabs"][0]["firstEntry"], "entry.placeholder");
    assert!(result["bank"]["entries"][0]["value"].is_null());
    assert_eq!(result["bank"]["entries"][0]["placeholder"], true);
    assert_eq!(result["bank"]["entries"][1]["id"], "entry.value");
    assert_eq!(result["appearance"]["base"]["sourceNpc"], 2063);
    assert_eq!(result["publicChat"]["messages"][0]["id"], "chat.fixture");
    assert_eq!(
        result["publicChat"]["messages"][0]["text"],
        "Exact public text"
    );
    assert_eq!(result["equipment"]["bonuses"]["attack"]["stab"], -1);
    assert!(result.get("active_interface").is_none());
    assert_eq!(
        source,
        view(),
        "Projection does not mutate native source DTOs."
    );
}

#[test]
fn ui_balances_xp_revisions_weights_and_confirmation_credit_never_pass_through_js_numbers() {
    let mut source = view();
    let value = gameplay_ui::project(&source).unwrap();
    for field in [
        &value["reward"]["xp"][0]["amountTenths"],
        &value["equipment"]["weightGrams"],
        &value["keptOnDeath"]["fullGraveFee"],
        &value["recovery"]["cofferBalance"],
    ] {
        assert_eq!(field, "18446744073709551615");
    }
    assert_eq!(value["confirmation"]["credit"], "9007199254740993");
    assert_eq!(value["bank"]["revision"], "9007199254740993");
    source.equipment.weight_grams = "-9007199254740993".into();
    assert_eq!(
        gameplay_ui::project(&source).unwrap()["equipment"]["weightGrams"],
        "-9007199254740993"
    );
    source.recovery.as_mut().unwrap().coffer_balance = "18446744073709551616".into();
    assert!(gameplay_ui::project(&source).is_err());
}

#[test]
fn invalid_versions_and_spendable_placeholder_shapes_fail_without_dummy_ui_values() {
    let mut source = view();
    source.version = 2;
    assert!(gameplay_ui::project(&source).is_err());
    source = view();
    source.bank.as_mut().unwrap().entries[1].placeholder = true;
    assert!(gameplay_ui::project(&source).is_err());
    source = view();
    source.bank.as_mut().unwrap().entries[1]
        .value
        .as_mut()
        .unwrap()
        .quantity = 0;
    assert!(gameplay_ui::project(&source).is_err());
    source = view();
    source.production = None;
    source.reward = None;
    source.bank = None;
    let result = gameplay_ui::project(&source).unwrap();
    assert!(
        result["production"].is_null() && result["reward"].is_null() && result["bank"].is_null()
    );
}

#[test]
fn inventory_only_production_preserves_an_explicit_null_without_a_dummy_facility() {
    let mut source = view();
    source.production.as_mut().unwrap().target = None;
    let raw = serde_json::to_value(&source).unwrap();
    assert!(raw["production"]["target"].is_null());
    let decoded: GameplayUiView = serde_json::from_value(raw).unwrap();
    let projected = gameplay_ui::project(&decoded).unwrap();
    assert_eq!(projected["production"]["id"], "menu.fixture");
    assert!(projected["production"]["target"].is_null());
    assert_eq!(
        projected["production"]["recipes"][0]["recipe"],
        "recipe.fixture"
    );
    assert_eq!(
        projected["production"]["recipes"][0]["single"],
        permission()
    );
    assert_eq!(
        decoded, source,
        "A targetless menu is not an absent UI view."
    );

    let mut raw = serde_json::to_value(&source).unwrap();
    raw["production"].as_object_mut().unwrap().remove("target");
    let absent_wire_target: GameplayUiView = serde_json::from_value(raw).unwrap();
    assert!(gameplay_ui::project(&absent_wire_target).unwrap()["production"]["target"].is_null());

    let mut raw = serde_json::to_value(view()).unwrap();
    raw["production"]["target"] =
        json!({"kind":"temporary_object","object":"dynamic_object.fixture"});
    let placed: GameplayUiView = serde_json::from_value(raw).unwrap();
    assert_eq!(
        gameplay_ui::project(&placed).unwrap()["production"]["target"],
        json!({"kind":"temporary_object","object":"dynamic_object.fixture"})
    );
}
