use std::{collections::BTreeMap, fmt};

use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, Visitor},
};

struct LevelKey(u16);

impl<'de> Deserialize<'de> for LevelKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct KeyVisitor;
        impl Visitor<'_> for KeyVisitor {
            type Value = LevelKey;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a canonical unsigned 16-bit level key")
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
                u16::try_from(value)
                    .map(LevelKey)
                    .map_err(|_| E::custom("level key is out of range"))
            }

            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
                u16::try_from(value)
                    .map(LevelKey)
                    .map_err(|_| E::custom("level key is out of range"))
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                if value.is_empty()
                    || !value.bytes().all(|byte| byte.is_ascii_digit())
                    || (value.len() > 1 && value.starts_with('0'))
                {
                    return Err(E::custom("level key is not canonical decimal"));
                }
                value
                    .parse::<u16>()
                    .map(LevelKey)
                    .map_err(|_| E::custom("level key is out of range"))
            }

            fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
                self.visit_str(&value)
            }
        }
        deserializer.deserialize_any(KeyVisitor)
    }
}

pub(crate) fn level_hits<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<u16, u16>, D::Error> {
    struct TableVisitor;
    impl<'de> Visitor<'de> for TableVisitor {
        type Value = BTreeMap<u16, u16>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a unique level-to-hit table")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut result = BTreeMap::new();
            while let Some((LevelKey(level), hit)) = map.next_entry::<LevelKey, u16>()? {
                if result.insert(level, hit).is_some() {
                    return Err(de::Error::custom("duplicate level key"));
                }
            }
            Ok(result)
        }
    }
    // Internally tagged enums buffer JSON map keys as strings, unlike direct numeric-map decoding.
    deserializer.deserialize_map(TableVisitor)
}

#[cfg(test)]
mod tests {
    use crate::{MaximumHitFormula, SourceBinding};

    #[test]
    fn source_wind_strike_table_survives_tagged_and_bound_json() {
        let json = r#"{"kind":"level_table","skill":"skill.magic","basis":"current","hits":{"1":2,"5":4,"9":6,"13":8}}"#;
        let formula: MaximumHitFormula = serde_json::from_str(json).unwrap();
        let MaximumHitFormula::LevelTable { hits, .. } = &formula else {
            panic!("wrong formula")
        };
        assert_eq!(
            hits.iter()
                .map(|(level, hit)| (*level, *hit))
                .collect::<Vec<_>>(),
            [(1, 2), (5, 4), (9, 6), (13, 8)]
        );
        let encoded = serde_json::to_string(&formula).unwrap();
        assert_eq!(
            serde_json::from_str::<MaximumHitFormula>(&encoded).unwrap(),
            formula
        );
        let bound = format!(r#"{{"status":"bound","value":{json},"source":[]}}"#);
        let binding: SourceBinding<MaximumHitFormula> = serde_json::from_str(&bound).unwrap();
        assert_eq!(binding.require().unwrap(), &formula);
    }

    #[test]
    fn malformed_or_ambiguous_numeric_keys_are_not_coerced() {
        for key in ["01", "+1", "-1", "65536", " 1", "1.0", ""] {
            let json = format!(
                r#"{{"kind":"level_table","skill":"skill.magic","basis":"current","hits":{{{key:?}:2}}}}"#
            );
            assert!(
                serde_json::from_str::<MaximumHitFormula>(&json).is_err(),
                "{key}"
            );
        }
        let duplicate = r#"{"kind":"level_table","skill":"skill.magic","basis":"current","hits":{"1":2,"1":4}}"#;
        assert!(serde_json::from_str::<MaximumHitFormula>(duplicate).is_err());
    }
}
