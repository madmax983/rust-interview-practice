//! # Cron Expression Parser & Scheduler
//!
//! Implements a high-performance parser and matching engine for cron expressions.
//! It converts dense cron strings (e.g., `*/5 9-17 * * 1-5`) into highly optimized internal
//! representations using bitwise operations (`u64`/`u32` bitmasks), enabling `O(1)` time complexity
//! matching logic.
//!
//! **Replaces Crates:** `cron`, `tokio-cron-scheduler`
//!
//! **Real-world Usage:**
//! - Task schedulers running periodic background jobs (e.g., clearing caches, sending emails).
//! - CI/CD pipelines configured via cron schedules.
//! - System administration utilities (like the traditional Unix `cron` daemon).
//!
//! **Why build it yourself?**
//! Understanding how cron works under the hood reveals the power of bitmasking for evaluating
//! complex set-membership rules. Instead of evaluating strings and iterators on every tick,
//! compiling a schedule into a few integers turns an otherwise expensive conditional check
//! into a few CPU clock cycles of bitwise `AND`s.

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//   Cron String: "*/15 9-17 * * 1-5"
//                   │    │  │ │  │
//                   ▼    ▼  ▼ ▼  ▼
//                ┌────┬────┬─┬──┬──┐
//    Bitmasks:   │ m  │ h  │d│mo│dw│
//                └────┴────┴─┴──┴──┘
//         Size:   u64  u32 u32 u16 u8
//
// Invariants:
// 1. **Validity:** Parsed bitmasks are guaranteed to only have bits set within their valid bounds.
//    (e.g., minute mask never has bits > 59 set).
// 2. **Sunday Normalization:** Day of week '7' (Sunday) is normalized to '0' during parsing.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Parse         │ O(L)        │ O(1)        │  (L = length of cron string)
// │ Matches       │ O(1)        │ O(1)        │  (Just a few bitwise ANDs)
// └───────────────┴─────────────┴─────────────┘
//
// Design Decisions & Tradeoffs:
// - **Zero-Allocation Matching:** The `matches()` function takes primitive values and performs purely
//   bitwise checks, meaning absolutely zero heap allocations during the hottest path (tick evaluation).
// - **Bitmasks vs Sets:** We use integer primitives instead of `HashSet` or `Vec<bool>`.
//   A `u64` fits entirely in a CPU register, making the matching check virtually zero-cost.
// - **Cron Standard Quirk:** Standard cron treats restricted day-of-month and day-of-week as an
//   `OR` condition (if either matches, it runs). We explicitly implement this standard quirk,
//   checking if both are restricted versus if only one is.

use std::fmt;
use std::str::FromStr;

/// Error type for cron parsing
#[derive(Debug, PartialEq, Eq)]
pub enum CronError {
    InvalidFormat,
    InvalidField,
    ValueOutOfBounds,
    InvalidStep,
    InvalidRange,
}

impl fmt::Display for CronError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat => write!(f, "Invalid cron format (expected 5 fields)"),
            Self::InvalidField => write!(f, "Invalid field syntax"),
            Self::ValueOutOfBounds => write!(f, "Value out of bounds"),
            Self::InvalidStep => write!(f, "Invalid step value"),
            Self::InvalidRange => write!(f, "Invalid range (start > end)"),
        }
    }
}

/// Represents an optimized Cron Schedule.
pub trait Schedule {
    fn matches(
        &self,
        minute: u8,
        hour: u8,
        day_of_month: u8,
        month: u8,
        day_of_week: u8,
    ) -> bool;
}

/// Represents an optimized Cron Schedule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronSchedule {
    pub minutes: u64,
    pub hours: u32,
    pub days_of_month: u32,
    pub months: u16,
    pub days_of_week: u8,

    // RUST INSIGHT: We track whether DOM and DOW were restricted (not just `*`).
    // This is required to implement the standard cron OR-quirk correctly without
    // losing information when parsing `*` into a full bitmask.
    dom_restricted: bool,
    dow_restricted: bool,
}

impl Schedule for CronSchedule {
    /// Checks if a given timestamp matches the cron schedule.
    ///
    /// Takes simple 0-indexed primitives (except DOM/Month which are 1-indexed by convention).

    fn matches(
        &self,
        minute: u8,
        hour: u8,
        day_of_month: u8,
        month: u8,
        day_of_week: u8,
    ) -> bool {
        let min_match = (self.minutes & (1 << minute)) != 0;
        let hr_match = (self.hours & (1 << hour)) != 0;
        let mo_match = (self.months & (1 << month)) != 0;

        let dom_match = (self.days_of_month & (1 << day_of_month)) != 0;
        let dow_match = (self.days_of_week & (1 << day_of_week)) != 0;

        // GOTCHA: The Cron Day-of-Month / Day-of-Week Quirk
        // If both the day-of-month AND day-of-week are restricted (i.e. not `*`),
        // standard cron rules state that the schedule matches if EITHER matches.
        let day_match = if self.dom_restricted && self.dow_restricted {
            dom_match || dow_match
        } else {
            dom_match && dow_match
        };

        min_match && hr_match && mo_match && day_match
    }
}

