-- 0.2.4: events from macOS Calendar (the Mac app reads EventKit) are source 'mac'. SQLite can't widen a CHECK,
-- so rebuild the table: same columns, ids and UNIQUE (source, ext_id); nothing references events.
CREATE TABLE events_new (
  id        INTEGER PRIMARY KEY,
  source    TEXT NOT NULL DEFAULT 'own' CHECK (source IN ('own','google','mac')),
  ext_id    TEXT,
  title     TEXT NOT NULL,
  starts_at TEXT NOT NULL,
  ends_at   TEXT,
  UNIQUE (source, ext_id)
);
INSERT INTO events_new(id, source, ext_id, title, starts_at, ends_at)
  SELECT id, source, ext_id, title, starts_at, ends_at FROM events;
DROP TABLE events;
ALTER TABLE events_new RENAME TO events;
CREATE INDEX events_start ON events(starts_at);
