# Atharu: notes for coding agents (Claude Code, Codex, ...)

Atharu is the owner's personal work log: one Rust binary serving http://127.0.0.1:7070, SQLite at
`data/atharu.db` (gitignored; it holds work data, never commit or paste it elsewhere). Spec:
`docs/superpowers/specs/2026-09-28-atharu-design.md`. `AGENTS.md` is a copy of this file; keep them identical.

## Reading

Read anything with `sqlite3 data/atharu.db` (e.g. `.tables`, `.schema entries`, `SELECT …`). Reads are always fine.

Tables (`author` = `'me'` for the owner, else the agent slug that wrote the row):

| table | what | key columns |
|---|---|---|
| `entries` | one logged line | `id`, `day` (YYYY-MM-DD), `epic_key`, `meeting_id`, `kind` (done · decision · blocker · impact · learn · plan), `topic` (learn only), `text`, `plan_state` (open · done · dropped, plans only), `closed_day`, `pinned`, `due`, `link`, `note_id` (the line's note, many lines → one note; NULL = none), `author` |
| `epics` | a Jira key the owner tracks | `key` (FK-123), `title`, `role` (owner · contributor), `okr`, `status` (active · done · dropped), `start`, `target`, `done_on`, `metric`, `before`, `after`, `tags` (`a,b`), `author` |
| `evidence` | links proving an epic's impact | `epic_key`, `url`, `note`, `author` |
| `meetings` | meeting mode; `name` is a series slug (oms-sync) | `id`, `day`, `name`, `notes`, `ended_at` (NULL = running) |
| `topics` | learn pages, **the owner's own understanding** | `slug`, `summary`, `own_words`, `resources` |
| `depth_ratings` | append-only self-rating 1 heard of · 2 can explain · 3 can build w/o AI · 4 can teach | `topic`, `day`, `depth` |
| `notes` | markdown notes; `[[X]]` links a page | `id`, `title`, `body`, `author`, `updated_at` |
| `events` | calendar events | `source` (own · google), `title`, `starts_at`, `ends_at` |
| `taggings` | lookup of `#tags`, rebuilt from the text / tags field on every save (never write it) | `tag`, `item_type` (entry · note · epic · …), `item_id` |
| `entry_tags` (view) | a line's tags incl. inherited from its epic / meeting series | `entry_id`, `tag`, `via` (NULL = in the text) |

`[[X]]` in a note resolves in this order: epic key (`OPS-101`) → `/epics/X`, a tag in use → `/tags/X` (its hub,
which also shows the topic and series of the same name), existing topic slug → `/learn/X`, existing meeting
name → `/meetings?name=X`, else nothing (a dead link).

## Line grammar (what `atharu log` and the capture bar accept)

`[EPIC-KEY] [prefix] text [#tag …] [link] [by <when>]`

| prefix | kind | example |
|---|---|---|
| none | done | `OPS-101 shipped index migration` |
| `!` | decision | `OPS-101 ! keep sync path, no queue` |
| `x ` | blocker | `OPS-101 x waiting on DBA` |
| `+` | impact | `OPS-101 + p95 1.8s → 0.6s` |
| `?` | learn | `? vector-search: HNSW trades recall for speed` (`slug:` sets the topic) |
| `>` | plan / to-do | `> reply to vendor https://… by fri` |
| `#tag` anywhere | tags it (any kind) | `OPS-140 booked e2e review #expense` |

- The key is optional and must be the first token (`^[A-Z][A-Z0-9]*-\d+$`); an unknown key creates a stub epic.
- The first `http(s)://` token is saved as `link` (and stays in the text).
- `by <when>` only on `>` lines: `today`, `tomorrow`, a weekday (`fri`, today counts), `20 Nov`, `2026-11-20`.
  Words after `by` that aren't a date stay text (`> refactor by hand`); an impossible date (`by 32 Nov`) is refused.
- `xml parser fixed` is a done line: `x` is a prefix only when followed by a space.
- `#tag` anywhere in the text (`#[a-z0-9][a-z0-9-]*`, case folded) tags the line or note: `booked e2e review #expense`.
  Not a tag: `#` alone, `#123`, a `#` mid-word or in a URL, inside `code`, a markdown heading `# x`.

## Write rules

| You may | You must not |
|---|---|
| read everything | write `topics` or `depth_ratings` |
| write `entries`, `epics`, `evidence` and your own `notes` **only through the `atharu` CLI**, as yourself (`--as <your name>`) | write any table with raw SQL |
| update a note you wrote (`atharu note … --id N`) | edit or delete rows with `author = 'me'`, or another agent's note |
| suggest `[[X]]` links in chat for the owner's notes | delete or edit anything: deletes and line edits are the owner's, on the web only |
| | rate depth or write "own words": depth only means something if it is the owner's |

Asked for something the CLI can't do yet (to-do done/pin/drop, meetings, topics, events, report):
say "atharu has no command for that yet", suggest the command (e.g. `atharu plan done 42 --as claude`),
and stop. Never fall back to SQL writes.

## CLI

Run as `cargo run -q -- <command> …` (or `target/release/atharu …`). No server needed; it writes the same
`data/atharu.db` (`--db PATH` before the command, or `ATHARU_DB`, picks another file).

```sh
atharu log "<line>" [--day YYYY-MM-DD] [--note N] [--meeting N] --as claude  # one line, same grammar as above
atharu epic <KEY|jira-url> [--title …] [--okr …] [--tags a,b] --as claude  # upsert; key from …/browse/ABC-123
atharu note "<title>" [--file PATH] [--links a,b] [--id N] --as claude  # agent note; body from stdin without --file
atharu evidence <KEY|jira-url> <url> [--label …] --as claude  # impact link on an existing epic, http(s) only
atharu today [--day YYYY-MM-DD]                             # that day's lines + open plans
atharu show <KEY|jira-url>                                  # epic + evidence + timeline
atharu meeting <N|latest>                                   # meeting + its lines/plans + transcript segments (read-only)
atharu tags                                                 # {tags: [{tag, count, lastUsed}]}, most used first
atharu version                                              # {version, tag, sha, built}; tag null on a dev build; no DB
atharu update [--app] [--docker]                            # owner's tool: newest release tag vX.Y.Z via scripts/install.sh, else {upToDate}
atharu dump                                                 # data/dump.sql next to the DB (plain SQL backup)
atharu help                                                 # this table (also -h / --help); bare `atharu` serves :7070
```

- Every write needs `--as <agent>` or `ATHARU_AGENT=<agent>`: a lowercase slug (`claude`, `codex`). `me` is refused.
- `epic` never overwrites a field the owner set: it only fills an empty title / OKR / tags.
- `log` / `note` tag from their text (`#tag`); the JSON echoes `tags`. A line also counts under its epic's tags.
- `log --meeting N` files the line under meeting N (refused with exit 1 if none); without it a line goes to the
  running meeting, if any. `/meeting` uses it to write up a recording after the meeting ended.
- `log --note N` links the line to existing note N (its detail page; refused with exit 1 if there is no such note).
- `note` saves a new note under your name (shown under "Written by LLM"); `--links a,b` appends `Links: [[a]] · [[b]]`.
  `--id N` rewrites title + body of a note **you** wrote; any other note is refused (exit 1).
- `evidence` needs the epic to exist (file it with `epic` first) and an `http://` / `https://` link.
- Output: one JSON document on stdout (camelCase keys; a referenced epic is nested as `epic{key, …}`, a line's
  note as `note{id, title}` or `null`).
- Exit codes: `0` saved/printed · `1` rejected, with the app's own message on stderr (e.g.
  `Nothing after “+”. Type what happened.`, `Can't read date ‘someday’`) · `2` usage, incl. a missing `--as`
  (`Say who is writing: --as claude`).
