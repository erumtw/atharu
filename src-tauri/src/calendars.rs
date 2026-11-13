//! macOS Calendar (EventKit, 0.2.4): the calendars picked in Settings › Mac calendars land in `events` as source
//! 'mac', so the Calendar page shows them like Google events (e.g. a work Google account synced in Calendar.app,
//! whose admin blocks the iCal address). Read-only. One thread owns the EKEventStore; it syncs on start, every
//! 15 min, when Calendar's data changes (EKEventStoreChangedNotification) and when Settings saves (the kick).
use atharu::{
    app::AppState,
    calendar::{MacEvent, mac_settings, replace_mac_events},
    recording::{Access, MacCalendar, MacCalendars},
};
use block2::RcBlock;
use chrono::{Days, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
use objc2::{rc::Retained, runtime::Bool};
use objc2_app_kit::{NSColor, NSColorSpace};
use objc2_event_kit::{
    EKAuthorizationStatus, EKCalendar, EKEntityType, EKEvent, EKEventStore,
    EKEventStoreChangedNotification,
};
use objc2_foundation::{NSArray, NSDate, NSError, NSNotification, NSNotificationCenter};
use std::{ptr::NonNull, sync::Arc, time::Duration};

const EVERY: Duration = Duration::from_secs(15 * 60);
/// The window each sync replaces: two weeks back (recent meetings) to two months ahead.
const BEFORE: u64 = 14;
const AFTER: u64 = 60;

pub fn start(st: AppState, mc: Arc<MacCalendars>) {
    std::thread::spawn(move || {
        // SAFETY (every `unsafe` here): plain EventKit / Foundation calls with valid arguments, on the one thread
        // that owns the store; EventKit's own threading rules allow a store per thread.
        let store = unsafe { EKEventStore::new() };
        let kick = mc.clone();
        let changed = RcBlock::new(move |_: NonNull<NSNotification>| kick.kick.notify_one());
        let token = unsafe {
            NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
                Some(EKEventStoreChangedNotification),
                None,
                None,
                &changed,
            )
        };
        // The observer lives as long as the app.
        std::mem::forget(token);
        loop {
            // Every sync makes autoreleased Foundation objects; a pool per pass frees them (a bare thread has none).
            if let Err(e) = objc2::rc::autoreleasepool(|_| sync(&st, &mc, &store)) {
                eprintln!("mac calendars: {e}");
            }
            tauri::async_runtime::block_on(async {
                let _ = tokio::time::timeout(EVERY, mc.kick.notified()).await;
            });
        }
    });
}

fn sync(st: &AppState, mc: &MacCalendars, store: &EKEventStore) -> Result<(), String> {
    let (on, chosen) = mac_settings(&st.conn()).map_err(|e| e.to_string())?;
    if !on {
        return Ok(());
    }
    let status = unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Event) };
    let access = match status {
        EKAuthorizationStatus::FullAccess => Access::Granted,
        // macOS asks once, the first time the switch is on; the answer comes back on another thread.
        EKAuthorizationStatus::NotDetermined => ask(store),
        _ => Access::Denied,
    };
    mc.set_access(access);
    if access != Access::Granted {
        return Ok(());
    }
    let cals = unsafe { store.calendarsForEntityType(EKEntityType::Event) };
    mc.set_list(cals.iter().map(|c| calendar(&c)).collect());
    let picked: Vec<Retained<EKCalendar>> = cals
        .iter()
        .filter(|c| chosen.contains(&unsafe { c.calendarIdentifier() }.to_string()))
        .collect();
    let today = Local::now().date_naive();
    let (from, to) = (today - Days::new(BEFORE), today + Days::new(AFTER));
    // No calendar ticked: the window empties (an unticked calendar's events go).
    let events: Vec<MacEvent> = if picked.is_empty() {
        Vec::new()
    } else {
        let arr = NSArray::from_retained_slice(&picked);
        let pred = unsafe {
            store.predicateForEventsWithStartDate_endDate_calendars(
                &ns_date(from),
                &ns_date(to + Days::new(1)),
                Some(&arr),
            )
        };
        unsafe { store.eventsMatchingPredicate(&pred) }
            .iter()
            .filter_map(|e| mac_event(&e))
            .collect()
    };
    replace_mac_events(&mut st.conn(), &iso(from), &iso(to), &events).map_err(|e| e.to_string())
}

