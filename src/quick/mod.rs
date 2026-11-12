//! `/quick`: the menu-bar popover (T81, D46). Capture with kind tiles, start/end a meeting, today's + overdue to-dos.
//! Every form posts to the owning slice's endpoint with `back=/quick`; this slice only reads.
mod handlers;
mod queries;
mod view;

use crate::app::AppState;
use crate::recording::{Mac, MeetingState};
use axum::{Router, routing::get};
use rusqlite::Connection;

pub fn routes() -> Router<AppState> {
    Router::new().route("/quick", get(handlers::page))
}

/// What the tray (Atharu.app) shows: its right-click menu and the icon state.
#[derive(Default, Debug, PartialEq)]
pub struct TrayMenu {
    /// The running meeting (id, name): the menu offers to end it, the icon wears the meeting badge.
    pub running: Option<(i64, String)>,
    /// Else the recent series to start.
    pub series: Vec<String>,
    /// Elapsed clock while audio is captured: the icon turns red (owner: red = capturing only, so a recording
    /// that is finishing or blocked by a permission keeps the meeting badge).
    pub recording: Option<String>,
}

pub fn tray_menu(conn: &Connection, mac: Option<&Mac>) -> rusqlite::Result<TrayMenu> {
    Ok(tray(MeetingState::load(conn, mac)?))
}

fn tray(mut s: MeetingState) -> TrayMenu {
    s.series.truncate(handlers::SERIES);
    TrayMenu {
        recording: if s.capturing() {
            s.rec.live.map(|l| l.elapsed)
        } else {
            None
        },
        running: s.running.map(|r| (r.id, r.name)),
        series: s.series,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recording::{Live, Running};

    #[test]
    fn tray_maps_the_meeting_state() {
        let series = |n: usize| (0..n).map(|i| format!("s{i}")).collect::<Vec<_>>();
        let idle = MeetingState {
            series: series(5),
            ..MeetingState::default()
        };
        assert_eq!(
            tray(idle),
            TrayMenu {
                series: series(3),
                ..TrayMenu::default()
            }
        );
        let live = || {
            Some(Live {
                elapsed: "02:58".into(),
                me: [false; 6],
                others: [false; 6],
                me_n: 0,
                others_n: 0,
            })
        };
        let running = |rec: crate::recording::Rec| MeetingState {
            running: Some(Running {
                id: 7,
                name: "oms-sync".into(),
                started_at: None,
            }),
            rec,
            series: Vec::new(),
        };
        let m = tray(running(crate::recording::Rec {
            live: live(),
            ..Default::default()
        }));
        assert_eq!(m.running, Some((7, "oms-sync".into())));
        assert_eq!(m.recording.as_deref(), Some("02:58"));
        // Finishing and blocked: badge, not red.
        let m = tray(running(crate::recording::Rec {
            finishing: true,
            ..Default::default()
        }));
        assert_eq!((m.running.is_some(), m.recording), (true, None));
        let m = tray(running(crate::recording::Rec {
            live: live(),
            blocked: true,
            ..Default::default()
        }));
        assert_eq!((m.running.is_some(), m.recording), (true, None));
    }
}
