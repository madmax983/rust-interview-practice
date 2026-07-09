//! # DateTime Implementation
//!
//! Implements a minimal custom Date and Time library to handle epoch conversions, leap year logic,
//! and ISO-8601 string formatting without external dependencies.
//!
//! **Replaces Crates:** `chrono`, `time`
//!
//! **Real-world Usage:**
//! - Logging and auditing timestamps.
//! - Scheduling (e.g., cron jobs).
//! - TTL calculations for caches and session expiration.
//!
//! **Why build it yourself?**
//! Building a datetime library teaches you about the Proleptic Gregorian Calendar, epoch arithmetic,
//! leap year rules, and how to safely cast and calculate complex offsets. It exposes the hidden
//! complexity of what seems like simple "time math".

use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      DateTime
//      ├─ year: i32
//      ├─ month: u8 (1-12)
//      ├─ day: u8 (1-31)
//      ├─ hour: u8 (0-23)
//      ├─ minute: u8 (0-59)
//      └─ second: u8 (0-59)
//
// Invariants:
// 1. Represents a valid date and time in the Proleptic Gregorian Calendar (UTC).
// 2. `month` is 1..=12.
// 3. `day` is valid for the given `year` and `month` (e.g., 29 in Feb on leap years).
// 4. `hour`, `minute`, `second` are within standard bounds.
//
// Complexity:
// ┌───────────────┬────────┬────────┐
// │ Operation     │ Time   │ Space  │
// ├───────────────┼────────┼────────┤
// │ from_timestamp│ O(N)   │ O(1)   │
// │ to_timestamp  │ O(N)   │ O(1)   │
// │ to_iso8601    │ O(1)   │ O(1)   │
// └───────────────┴────────┴────────┘
// Note: Time complexity for conversions is O(N) where N is the difference in years from 1970,
// due to the simple iterative approach used for learning.
//
// Design Decisions:
// - **UTC Only**: Timezones and Daylight Saving Time (DST) introduce massive complexity
//   and usually require an external database (tzdata). We stick to UTC.
// - **Proleptic Gregorian**: We assume the Gregorian leap year rules apply backwards infinitely,
//   even before 1582, consistent with ISO-8601.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DateTime {
    year: i32,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
}

impl DateTime {
    const SECONDS_IN_MINUTE: i64 = 60;
    const SECONDS_IN_HOUR: i64 = 3600;
    const SECONDS_IN_DAY: i64 = 86400;

    /// Creates a new DateTime, validating the inputs.
    #[must_use]
    pub fn new(year: i32, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Option<Self> {
        if !(1..=12).contains(&month) {
            return None;
        }
        if day < 1 || day > Self::days_in_month(year, month) {
            return None;
        }
        if hour > 23 || minute > 59 || second > 59 {
            return None;
        }

        Some(Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        })
    }

    /// Returns the current UTC DateTime using system time.
    #[must_use]
    pub fn now() -> Self {
        // RUST INSIGHT: `SystemTime` handles the OS-level interaction to get the current time,
        // but it doesn't provide structured date formatting.
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");

        // We cast u64 seconds to i64 for epoch math. Safe until the year 292,277,026,596.
        #[allow(clippy::cast_possible_wrap)]
        Self::from_timestamp(duration.as_secs() as i64)
    }

    /// Converts a UNIX timestamp (seconds since 1970-01-01T00:00:00Z) to a DateTime.
    #[must_use]
    pub fn from_timestamp(timestamp: i64) -> Self {
        // GOTCHA: Epoch arithmetic is tricky with negative timestamps (before 1970).
        // For simplicity, this implementation uses `div_euclid` and `rem_euclid`
        // which correctly handle negative timestamps.

        let mut days = timestamp.div_euclid(Self::SECONDS_IN_DAY);
        let mut secs_of_day = timestamp.rem_euclid(Self::SECONDS_IN_DAY);

        let hour = (secs_of_day / Self::SECONDS_IN_HOUR) as u8;
        secs_of_day %= Self::SECONDS_IN_HOUR;
        let minute = (secs_of_day / Self::SECONDS_IN_MINUTE) as u8;
        let second = (secs_of_day % Self::SECONDS_IN_MINUTE) as u8;

        let mut year = 1970;

        // Calculate year
        // We can optimize this using cycle lengths (400 years = 146097 days),
        // but an iterative approach is clearer for learning.
        if days >= 0 {
            loop {
                let days_in_year = if Self::is_leap_year(year) { 366 } else { 365 };
                if days < days_in_year {
                    break;
                }
                days -= days_in_year;
                year += 1;
            }
        } else {
            loop {
                year -= 1;
                let days_in_year = if Self::is_leap_year(year) { 366 } else { 365 };
                days += days_in_year;
                if days >= 0 {
                    break;
                }
            }
        }

        let mut month = 1;
        loop {
            let dim = Self::days_in_month(year, month) as i64;
            if days < dim {
                break;
            }
            days -= dim;
            month += 1;
        }

        let day = (days + 1) as u8;

        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        }
    }

    /// Converts the DateTime to a UNIX timestamp.
    #[must_use]
    pub fn to_timestamp(&self) -> i64 {
        let mut days = 0;

        // Years
        if self.year >= 1970 {
            for y in 1970..self.year {
                days += if Self::is_leap_year(y) { 366 } else { 365 };
            }
        } else {
            for y in self.year..1970 {
                days -= if Self::is_leap_year(y) { 366 } else { 365 };
            }
        }

        // Months
        for m in 1..self.month {
            days += Self::days_in_month(self.year, m) as i64;
        }

        // Days
        days += (self.day - 1) as i64;

        days * Self::SECONDS_IN_DAY
            + i64::from(self.hour) * Self::SECONDS_IN_HOUR
            + i64::from(self.minute) * Self::SECONDS_IN_MINUTE
            + i64::from(self.second)
    }

    /// Returns the ISO-8601 string representation (YYYY-MM-DDTHH:MM:SSZ).
    #[must_use]
    pub fn to_iso8601(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }

    /// Checks if a year is a leap year in the Gregorian calendar.
    #[must_use]
    pub fn is_leap_year(year: i32) -> bool {
        (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
    }

    /// Returns the number of days in a given month of a given year.
    #[must_use]
    pub fn days_in_month(year: i32, month: u8) -> u8 {
        match month {
            4 | 6 | 9 | 11 => 30,
            2 => {
                if Self::is_leap_year(year) {
                    29
                } else {
                    28
                }
            }
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            _ => 0, // Should be unreachable with valid DateTime
        }
    }
}

