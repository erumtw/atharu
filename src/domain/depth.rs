//! Learn depth: 1 heard of it · 2 can explain · 3 can build without AI · 4 can teach.
use chrono::{Datelike, Days, Months, NaiveDate};

/// Days without a rating before a topic counts as stale ("re-rate").
pub const STALE_DAYS: i64 = 30;

pub fn label(depth: u8) -> &'static str {
    match depth {
        1 => "heard of it",
        2 => "can explain",
        3 => "can build without AI",
        _ => "can teach",
    }
}

/// One `depth_ratings` row; slices hand these in oldest first.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rating {
    pub day: NaiveDate,
    pub depth: u8,
}

/// Latest depth on or before `day` (ratings oldest first), `None` if not rated yet.
pub fn as_of(ratings: &[Rating], day: NaiveDate) -> Option<u8> {
    ratings.iter().rev().find(|r| r.day <= day).map(|r| r.depth)
}

/// Never rated counts as stale too: it is exactly what needs a rating.
pub fn is_stale(last_rated: Option<NaiveDate>, today: NaiveDate) -> bool {
    last_rated.is_none_or(|d| (today - d).num_days() > STALE_DAYS)
}

/// Months touching [from, to], each as (first day, last day).
pub fn months(from: NaiveDate, to: NaiveDate) -> Vec<(NaiveDate, NaiveDate)> {
    let mut out = Vec::new();
    let mut m = from.with_day(1).unwrap_or(from);
    while m <= to {
        let next = m + Months::new(1);
        out.push((m, next - Days::new(1)));
        m = next;
    }
    out
}

/// Heatmap row: depth as of each month's end, `None` = blank cell.
pub fn by_month(ratings: &[Rating], months: &[(NaiveDate, NaiveDate)]) -> Vec<Option<u8>> {
    months.iter().map(|(_, end)| as_of(ratings, *end)).collect()
}

/// Depth change for the report: (start, end). End = last rating in range; start = the depth
/// carried into the range, else the first rating in it. `None` = not rated in range.
pub fn change(ratings: &[Rating], from: NaiveDate, to: NaiveDate) -> Option<(u8, u8)> {
    let inside: Vec<&Rating> = ratings
        .iter()
        .filter(|r| r.day >= from && r.day <= to)
        .collect();
    let last = inside.last()?.depth;
    let start = from
        .pred_opt()
        .and_then(|d| as_of(ratings, d))
        .unwrap_or(inside[0].depth);
    Some((start, last))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    fn r(day: &str, depth: u8) -> Rating {
        Rating { day: d(day), depth }
    }

    #[test]
    fn depth_as_of_month_end() {
        let rs = [r("2026-09-12", 1), r("2026-10-10", 2), r("2026-10-30", 3)];
        let ms = months(d("2026-08-20"), d("2026-11-11"));
        assert_eq!(ms.len(), 4);
        assert_eq!(ms[1], (d("2026-09-01"), d("2026-09-30")));
        // Aug blank; Oct takes the later of two ratings; Nov carries Oct's.
        assert_eq!(by_month(&rs, &ms), vec![None, Some(1), Some(3), Some(3)]);
        assert_eq!(as_of(&rs, d("2026-10-29")), Some(2));
    }

    #[test]
    fn stale_after_30_days_or_never_rated() {
        let today = d("2026-11-11");
        assert!(!is_stale(Some(d("2026-10-12")), today)); // 30 days
        assert!(is_stale(Some(d("2026-10-11")), today)); // 31 days
        assert!(is_stale(None, today));
    }

    #[test]
    fn change_in_range() {
        let rs = [r("2026-10-10", 2), r("2026-11-10", 3), r("2026-11-12", 4)];
        let (f, t) = (d("2026-11-09"), d("2026-11-15"));
        assert_eq!(change(&rs, f, t), Some((2, 4)));
        assert_eq!(change(&rs[1..], f, t), Some((3, 4)));
        assert_eq!(change(&rs[..1], f, t), None);
        assert_eq!(label(3), "can build without AI");
    }
}
