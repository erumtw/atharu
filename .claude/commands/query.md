---
description: Answer a question about my work from the atharu DB, citing entries
---
Answer from my atharu data: $ARGUMENTS

- Read `CLAUDE.md` for the schema, then query with `sqlite3 data/atharu.db` (read-only: SELECT only).
- Look in `entries` (decisions are `kind='decision'`, results `impact`, to-dos `plan`), `epics` (OKR, status,
  metric before→after, `done_on`), `evidence`, `meetings`, and `notes` (`[[X]]` links tie notes to pages).
- Cite every claim with the entry id and day (`#42 · 2026-11-10`) or note id; say which lines an agent filed
  (`author` ≠ `me`).
- If the data doesn't answer it, say so and say what I could log to make it answerable. Don't guess.
- Write nothing.
