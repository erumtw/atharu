-- Jira read-only titles (spec P6): display cache for keys we already know. Never copied into our rows,
-- so dropping this table loses nothing; the next refresh rebuilds it.
CREATE TABLE jira_issues (
  key        TEXT PRIMARY KEY,
  summary    TEXT NOT NULL,
  status     TEXT NOT NULL,
  fetched_at TEXT NOT NULL DEFAULT (datetime('now'))
);
