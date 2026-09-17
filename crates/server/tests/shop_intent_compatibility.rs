use clubscape_game_types::{GameIntent, ItemId, Quantity, ShopId};
use sha2::{Digest, Sha256};

fn canonical_bytes(intent: &GameIntent) -> Vec<u8> {
    let mut value = serde_json::to_value(intent).unwrap();
    value.sort_all_objects();
    serde_json::to_vec(&value).unwrap()
}

fn persisted_hash(intent: &GameIntent) -> String {
    let mut hash = Sha256::new();
    hash.update(b"clubscape.game-intent.v1\0");
    hash.update(canonical_bytes(intent));
    format!("{:x}", hash.finalize())
}

#[test]
fn legacy_shop_buy_constructors_keep_exact_json_canonical_bytes_and_v1_hashes() {
    // Fixed and formerly committed extra-row requests captured before expected_item existed.
    for (index, quantity, json, canonical, hash) in [
        (
            0,
            1,
            r#"{"kind":"shop_buy","shop":"shop.synthetic.store","item_index":0,"quantity":1}"#,
            r#"{"item_index":0,"kind":"shop_buy","quantity":1,"shop":"shop.synthetic.store"}"#,
            "0ee2cb464004eb50443397455fd0db58ed5853f9285932dd6144bc14fa3e5184",
        ),
        (
            2,
            50,
            r#"{"kind":"shop_buy","shop":"shop.synthetic.store","item_index":2,"quantity":50}"#,
            r#"{"item_index":2,"kind":"shop_buy","quantity":50,"shop":"shop.synthetic.store"}"#,
            "d443138a19f2caddfc2ecbfb2a70dea15b704fce0d50d4e8f980c8532ad403ee",
        ),
    ] {
        let intent = GameIntent::ShopBuy {
            shop: ShopId::new("shop.synthetic.store").unwrap(),
            item_index: index,
            quantity: Quantity::new(quantity).unwrap(),
            expected_item: None,
        };
        let restored: GameIntent = serde_json::from_str(json).unwrap();
        assert_eq!(restored, intent);
        assert_eq!(serde_json::to_vec(&intent).unwrap(), json.as_bytes());
        assert_eq!(serde_json::to_vec(&restored).unwrap(), json.as_bytes());
        assert_eq!(canonical_bytes(&intent), canonical.as_bytes());
        assert_eq!(persisted_hash(&intent), hash);
        assert_eq!(persisted_hash(&restored), hash);
    }
}

#[test]
fn explicit_null_keeps_legacy_hash_but_supplied_item_identity_is_part_of_the_hash() {
    let legacy_hash = "0ee2cb464004eb50443397455fd0db58ed5853f9285932dd6144bc14fa3e5184";
    let restored: GameIntent = serde_json::from_str(
        r#"{"kind":"shop_buy","shop":"shop.synthetic.store","item_index":0,"quantity":1,"expected_item":null}"#,
    )
    .unwrap();
    assert_eq!(persisted_hash(&restored), legacy_hash);

    let selected = |item| GameIntent::ShopBuy {
        shop: ShopId::new("shop.synthetic.store").unwrap(),
        item_index: 0,
        quantity: Quantity::new(1).unwrap(),
        expected_item: Some(ItemId::new(item).unwrap()),
    };
    let pot = selected("item.synthetic.pot");
    let tin = selected("item.synthetic.tin");
    assert_eq!(
        canonical_bytes(&pot),
        br#"{"expected_item":"item.synthetic.pot","item_index":0,"kind":"shop_buy","quantity":1,"shop":"shop.synthetic.store"}"#,
    );
    assert_ne!(persisted_hash(&pot), legacy_hash);
    assert_ne!(persisted_hash(&pot), persisted_hash(&tin));
}
