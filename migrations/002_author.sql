-- Agent access (spec "Agent access"): who wrote each row. 'me' = the user; anything else = an agent slug
-- (claude, codex, ...) writing through the CLI. Existing rows are all mine.
ALTER TABLE entries  ADD COLUMN author TEXT NOT NULL DEFAULT 'me';
ALTER TABLE epics    ADD COLUMN author TEXT NOT NULL DEFAULT 'me';
ALTER TABLE evidence ADD COLUMN author TEXT NOT NULL DEFAULT 'me';

-- notes.author was 'me' | 'llm'; widen to 'me' or any agent slug. SQLite can't alter a CHECK, so rebuild.
CREATE TABLE notes_new (
  id         INTEGER PRIMARY KEY,
  title      TEXT NOT NULL,
  body       TEXT NOT NULL DEFAULT '',
  author     TEXT NOT NULL DEFAULT 'me'
             CHECK (author <> '' AND author NOT GLOB '*[^a-z0-9-]*'),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
INSERT INTO notes_new SELECT id, title, body, author, updated_at FROM notes;
DROP TABLE notes;
ALTER TABLE notes_new RENAME TO notes;
