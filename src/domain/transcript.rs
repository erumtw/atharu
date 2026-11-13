/// Where a recording is in its life. Stored as the lowercase `meeting_transcripts.status` string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxStatus {
    Live,
    /// Stopped; the app transcribes the tail, then posts `done`.
    Finishing,
    Done,
    /// Its recorder stopped without a `done` (crash, quit, meeting ended).
    Interrupted,
}

impl TxStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TxStatus::Live => "live",
            TxStatus::Finishing => "finishing",
            TxStatus::Done => "done",
            TxStatus::Interrupted => "interrupted",
        }
    }

    pub fn parse(s: &str) -> Option<TxStatus> {
        [
            TxStatus::Live,
            TxStatus::Finishing,
            TxStatus::Done,
            TxStatus::Interrupted,
        ]
        .into_iter()
        .find(|k| k.as_str() == s)
    }
}

#[cfg(test)]
mod tests {
    use super::TxStatus;

    #[test]
    fn parse_round_trips_every_status() {
        for s in ["live", "finishing", "done", "interrupted"] {
            assert_eq!(TxStatus::parse(s).map(TxStatus::as_str), Some(s));
        }
        assert_eq!(TxStatus::parse("paused"), None);
    }
}