fn ask(store: &EKEventStore) -> Access {
    let (tx, rx) = std::sync::mpsc::channel();
    let done = RcBlock::new(move |granted: Bool, _: *mut NSError| {
        let _ = tx.send(granted.as_bool());
    });
    unsafe { store.requestFullAccessToEventsWithCompletion(RcBlock::as_ptr(&done)) };
    // The owner may take a while to answer the dialog; no answer yet = ask again on the next sync.
    match rx.recv_timeout(Duration::from_secs(600)) {
        Ok(true) => Access::Granted,
        Ok(false) => Access::Denied,
        Err(_) => Access::Unknown,
    }
}

fn calendar(c: &EKCalendar) -> MacCalendar {
    unsafe {
        MacCalendar {
            id: c.calendarIdentifier().to_string(),
            title: c.title().to_string(),
            account: c
                .source()
                .map(|s| s.title().to_string())
                .unwrap_or_default(),
            color: hex(&c.color()),
        }
    }
}

fn hex(c: &NSColor) -> String {
    let Some(c) = c.colorUsingColorSpace(&NSColorSpace::sRGBColorSpace()) else {
        return "#8e8e93".into();
    };
    let b = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!(
        "#{:02x}{:02x}{:02x}",
        b(c.redComponent()),
        b(c.greenComponent()),
        b(c.blueComponent())
    )
}

fn mac_event(e: &EKEvent) -> Option<MacEvent> {
    let local = |d: Retained<NSDate>| {
        Local
            .timestamp_opt(d.timeIntervalSince1970().floor() as i64, 0)
            .single()
            .map(|t| t.naive_local())
    };
    let (start, end) = unsafe { (local(e.startDate())?, local(e.endDate())?) };
    let (starts_at, ends_at) = stored(unsafe { e.isAllDay() }, start, end);
    let id = unsafe { e.calendarItemExternalIdentifier() }
        .unwrap_or_else(|| unsafe { e.calendarItemIdentifier() });
    let title = unsafe { e.title() }.to_string();
    Some(MacEvent {
        // One row per occurrence: a repeating series shares its identifier.
        ext_id: format!("{id}@{starts_at}"),
        title: if title.trim().is_empty() {
            "(No title)".into()
        } else {
            title
        },
        starts_at,
        ends_at: Some(ends_at),
    })
}

/// `events`' stored forms: all-day = dates with the end exclusive (as iCal; EventKit ends an all-day event at
/// 23:59:59 of its last day), timed = local `YYYY-MM-DDTHH:MM`.
fn stored(all_day: bool, start: NaiveDateTime, end: NaiveDateTime) -> (String, String) {
    if all_day {
        let last = if end.time() == NaiveTime::MIN {
            end.date()
        } else {
            end.date() + Days::new(1)
        };
        (
            iso(start.date()),
            iso(last.max(start.date() + Days::new(1))),
        )
    } else {
        let f = |t: NaiveDateTime| t.format("%Y-%m-%dT%H:%M").to_string();
        (f(start), f(end))
    }
}

fn iso(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

fn ns_date(d: NaiveDate) -> Retained<NSDate> {
    // A DST change at midnight skips 00:00 in some zones: start the day at 01:00 then.
    let at = |h: u32| {
        Local
            .from_local_datetime(&d.and_hms_opt(h, 0, 0)?)
            .earliest()
    };
    let t = at(0).or_else(|| at(1)).map_or(0, |t| t.timestamp());
    NSDate::dateWithTimeIntervalSince1970(t as f64)
}

#[cfg(test)]
mod tests {
    use super::stored;
    use chrono::NaiveDateTime;

    fn t(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").unwrap()
    }

    #[test]
    fn stored_forms() {
        // EventKit's all-day 20–22 Nov ends at 22 Nov 23:59:59: exclusive end 23 Nov, like iCal.
        assert_eq!(
            stored(true, t("2026-11-20 00:00:00"), t("2026-11-22 23:59:59")),
            ("2026-11-20".into(), "2026-11-23".into())
        );
        // A source that ends it at midnight already: kept; never shorter than a day.
        assert_eq!(
            stored(true, t("2026-11-20 00:00:00"), t("2026-11-21 00:00:00")),
            ("2026-11-20".into(), "2026-11-21".into())
        );
        assert_eq!(
            stored(true, t("2026-11-20 00:00:00"), t("2026-11-20 00:00:00")),
            ("2026-11-20".into(), "2026-11-21".into())
        );
        assert_eq!(
            stored(false, t("2026-11-12 22:00:00"), t("2026-11-13 02:00:00")),
            ("2026-11-12T22:00".into(), "2026-11-13T02:00".into())
        );
    }
}
