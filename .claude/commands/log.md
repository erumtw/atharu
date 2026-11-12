---
description: Turn pasted text (an epic link, notes, a chat log) into atharu entries, then show what was saved
---
File this into atharu for me:

$ARGUMENTS

1. Read `CLAUDE.md` (grammar, write rules, CLI). Run `cargo run -q -- today` and, for each epic key involved,
   `cargo run -q -- show <KEY>` so you don't file duplicates.
2. A Jira link or key → `cargo run -q -- epic <link> --as claude`, adding `--title` / `--okr` only when the text
   states them. It never overwrites what I set.
3. Each fact → one `cargo run -q -- log "<line>" --as claude` in the line grammar: decisions `!`, blockers `x `,
   measured results `+`, to-dos `>` (with `by <when>` if a date is given), things learned `? slug:`, plain done
   lines otherwise. Use the epic key first when the line belongs to one. Use `--day` for things that happened on
   another day. One thing per line; keep my wording, don't embellish.
4. Evidence links, closing/pinning to-dos, meetings, topics, notes: atharu has no command for that yet. Say so,
   list what you would have filed, and suggest the command. Never write SQL.
5. If a line is rejected (exit 1), fix it per the message and retry once; otherwise report it.
6. Finish with a short table of what was saved (id · kind · epic · text) and anything you skipped.
