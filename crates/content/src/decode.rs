use std::fmt;

use clubscape_game_types::{GameContent, GameResult};
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};

use crate::invalid;

pub const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const MAX_DATA_DEPTH: usize = 120;
pub(crate) const MAX_DATA_NODES: usize = 2_000_000;
pub(crate) const MAX_COLLECTION_ENTRIES: usize = 250_000;
pub(crate) const MAX_TEXT_BYTES: usize = 65_536;

/// Unlike direct serde deserialization into a BTreeMap, this entry point rejects
/// duplicate JSON keys, unknown fields, excessive nesting, and trailing input.
pub fn read_content_json(input: &[u8]) -> GameResult<GameContent> {
    check_size(input)?;
    let mut decoder = serde_json::Deserializer::from_slice(input);
    let value = read_value(&mut decoder).map_err(|error| invalid("input.json", error))?;
    decoder
        .end()
        .map_err(|error| invalid("input.json", error))?;
    into_content(value)
}

pub(crate) fn check_size(input: &[u8]) -> GameResult<()> {
    if input.is_empty() || input.len() > MAX_INPUT_BYTES {
        return Err(invalid(
            "input",
            format!("expected 1..={MAX_INPUT_BYTES} bytes, got {}", input.len()),
        ));
    }
    Ok(())
}

pub(crate) fn read_value<'de, D: de::Deserializer<'de>>(decoder: D) -> Result<Value, D::Error> {
    let mut budget = MAX_DATA_NODES;
    ValueSeed {
        budget: &mut budget,
        depth: 0,
    }
    .deserialize(decoder)
}

pub(crate) fn into_content(value: Value) -> GameResult<GameContent> {
    let content: GameContent =
        serde_json::from_value(value.clone()).map_err(|error| invalid("definition", error))?;
    let canonical = serde_json::to_value(&content).map_err(|error| invalid("definition", error))?;
    reject_unknown_fields(&value, &canonical, "definition")?;
    Ok(content)
}

fn reject_unknown_fields(input: &Value, canonical: &Value, path: &str) -> GameResult<()> {
    match (input, canonical) {
        (Value::Object(input), Value::Object(canonical)) => {
            for (key, value) in input {
                let nested = format!("{path}.{key}");
                let expected = canonical
                    .get(key)
                    .ok_or_else(|| invalid(&nested, "unknown field"))?;
                reject_unknown_fields(value, expected, &nested)?;
            }
        }
        (Value::Array(input), Value::Array(canonical)) => {
            for (index, (value, expected)) in input.iter().zip(canonical).enumerate() {
                reject_unknown_fields(value, expected, &format!("{path}[{index}]"))?;
            }
        }
        _ => {}
    }
    Ok(())
}

struct ValueSeed<'a> {
    budget: &'a mut usize,
    depth: usize,
}

impl<'de> DeserializeSeed<'de> for ValueSeed<'_> {
    type Value = Value;

    fn deserialize<D: de::Deserializer<'de>>(self, decoder: D) -> Result<Value, D::Error> {
        if self.depth > MAX_DATA_DEPTH {
            return Err(de::Error::custom("content nesting limit exceeded"));
        }
        *self.budget = self
            .budget
            .checked_sub(1)
            .ok_or_else(|| de::Error::custom("content node budget exceeded"))?;
        decoder.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for ValueSeed<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded content value with unique string map keys")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_none<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(Number::from(value)))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(Number::from(value)))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        if value.len() > MAX_TEXT_BYTES {
            return Err(E::custom("content string length limit exceeded"));
        }
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<Value, E> {
        if value.len() > MAX_TEXT_BYTES {
            return Err(E::custom("content string length limit exceeded"));
        }
        Ok(Value::String(value))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        check_hint(sequence.size_hint(), *self.budget)?;
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(ValueSeed {
            budget: self.budget,
            depth: self.depth + 1,
        })? {
            if values.len() == MAX_COLLECTION_ENTRIES {
                return Err(de::Error::custom(
                    "content collection length limit exceeded",
                ));
            }
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut entries: A) -> Result<Value, A::Error> {
        check_hint(entries.size_hint(), *self.budget)?;
        let mut values = Map::new();
        while let Some(key) = entries.next_key_seed(KeySeed)? {
            if values.len() == MAX_COLLECTION_ENTRIES {
                return Err(de::Error::custom(
                    "content collection length limit exceeded",
                ));
            }
            *self.budget = self
                .budget
                .checked_sub(1)
                .ok_or_else(|| de::Error::custom("content node budget exceeded"))?;
            if values.contains_key(&key) {
                return Err(de::Error::custom(format!("duplicate map key {key:?}")));
            }
            let value = entries.next_value_seed(ValueSeed {
                budget: self.budget,
                depth: self.depth + 1,
            })?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}

fn check_hint<E: de::Error>(hint: Option<usize>, budget: usize) -> Result<(), E> {
    if hint.is_some_and(|length| length > MAX_COLLECTION_ENTRIES || length > budget) {
        return Err(E::custom(
            "untrusted collection length exceeds content limits",
        ));
    }
    Ok(())
}

struct KeySeed;

impl<'de> DeserializeSeed<'de> for KeySeed {
    type Value = String;

    fn deserialize<D: de::Deserializer<'de>>(self, decoder: D) -> Result<String, D::Error> {
        decoder.deserialize_str(self)
    }
}

impl Visitor<'_> for KeySeed {
    type Value = String;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a string map key of at most 256 bytes")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<String, E> {
        if value.len() > 256 {
            return Err(E::custom("map key length limit exceeded"));
        }
        Ok(value.to_owned())
    }
}
