use crate::constants::SECONDS_PER_DAY;

/// Calculate the increment and remainder for the rate limit based on the slope and the time since the last refresh.
pub fn calculate_rate_limit_increment(
    unix_timestamp: i64,
    last_refresh_timestamp: i64,
    rate_limit_slope: u64,
    rate_limit_remainder: u64,
) -> (u64, u64) {
    let time_passed = unix_timestamp.checked_sub(last_refresh_timestamp).unwrap();
    // Calculate the amount of units that accrued via lapsed time.
    // Carries the remainder over from the last time this ran to
    // prevent precision errors that could lead to DOS.
    let accrued_units = u128::from(rate_limit_slope)
        .checked_mul(time_passed as u128)
        .unwrap()
        .checked_add(rate_limit_remainder as u128)
        .unwrap();
    // clamp to u64::MAX in the event of overflow
    let increment = u64::try_from(accrued_units / SECONDS_PER_DAY as u128).unwrap_or(u64::MAX);
    // Clamp to 0 if remainder is larger than u64::MAX
    let remainder = u64::try_from(accrued_units % SECONDS_PER_DAY as u128).unwrap_or(0);
    (increment, remainder)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_rate_limit_increment() {
        let unix_timestamp = 101;
        let last_refresh_timestamp = 100;
        let rate_limit_slope = SECONDS_PER_DAY; // 1 per second

        let (increment, remainder) = calculate_rate_limit_increment(
            unix_timestamp,
            last_refresh_timestamp,
            rate_limit_slope,
            0,
        );

        assert_eq!(increment, 1);
        assert_eq!(remainder, 0);

        // Should have remainder and no increment when not enough time elapsed
        let rate_limit_slope = 32_200; // 0.5 per second
        let (increment, remainder) = calculate_rate_limit_increment(
            unix_timestamp,
            last_refresh_timestamp,
            rate_limit_slope,
            0,
        );

        assert_eq!(increment, 0);
        assert_eq!(remainder, rate_limit_slope);

        let unix_timestamp = (SECONDS_PER_DAY * 2 + 200) as i64;
        let last_refresh_timestamp = SECONDS_PER_DAY as i64;
        let (increment, remainder) =
            calculate_rate_limit_increment(unix_timestamp, last_refresh_timestamp, u64::MAX, 0);
        assert_eq!(increment, u64::MAX);
        assert_eq!(remainder, 31_800);
    }
}
