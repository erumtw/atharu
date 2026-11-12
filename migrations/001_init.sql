-- Full schema for every phase (spec "Data model"): later phases add no migrations,
-- unused tables sit empty. New schema changes go in 002_*.sql (db.rs MIGRATIONS).
CREATE TABLE IF NOT EXISTS epics (
  key    TEXT PRIMARY KEY,
  title  TEXT NOT NULL DEFAULT '',
  role   TEXT NOT NULL DEFAULT 'owner' CHECK (role IN ('owner','contributor')),
  okr    TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','done','dropped')),
  start  TEXT, target TEXT, done_on TEXT,
  metric TEXT NOT NULL DEFAULT '', before TEXT NOT NULL DEFAULT '', after TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS meetings (
  id       INTEGER PRIMARY KEY,
  day      TEXT NOT NULL,
  name     TEXT NOT NULL,
  notes    TEXT NOT NULL DEFAULT '',
  ended_at TEXT
);
-- At most one meeting without ended_at: the 2nd start hits this index.
CREATE UNIQUE INDEX IF NOT EXISTS meetings_one_active ON meetings((1)) WHERE ended_at IS NULL;

CREATE TABLE IF NOT EXISTS entries (
  id         INTEGER PRIMARY KEY,
  day        TEXT NOT NULL,
  epic_key   TEXT REFERENCES epics(key),
  meeting_id INTEGER REFERENCES meetings(id) ON DELETE SET NULL,
  kind       TEXT NOT NULL CHECK (kind IN ('done','decision','blocker','impact','learn','plan')),
  topic      TEXT,
  text       TEXT NOT NULL,
  plan_state TEXT CHECK (plan_state IN ('open','done','dropped')),
  closed_day TEXT,
  pinned     INTEGER NOT NULL DEFAULT 0,
  due        TEXT,
  link       TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  CHECK ((kind = 'plan') = (plan_state IS NOT NULL))
);
CREATE INDEX IF NOT EXISTS entries_day     ON entries(day);
CREATE INDEX IF NOT EXISTS entries_epic    ON entries(epic_key);
CREATE INDEX IF NOT EXISTS entries_meeting ON entries(meeting_id);
CREATE INDEX IF NOT EXISTS entries_open_plans ON entries(due) WHERE plan_state = 'open';

CREATE TABLE IF NOT EXISTS evidence (
  id       INTEGER PRIMARY KEY,
  epic_key TEXT NOT NULL REFERENCES epics(key) ON DELETE CASCADE,
  url      TEXT NOT NULL,
  note     TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS topics (
  slug      TEXT PRIMARY KEY,
  summary   TEXT NOT NULL DEFAULT '',
  own_words TEXT NOT NULL DEFAULT '',
  resources TEXT NOT NULL DEFAULT ''
);

-- Append-only: the history is the point (depth as of any month end).
CREATE TABLE IF NOT EXISTS depth_ratings (
  topic TEXT NOT NULL REFERENCES topics(slug) ON DELETE CASCADE,
  day   TEXT NOT NULL,
  depth INTEGER NOT NULL CHECK (depth BETWEEN 1 AND 4)
);

CREATE TABLE IF NOT EXISTS events (
  id        INTEGER PRIMARY KEY,
  source    TEXT NOT NULL DEFAULT 'own' CHECK (source IN ('own','google')),
  ext_id    TEXT,
  title     TEXT NOT NULL,
  starts_at TEXT NOT NULL,
  ends_at   TEXT,
  UNIQUE (source, ext_id)
);
CREATE INDEX IF NOT EXISTS events_start ON events(starts_at);

CREATE TABLE IF NOT EXISTS notes (
  id         INTEGER PRIMARY KEY,
  title      TEXT NOT NULL,
  body       TEXT NOT NULL DEFAULT '',
  author     TEXT NOT NULL DEFAULT 'me' CHECK (author IN ('me','llm')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
