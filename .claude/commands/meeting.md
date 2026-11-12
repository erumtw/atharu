---
description: Write meeting notes from a recorded meeting's transcript into atharu (summary note + decisions, to-dos, blockers)
---
Write up meeting $ARGUMENTS (a meeting number, or `latest` when empty) in atharu.

1. Read `CLAUDE.md` (grammar, write rules, CLI). Run `cargo run -q -- meeting ${ARGUMENTS:-latest}` (or
   `atharu meeting …`). You get the meeting (`name` is its series), what I already logged in it (`lines`,
   `plans`), my own `notes`, `shots[]` (screenshots taken during the meeting: open each `path` image, they often
   hold the slide or ticket being discussed), and `transcripts[].segments` in time order: `speaker` is `me` (my mic),
   `others` (the call), or one of the transcript's `speakers` (the call split by voice: `Speaker 2`, or the name I gave
   it; a split voice can still be two people or one person cut in two), `t` seconds from the recording's start.
   Speech-to-text is imperfect, Thai and English are mixed, and names / epic keys may be misheard: `people[]`
   (`said` → `who`, from Settings › Names) is the truth for names and owners; a name not in it, write as heard with
   `(?)`. Otherwise prefer what I logged and the active epics (`cargo run -q -- show <KEY>`)
   when they disagree. No transcript → work from `lines` + `notes` only and say so.
2. Summary note → `cargo run -q -- note "<series> · <day> notes" --file <tmp.md> --links <series> --as claude`.
   Body, in the language the meeting was mostly in (keep English tech terms as said):
   `## Summary` (3–5 bullets) · `## Decisions` · `## Action items` (owner if said, due if said) · `## Open questions`.
   Quote sparingly, with the timestamp (`[12:30]`). Don't invent owners, dates or numbers.
3. Each decision, to-do and blocker that is **mine or the team's** and not already in `lines` / `plans` →
   `cargo run -q -- log "<line>" --meeting <id> --note <note id> --as claude` (`!` decision, `> … by <when>` to-do,
   `x ` blocker; epic key first when it belongs to one). `--meeting` files it under this meeting even after it
   ended; `--note` links the line to the summary. One thing per line; skip what's only someone else's chatter.
4. Got a line of yours wrong (a misheard owner)? `cargo run -q -- edit <id> "<line>" --as claude` rewrites lines you
   wrote; mine stay mine. Ending the meeting, editing my notes field, closing or pinning to-dos: atharu has no command for that yet. Say so.
   Never write SQL. Never paste the transcript anywhere else.
5. Finish with: the note's id and title, a table of what was logged (id · kind · epic · text), and anything you
   skipped as unclear.
