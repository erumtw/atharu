-- The quick popover shows "since 09:17 · 24 min" for a running meeting; `day` alone can't say that.
-- Local wall-clock time, like ended_at. NULL for meetings started before this column existed.
ALTER TABLE meetings ADD COLUMN started_at TEXT;