- What you file shows in the app with your name as a badge, and the report marks it `(via claude)`.

Prompts for the common jobs: `.claude/commands/{log,query,lint,ingest,meeting}.md`.

## UI work: always DESIGN.md

`DESIGN.md` is the design system (Apple look · Liquid Glass on floating chrome · Notion UX). Read it before any change
to `templates/`, `static/css/` or `editor/`, and follow it: tokens from `tokens.css` only, glass only where §4 lists it,
never glass on glass, Reduce Transparency / Increase Contrast fallbacks, AA measured and noted in a CSS comment, no
Save buttons (autosave), and the self-explanatory goal (empty states name the next action). A change that adds or
alters a token, component, glass surface or pattern updates `DESIGN.md` in the same commit; when the CSS and
`DESIGN.md` disagree, fix whichever is wrong and say so.

## Working on the code

- `export PATH=/opt/homebrew/opt/rustup/bin:$PATH` (rustup is keg-only here).
- Check before every commit: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.
- Layout: vertical slices (`src/<feature>/{mod,handlers,queries,view}.rs`) + pure `src/domain/`. `domain/` imports
  no feature; features never import each other; SQL only in a slice's `queries.rs`; HTML only in `templates/`;
  every htmx action returns one partial and also works as a plain form POST (303 back). `src/cli/` is an entry
  point like `app.rs` and may call slices' queries. `src/layout/` is shared page chrome (sidebar, breadcrumb, look), not a
  feature: every slice may import it, like `web.rs`.
- Releases (D40, D41): `scripts/release.sh X.Y.Z` on main (`--hotfix` on `release/X.Y`) bumps, writes CHANGELOG.md,
  tags `vX.Y.Z`; install/update follow tags. Cutting or pushing a release is the owner's call; an agent asked to run it
  sets `ATHARU_COAUTHOR` and answers N to the push prompt.
- Schema changes: a new `migrations/00N_*.sql` appended in `src/db.rs`; never edit a shipped one.
- Frontend JS: strict TypeScript in `editor/src/*.ts` (pinned Tiptap, esbuild) builds the committed `static/js/*.js`
  (`htmx.min.js` is vendored): `cd editor && npm ci && npm run check && node build.mjs`.
- JSON (HTTP and CLI): camelCase keys, references nested (`epic{key, title}`), HTTP collections as
  `data.<resource>[]`; query params snake_case.
