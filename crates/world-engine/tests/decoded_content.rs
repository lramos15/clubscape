//! Synthetic world execution through the real tagged content decoder.
//! Wind Strike's independent source bands are not a fixed-hit substitute.
mod support;

use std::collections::{BTreeMap, VecDeque};

use clubscape_game_types::*;
use clubscape_world_engine::RandomSource;
use support::{v2 as v, *};

struct SourceDraws(VecDeque<(u32, u32)>);

impl RandomSource for SourceDraws {
    fn draw_below(&mut self, upper_exclusive: u32) -> GameResult<u32> {
        let (expected_upper, value) = self.0.pop_front().expect("unexpected source draw");
        assert_eq!(upper_exclusive, expected_upper);
        Ok(value)
    }
}

#[test]
fn decoded_source_wind_strike_table_executes_every_band_and_boundary() {
    for (level, expected_hit) in [
        (1_u16, 2_u16),
        (4, 2),
        (5, 4),
        (8, 4),
        (9, 6),
        (12, 6),
        (13, 8),
        (99, 8),
    ] {
        let mut content = v::content();
        v::with_combat(&mut content);
        let magic = v::named_skill("magic");
        content.skills.get_mut(&magic).unwrap().xp_thresholds_tenths =
            (0..99).map(|index| index * 100).collect();
        let initial_xp = u64::from(level - 1) * 100;
        content.initial_state.skills.insert(
            magic.clone(),
            SkillState {
                xp_tenths: initial_xp,
                current_level: level,
            },
        );
        content
            .npcs
            .get_mut(&v::npc())
            .unwrap()
            .combat
            .as_mut()
            .unwrap()
            .hitpoints = 20;
        give_initial(&mut content, &[stack("rune", 1), stack("coins", 1)]);

        let json = serde_json::to_string(&content).unwrap();
        let decoded: GameContent = serde_json::from_str(&json).unwrap();
        let maximum = decoded.mechanics.combat_styles[&v::style("magic")]
            .maximum_hit
            .require()
            .unwrap();
        assert!(matches!(maximum, MaximumHitFormula::LevelTable { hits, .. }
            if hits == &BTreeMap::from([(1, 2), (5, 4), (9, 6), (13, 8)])));

        let (engine, mut world) = setup(decoded);
        let attack = (u32::from(level) + 8) * 64;
        let mut random = SourceDraws(
            [
                (attack + 1, attack),
                (491, 0),
                (u32::from(expected_hit) + 1, u32::from(expected_hit)),
            ]
            .into(),
        );
        engine
            .apply_intent(
                &mut world,
                &actor(),
                &GameIntent::Cast {
                    spell: v::spell().to_string(),
                    target: Some(spawn("enemy")),
                },
                &mut random,
            )
            .unwrap();
        assert!(random.0.is_empty());
        assert_eq!(count(&engine, &world, "rune"), 0);
        assert_eq!(count(&engine, &world, "coins"), 0);
        assert_eq!(world.entities[&spawn("enemy")].hitpoints, 20);

        let events = v::tick_n(&engine, &mut world, 2, &mut NeverDraw);
        assert_eq!(world.entities[&spawn("enemy")].hitpoints, 20 - expected_hit);
        assert!(events.iter().any(|event| matches!(&event.event,
            GameEvent::SpellResolved { spell, outcome: SpellOutcome::Hit, damage, .. }
            if spell == &v::spell() && *damage == expected_hit)));
        assert_eq!(
            state(&world).skills[&magic].xp_tenths,
            initial_xp + 55 + 20 * u64::from(expected_hit),
        );
    }
}

#[test]
fn malformed_hit_table_keys_fail_inside_full_bound_game_content() {
    let mut content = v::content();
    v::with_combat(&mut content);
    let canonical = serde_json::to_string(&content).unwrap();
    let original = r#""hits":{"1":2,"5":4,"9":6,"13":8}"#;
    assert_eq!(canonical.matches(original).count(), 1);
    for invalid in [
        r#""hits":{"01":2,"5":4,"9":6,"13":8}"#,
        r#""hits":{"1":2,"1":4,"9":6,"13":8}"#,
        r#""hits":{"65536":2,"5":4,"9":6,"13":8}"#,
        r#""hits":{"-1":2,"5":4,"9":6,"13":8}"#,
    ] {
        let malformed = canonical.replacen(original, invalid, 1);
        assert!(serde_json::from_str::<GameContent>(&malformed).is_err());
    }
}
