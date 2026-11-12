-- T90 (D58): a meeting's notes are a real note (editor, [[links]], #tags, drawings). The old `notes` text column
-- stays (rollback); db.rs moves non-empty text into one note per meeting inside this migration's transaction.
ALTER TABLE meetings ADD COLUMN note_id INTEGER REFERENCES notes(id) ON DELETE SET NULL;
