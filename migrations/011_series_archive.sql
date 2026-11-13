-- 0.2.0 (D4, D14): an archived meeting series is hidden from the Meetings list and the Calendar; its pages stay.
-- A series is only `meetings.name`, so no FK: rename_series moves the row in the same transaction.
CREATE TABLE series_archive (
  name TEXT PRIMARY KEY,
  archived_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);
