-- Line notes (D49): many lines → one note; the note page is the line's detail page. SET NULL, not CASCADE:
-- deleting a note unlinks its lines, deleting a line keeps its note. The index serves the note header
-- (`WHERE note_id = ?`) and makes the SET NULL on a note delete an index lookup, not a table scan.
ALTER TABLE entries ADD COLUMN note_id INTEGER REFERENCES notes(id) ON DELETE SET NULL;
CREATE INDEX entries_note ON entries(note_id);
