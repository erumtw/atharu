-- Tags (D29, D36 option A): text and the tags fields stay the source of truth; taggings is a lookup
-- rebuilt per item on save (db::retag), so hub / filters / counts are one indexed query. No tags table:
-- a tag exists while a tagging uses it. Meeting series have no row: their tags live here only
-- (item_type 'series', item_id = meetings.name). Inheritance is read time (entry_tags), never stored.
CREATE TABLE taggings (
  tag       TEXT NOT NULL,
  item_type TEXT NOT NULL CHECK (item_type IN ('entry','note','event','epic','topic','series')),
  item_id   TEXT NOT NULL,
  PRIMARY KEY (tag, item_type, item_id)
) WITHOUT ROWID;
CREATE INDEX taggings_item ON taggings(item_type, item_id);

ALTER TABLE epics  ADD COLUMN tags TEXT NOT NULL DEFAULT '';
ALTER TABLE topics ADD COLUMN tags TEXT NOT NULL DEFAULT '';

-- Deletes from any path (web, undo, sqlite3) leave no stale taggings; plain SQL, no app function.
CREATE TRIGGER entries_untag AFTER DELETE ON entries BEGIN
  DELETE FROM taggings WHERE item_type = 'entry' AND item_id = CAST(OLD.id AS TEXT);
END;
CREATE TRIGGER notes_untag AFTER DELETE ON notes BEGIN
  DELETE FROM taggings WHERE item_type = 'note' AND item_id = CAST(OLD.id AS TEXT);
END;
CREATE TRIGGER epics_untag AFTER DELETE ON epics BEGIN
  DELETE FROM taggings WHERE item_type = 'epic' AND item_id = OLD.key;
END;

-- A line's tags: its own (via NULL), its epic's (via = key), its meeting series' (via = name).
-- A line shows once per (tag, source), so a tag both typed and inherited yields two rows.
CREATE VIEW entry_tags AS
  -- Joined through entries (not CAST(item_id)) so `entry_id = ?` hits the rowid and taggings_item.
  SELECT e.id AS entry_id, t.tag, NULL AS via
    FROM entries e JOIN taggings t ON t.item_type = 'entry' AND t.item_id = CAST(e.id AS TEXT)
  UNION ALL
  SELECT e.id, t.tag, e.epic_key
    FROM entries e JOIN taggings t ON t.item_type = 'epic' AND t.item_id = e.epic_key
  UNION ALL
  SELECT e.id, t.tag, m.name
    FROM entries e JOIN meetings m ON m.id = e.meeting_id
    JOIN taggings t ON t.item_type = 'series' AND t.item_id = m.name;