impl FromStr for CronSchedule {
    type Err = CronError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split_whitespace().collect();
        if parts.len() != 5 {
            return Err(CronError::InvalidFormat);
        }

        let minutes = parse_field(parts[0], 0, 59)?;
        let hours = parse_field(parts[1], 0, 23)?;
        let days_of_month = parse_field(parts[2], 1, 31)?;
        let months = parse_field(parts[3], 1, 12)?;

        // DOW parsing is slightly special because 7 normalizes to 0 (Sunday)
        let raw_dow = parse_field(parts[4], 0, 7)?;
        let mut days_of_week = (raw_dow & 0x7F) as u8; // Keep 0-6
        if (raw_dow & (1 << 7)) != 0 {
            days_of_week |= 1 << 0; // 7 -> 0
        }

        // PRODUCTION NOTE: In standard production cron daemons, they keep the raw string to serialize back later. We discard it for memory efficiency.
        // Detect restrictions by checking if the original string was strictly `*`
        // (A string like `1-31` is technically restricted even if it covers all bits).
        let dom_restricted = parts[2] != "*";
        let dow_restricted = parts[4] != "*";

        Ok(Self {
            minutes,
            hours: hours as u32,
            days_of_month: days_of_month as u32,
            months: months as u16,
            days_of_week,
            dom_restricted,
            dow_restricted,
        })
    }
}

// =========================================================================================
// Parsing Logic (Internal)
// =========================================================================================

