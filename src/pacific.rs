//! Pacific wall-clock time for anything the bot displays.
//!
//! Everything the bot shows a time for is Pacific: event schedules are typed and
//! stored as Pacific wall-clock, and the luckymon reset is announced in local
//! terms. Which of PST or PDT applies depends on the date, so it is asked for
//! rather than hardcoded -- a literal "PDT" in a footer is simply wrong for the
//! four months a year the zone is on standard time.

use chrono::{Duration, NaiveDateTime, TimeZone, Utc};
use chrono_tz::America::Los_Angeles;
use chrono_tz::Tz;

pub const PACIFIC: Tz = Los_Angeles;

/// "PST" or "PDT", whichever applies at that Pacific wall-clock time.
///
/// The DST seams need an answer rather than a panic: in spring an hour does not
/// exist, and in autumn one happens twice.
pub fn abbrev_at(naive: NaiveDateTime) -> String {
    let resolved = PACIFIC
        .from_local_datetime(&naive)
        .earliest()
        .or_else(|| PACIFIC.from_local_datetime(&naive).latest())
        .unwrap_or_else(|| Utc.from_utc_datetime(&naive).with_timezone(&PACIFIC));

    resolved.format("%Z").to_string()
}

/// When the daily luckymon roll next turns over, in local terms.
///
/// The boundary itself is midnight UTC. What that reads as on a Pacific clock
/// moves by an hour across the year -- 5PM on daylight time, 4PM on standard --
/// so it is computed from the real upcoming boundary rather than written into
/// the string.
pub fn reset_label() -> String {
    let next_utc_midnight = (Utc::now().date_naive() + Duration::days(1))
        .and_hms_opt(0, 0, 0)
        .expect("midnight is a valid time")
        .and_utc();

    let local = next_utc_midnight.with_timezone(&PACIFIC);
    format!("{} {}", local.format("%-I%p"), local.format("%Z"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(h, min, 0)
            .unwrap()
    }

    #[test]
    fn summer_is_daylight_time() {
        assert_eq!(abbrev_at(at(2026, 7, 4, 12, 0)), "PDT");
    }

    #[test]
    fn winter_is_standard_time() {
        assert_eq!(abbrev_at(at(2026, 1, 15, 12, 0)), "PST");
    }

    #[test]
    fn dst_seams_still_produce_an_answer() {
        for naive in [at(2026, 3, 8, 2, 30), at(2026, 11, 1, 1, 30)] {
            let a = abbrev_at(naive);
            assert!(a == "PST" || a == "PDT", "got {} for {}", a, naive);
        }
    }

    /// The reset label has to be one of exactly two readings, and must never
    /// silently keep saying PDT in January.
    #[test]
    fn reset_label_is_a_real_pacific_reading() {
        let label = reset_label();
        assert!(
            label == "5PM PDT" || label == "4PM PST",
            "midnight UTC should read as 5PM PDT or 4PM PST, got {}",
            label
        );
    }
}
