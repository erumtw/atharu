//! Day arithmetic. "today" is always passed in so tests can pin it.
use chrono::{Datelike, Days, Months, NaiveDate, Weekday};

/// `<when>` in `> … by <when>`: today · tomorrow · weekday (next one, today counts) · `20 Nov` · `2026-11-20`.
/// `when` is one token, or two for `20 Nov`.
pub fn parse_when(when: &str, today: NaiveDate) -> Option<NaiveDate> {
    let w = when.to_ascii_lowercase();
    match w.as_str() {
        "today" => return Some(today),
        "tomorrow" => return today.succ_opt(),
        _ => {}
    }
    if let Ok(day) = w.parse::<Weekday>() {
        let ahead = (7 + day.num_days_from_monday() - today.weekday().num_days_from_monday()) % 7;
        return today.checked_add_days(Days::new(ahead.into()));
    }
    if let Ok(d) = NaiveDate::parse_from_str(&w, "%Y-%m-%d") {
        return Some(d);
    }
    // `20 Nov` has no year: the next such date, so a due date never lands in the past.
    let d = NaiveDate::parse_from_str(&format!("{w} {}", today.year()), "%d %b %Y")
        .or_else(|_| NaiveDate::parse_from_str(&format!("{w} {}", today.year()), "%d %B %Y"))
        .ok()?;
    if d < today {
        d.with_year(d.year() + 1)
    } else {
        Some(d)
    }
}

pub fn week_start(d: NaiveDate) -> NaiveDate {
    d - Days::new(d.weekday().num_days_from_monday().into())
}

pub fn month_start(d: NaiveDate) -> NaiveDate {
    d.with_day(1).expect("day 1 exists")
}

pub fn quarter_start(d: NaiveDate) -> NaiveDate {
    let m = (d.month0() / 3) * 3 + 1;
    NaiveDate::from_ymd_opt(d.year(), m, 1).expect("quarter start exists")
}

/// Last day of the month/quarter that starts at `start`.
pub fn end_of(start: NaiveDate, months: u32) -> NaiveDate {
    start + Months::new(months) - Days::new(1)
}

/// `Wed 11 Nov`, the canvas's day label.
pub fn label(d: NaiveDate) -> String {
    d.format("%a %-d %b").to_string()
}

pub fn iso(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    #[test]
    fn when_forms() {
        let today = d("2026-11-11"); // Wednesday
        for (when, want) in [
            ("today", Some("2026-11-11")),
            ("tomorrow", Some("2026-11-12")),
            ("wed", Some("2026-11-11")),
            ("fri", Some("2026-11-13")),
            ("Monday", Some("2026-11-16")),
            ("20 Nov", Some("2026-11-20")),
            ("5 jan", Some("2027-01-05")),
            ("2026-11-20", Some("2026-11-20")),
            ("someday", None),
            ("32 Nov", None),
        ] {
            assert_eq!(parse_when(when, today), want.map(d), "{when}");
        }
    }

    #[test]
    fn ranges() {
        assert_eq!(week_start(d("2026-11-15")), d("2026-11-09"));
        assert_eq!(week_start(d("2026-11-09")), d("2026-11-09"));
        assert_eq!(quarter_start(d("2026-11-11")), d("2026-10-01"));
        assert_eq!(end_of(d("2026-10-01"), 3), d("2026-12-31"));
        assert_eq!(end_of(d("2026-02-01"), 1), d("2026-02-28"));
        assert_eq!(label(d("2026-11-11")), "Wed 11 Nov");
    }
}
