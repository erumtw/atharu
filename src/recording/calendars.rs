//! macOS Calendar (EventKit, 0.2.4) as the server sees it. Only Atharu.app can read EventKit: it sets
//! `AppState.mac_cals`, fills the calendar list and the access state, and writes events through
//! `calendar::replace_mac_events`. `None` = a browser view, which shows what the app synced.
use std::sync::{Mutex, MutexGuard};

/// One calendar in Calendar.app, for Settings' checkboxes.
#[derive(Clone, Debug, PartialEq)]
pub struct MacCalendar {
    /// EventKit `calendarIdentifier`; `mac_cals` stores these.
    pub id: String,
    pub title: String,
    /// The account it syncs from (e.g. the work Google account), shown muted.
    pub account: String,
    /// `#rrggbb`, the calendar's own colour.
    pub color: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Access {
    /// Not asked yet: the app asks when the switch turns on.
    #[default]
    Unknown,
    Granted,
    Denied,
}

#[derive(Default)]
pub struct MacCalendars {
    list: Mutex<Vec<MacCalendar>>,
    access: Mutex<Access>,
    /// A settings save: the app asks for access if needed and syncs now.
    pub kick: tokio::sync::Notify,
}

/// A panicked writer poisons the lock; the value inside is still fine to read.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl MacCalendars {
    pub fn list(&self) -> Vec<MacCalendar> {
        lock(&self.list).clone()
    }

    pub fn set_list(&self, cals: Vec<MacCalendar>) {
        *lock(&self.list) = cals;
    }

    pub fn access(&self) -> Access {
        *lock(&self.access)
    }

    pub fn set_access(&self, a: Access) {
        *lock(&self.access) = a;
    }
}
