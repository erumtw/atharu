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
4. Links that prove an epic's impact (PR, dashboard, doc with the numbers) →
   `cargo run -q -- evidence <KEY> <url> --label "<what it shows>" --as claude` (after the epic exists). Longer
   context worth keeping (a design summary, meeting write-up) → `cargo run -q -- note "<title>" --file <tmp.md>
   --as claude`.
5. Closing/pinning to-dos, meetings, topics, deleting or editing anything: atharu has no command for that yet.
   Say so, list what you would have filed, and suggest the command. Never write SQL.
6. If a line is rejected (exit 1), fix it per the message and retry once; otherwise report it.
7. Finish with a short table of what was saved (id · kind · epic · text, plus evidence and notes) and anything you skipped.

Tags: prefer a `#tag` as the first word or as a word inside the sentence (`> #comops set up the meeting on topic X by fri`, `> meet on topic X with the #comops team by fri`), and avoid tacking it on at the end, unless neither reads naturally (CLAUDE.md "Line grammar").