fn parse_field(field: &str, min: u8, max: u8) -> Result<u64, CronError> {
    let mut mask: u64 = 0;

    for part in field.split(',') {
        if let Some(step_idx) = part.find('/') {
            // Case: range/step or */step
            let range_str = &part[..step_idx];
            let step_str = &part[step_idx + 1..];
            let step = step_str.parse::<u8>().map_err(|_| CronError::InvalidStep)?;
            if step == 0 {
                return Err(CronError::InvalidStep);
            }

            if range_str == "*" {
                // RUST INSIGHT: `step_by` is an elegant zero-cost abstraction for iterators.
                // It replaces manual while-loops perfectly here.
                for v in (min..=max).step_by(step as usize) {
                    mask |= 1 << v;
                }
            } else if let Some(dash_idx) = range_str.find('-') {
                let start = range_str[..dash_idx]
                    .parse::<u8>()
                    .map_err(|_| CronError::InvalidField)?;
                let end = range_str[dash_idx + 1..]
                    .parse::<u8>()
                    .map_err(|_| CronError::InvalidField)?;

                if start < min || end > max {
                    return Err(CronError::ValueOutOfBounds);
                }
                if start > end {
                    return Err(CronError::InvalidRange);
                }

                for v in (start..=end).step_by(step as usize) {
                    mask |= 1 << v;
                }
            } else {
                // e.g. "5/2" is sometimes supported as "start at 5, step by 2", but
                // standard cron technically expects a range or wildcard before a step.
                // We'll support standard range/wildcard step syntax.
                return Err(CronError::InvalidField);
            }
        } else if part == "*" {
            // Case: *
            for v in min..=max {
                mask |= 1 << v;
            }
        } else if let Some(dash_idx) = part.find('-') {
            // Case: start-end
            let start = part[..dash_idx]
                .parse::<u8>()
                .map_err(|_| CronError::InvalidField)?;
            let end = part[dash_idx + 1..]
                .parse::<u8>()
                .map_err(|_| CronError::InvalidField)?;

            if start < min || end > max {
                return Err(CronError::ValueOutOfBounds);
            }
            if start > end {
                return Err(CronError::InvalidRange);
            }

            for v in start..=end {
                mask |= 1 << v;
            }
        } else {
            // Case: single value
            let v = part.parse::<u8>().map_err(|_| CronError::InvalidField)?;
            if v < min || v > max {
                return Err(CronError::ValueOutOfBounds);
            }
            mask |= 1 << v;
        }
    }

    Ok(mask)
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `cron`: Fully featured, robust parser that also includes iteration over future timestamps
//   using the `chrono` crate.
// - `tokio-cron-scheduler`: Integrates cron parsing directly with tokio async tasks.
//
// Missing vs. Production:
// - **Next Execution Calculation:** We implement `matches()`, but a production crate like `cron`
//   provides a `upcoming(timezone)` iterator to jump to the exact next `DateTime`. This is critical
//   for actually scheduling sleeps rather than polling every minute.
// - **String Replacements:** Real cron supports `@hourly`, `@daily`, and month names (`JAN-DEC`),
//   and day names (`MON-FRI`). We strictly parse numeric boundaries.
// - **L, W, # syntax:** We omit Last day of month (`L`), nearest Weekday (`W`), and Nth day of week (`#`).
//
// Next Steps:
// 1. Write an `upcoming_from(datetime)` method to calculate the exact duration until the next match.
// 2. Add support for alphanumeric month and day abbreviations.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_all_wildcards() {
        let schedule = CronSchedule::from_str("* * * * *").unwrap();
        // 0..=59
        assert_eq!(schedule.minutes, 0x0FFF_FFFF_FFFF_FFFF);
        // 0..=23
        assert_eq!(schedule.hours, 0x00FF_FFFF);
        // 1..=31 (bit 0 is 0)
        assert_eq!(schedule.days_of_month, 0xFFFF_FFFE);
        // 1..=12 (bit 0 is 0)
        assert_eq!(schedule.months, 0x1FFE);
        // 0..=6
        assert_eq!(schedule.days_of_week, 0x7F);
    }

    #[test]
    fn test_parse_specific_values() {
        let schedule = CronSchedule::from_str("5 9 15 2 3").unwrap();
        assert_eq!(schedule.minutes, 1 << 5);
        assert_eq!(schedule.hours, 1 << 9);
        assert_eq!(schedule.days_of_month, 1 << 15);
        assert_eq!(schedule.months, 1 << 2);
        assert_eq!(schedule.days_of_week, 1 << 3);
    }

    #[test]
    fn test_parse_ranges() {
        let schedule = CronSchedule::from_str("1-5 * * * *").unwrap();
        // bits 1,2,3,4,5
        assert_eq!(schedule.minutes, 0b111110);
    }

    #[test]
    fn test_parse_steps() {
        let schedule = CronSchedule::from_str("*/15 * * * *").unwrap();
        // bits 0, 15, 30, 45
        assert_eq!(
            schedule.minutes,
            (1 << 0) | (1 << 15) | (1 << 30) | (1 << 45)
        );
    }

    #[test]
    fn test_parse_range_with_steps() {
        let schedule = CronSchedule::from_str("10-20/5 * * * *").unwrap();
        // bits 10, 15, 20
        assert_eq!(schedule.minutes, (1 << 10) | (1 << 15) | (1 << 20));
    }

    #[test]
    fn test_parse_lists() {
        let schedule = CronSchedule::from_str("1,5,10-12 * * * *").unwrap();
        // bits 1, 5, 10, 11, 12
        assert_eq!(
            schedule.minutes,
            (1 << 1) | (1 << 5) | (1 << 10) | (1 << 11) | (1 << 12)
        );
    }

    #[test]
    fn test_sunday_normalization() {
        // Both 0 and 7 should set bit 0
        let sched0 = CronSchedule::from_str("* * * * 0").unwrap();
        let sched7 = CronSchedule::from_str("* * * * 7").unwrap();
        assert_eq!(sched0.days_of_week, 1 << 0);
        assert_eq!(sched7.days_of_week, 1 << 0);
    }

    #[test]
    fn test_matches_basic() {
        let schedule = CronSchedule::from_str("0 12 * * *").unwrap();
        // Matches exactly at 12:00 every day
        assert!(schedule.matches(0, 12, 1, 1, 1));
        assert!(!schedule.matches(1, 12, 1, 1, 1));
        assert!(!schedule.matches(0, 11, 1, 1, 1));
    }

    #[test]
    fn test_matches_dom_dow_quirk() {
        // Run on the 1st of the month OR on a Monday
        let schedule = CronSchedule::from_str("* * 1 * 1").unwrap();

        // Matches on 1st, Tuesday (DOW mismatch, DOM match)
        assert!(schedule.matches(0, 0, 1, 1, 2));

        // Matches on 2nd, Monday (DOM mismatch, DOW match)
        assert!(schedule.matches(0, 0, 2, 1, 1));

        // Matches on 1st, Monday (Both match)
        assert!(schedule.matches(0, 0, 1, 1, 1));

        // Fails on 2nd, Tuesday (Both mismatch)
        assert!(!schedule.matches(0, 0, 2, 1, 2));
    }

    #[test]
    fn test_errors() {
        assert_eq!(
            CronSchedule::from_str("* * * *").unwrap_err(),
            CronError::InvalidFormat
        );
        assert_eq!(
            CronSchedule::from_str("60 * * * *").unwrap_err(),
            CronError::ValueOutOfBounds
        );
        assert_eq!(
            CronSchedule::from_str("* 24 * * *").unwrap_err(),
            CronError::ValueOutOfBounds
        );
        assert_eq!(
            CronSchedule::from_str("* * 0 * *").unwrap_err(),
            CronError::ValueOutOfBounds
        ); // DOM is 1-31
        assert_eq!(
            CronSchedule::from_str("* * 32 * *").unwrap_err(),
            CronError::ValueOutOfBounds
        );
        assert_eq!(
            CronSchedule::from_str("* * * 13 *").unwrap_err(),
            CronError::ValueOutOfBounds
        );
        assert_eq!(
            CronSchedule::from_str("* * * * 8").unwrap_err(),
            CronError::ValueOutOfBounds
        );
        assert_eq!(
            CronSchedule::from_str("10-5 * * * *").unwrap_err(),
            CronError::InvalidRange
        );
        assert_eq!(
            CronSchedule::from_str("*/0 * * * *").unwrap_err(),
            CronError::InvalidStep
        );
        assert_eq!(
            CronSchedule::from_str("x * * * *").unwrap_err(),
            CronError::InvalidField
        );
    }
}
