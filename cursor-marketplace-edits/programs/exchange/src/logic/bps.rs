use anchor_lang::prelude::*;
use crate::errors::ExchangeError;

/// Equivalent to LibBps.bps: value * bpsValue / 10000
/// Uses u128 intermediate to prevent overflow (RULE IR-1).
pub fn bps(value: u64, bps_value: u16) -> Result<u64> {
    let result = (value as u128)
        .checked_mul(bps_value as u128)
        .ok_or(ExchangeError::ArithmeticOverflow)?
        / 10000u128;
    Ok(result as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bps_basic() {
        assert_eq!(bps(10000, 500).unwrap(), 500);
        assert_eq!(bps(10000, 10000).unwrap(), 10000);
        assert_eq!(bps(10000, 0).unwrap(), 0);
        assert_eq!(bps(0, 500).unwrap(), 0);
    }

    #[test]
    fn test_bps_large_values() {
        assert_eq!(bps(u64::MAX, 1).unwrap(), 1844674407370955);
    }
}