impl fmt::Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_iso8601())
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `chrono`: `chrono` handles local timezones, DST rules, parsing from many formats,
//   and nanosecond precision. It is vastly more robust.
// - `time`: The `time` crate is another modern alternative focusing on strict safety,
//   `const fn` evaluations, and macro-based format descriptions.
//
// Missing vs. Production:
// - **Timezones/DST**: The hardest part of dates. Real implementations require the tz database.
// - **Sub-second Precision**: We only track down to the second. Production tools track nanoseconds.
// - **Performance**: The loop-based year and month calculations are O(N) relative to the offset from 1970.
//   Production crates use O(1) integer arithmetic based on 400-year cycle calculations.
// - **Parsing**: No functionality to parse a string back into a `DateTime`.
//
// Next Steps:
// 1. Implement O(1) epoch conversions using 146097-day cycle math.
// 2. Add sub-second (millisecond/nanosecond) tracking.
// 3. Write an ISO-8601 parser to convert string -> DateTime.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_creation() {
        assert!(DateTime::new(2023, 10, 27, 12, 30, 45).is_some());
        // Invalid month
        assert!(DateTime::new(2023, 13, 27, 12, 30, 45).is_none());
        // Invalid day for month
        assert!(DateTime::new(2023, 2, 29, 12, 0, 0).is_none());
        // Valid leap year
        assert!(DateTime::new(2024, 2, 29, 12, 0, 0).is_some());
    }

    #[test]
    fn test_leap_years() {
        assert!(DateTime::is_leap_year(2000)); // 400 rule
        assert!(!DateTime::is_leap_year(1900)); // 100 rule
        assert!(DateTime::is_leap_year(2024)); // 4 rule
        assert!(!DateTime::is_leap_year(2023));
    }

    #[test]
    fn test_epoch_conversion_1970() {
        let dt = DateTime::from_timestamp(0);
        assert_eq!(dt.year, 1970);
        assert_eq!(dt.month, 1);
        assert_eq!(dt.day, 1);
        assert_eq!(dt.hour, 0);
        assert_eq!(dt.minute, 0);
        assert_eq!(dt.second, 0);

        assert_eq!(dt.to_timestamp(), 0);
    }

    #[test]
    fn test_epoch_conversion_positive() {
        // 2023-10-27T12:30:45Z
        let ts = 1698409845;
        let dt = DateTime::from_timestamp(ts);
        assert_eq!(dt.year, 2023);
        assert_eq!(dt.month, 10);
        assert_eq!(dt.day, 27);
        assert_eq!(dt.hour, 12);
        assert_eq!(dt.minute, 30);
        assert_eq!(dt.second, 45);

        assert_eq!(dt.to_timestamp(), ts);
    }

    #[test]
    fn test_epoch_conversion_negative() {
        // 1969-12-31T23:59:59Z
        let ts = -1;
        let dt = DateTime::from_timestamp(ts);
        assert_eq!(dt.year, 1969);
        assert_eq!(dt.month, 12);
        assert_eq!(dt.day, 31);
        assert_eq!(dt.hour, 23);
        assert_eq!(dt.minute, 59);
        assert_eq!(dt.second, 59);

        assert_eq!(dt.to_timestamp(), ts);
    }

    #[test]
    fn test_iso8601_formatting() {
        let dt = DateTime::new(2023, 10, 27, 12, 30, 45).unwrap();
        assert_eq!(dt.to_iso8601(), "2023-10-27T12:30:45Z");
        assert_eq!(dt.to_string(), "2023-10-27T12:30:45Z");
    }
}

// Benchmarking Note:
// To benchmark `DateTime`, you can use Criterion.rs.
// Create tests that repeatedly call `DateTime::now()` and `DateTime::from_timestamp()`.
// Use `std::hint::black_box` to prevent the compiler from optimizing away the return values.
// Example:
// ```rust
// pub fn criterion_benchmark(c: &mut Criterion) {
//     c.bench_function("datetime_from_timestamp", |b| b.iter(|| {
//         std::hint::black_box(DateTime::from_timestamp(std::hint::black_box(1698409845)));
//     }));
// }
// ```
