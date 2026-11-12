//! Progress flags: computed from what is logged, never stored (spec "Progress flags").
//! Blockers never count: they have no resolve state, so a flag on them would never clear.
use super::dates;
use chrono::{Days, NaiveDate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flag {
    Overdue,
    AtRisk,
    Stale,
}

impl Flag {
    pub fn label(self) -> &'static str {
        match self {
            Flag::Overdue => "overdue",
            Flag::AtRisk => "at risk",
            Flag::Stale => "stale",
        }
    }

    /// `.flag` modifier from the board.
    pub fn class(self) -> &'static str {
        match self {
            Flag::Overdue => "flag over",
            Flag::AtRisk => "flag risk",
            Flag::Stale => "flag stale",
        }
    }
}

/// What the flag rules read about one epic.
pub struct EpicFacts<'a> {
    pub status: &'a str,
    pub target: Option<NaiveDate>,
    /// Latest `entries.day` on the epic.
    pub last_activity: Option<NaiveDate>,
    pub open_plans: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flagged {
    pub flag: Flag,
    /// `6 days past target` · `due in 3 days, 1 plan open` · `nothing logged for 9 days`.
    pub reason: String,
}

fn days(n: i64) -> String {
    if n == 1 {
        "1 day".into()
    } else {
        format!("{n} days")
    }
}

/// The one flag an epic shows, worst first: overdue, then at risk, then stale.
pub fn flag(e: &EpicFacts, today: NaiveDate) -> Option<Flagged> {
    if e.status != "active" {
        return None;
    }
    if let Some(t) = e.target {
        let left = (t - today).num_days();
        if left < 0 {
            return Some(Flagged {
                flag: Flag::Overdue,
                reason: format!("{} past target", days(-left)),
            });
        }
        if left <= 7 && e.open_plans > 0 {
            let when = if left == 0 {
                "due today".to_string()
            } else {
                format!("due in {}", days(left))
            };
            let plans = if e.open_plans == 1 {
                "1 plan open".to_string()
            } else {
                format!("{} plans open", e.open_plans)
            };
            return Some(Flagged {
                flag: Flag::AtRisk,
                reason: format!("{when}, {plans}"),
            });
        }
    }
    match e.last_activity.map(|d| (today - d).num_days()) {
        None => Some(Flagged {
            flag: Flag::Stale,
            reason: "nothing logged yet".into(),
        }),
        Some(n) if n >= 7 => Some(Flagged {
            flag: Flag::Stale,
            reason: format!("nothing logged for {}", days(n)),
        }),
        _ => None,
    }
}

/// Entries per Monday-started week for the last 8 weeks, oldest first, this week last.
pub fn weekly_counts(entry_days: &[NaiveDate], today: NaiveDate) -> [i64; 8] {
    let first = dates::week_start(today) - Days::new(7 * 7);
    let mut out = [0; 8];
    for d in entry_days {
        if *d < first || *d > today {
            continue;
        }
        let i = ((dates::week_start(*d) - first).num_days() / 7) as usize;
        if let Some(c) = out.get_mut(i) {
            *c += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    const TODAY: &str = "2026-11-11";

    fn facts<'a>(
        status: &'a str,
        target: Option<&str>,
        last: Option<&str>,
        open_plans: i64,
    ) -> EpicFacts<'a> {
        EpicFacts {
            status,
            target: target.map(d),
            last_activity: last.map(d),
            open_plans,
        }
    }

    #[test]
    fn flags_against_fixed_today() {
        let cases: &[(EpicFacts, Option<(Flag, &str)>)] = &[
            (
                facts("active", Some("2026-11-05"), Some("2026-11-09"), 0),
                Some((Flag::Overdue, "6 days past target")),
            ),
            (
                facts("active", Some("2026-11-10"), Some("2026-11-11"), 0),
                Some((Flag::Overdue, "1 day past target")),
            ),
            (
                facts("active", Some("2026-11-14"), Some("2026-11-11"), 1),
                Some((Flag::AtRisk, "due in 3 days, 1 plan open")),
            ),
            (
                facts("active", Some("2026-11-11"), Some("2026-11-11"), 2),
                Some((Flag::AtRisk, "due today, 2 plans open")),
            ),
            // Within 7 days but nothing left to do: fine.
            (
                facts("active", Some("2026-11-14"), Some("2026-11-11"), 0),
                None,
            ),
            // 8 days out: not yet at risk.
            (
                facts("active", Some("2026-11-19"), Some("2026-11-11"), 3),
                None,
            ),
            (
                facts("active", Some("2026-11-30"), Some("2026-11-02"), 0),
                Some((Flag::Stale, "nothing logged for 9 days")),
            ),
            (
                facts("active", None, Some("2026-11-04"), 0),
                Some((Flag::Stale, "nothing logged for 7 days")),
            ),
            (facts("active", None, Some("2026-11-05"), 0), None),
            (
                facts("active", None, None, 0),
                Some((Flag::Stale, "nothing logged yet")),
            ),
            // At risk beats stale.
            (
                facts("active", Some("2026-11-12"), Some("2026-10-01"), 1),
                Some((Flag::AtRisk, "due in 1 day, 1 plan open")),
            ),
            (facts("done", Some("2026-11-01"), None, 4), None),
            (facts("dropped", None, None, 0), None),
        ];
        for (f, want) in cases {
            let got = flag(f, d(TODAY)).map(|g| (g.flag, g.reason));
            assert_eq!(got, want.map(|(fl, r)| (fl, r.to_string())));
        }
    }

    #[test]
    fn weekly_counts_last_eight_weeks() {
        // Weeks start Mon 21 Sep … Mon 9 Nov.
        let days: Vec<NaiveDate> = [
            "2026-09-20", // before the window
            "2026-09-21",
            "2026-10-14",
            "2026-11-09",
            "2026-11-11",
            "2026-11-12", // after today
        ]
        .iter()
        .map(|s| d(s))
        .collect();
        assert_eq!(weekly_counts(&days, d(TODAY)), [1, 0, 0, 1, 0, 0, 0, 2]);
    }
}
