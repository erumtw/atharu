-- Meeting recording (T53, schema D33): one row per recording. Segments land as JSON while the meeting
-- runs, so a crash keeps what was already transcribed. Audio is never stored.
CREATE TABLE meeting_transcripts (
  id         INTEGER PRIMARY KEY,
  meeting_id INTEGER NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
  started_at TEXT NOT NULL,
  ended_at   TEXT,
  status     TEXT NOT NULL DEFAULT 'live'
             CHECK (status IN ('live', 'finishing', 'done', 'interrupted')),
  model      TEXT NOT NULL,
  lang       TEXT NOT NULL,
  -- [{t: seconds from started_at, speaker: me|others, text}]
  segments   TEXT NOT NULL DEFAULT '[]'
);
CREATE INDEX meeting_transcripts_meeting ON meeting_transcripts(meeting_id);
-- One recorder on the Mac: at most one recording is being written (live, or finishing its tail).
CREATE UNIQUE INDEX meeting_transcripts_one_running ON meeting_transcripts((1))
  WHERE status IN ('live', 'finishing');
