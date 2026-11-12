---
description: Summarise a link or file into an agent-written note with [[topic]] links, plus questions for me
---
Ingest this source for my atharu notes: $ARGUMENTS

1. Read the source (URL or file). Read `CLAUDE.md`; list existing topics with
   `sqlite3 data/atharu.db "SELECT slug, summary FROM topics"` (SELECT only).
2. Draft one note in markdown: a title, a short summary of the source in your words, key points, the source link,
   and `[[slug]]` links to existing topic slugs where they fit (only slugs that exist; mention possible new topics
   in plain text instead).
3. Never fill a topic page: no `topics`/`depth_ratings` writes, no "own words". Instead list 3–5 questions I
   should answer myself on `/learn/<slug>` (things the source claims that I should be able to explain or test).
4. Save it: write the markdown body to a temp file, then
   `cargo run -q -- note "<title>" --file <tmp.md> --links <slug>,<slug> --as claude` (the `--links` are the
   topic slugs you linked, if not already in the body). Never raw SQL. To revise a note you wrote earlier, add
   `--id <its id>`; a note I wrote is refused, so make a new one instead.
5. Report the saved note's id (`/notes/<id>`) and the questions from step 3.
