//! Checked integer arithmetic used by the typed runtime.
//! Definitions provide source policy values; standalone numeric helpers are not content.

use clubscape_game_types::{GameError, GameErrorCode, GameResult};

use crate::RandomSource;
use crate::random::draw;

pub fn skilling_success_count(level: u16, low: u32, high: u32) -> GameResult<u32> {
    if !(1..=99).contains(&level) {
        return Err(invalid(
            "Skilling formula is defined for levels 1 through 99.",
        ));
    }
    let level = u64::from(level);
    Ok(
        (1 + (u64::from(low) * (99 - level) + u64::from(high) * (level - 1) + 49) / 98).min(256)
            as u32,
    )
}

pub fn run_drain_units(agility: u16, carried_weight_grams: i64) -> GameResult<u16> {
    if !(1..=99).contains(&agility) {
        return Err(invalid(
            "Run formula requires the bound Agility domain 1 through 99.",
        ));
    }
    let weight = carried_weight_grams.clamp(0, 64_000) as u64;
    let weighted = 60 + 67 * weight / 64_000;
    Ok((weighted * (300 - u64::from(agility)) / 300) as u16)
}

pub fn run_recovery_units(agility: u16) -> GameResult<u16> {
    if !(1..=99).contains(&agility) {
        return Err(invalid(
            "Run formula requires the bound Agility domain 1 through 99.",
        ));
    }
    Ok(agility / 10 + 15)
}

pub fn effective_level(
    current: u16,
    prayer_numerator: u16,
    prayer_denominator: u16,
    style_bonus: u16,
) -> GameResult<u32> {
    if prayer_denominator == 0 {
        return Err(invalid("Prayer multiplier has a zero denominator."));
    }
    Ok(
        u32::from(current) * u32::from(prayer_numerator) / u32::from(prayer_denominator)
            + u32::from(style_bonus)
            + 8,
    )
}

pub fn maximum_hit(effective_strength: u32, strength_bonus: i32) -> GameResult<u16> {
    let adjusted = i64::from(strength_bonus) + 64;
    if adjusted < 0 {
        return Err(invalid(
            "Damage formula is not bound for a negative strength roll.",
        ));
    }
    let hit = (u64::from(effective_strength) * adjusted as u64 + 320) / 640;
    u16::try_from(hit).map_err(|_| invalid("Maximum hit exceeds the shared hitpoint range."))
}

/// The negative-roll clamp is an explicit source-contract inference.
pub fn maximum_accuracy_roll(effective: u32, bonus: i32) -> GameResult<u32> {
    let roll = u64::from(effective) * (i64::from(bonus) + 64).max(0) as u64;
    u32::try_from(roll).map_err(|_| invalid("Accuracy roll exceeds the supported integer range."))
}

pub fn accuracy_fraction(attack: u32, defence: u32) -> (u64, u64) {
    let attack = u64::from(attack);
    let defence = u64::from(defence);
    let (numerator, denominator) = if attack > defence {
        (2 * (attack + 1) - (defence + 2), 2 * (attack + 1))
    } else {
        (attack, 2 * (defence + 1))
    };
    let divisor = gcd(numerator, denominator);
    (numerator / divisor, denominator / divisor)
}

pub fn opposed_accuracy(
    attack: u32,
    defence: u32,
    random: &mut impl RandomSource,
) -> GameResult<bool> {
    let attack_range = attack
        .checked_add(1)
        .ok_or_else(|| invalid("Attack roll overflow."))?;
    let defence_range = defence
        .checked_add(1)
        .ok_or_else(|| invalid("Defence roll overflow."))?;
    Ok(draw(random, attack_range)? > draw(random, defence_range)?)
}

pub fn player_damage(hit: bool, raw_damage: u16, maximum: u16, hp: u16) -> GameResult<u16> {
    if raw_damage > maximum || (hit && maximum == 0) {
        return Err(invalid("Invalid successful player damage roll."));
    }
    Ok(if hit { raw_damage.max(1).min(hp) } else { 0 })
}

pub fn incoming_damage(hp: u16, rolled_damage: u16, nonfatal: bool) -> u16 {
    rolled_damage.min(hp.saturating_sub(u16::from(nonfatal)))
}

pub fn hitpoints_xp_tenths(damage: u16, on_tutorial: bool) -> u64 {
    if on_tutorial {
        0
    } else {
        40 * u64::from(damage) / 3
    }
}

pub fn general_store_buy_price(base_value: u32, base_stock: u32, stock: u32) -> u64 {
    let rate = (1300 + 30 * (i64::from(base_stock) - i64::from(stock))).clamp(300, 6300);
    (u64::from(base_value) * rate as u64 / 1000).max(1)
}

pub fn general_store_sell_price(base_value: u32, base_stock: u32, stock: u32) -> u64 {
    let rate = (400 + 30 * (i64::from(base_stock) - i64::from(stock))).clamp(100, 1400);
    u64::from(base_value) * rate as u64 / 1000
}

pub fn grave_fee(values: &[u64]) -> u64 {
    values.iter().fold(0, |total, value| {
        let fee = match value {
            0..100_000 => 0,
            100_000..1_000_000 => 1000,
            1_000_000..10_000_000 => 10_000,
            _ => 100_000,
        };
        (total + fee).min(500_000)
    })
}

pub fn office_fee(values: &[u64]) -> GameResult<u64> {
    values.iter().try_fold(0_u64, |total, value| {
        // floor(value * 5 / 100) without overflowing the intermediate.
        total
            .checked_add(if *value < 100_000 { 0 } else { value / 20 })
            .ok_or_else(|| invalid("Office fees overflow the currency accumulator."))
    })
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

fn invalid(message: &str) -> GameError {
    GameError::new(GameErrorCode::InvalidInput, message)
}

pub(crate) fn rounded(
    numerator: u128,
    denominator: u128,
    rounding: &clubscape_game_types::IntegerRounding,
) -> GameResult<u64> {
    if denominator == 0 {
        return Err(invalid("Zero arithmetic denominator."));
    }
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    let value = quotient
        + u128::from(
            matches!(
                rounding,
                clubscape_game_types::IntegerRounding::NearestTiesUp
            ) && remainder >= denominator - remainder,
        );
    u64::try_from(value).map_err(|_| invalid("Arithmetic result overflow."))
}

pub(crate) fn add_fraction(
    a: &clubscape_game_types::FractionalAccumulator,
    numerator: u64,
    denominator: u64,
) -> GameResult<(u64, clubscape_game_types::FractionalAccumulator)> {
    a.validate()?;
    if denominator == 0 {
        return Err(invalid("Zero prayer-drain denominator."));
    }
    let divisor = gcd(a.denominator, denominator);
    let common = u128::from(a.denominator / divisor) * u128::from(denominator);
    let sum = (u128::from(a.numerator) * u128::from(denominator / divisor))
        .checked_add(u128::from(numerator) * u128::from(a.denominator / divisor))
        .ok_or_else(|| invalid("Fractional accumulation overflow."))?;
    let whole = u64::try_from(sum / common).map_err(|_| invalid("Fractional whole overflow."))?;
    let mut rem = sum % common;
    let mut den = common;
    let (mut x, mut y) = (rem, den);
    while y != 0 {
        (x, y) = (y, x % y);
    }
    rem /= x;
    den /= x;
    Ok((
        whole,
        clubscape_game_types::FractionalAccumulator {
            numerator: u64::try_from(rem).map_err(|_| invalid("Fractional numerator overflow."))?,
            denominator: u64::try_from(den)
                .map_err(|_| invalid("Fractional denominator overflow."))?,
        },
    ))
}
