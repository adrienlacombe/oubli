use crate::error::WalletError;

/// Tongo rate: 1 tongo unit = RATE satoshis (smallest WBTC unit).
/// The on-chain Tongo contract's `get_rate()` returns this value.
const RATE: u64 = 10;

/// Convert a satoshi display string (e.g. "1000") to tongo units (internal u64).
///
/// `"1000"` → 1000 satoshis → 100 tongo units
pub fn sats_to_tongo_units(sats_str: &str) -> Result<u64, WalletError> {
    let sats_str = sats_str.trim();

    let satoshis: u64 = sats_str
        .parse()
        .map_err(|_| WalletError::Denomination(format!("invalid satoshi amount: {sats_str}")))?;

    if satoshis == 0 {
        return Err(WalletError::Denomination("amount must be > 0".into()));
    }

    // Round up to the nearest tongo unit so the user never sees a denomination error.
    Ok((satoshis + RATE - 1) / RATE)
}

/// Convert tongo units back to a satoshi display string.
pub fn tongo_units_to_sats(units: u64) -> String {
    let satoshis = units * RATE;
    satoshis.to_string()
}

/// Format a satoshi amount for user display (just the integer, no trimming needed).
pub fn format_sats_display(sats: &str) -> String {
    sats.trim().to_string()
}

/// Calculate the fee in satoshis for a given amount and percentage.
/// Returns 0 if fee_percent is zero or negative.
/// Result is always a valid tongo amount (multiple of RATE sats).
pub fn calculate_fee_sats(amount_sats: u64, fee_percent: f64) -> u64 {
    if fee_percent <= 0.0 || amount_sats == 0 {
        return 0;
    }
    let raw = (amount_sats as f64 * fee_percent / 100.0).ceil() as u64;
    // Round up to nearest multiple of RATE (1 tongo unit = RATE sats).
    // If the raw fee is 0 after rounding, no fee is charged.
    ((raw + RATE - 1) / RATE) * RATE
}

// Keep the old names as aliases during migration — the bridge and core still reference them.
pub fn btc_to_tongo_units(btc_str: &str) -> Result<u64, WalletError> {
    sats_to_tongo_units(btc_str)
}

pub fn tongo_units_to_btc(units: u64) -> String {
    tongo_units_to_sats(units)
}

pub fn format_btc_display(btc: &str) -> String {
    format_sats_display(btc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sats_to_tongo_basic() {
        // 1000 sats / 10 = 100 tongo units
        assert_eq!(sats_to_tongo_units("1000").unwrap(), 100);
    }

    #[test]
    fn sats_to_tongo_minimum() {
        // 10 sats = 1 tongo unit (minimum)
        assert_eq!(sats_to_tongo_units("10").unwrap(), 1);
    }

    #[test]
    fn sats_to_tongo_rounds_up() {
        // 7 sats rounds up to 1 tongo unit (10 sats)
        assert_eq!(sats_to_tongo_units("7").unwrap(), 1);
        // 11 sats rounds up to 2 tongo units (20 sats)
        assert_eq!(sats_to_tongo_units("11").unwrap(), 2);
        // 15 sats rounds up to 2 tongo units (20 sats)
        assert_eq!(sats_to_tongo_units("15").unwrap(), 2);
    }

    #[test]
    fn sats_to_tongo_zero() {
        assert!(sats_to_tongo_units("0").is_err());
    }

    #[test]
    fn tongo_to_sats_basic() {
        assert_eq!(tongo_units_to_sats(100), "1000");
        assert_eq!(tongo_units_to_sats(1), "10");
    }

    #[test]
    fn round_trip() {
        let original = "1000";
        let units = sats_to_tongo_units(original).unwrap();
        let back = tongo_units_to_sats(units);
        assert_eq!(back, original);
    }

    #[test]
    fn invalid_input() {
        assert!(sats_to_tongo_units("abc").is_err());
        assert!(sats_to_tongo_units("").is_err());
        assert!(sats_to_tongo_units("-1").is_err());
        assert!(sats_to_tongo_units("1.5").is_err());
    }

    #[test]
    fn fee_basic() {
        // 1% of 1000 sats = 10 sats (1 tongo unit)
        assert_eq!(calculate_fee_sats(1000, 1.0), 10);
    }

    #[test]
    fn fee_rounds_up_to_rate() {
        // 1% of 100 sats = 1 sat → rounds up to 10 (RATE)
        assert_eq!(calculate_fee_sats(100, 1.0), 10);
        // 1% of 50 sats = 0.5 → ceil = 1 → rounds to 10
        assert_eq!(calculate_fee_sats(50, 1.0), 10);
    }

    #[test]
    fn fee_zero_percent() {
        assert_eq!(calculate_fee_sats(1000, 0.0), 0);
        assert_eq!(calculate_fee_sats(1000, -1.0), 0);
    }

    #[test]
    fn fee_zero_amount() {
        assert_eq!(calculate_fee_sats(0, 1.0), 0);
    }

    #[test]
    fn fee_large_amount() {
        // 1% of 100_000 sats = 1000 sats
        assert_eq!(calculate_fee_sats(100_000, 1.0), 1000);
        // 0.5% of 100_000 sats = 500 sats
        assert_eq!(calculate_fee_sats(100_000, 0.5), 500);
    }
}
