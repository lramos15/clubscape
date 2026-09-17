use clubscape_game_types::{GameIntent, ItemId, Quantity, ShopId};
use clubscape_protocol::{ReadOnlyQuote, game, game_intent, quote_request};
use prost::Message;

fn input(shop: game::ShopBuy) -> game::WorldInput {
    game::WorldInput {
        world_session_id: "00000000-0000-4000-8000-000000000001".into(),
        sequence: 1,
        action: Some(game::world_input::Action::ShopBuy(shop)),
        ..Default::default()
    }
}

#[test]
fn legacy_shop_buy_wire_bytes_and_quote_envelopes_remain_exact() {
    for (index, count, encoded) in [
        (0, 1, b"\x0a\x14shop.synthetic.store\x18\x01".as_slice()),
        (
            2,
            50,
            b"\x0a\x14shop.synthetic.store\x10\x02\x18\x32".as_slice(),
        ),
    ] {
        let shop = game::ShopBuy {
            shop: "shop.synthetic.store".into(),
            item_index: index,
            quantity: count,
            expected_item: None,
        };
        assert_eq!(shop.encode_to_vec(), encoded);
        assert_eq!(game::ShopBuy::decode(encoded).unwrap(), shop);
        let request = input(shop.clone());
        let mut old_action_suffix = vec![0xca, 0x01, encoded.len() as u8];
        old_action_suffix.extend_from_slice(encoded);
        assert!(request.encode_to_vec().ends_with(&old_action_suffix));
        assert_eq!(
            game_intent(&input(shop.clone())).unwrap(),
            GameIntent::ShopBuy {
                shop: ShopId::new("shop.synthetic.store").unwrap(),
                item_index: index as u16,
                quantity: Quantity::new(count).unwrap(),
                expected_item: None,
            }
        );
        let quote = game::QuoteRequest {
            request: Some(game::quote_request::Request::ShopBuy(shop)),
        };
        let mut expected = vec![0x1a, encoded.len() as u8];
        expected.extend_from_slice(encoded);
        assert_eq!(quote.encode_to_vec(), expected);
        assert_eq!(
            quote_request(&quote).unwrap(),
            ReadOnlyQuote::ShopBuy {
                shop: ShopId::new("shop.synthetic.store").unwrap(),
                index: index as u16,
                quantity: Quantity::new(count).unwrap(),
                expected_item: None,
            }
        );
    }
}

#[test]
fn expected_item_uses_additive_tag_four_for_actions_and_read_only_quotes() {
    let encoded = b"\x0a\x14shop.synthetic.store\x10\x02\x18\x05\x22\x12item.synthetic.tin";
    let shop = game::ShopBuy {
        shop: "shop.synthetic.store".into(),
        item_index: 2,
        quantity: 5,
        expected_item: Some("item.synthetic.tin".into()),
    };
    assert_eq!(shop.encode_to_vec(), encoded);
    assert_eq!(game::ShopBuy::decode(encoded.as_slice()).unwrap(), shop);
    let request = input(shop.clone());
    let decoded = game::WorldInput::decode(request.encode_to_vec().as_slice()).unwrap();
    assert_eq!(
        game_intent(&decoded).unwrap(),
        GameIntent::ShopBuy {
            shop: ShopId::new("shop.synthetic.store").unwrap(),
            item_index: 2,
            quantity: Quantity::new(5).unwrap(),
            expected_item: Some(ItemId::new("item.synthetic.tin").unwrap()),
        }
    );
    let quote = game::QuoteRequest {
        request: Some(game::quote_request::Request::ShopBuy(shop)),
    };
    let decoded = game::QuoteRequest::decode(quote.encode_to_vec().as_slice()).unwrap();
    assert_eq!(
        quote_request(&decoded).unwrap(),
        ReadOnlyQuote::ShopBuy {
            shop: ShopId::new("shop.synthetic.store").unwrap(),
            index: 2,
            quantity: Quantity::new(5).unwrap(),
            expected_item: Some(ItemId::new("item.synthetic.tin").unwrap()),
        }
    );
}

#[test]
fn malformed_expected_item_is_not_silently_downgraded_to_a_legacy_request() {
    for invalid in [
        String::new(),
        "spawn.synthetic.tin".into(),
        "item.invalid id".into(),
        format!("item.{}", "x".repeat(4096)),
    ] {
        let shop = game::ShopBuy {
            shop: "shop.synthetic.store".into(),
            item_index: 2,
            quantity: 1,
            expected_item: Some(invalid.clone()),
        };
        let decoded = game::ShopBuy::decode(shop.encode_to_vec().as_slice()).unwrap();
        assert_eq!(decoded.expected_item, Some(invalid));
        assert!(game_intent(&input(decoded.clone())).is_err());
        assert!(
            quote_request(&game::QuoteRequest {
                request: Some(game::quote_request::Request::ShopBuy(decoded)),
            })
            .is_err()
        );
    }
}
