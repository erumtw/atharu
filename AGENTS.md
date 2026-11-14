# Atharu: notes for coding agents (Claude Code, Codex, ...)

Atharu is the owner's personal work log: one Rust binary serving http://127.0.0.1:7070, SQLite at
`data/atharu.db` (gitignored; it holds work data, never commit or paste it elsewhere). Spec:
`docs/superpowers/specs/2026-09-28-atharu-design.md`. `AGENTS.md` is a copy of this file; keep them identical.

## Reading

Read anything with `sqlite3 data/atharu.db` (e.g. `.tables`, `.schema entries`, `SELECT …`). Reads are always fine.

**Never read `data/config.toml`** (no `cat`, Read, grep or parsing it in a script): it holds the owner's credentials
(iCal secret address, Jira API token). Need to know whether an integration is set up? Ask the owner, or check the app's
Settings › Connections page. Writing a fresh template is fine only when the owner asks and the file doesn't exist yet.

Tables (`author` = `'me'` for the owner, else the agent slug that wrote the row):

| table | what | key columns |
|---|---|---|
| `entries` | one logged line | `id`, `day` (YYYY-MM-DD), `epic_key`, `meeting_id`, `kind` (done · decision · blocker · impact · learn · plan), `topic` (learn only), `text`, `plan_state` (open · done · dropped, plans only), `closed_day`, `pinned`, `due`, `link`, `note_id` (the line's note, many lines → one note; NULL = none), `author` |
| `epics` | a Jira key the owner tracks | `key` (FK-123), `title`, `role` (owner · contributor), `okr`, `status` (active · done · dropped), `start`, `target`, `done_on`, `metric`, `before`, `after`, `tags` (`a,b`), `author` |
| `evidence` | links proving an epic's impact | `epic_key`, `url`, `note`, `author` |
| `meetings` | meeting mode; `name` is a series slug (oms-sync) | `id`, `day`, `name`, `note_id` (its note, the meeting's notes; `notes` = pre-010 text, unused), `ended_at` (NULL = running) |
| `series_archive` | a meeting series the owner archived (hidden from Meetings + Calendar; starting a new meeting in it un-archives) | `name`, `archived_at` |
| `topics` | learn pages, **the owner's own understanding** | `slug`, `summary`, `own_words`, `resources` |
| `depth_ratings` | append-only self-rating 1 heard of · 2 can explain · 3 can build w/o AI · 4 can teach | `topic`, `day`, `depth` |
| `notes` | markdown notes; `[[X]]` links a page | `id`, `title`, `body`, `author`, `updated_at` |
| `events` | calendar events | `source` (own · google · mac = macOS Calendar via the Mac app), `title`, `starts_at`, `ends_at` |
| `taggings` | lookup of `#tags`, rebuilt from the text / tags field on every save (never write it) | `tag`, `item_type` (entry · note · epic · …), `item_id` |
| `entry_tags` (view) | a line's tags incl. inherited from its epic / meeting series | `entry_id`, `tag`, `via` (NULL = in the text) |

`[[X]]` in a note resolves in this order: an existing epic's key (`OPS-101`) → `/epics/X`, a tag in use → `/tags/X` (its hub,
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
- `n ` (n + space) at the start of the capture box (Today or the menu-bar popover) saves a **note**, not a line
  (`nginx fixed` stays a line). `atharu log` is unchanged: agents write notes with `atharu note`.
- `#tag` anywhere in the text (`#[a-z0-9][a-z0-9-]*`, case folded) tags the line or note: `booked e2e review #expense`.
  Not a tag: `#` alone, `#123`, a `#` mid-word or in a URL, inside `code`, a markdown heading `# x`.

## Write rules

| You may | You must not |
|---|---|
| read everything | write `topics` or `depth_ratings` |
| write `entries`, `epics`, `evidence` and your own `notes` **only through the `atharu` CLI**, as yourself (`--as <your name>`) | write any table with raw SQL |
| update a note you wrote (`atharu note … --id N`) or a line you wrote (`atharu edit N "<line>"`) | edit or delete rows with `author = 'me'`, or another agent's note / line |
| suggest `[[X]]` links in chat for the owner's notes | delete anything, or edit the owner's lines: those are the owner's, on the web only |
| | rate depth or write "own words": depth only means something if it is the owner's |

Asked for something the CLI can't do yet (to-do done/pin/drop, meetings, topics, events, report):
say "atharu has no command for that yet", suggest the command (e.g. `atharu plan done 42 --as claude`),
and stop. Never fall back to SQL writes.

## CLI

Run as `cargo run -q -- <command> …` (or `target/release/atharu …`). No server needed; it writes the same
`data/atharu.db` (`--db PATH` before the command, or `ATHARU_DB`, picks another file).

```sh
atharu log "<line>" [--day YYYY-MM-DD] [--note N] [--meeting N] --as claude  # one line, same grammar as above
atharu edit <N> "<line>" --as claude                        # rewrite a line YOU wrote (same grammar); others' refused
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
- `edit N "<line>"` rewrites line N (kind, epic, text, due, link, tags) keeping its day, meeting, note and author;
  refused (exit 1) unless that line's author is you. For fixing your own mistakes, e.g. a misheard owner from /meeting.
- `log --note N` links the line to existing note N (its detail page; refused with exit 1 if there is no such note).
- `note` saves a new note under your name (shown under "Written by LLM"); `--links a,b` appends `Links: [[a]] · [[b]]`. Names that resolve to nothing are saved anyway (a `[[slug]]` can invite a topic); the JSON lists them in
  `deadLinks` and stderr warns `Not linked yet: ‘x’`.
  `--id N` rewrites title + body of a note **you** wrote; any other note is refused (exit 1).
- A drawing in a note is one body line `![Drawing](data:image/svg+xml;base64,…)` (editable Excalidraw scene inside):
  build it with `scripts/drawing.py` (`Drawing().box/arrow/text`, then `.line()`), put the line in the body, save with
  `note … --file` (new note, or `--id N` on your own). Never in the owner's notes.
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

**Long real data (owner, 2026-11-14: five overflow bugs in one day, all long text in a fixed slot).** The demo data is
short; the owner's is not (60-char series names, long Jira titles, 30+ assigned tickets, Thai). So:
- Any text in a fixed-width slot (table cell, grid column, chip, row) ends in `…` with the full text in `title`; nothing
  may print over a neighbour or past its card. A new table or grid states its column widths and which column gives way.
- Check UI changes with the long-data seed (`scripts/long-seed.sh`: 60+-char names, long Jira titles, 30 assigned, Thai
  text), not only the short demo DB; a board or a parity match on short text proves nothing about long text.
- Every UI bug the owner reports gets a row in `docs/ui-mistakes.md` (what they saw · cause · rule · check) and a check
  that fails on the old code (a `scripts/visual-check.mjs` rule or a test), proven failing before the fix. A bug without a
  failing check is not fixed. Read that log before UI work.
- Before a release, the master looks at every page (not only the changed ones) on a `.backup` copy of the real DB at
  1180 and 390 px, besides `scripts/visual-check.sh`.

## Working on the code

- `export PATH=/opt/homebrew/opt/rustup/bin:$PATH` (rustup is keg-only here).
- Check before every commit: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.
- Building the Mac app (`--workspace`, `cargo tauri build`) needs `scripts/sherpa-onnx.sh` once: the sha-pinned
  sherpa-onnx static lib for the speaker split (D37); `.cargo/config.toml` points the crate at it.
- Layout: vertical slices (`src/<feature>/{mod,handlers,queries,view}.rs`) + pure `src/domain/`. `domain/` imports
  no feature; features never import each other; SQL only in a slice's `queries.rs`; HTML only in `templates/`;
  every htmx action returns one partial and also works as a plain form POST (303 back). `src/db.rs` holds the only shared SQL (migrations, `retag`, `SELECT_PLAN`, `still_resolves` for renames). `src/cli/` is an entry
  point like `app.rs` and may call slices' queries. `src/layout/` is shared page chrome (sidebar, breadcrumb, look), not a
  feature: every slice may import it, like `web.rs`. `src/recording/` is shared meeting-recording state (running
  meeting, recorder status, the Mac app's `Want`), not a feature: slices may import it, like `layout`. `src/updater.rs` is shared
  update logic (newest release tag, `is_newer`, the one-at-a-time install runner), used by `cli/update.rs` and Settings.
- Releases (D40, D41): `scripts/release.sh X.Y.Z` on main (`--hotfix` on `release/X.Y`) bumps, writes CHANGELOG.md,
  tags `vX.Y.Z`; install/update follow tags. Cutting or pushing a release is the owner's call; an agent asked to run it
  sets `ATHARU_COAUTHOR` and answers N to the push prompt.
- Before every `release.sh`: `scripts/visual-check.sh` (every page × 1180 / 900 / 390 px on a `.backup` copy of the real DB;
  fails on overflow — of the page, out of its card, out of its table cell —, squeezed line text, a jumping page edge,
  console errors; also run it on the long-data seed). Zero problems, or no release. Its screenshots
  hold work data: never commit them; release-page shots come from a seeded demo DB.
- Release page: release.sh's draft (`.git/release-notes-vX.Y.Z.md`) goes to CHANGELOG.md as is; for the GitHub release, rewrite it
  as `## Highlights` (one `###` per user-visible change, two plain sentences on what the owner can now do) + Fixes / Performance
  bullets in plain words + `## Upgrade` (`atharu update --app`; data stays) + `**Full changelog:** vPREV...vX.Y.Z`. A UI change
  gets a screenshot: from a freshly seeded demo DB only (never `data/atharu.db` or a copy of it, not even its settings: the
  repo is public; v0.2.4 leaked real coworker names from Settings › Names and history had to be rewritten). Before
  committing a shot, look at it for real names, companies, tickets or URLs. 1280 wide, saved as
  `docs/releases/vX.Y.Z/<name>.png` and committed before `release.sh`, linked as
  `https://raw.githubusercontent.com/erumtw/atharu/vX.Y.Z/docs/releases/vX.Y.Z/<name>.png`. Perf / internal changes get none.
- Schema changes: a new `migrations/00N_*.sql` appended in `src/db.rs`; never edit a shipped one.
- Frontend JS: strict TypeScript in `editor/src/*.ts` (pinned Tiptap, esbuild) builds the committed `static/js/*.js`
  (`htmx.min.js` is vendored): `cd editor && npm ci && npm run check && node build.mjs`.
- JSON (HTTP and CLI): camelCase keys, references nested (`epic{key, title}`), HTTP collections as
  `data.<resource>[]`; query params snake_case.
