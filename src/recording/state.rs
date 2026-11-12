//! The meeting-state module: "is a meeting running, is it recording, which series exist" behind one `load`.
//! Today, the meetings pages, the popover and the tray all read this interface, so they can't drift apart again
//! (before it, five assemblers held seven copies of the running-meeting query and the tray ignored `finishing`).
use super::{Mac, Rec, queries};
use rusqlite::Connection;

/// The meeting with no `ended_at` (a unique partial index keeps it to one).
pub struct Running {
    pub id: i64,
    pub name: String,
    /// `YYYY-MM-DD HH:MM:SS` local; NULL for meetings started before migration 007.
    pub started_at: Option<String>,
}

#[derive(Default)]
pub struct MeetingState {
    pub running: Option<Running>,
    pub rec: Rec,
    /// Every series name, most recently met first (MAX(day), then the newest meeting); callers cap it.
    pub series: Vec<String>,
}

impl MeetingState {
    pub fn load(conn: &Connection, mac: Option<&Mac>) -> rusqlite::Result<MeetingState> {
        Ok(MeetingState {
            running: queries::running_meeting(conn)?,
            rec: Rec::load(conn, mac)?,
            series: queries::series(conn)?,
        })
    }

    /// Audio is being captured right now: a live recording whose capture actually started (blocked = not yet).
    pub fn capturing(&self) -> bool {
        self.rec.live.is_some() && !self.rec.blocked
    }
}

/// What `partials/meeting_banner.html` reads: the state plus the page's own bits. Start / End live in the
/// meetings slice; Today and the meetings pages render the same partial.
pub struct Banner {
    pub state: MeetingState,
    /// Kept in the box after a rejected start.
    pub name: String,
    pub error: Option<String>,
    /// Page a plain POST returns to.
    pub back: String,
    /// Only End's own answer sets it: (id, name) for "Ended X · Open meeting"; a page load never shows it.
    pub ended: Option<(i64, String)>,
}

impl Banner {
    pub fn new(state: MeetingState, back: String) -> Banner {
        Banner {
            state,
            name: String::new(),
            error: None,
            back,
            ended: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exec(c: &Connection, sql: &str) {
        c.execute_batch(sql).unwrap();
    }

    #[test]
    fn interface_across_states() {
        let c = crate::db::open_in_memory().unwrap();
        // No meeting at all.
        let s = MeetingState::load(&c, None).unwrap();
        assert!(s.running.is_none() && s.series.is_empty() && !s.capturing());

        // Series order: latest day first, same day → the newer meeting first (not the name).
        exec(
            &c,
            "INSERT INTO meetings(id, day, name, started_at, ended_at) VALUES
               (1, '2026-11-10', 'zeta', '2026-11-10 09:00:00', '2026-11-10 10:00:00'),
               (4, '2026-11-11', 'beta', '2026-11-11 09:00:00', '2026-11-11 10:00:00'),
               (3, '2026-11-11', 'alpha', '2026-11-11 11:00:00', '2026-11-11 12:00:00'),
               (2, '2026-11-09', 'beta', '2026-11-09 09:00:00', '2026-11-09 10:00:00');",
        );
        let s = MeetingState::load(&c, None).unwrap();
        assert_eq!(s.series, ["beta", "alpha", "zeta"]);
        assert!(s.running.is_none());

        // Running, no recording.
        exec(
            &c,
            "INSERT INTO meetings(id, day, name, started_at) VALUES (5, '2026-11-12', 'oms-sync', '2026-11-12 09:00:00');",
        );
        let s = MeetingState::load(&c, None).unwrap();
        let r = s.running.as_ref().unwrap();
        assert_eq!((r.id, r.name.as_str()), (5, "oms-sync"));
        assert_eq!(r.started_at.as_deref(), Some("2026-11-12 09:00:00"));
        assert_eq!(s.series[0], "oms-sync");
        assert!(s.rec.live.is_none() && !s.rec.finishing && !s.capturing());

        // Live: capturing (in a browser too: the Mac app records, the browser watches).
        exec(
            &c,
            "INSERT INTO meeting_transcripts(id, meeting_id, status, started_at, model, lang)
             VALUES (1, 5, 'live', '2026-11-12 09:01:00', 'm', 'th');",
        );
        let s = MeetingState::load(&c, None).unwrap();
        assert!(s.rec.live.is_some() && s.capturing() && !s.rec.mac);

        // Live but blocked inside the app: the meeting runs, nothing is captured yet.
        let mac = Mac::new(std::env::temp_dir().join("atharu-state-test"));
        *mac.blocked.lock().unwrap() = true;
        mac.perms.lock().unwrap().0 = true;
        let s = MeetingState::load(&c, Some(&mac)).unwrap();
        assert!(s.rec.mac && s.rec.blocked && s.rec.mic_ok && !s.capturing());

        // Finishing: the tail is transcribing, nothing captured.
        exec(
            &c,
            "UPDATE meeting_transcripts SET status = 'finishing' WHERE id = 1;",
        );
        let s = MeetingState::load(&c, None).unwrap();
        assert!(s.rec.finishing && s.rec.live.is_none() && !s.capturing());
    }
}
