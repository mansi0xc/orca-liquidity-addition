use anchor_lang::prelude::*;
use crate::errors::ExchangeError;

/// Equivalent to LibMath.safeGetPartialAmountFloor.
/// Computes (numerator * target) / denominator with 0.1% rounding error check.
/// Uses u128 intermediates (RULE IR-2, L-1).
pub fn safe_get_partial_amount_floor(
    numerator: u64,
    denominator: u64,
    target: u64,
) -> Result<u64> {
    require!(denominator > 0, ExchangeError::DivisionByZero);

    if target == 0 || numerator == 0 {
        return Ok(0);
    }

    let num = numerator as u128;
    let den = denominator as u128;
    let tar = target as u128;

    let product = num.checked_mul(tar).ok_or(ExchangeError::ArithmeticOverflow)?;
    let remainder = product % den;

    // Rounding error check: 1000 * remainder >= numerator * target
    // means relative error >= 0.1%
    let lhs = remainder.checked_mul(1000).ok_or(ExchangeError::ArithmeticOverflow)?;
    require!(lhs < product, ExchangeError::RoundingError);

    Ok((product / den) as u64)
}

/// Equivalent to LibMath.safeGetPartialAmountCeil.
pub fn safe_get_partial_amount_ceil(
    numerator: u64,
    denominator: u64,
    target: u64,
) -> Result<u64> {
    require!(denominator > 0, ExchangeError::DivisionByZero);

    if target == 0 || numerator == 0 {
        return Ok(0);
    }

    let num = numerator as u128;
    let den = denominator as u128;
    let tar = target as u128;

    let product = num.checked_mul(tar).ok_or(ExchangeError::ArithmeticOverflow)?;
    let remainder = product % den;

    // Rounding error check for ceiling
    let ceil_remainder = if remainder == 0 { 0 } else { den - remainder };
    let lhs = ceil_remainder.checked_mul(1000).ok_or(ExchangeError::ArithmeticOverflow)?;
    require!(lhs < product, ExchangeError::RoundingError);

    // Ceiling division: (product + denominator - 1) / denominator
    let result = product
        .checked_add(den - 1)
        .ok_or(ExchangeError::ArithmeticOverflow)?
        / den;
    Ok(result as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_floor_basic() {
        assert_eq!(safe_get_partial_amount_floor(1, 2, 100).unwrap(), 50);
        assert_eq!(safe_get_partial_amount_floor(1, 3, 99).unwrap(), 33);
        assert_eq!(safe_get_partial_amount_floor(0, 1, 100).unwrap(), 0);
        assert_eq!(safe_get_partial_amount_floor(1, 1, 0).unwrap(), 0);
    }

    #[test]
    fn test_floor_div_by_zero() {
        assert!(safe_get_partial_amount_floor(1, 0, 100).is_err());
    }
}
