mod common;

use clubscape_content::*;
use common::*;
use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn artifact() -> Vec<u8> {
    encode_compiled(&compile_content(fixture(), ValidationMode::TestFixture).unwrap()).unwrap()
}

fn replace_payload(base: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut output = base[..ARTIFACT_HEADER_BYTES].to_vec();
    output[12..20].copy_from_slice(&(payload.len() as u64).to_le_bytes());
    output[20..52].copy_from_slice(&Sha256::digest(payload));
    output.extend_from_slice(payload);
    output
}

#[test]
fn strict_json_round_trips_and_distinguishes_content_validation_from_decoding() {
    let expected = fixture();
    let json = serde_json::to_vec(&expected).unwrap();
    let parsed = read_content_json(&json).unwrap();
    assert_eq!(parsed, expected);
    compile_content(parsed, ValidationMode::TestFixture).unwrap();
    let mut value = serde_json::to_value(expected).unwrap();
    value["schema_version"] = json!(clubscape_game_types::CONTENT_SCHEMA_VERSION + 1);
    let parsed = read_content_json(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(compile_content(parsed, ValidationMode::TestFixture).is_err());
}

#[test]
fn unknown_fields_cannot_hide_misspelled_or_unimplemented_rules() {
    for path in [
        "",
        "/initial_state",
        "/items/item.test.pickaxe",
        "/spawns/spawn.test.rock/interactions/0/guard",
        "/spawns/spawn.test.rock/interactions/0/action",
        "/quests/quest.test.errand/transitions/0/effects/0",
    ] {
        let mut value = serde_json::to_value(fixture()).unwrap();
        value
            .pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unknown_field".into(), json!(1));
        let error = read_content_json(&serde_json::to_vec(&value).unwrap()).unwrap_err();
        assert!(error.message.contains("unknown field"), "{path}: {error}");
    }
}

#[test]
fn json_rejects_duplicate_keys_at_root_and_inside_definition_maps() {
    let content = fixture();
    let json = serde_json::to_string(&content).unwrap();
    let duplicate = format!("{{\"revision\":\"duplicate\",{}", &json[1..]);
    assert!(
        read_content_json(duplicate.as_bytes())
            .unwrap_err()
            .message
            .contains("duplicate map key")
    );
    let item = serde_json::to_string(&content.items[&id("item.test.coins")]).unwrap();
    let duplicate = json.replacen(
        "\"items\":{",
        &format!("\"items\":{{\"item.test.coins\":{item},"),
        1,
    );
    assert!(
        read_content_json(duplicate.as_bytes())
            .unwrap_err()
            .message
            .contains("duplicate map key")
    );
}

#[test]
fn invalid_ids_coordinates_quantities_inventory_shape_and_unknown_variants_fail_on_input() {
    for (path, replacement) in [
        ("/initial_state/tile/plane", json!(4)),
        ("/initial_state/tile/x", json!(16384)),
        ("/initial_state/inventory/slots/0/quantity", json!(0)),
        ("/initial_state/inventory/slots/0/quantity", json!(u32::MAX)),
        ("/initial_state/inventory/slots/0/quantity", json!(-1)),
        ("/initial_state/inventory/slots", json!([])),
        ("/items/item.test.pickaxe/id", json!("npc.test.pickaxe")),
        (
            "/spawns/spawn.test.rock/interactions/0/guard/kind",
            json!("invented_guard"),
        ),
        (
            "/spawns/spawn.test.rock/interactions/0/action/kind",
            json!("invented_action"),
        ),
        (
            "/quests/quest.test.errand/transitions/0/effects/0/kind",
            json!("invented_effect"),
        ),
        ("/skills/skill.test.mining/maximum_xp_tenths", json!(1.5)),
    ] {
        let mut value = serde_json::to_value(fixture()).unwrap();
        *value.pointer_mut(path).unwrap() = replacement;
        assert!(
            read_content_json(&serde_json::to_vec(&value).unwrap()).is_err(),
            "{path}"
        );
    }
    let mut value = serde_json::to_value(fixture()).unwrap();
    value["initial_state"]
        .as_object_mut()
        .unwrap()
        .remove("skills");
    assert!(read_content_json(&serde_json::to_vec(&value).unwrap()).is_err());
}

#[test]
fn input_resource_limits_trailing_documents_and_oversized_strings_fail() {
    let mut json = serde_json::to_vec(&fixture()).unwrap();
    json.extend_from_slice(b" {}");
    assert!(
        read_content_json(&json)
            .unwrap_err()
            .message
            .contains("trailing")
    );
    assert!(read_content_json(&[]).is_err());
    let too_large = vec![b' '; MAX_INPUT_BYTES + 1];
    assert!(read_content_json(&too_large).is_err());
    assert!(load_compiled(&too_large, ValidationMode::TestFixture).is_err());
    let nested = format!("{}0{}", "[".repeat(150), "]".repeat(150));
    assert!(read_content_json(nested.as_bytes()).is_err());
    let mut value = serde_json::to_value(fixture()).unwrap();
    value["revision"] = json!("x".repeat(65_537));
    assert!(
        read_content_json(&serde_json::to_vec(&value).unwrap())
            .unwrap_err()
            .message
            .contains("string length")
    );
}

#[test]
fn artifacts_round_trip_deterministically_without_serialized_indexes_or_reports() {
    let compiled = compile_content(fixture(), ValidationMode::TestFixture).unwrap();
    let bytes = encode_compiled(&compiled).unwrap();
    assert_eq!(bytes, encode_compiled(&compiled).unwrap());
    assert_eq!(&bytes[..8], b"CLSCONT\0");
    assert_eq!(&bytes[8..10], &ARTIFACT_VERSION.to_le_bytes());
    assert_eq!(&bytes[10..12], &1_u16.to_le_bytes());
    let loaded = load_compiled(&bytes, ValidationMode::TestFixture).unwrap();
    assert_eq!(loaded.definition(), compiled.definition());
    assert_eq!(loaded.counts(), compiled.counts());
    assert_eq!(
        loaded.collision(tile(1003, 1003)),
        compiled.collision(tile(1003, 1003))
    );
    assert_eq!(loaded.spawns_at_tile(tile(1003, 1003)).count(), 2);
    assert_eq!(bytes, encode_compiled(&loaded).unwrap());
    let payload: Value = rmp_serde::from_slice(&bytes[ARTIFACT_HEADER_BYTES..]).unwrap();
    assert!(payload.get("collision").is_none());
    assert!(payload.get("report").is_none());
    assert!(payload.get("baseline").is_some());
    assert!(payload["initial_state"].get("source").is_some());
    assert_eq!(
        sha256(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn json_whitespace_and_field_order_do_not_change_the_binary_artifact() {
    let value = serde_json::to_value(fixture()).unwrap();
    let reversed = format!(
        "{{\n{}\n}}",
        value
            .as_object()
            .unwrap()
            .iter()
            .rev()
            .map(|(key, value)| {
                format!(
                    "{} : {}",
                    serde_json::to_string(key).unwrap(),
                    serde_json::to_string_pretty(value).unwrap()
                )
            })
            .collect::<Vec<_>>()
            .join(",\n")
    );
    let definition = read_content_json(reversed.as_bytes()).unwrap();
    let compiled = compile_content(definition, ValidationMode::TestFixture).unwrap();
    assert_eq!(encode_compiled(&compiled).unwrap(), artifact());
}

#[test]
fn every_truncation_and_corrupt_envelope_is_rejected() {
    let good = artifact();
    for cut in 0..good.len() {
        assert!(
            load_compiled(&good[..cut], ValidationMode::TestFixture).is_err(),
            "cut {cut}"
        );
    }
    for index in [0, 8, 10, 12, 20, 52, ARTIFACT_HEADER_BYTES + 7] {
        let mut corrupt = good.clone();
        corrupt[index] ^= 0x80;
        assert!(
            load_compiled(&corrupt, ValidationMode::TestFixture).is_err(),
            "byte {index}"
        );
    }
    let mut oversized = good.clone();
    oversized[12..20].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(
        load_compiled(&oversized, ValidationMode::TestFixture)
            .unwrap_err()
            .message
            .contains("size limit")
    );
    let mut trailing = good.clone();
    trailing.push(0);
    assert!(
        load_compiled(&trailing, ValidationMode::TestFixture)
            .unwrap_err()
            .message
            .contains("trailing")
    );
    let mut payload = good[ARTIFACT_HEADER_BYTES..].to_vec();
    payload.push(0xc0);
    let trailing = replace_payload(&good, &payload);
    assert!(
        load_compiled(&trailing, ValidationMode::TestFixture)
            .unwrap_err()
            .message
            .contains("trailing")
    );
}

#[test]
fn loader_revalidates_definitions_modes_identity_and_optional_manifest_checks() {
    let good = artifact();
    let mut content = fixture();
    content.recipes.values_mut().next().unwrap().ticks = Some(0);
    let corrupt = replace_payload(
        &good,
        &rmp_serde::to_vec_named(&serde_json::to_value(&content).unwrap()).unwrap(),
    );
    assert!(
        load_compiled(&corrupt, ValidationMode::TestFixture)
            .unwrap_err()
            .message
            .contains("duration")
    );
    assert!(load_compiled(&good, ValidationMode::Runtime).is_err());
    let mut content = fixture();
    content.revision = "different-content-revision".into();
    let corrupt = replace_payload(
        &good,
        &rmp_serde::to_vec_named(&serde_json::to_value(&content).unwrap()).unwrap(),
    );
    assert!(
        load_compiled(&corrupt, ValidationMode::TestFixture)
            .unwrap_err()
            .message
            .contains("identity")
    );
    let mut compiled = compile_content(fixture(), ValidationMode::TestFixture).unwrap();
    let manifest = AssetManifest {
        identity: "synthetic-assets-v1".into(),
        assets: compiled.referenced_assets().clone(),
    };
    compiled.check_asset_manifest(&manifest).unwrap();
    let loaded = load_compiled(
        &encode_compiled(&compiled).unwrap(),
        ValidationMode::TestFixture,
    )
    .unwrap();
    assert!(
        loaded.report().asset_manifest.is_none(),
        "loader cannot trust a past manifest check"
    );
    let runtime = compile_content(runtime_policy_fixture(), ValidationMode::Runtime).unwrap();
    let runtime =
        load_compiled(&encode_compiled(&runtime).unwrap(), ValidationMode::Runtime).unwrap();
    assert!(runtime.report().evidence.inference > 0);
    assert!(!runtime.report().source_verification_performed);
}

#[test]
fn forged_collection_and_string_lengths_are_rejected_without_declared_length_allocations() {
    let good = artifact();
    for payload in [
        &[0xdd, 0xff, 0xff, 0xff, 0xff][..],
        &[0xdf, 0xff, 0xff, 0xff, 0xff][..],
        &[0xdb, 0xff, 0xff, 0xff, 0xff][..],
        &[0xc6, 0xff, 0xff, 0xff, 0xff][..],
        &[0xc1][..],
    ] {
        let corrupt = replace_payload(&good, payload);
        assert!(load_compiled(&corrupt, ValidationMode::TestFixture).is_err());
    }
    let mut nested = vec![0x91; 150];
    nested.push(0xc0);
    let corrupt = replace_payload(&good, &nested);
    assert!(
        load_compiled(&corrupt, ValidationMode::TestFixture)
            .unwrap_err()
            .message
            .contains("nesting")
    );
}

struct DuplicateRoot(Value);

struct ReversedRoot(Value);

impl Serialize for ReversedRoot {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let entries = self.0.as_object().unwrap();
        let mut map = serializer.serialize_map(Some(entries.len()))?;
        for (key, value) in entries.iter().rev() {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

impl Serialize for DuplicateRoot {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let entries = self.0.as_object().unwrap();
        let mut map = serializer.serialize_map(Some(entries.len() + 1))?;
        for (key, value) in entries {
            map.serialize_entry(key, value)?;
        }
        map.serialize_entry("revision", &entries["revision"])?;
        map.end()
    }
}

#[test]
fn messagepack_rejects_duplicate_unknown_and_noncanonical_fields_even_with_valid_checksums() {
    let good = artifact();
    let value = serde_json::to_value(fixture()).unwrap();
    let duplicate = rmp_serde::to_vec_named(&DuplicateRoot(value.clone())).unwrap();
    let error = load_compiled(
        &replace_payload(&good, &duplicate),
        ValidationMode::TestFixture,
    )
    .unwrap_err();
    assert!(error.message.contains("duplicate map key"));
    let noncanonical = rmp_serde::to_vec_named(&ReversedRoot(value.clone())).unwrap();
    let error = load_compiled(
        &replace_payload(&good, &noncanonical),
        ValidationMode::TestFixture,
    )
    .unwrap_err();
    assert!(error.message.contains("noncanonical"));
    let mut value = value;
    value["serialized_index"] = json!({});
    let payload = rmp_serde::to_vec_named(&value).unwrap();
    let error = load_compiled(
        &replace_payload(&good, &payload),
        ValidationMode::TestFixture,
    )
    .unwrap_err();
    assert!(error.message.contains("unknown field"));
}
