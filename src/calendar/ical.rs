//! Google Calendar read-only: fetch the secret iCal address, mirror its events into `events`.
//! Never writes back. The URL is a credential: it never reaches a log line or an error message.
use super::queries::{self, GoogleEvent};
use crate::app::AppState;
use icalendar::{
    Calendar, CalendarComponent, CalendarDateTime, Component, DatePerhapsTime, EventStatus,
};
use rusqlite::Connection;
use std::time::Duration;

/// Stored form: `YYYY-MM-DD` for all-day, else local `YYYY-MM-DDTHH:MM`.
fn when(d: DatePerhapsTime) -> String {
    match d {
        DatePerhapsTime::Date(d) => d.format("%Y-%m-%d").to_string(),
        DatePerhapsTime::DateTime(CalendarDateTime::Utc(t)) => t
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%dT%H:%M")
            .to_string(),
        // ponytail: TZID times are taken as local wall time (no tz database, Google feeds use the
        // calendar's own zone = this machine's); add icalendar's `chrono-tz` feature if zones differ.
        DatePerhapsTime::DateTime(
            CalendarDateTime::Floating(n) | CalendarDateTime::WithTimezone { date_time: n, .. },
        ) => n.format("%Y-%m-%dT%H:%M").to_string(),
    }
}

/// Events of one .ics body. Skips cancelled ones and anything without UID or start.
/// ponytail: no RRULE expansion, a recurring series shows on its first date only (plus moved
/// instances); enable icalendar's `recurrence` feature (rrule + chrono-tz) when that's missed.
pub fn parse(ics: &str) -> Result<Vec<GoogleEvent>, String> {
    let cal: Calendar = ics.parse()?;
    Ok(cal
        .components
        .iter()
        .filter_map(|c| match c {
            CalendarComponent::Event(e) => Some(e),
            _ => None,
        })
        .filter(|e| e.get_status() != Some(EventStatus::Cancelled))
        .filter_map(|e| {
            let uid = e.get_uid()?;
            // A moved instance of a recurring series shares the series UID; RECURRENCE-ID tells them apart.
            let ext_id = match e.property_value("RECURRENCE-ID") {
                Some(rid) => format!("{uid}#{rid}"),
                None => uid.to_string(),
            };
            Some(GoogleEvent {
                ext_id,
                title: e.get_summary().unwrap_or("(no title)").to_string(),
                starts_at: when(e.get_start()?),
                ends_at: e.get_end().map(when),
            })
        })
        .collect())
}

/// Parse + mirror one .ics body; returns how many events it holds. Tests call this with a fixture.
pub fn import(conn: &mut Connection, ics: &str) -> Result<usize, String> {
    let events = parse(ics)?;
    queries::sync_google(conn, &events).map_err(|e| e.to_string())?;
    Ok(events.len())
}

async fn fetch(url: &str) -> Result<String, String> {
    // `without_url`: reqwest errors quote the URL, and this one is a secret.
    let hide = |e: reqwest::Error| e.without_url().to_string();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(hide)?;
    let res = client
        .get(url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(hide)?;
    res.text().await.map_err(hide)
}

/// One refresh. `Ok(None)` = no `ical_url` configured. The lock is taken only after the fetch,
/// so a slow Google never stalls page loads.
pub async fn refresh(st: &AppState) -> Result<Option<usize>, String> {
    let Some(url) = st.settings.ical_url.as_deref() else {
        return Ok(None);
    };
    let ics = fetch(url).await?;
    import(&mut st.conn(), &ics).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forms_of_start() {
        let ics = include_str!("../../tests/fixtures/google.ics");
        let evs = parse(ics).unwrap();
        let get = |id: &str| evs.iter().find(|e| e.ext_id == id).unwrap();
        assert_eq!(get("tz@google.com").starts_at, "2026-11-11T10:00");
        assert_eq!(get("tz@google.com").title, "oms-sync, weekly");
        assert_eq!(get("allday@google.com").starts_at, "2026-11-13");
        assert!(
            evs.iter().all(|e| e.ext_id != "gone@google.com"),
            "cancelled skipped"
        );
        assert_eq!(
            get("series@google.com#20261112T090000").starts_at,
            "2026-11-12T09:30"
        );
        assert!(
            parse("not a calendar")
                .map(|v| v.is_empty())
                .unwrap_or(true)
        );
    }
}
