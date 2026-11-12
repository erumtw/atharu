---
description: Check my learn topics and notes for problems that need judgment; report only, change nothing
---
Lint my atharu learning data. Read `CLAUDE.md`, then query `sqlite3 data/atharu.db` (SELECT only). Report in chat;
write nothing (no CLI, no SQL).

Check:
1. Near-duplicate topic slugs across `entries.topic` and `topics.slug` (`vector-search` vs `vectorsearch`,
   singular/plural, typos) → suggest which to merge into.
2. `[[X]]` in `notes.body` that resolves to nothing (not an epic key, topic slug or meeting name) → likely typo
   (suggest the closest page) or a missing topic.
3. A topic `summary` that later learn entries or notes contradict → quote both and say
   "re-explain this in your own words on /learn/<slug>". Never propose the new wording yourself.
4. Learn entries with no topic, and topics never rated → a short nudge list, not a fix.

The app already flags stale and unrated topics on /learn; only add what needs judgment. Group findings by check,
most useful first, with ids/slugs so I can find them. If nothing is wrong, say so in one line.
