use clubscape_game_types::{ChanceRule, GameError, GameErrorCode, GameResult};

/// Supplied by trusted authority, never by an intent or a player's connection.
pub trait RandomSource {
    /// Return an independently uniform integer in `0..upper_exclusive`.
    fn draw_below(&mut self, upper_exclusive: u32) -> GameResult<u32>;
}

pub(crate) fn draw(random: &mut impl RandomSource, upper: u32) -> GameResult<u32> {
    if upper == 0 {
        return Err(GameError::new(
            GameErrorCode::InvalidContent,
            "A random draw requires a positive range.",
        ));
    }
    let value = random.draw_below(upper)?;
    if value >= upper {
        return Err(GameError::new(
            GameErrorCode::InvalidInput,
            "Trusted random source returned a value outside the requested range.",
        ));
    }
    Ok(value)
}

/// Endpoints are actual success counts, not the source formula's low/high parameters.
/// Convert source low/high to low+1/high+1 before constructing a ChanceRule.
pub fn chance_numerator(rule: &ChanceRule, level: u16) -> GameResult<u32> {
    rule.numerator(level)
}

pub(crate) fn roll(
    rule: &ChanceRule,
    level: u16,
    random: &mut impl RandomSource,
) -> GameResult<bool> {
    let numerator = chance_numerator(rule, level)?;
    if numerator == 0 || numerator == rule.denominator {
        return Ok(numerator != 0);
    }
    Ok(draw(random, rule.denominator)? < numerator)
}
