# Atharu (أثر) — design

2026-09-28 · status: approved in chat, pending spec review

## Goal

Personal work log → prove impact at OKR/perf review + track real understanding vs AI-surface knowledge.
Jobs: ownership · progress · planning · to-dos · meetings · calendar · impact evidence · deep learning · notes.

Success:
- capture ≤ 2 min/day, one line per thing
- paste-ready weekly note / self-review / OKR draft from one page, no hand-writing summaries
- plan hit rate + depth history show growth over time
- self-explanatory: a first-time user can use every page with no tutorial or guide (empty states show the first action, the input teaches the line grammar, no unexplained jargon)

Rule: never store what Jira already knows. Store plans, decisions, impact, learning only. (P6 caches Jira titles for display only; it is a cache, never copied into our rows.)

## Assumptions

- single user, localhost, no auth, no hosting
- Jira = key + link; read-only title/status lookup in P6 (no writes, no issue lists)
- SQLite = single source of truth (no markdown files)
- not a Notion clone: no block editor, nesting, DB views, drag-drop

## Phases

Each phase shippable + used daily before next starts.

| Phase | Scope | Why this order |
|---|---|---|
| 1 | capture + grammar · epics · report | core loop, usable day 1 |
| 2 | to-dos (`>` + link + due) · progress flags · meeting mode | builds on `entries` |
| 3 | learn pages + depth heatmap | needs weeks of `?` data first |
| 4 | notes + `[[links]]` + LLM layer | only if Obsidian/Notion not enough; LLM needs notes + topics to work on |
| 5 | calendar view → Google Calendar read-only (iCal) | needs due dates + meetings from P2 to have something to show |
| 6 | Jira read-only titles (cache summary · status for known keys) | needs epics + links from P1/P2; added 2026-11-11 |
| 7 | Mac app: Tauri 2 shell around the same server | needs every page to exist; added 2026-11-11 |

Phase 1 grammar already parses all prefixes (`>` and `?` saved as entries); their pages arrive in later phases.

## Architecture

2026-11-11: Go → Rust (owner's choice: learning Rust is part of the point). Same shape: one binary, server-rendered.

- one Rust binary `atharu`, serves `http://localhost:7070`
- crates: `axum` (HTTP) · `askama` (Jinja-style templates, checked at compile time) · `rusqlite` with `bundled` (SQLite compiled in) · `rust-embed` (templates + CSS + htmx in the binary) · `pulldown-cmark` (phase 4 only, raw HTML off)
- frontend = the wireframe canvas's own HTML/CSS ported to askama templates → design parity by construction; no CSS framework, no JS build
- `htmx` (one vendored file, ~14 KB) for no-reload actions: log line, delete/undo, plan check/pin/drop, property autosave (`hx-trigger="change"`, Notion-style, no Save button); every action still works as a plain `<form>` POST without JS
- other JS: copy-to-clipboard · `editor.js` (Tiptap, notes page only, P4)
- heatmap / sparkline = CSS, no chart lib
- design: canvas https://claude.ai/artifact/SJY9VrvtZb5r6iZzRzLjSZ (Apple DESIGN.md look + Liquid Glass on floating chrome, Notion UX patterns)
- DB path: `./data/atharu.db` (flag `-db` overrides), created + migrated on start
- `data/` gitignored — DB holds Freshket work data (epic keys, metrics); never pushed

Subcommands:
- `atharu` → serve
- `atharu dump` → SQL dump to `data/dump.sql` (backup; stays local unless user opts in)

## Data model

Full schema created in phase 1 → no migrations between phases; unused tables sit empty.
Later schema changes → `PRAGMA user_version` + ordered migration list.

```sql
CREATE TABLE epics (
  key    TEXT PRIMARY KEY,                  -- e.g. FK-123
  title  TEXT NOT NULL DEFAULT '',
  role   TEXT NOT NULL DEFAULT 'owner' CHECK (role IN ('owner','contributor')),
  okr    TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','done','dropped')),
  start  TEXT, target TEXT, done_on TEXT,   -- YYYY-MM-DD
  metric TEXT NOT NULL DEFAULT '', before TEXT NOT NULL DEFAULT '', after TEXT NOT NULL DEFAULT ''
);

CREATE TABLE meetings (
  id       INTEGER PRIMARY KEY,
  day      TEXT NOT NULL,
  name     TEXT NOT NULL,                   -- slug, e.g. oms-sync; reused across days
  notes    TEXT NOT NULL DEFAULT '',
  ended_at TEXT                             -- NULL = active; max one active
);
CREATE UNIQUE INDEX meetings_one_active ON meetings((1)) WHERE ended_at IS NULL;

CREATE TABLE entries (
  id         INTEGER PRIMARY KEY,
  day        TEXT NOT NULL,                 -- YYYY-MM-DD
  epic_key   TEXT REFERENCES epics(key),
  meeting_id INTEGER REFERENCES meetings(id) ON DELETE SET NULL,
  kind       TEXT NOT NULL CHECK (kind IN ('done','decision','blocker','impact','learn','plan')),
  topic      TEXT,                          -- kind='learn' with "slug:" only
  text       TEXT NOT NULL,
  plan_state TEXT CHECK (plan_state IN ('open','done','dropped')),  -- kind='plan' only
  closed_day TEXT,                          -- plan done/dropped day
  pinned     INTEGER NOT NULL DEFAULT 0,    -- plan in "this week top 3"
  due        TEXT,                          -- YYYY-MM-DD, kind='plan' only, optional
  link       TEXT,                          -- first http(s) URL in the line: source task (Jira/Slack/PR/doc), any kind
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  CHECK ((kind = 'plan') = (plan_state IS NOT NULL))
);
CREATE INDEX entries_day     ON entries(day);
CREATE INDEX entries_epic    ON entries(epic_key);
CREATE INDEX entries_meeting ON entries(meeting_id);
CREATE INDEX entries_open_plans ON entries(due) WHERE plan_state = 'open';

CREATE TABLE evidence (
  id       INTEGER PRIMARY KEY,
  epic_key TEXT NOT NULL REFERENCES epics(key) ON DELETE CASCADE,
  url      TEXT NOT NULL,
  note     TEXT NOT NULL DEFAULT ''
);

CREATE TABLE topics (
  slug      TEXT PRIMARY KEY,               -- e.g. vector-search
  summary   TEXT NOT NULL DEFAULT '',       -- current one-line understanding
  own_words TEXT NOT NULL DEFAULT '',       -- no-AI explanation
  resources TEXT NOT NULL DEFAULT ''
);

CREATE TABLE depth_ratings (               -- append-only
  topic TEXT NOT NULL REFERENCES topics(slug) ON DELETE CASCADE,
  day   TEXT NOT NULL,
  depth INTEGER NOT NULL CHECK (depth BETWEEN 1 AND 4)
);

CREATE TABLE events (                      -- P5: calendar; own events now, Google read-only later
  id        INTEGER PRIMARY KEY,
  source    TEXT NOT NULL DEFAULT 'own' CHECK (source IN ('own','google')),
  ext_id    TEXT,                           -- iCal UID when source='google'; NULL for own
  title     TEXT NOT NULL,
  starts_at TEXT NOT NULL,                  -- local ISO datetime, or YYYY-MM-DD for all-day
  ends_at   TEXT,
  UNIQUE (source, ext_id)
);
CREATE INDEX events_start ON events(starts_at);

CREATE TABLE notes (
  id         INTEGER PRIMARY KEY,
  title      TEXT NOT NULL,
  body       TEXT NOT NULL DEFAULT '',      -- markdown
  author     TEXT NOT NULL DEFAULT 'me' CHECK (author IN ('me','llm')),  -- (P4 migration 002 widens to any agent slug, see "Agent access") LLM pages shown apart, never mixed with mine
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

Depth: 1 heard of · 2 can explain · 3 can build w/o AI · 4 can teach.
All `CREATE ... IF NOT EXISTS`, `PRAGMA foreign_keys=ON`.
No attendee field on meetings — privacy (no personal data) + YAGNI. Team names go in notes/text if needed.

## Line grammar (parser) — phase 1

`[EPIC-KEY] [prefix] text [link] [by <when>]` — pure fn `parse(line: &str, today: NaiveDate) -> Result<Entry, ParseError>`.

| prefix | kind |
|---|---|
| none | done |
| `!` | decision |
| `x` | blocker |
| `+` | impact |
| `?` | learn |
| `>` | plan (`plan_state='open'`) |

- key = first token matching `^[A-Z][A-Z0-9]*-\d+$`; optional, must be first token
- prefix = next token if exactly one of `! x + ? >`
- `?` with `slug: rest` → topic = slug (lowercased, `[a-z0-9-]+`), text = rest; no colon → topic empty
- empty/whitespace-only or empty text after prefix → error, line stays in input box
- unknown key → stub epic row inserted (title empty) → listed as "unfiled"
- `x` only a prefix when followed by space (so `xml parser fixed` = done)
- active meeting exists → `meeting_id` set on save (handler, not parser)
- first `http(s)://…` token anywhere → `link` (kept in text too); shown as a chip labelled by host: Jira · Slack · GitHub · Docs · else domain
- `>` lines only: trailing `by <when>` → `due`; `<when>` = `today` · `tomorrow` · weekday (`fri` = next one, today counts) · `20 Nov` / `2026-11-20`; unparseable → error "Can't read date ‘…’", line kept

## Pages

### Phase 1

**`/` Today** — autofocus input, Enter submits, date field (default today) for backfill · today's entries grouped by epic · delete per entry (no edit; delete + retype) · parse error → inline, input kept.

**`/epics`** — grouped by OKR · status + target · unfiled stubs on top.
**`/epics/{key}`** — edit form (title, role, OKR, status, dates, metric, before, after) · add/delete evidence links · entry timeline for key · impact line `Did {title} → {metric} {before}→{after} → {evidence urls}` + copy button.

**`/report?from=&to=`** — presets: this week (Mon start) / month / quarter · default = this week.
- visual: epic status counts by OKR · impact entries per week in range
- text: decisions / blockers / impact grouped by epic · epics with `done_on` in range + impact line
- "copy as markdown": same content rendered via markdown template into hidden textarea

Blockers = plain entries, no resolve state. Close by logging a follow-up line. Add toggle only if report noise proves it.

### Phase 2

**To-dos** — a to-do is a `>` line; epic optional, source link optional, due optional
- e.g. `> reply to vendor https://…slack.com/archives/… by fri` · `OPS-101 > profile join` · `> renew parking`
- **`/todos`** — all open, grouped Overdue · Today · This week · Later · No date; row = checkbox · text · link chip · epic chip · due (date picker, autosave) · Pin · Drop; "Done this week" toggle at the bottom
- Today keeps "This week" (pinned) + "Other open plans" toggle; overdue to-dos also appear in "Needs you"

**Planning** (on `/` and `/todos`)
- open plans list = all `plan_state='open'` any day → carry-over free, no row copying
- order: pinned first ("this week top 3"), then oldest
- actions: check → `done` + `closed_day=today` · drop → `dropped` + `closed_day` · pin/unpin
- no auto-unpin; pin cap 3 (4th pin rejected with message)
- report adds: plans with `day` in range → done / open / dropped counts + hit rate = done ÷ total

**Progress flags** (query only, no stored cols) — on `/epics` + side list on `/`
- `last_activity` = max `entries.day` per epic
- **stale**: `status='active'` + `last_activity` ≥ 7 days ago (or no entries)
- **at risk**: `status='active'` + `target` within 7 days + open plans on epic
- **overdue**: `status='active'` + `target` < today
- per-epic entries/week sparkline (last 8 weeks) on `/epics/{key}`
- blockers excluded from flags — no resolve state → flag would never clear

**Meeting mode**
- `/` shows "Start meeting" (name input, slug) or, when active, banner "In {name} · End"
- start → insert `meetings` row; end → set `ended_at`; unique partial index blocks 2nd active
- active meeting → every captured line gets `meeting_id`
- **`/meetings`** — list by day desc · filter `?name=` → e.g. all `oms-sync` across months
- **`/meetings/{id}`** — entries grouped by kind (decisions, plans = action items, learn) · notes textarea (save form)
- report adds: meetings count in range + decisions made in meetings

### Phase 3

**`/learn`** — inbox: distinct `entries.topic` with no `topics` row, + learn entries with empty topic · topic list: latest depth + last-rated day, stale (> 30 days since rating) highlighted.
**`/learn/{slug}`** — edit summary / own words / resources · rate 1–4 (appends row) · rating history · learn entries that raised it.
Report adds: heatmap topics × months (cell = latest depth as of month end, blank if none) · depth changes in range (first → last rating).

### Phase 4

**`/notes`** — flat list by `updated_at` desc · search `?q=` = `LIKE` on title + body (FTS5 later if slow).
**`/notes/{id}`** — Notion-style editor (decided 2026-11-11, option B): **Tiptap** (MIT, ProseMirror) on this page only
- type-to-format: `#` heading · `-` list · `[]` checkbox · `>` quote · `---` divider · `**bold**` · `` `code` ``
- `/` menu: heading, to-do list, toggle, callout, code block, divider, link
- `[[` → page picker (epics, topics, meetings) → inline chip; paste a URL → link
- storage stays **markdown** in `notes.body` (Tiptap markdown extension) → search, `[[X]]` backlinks and the LLM layer read it unchanged; server render for read-only views via pulldown-cmark (raw HTML off)
- autosave: debounced 1 s htmx POST of the markdown; "✓ saved" in the top bar
- not taken: drag-to-reorder blocks, columns, databases, embeds (spec out of scope)
- JS: one prebuilt `static/js/editor.js`, bundled once with `esbuild` from pinned npm versions and committed; not part of `cargo build`; other pages ship no editor JS
- `[[X]]` in body → link at render: epic key regex → `/epics/X` · topic slug exists → `/learn/X` · meeting name exists → `/meetings?name=X` · else plain text
- backlinks: epic / topic / meeting pages show "mentioned in notes" = `body LIKE '%[[X]]%'`, no link table
- agent notes (`author` ≠ `'me'`, e.g. `claude`, `codex`): badge + separate section on `/notes`; editable by me (edit flips nothing, stays `llm`)

### LLM layer (phase 4)

Borrowed from Karpathy's LLM Wiki pattern (LLM maintains a linked wiki from sources, guided by a schema file), with one inversion: **the LLM never writes my understanding.** Depth tracking only means something if `topics.summary`, `own_words` and `depth_ratings` are mine.

- LLM = a Claude Code session in this repo, not an API call from the binary → no key, no new dep, no code
- `CLAUDE.md` in repo = the schema file: tables, grammar, and the write rules below; LLM reads the DB with `sqlite3 data/atharu.db`
- write rules:

| LLM may | LLM must not |
|---|---|
| read everything | write `topics`, `depth_ratings` |
| write `entries`, `epics` **only through the `atharu` CLI**, tagged with its own name (see Agent access) | write any table with raw SQL |
| insert/update `notes` with `author=<its name>` | edit or delete `author='me'` rows |
| suggest `[[X]]` links in chat for my notes | rate depth or write "own words" |

Operations (each = a `.claude/commands/*.md` prompt, no app code):
- **query** — "what did I decide about X", "what did I ship for OKR Y" → answers from `entries` + `notes`, cites entry ids/days. Free once `CLAUDE.md` exists.
- **lint** — reports in chat, writes nothing:
  - learn entries with near-duplicate topic slugs (`vector-search` vs `vectorsearch`) → suggest merge
  - `[[X]]` in notes that resolves to nothing → typo or missing topic
  - topic `summary` contradicted by later learn entries/notes → "re-explain this in your own words"
  - learn entries with no topic, topics never rated → nudge, not fix
- **ingest** — I give a URL/file → LLM writes one agent-authored note (summary + `[[topic]]` links to existing slugs) and lists questions I should answer myself on `/learn/{slug}`. It never fills the topic page.

**Agent access (P4, added 2026-11-11 by user)** — so Claude, Codex or any coding agent can file work for me, e.g. I paste an epic link and it does the rest
- CLI subcommands on the same binary, same parser + queries as the web (one validation path), no server needed, no new crate:
  - `atharu log "<line>" [--day YYYY-MM-DD]` → same grammar as the capture bar; prints the saved row as one JSON line
  - `atharu epic <KEY|jira-url> [--title …] [--okr …]` → upsert epic (key from `…/browse/ABC-123`); never overwrites a field I set
  - `atharu today [--day]` · `atharu show <KEY>` → JSON on stdout for reading
  - `atharu note "<title>" [--file|stdin] --as <agent>` (agent-authored note; `--id` updates only its own) · `atharu evidence <KEY> <url> --as <agent>` (http(s) only) — added 2026-11-12 so ingest/log can save
  - core set only (2026-11-11): `log` already covers to-dos (`>`), learn (`?`) and every line kind via the grammar. Not in the CLI yet: evidence, to-do done/pin/drop, meetings, topics, notes, events, report. An agent asked for one of those says "atharu has no command for that yet" and suggests the command; it never falls back to raw SQL writes. Add a command when an instruction actually needs it.
  - errors → exit 1 + the same copy as the States board on stderr
- every write needs `--as <agent>` (or env `ATHARU_AGENT`), lowercase slug (`claude`, `codex`, …); missing → exit 2 "Say who is writing: --as claude". `me` is refused from the CLI's agent flag so an agent can't pose as me.
- migration 002: `author TEXT NOT NULL DEFAULT 'me'` on `entries`, `epics`, `evidence`; `notes.author` CHECK widened to `'me'` or a slug (rebuild the empty table)
- UI: small badge with the agent name on its rows (Today, epic timeline, evidence); report note marks agent-filed lines `(via claude)` so the copy stays honest
- `CLAUDE.md` + `AGENTS.md` (same content, for Codex) document the commands; `.claude/commands/log.md` = "paste anything → turn it into `atharu log/epic/evidence` calls, show what was saved"

Deterministic checks (stale topic, unrated topic, dead `[[X]]`) stay in Rust on `/learn` + `/notes`; the LLM only does what needs judgment (duplicates, contradictions).

### Phase 5 — calendar

**`/calendar`** — week view (default) + month toggle; today highlighted. Shows, from data we already have:
- to-dos by `due` · epic `target` dates · meetings (by day) · days with log entries (dot) · `events` rows
- click a day → Today for that day (backfill) · click a meeting/event → start meeting mode with that name
- own events: "+ Event" (title, date, optional time) → `events` with `source='own'`

**Google read-only (P5b)** — the calendar's secret iCal address, fetched every 15 min, upsert by `ext_id`, never written back
- URL lives in local config (`data/config.toml`, gitignored), never in the repo; it is a credential
- check first: Google Calendar → Settings → your calendar → "Integrate calendar" → "Secret address in iCal format". Workspace admins often disable it → then Google API (OAuth) or skip; decide then
- crates: `reqwest` + `icalendar` (P5b only)

**Jira read-only titles (P6, added 2026-11-11 by user)** — fill in what a Jira key means, nothing more
- keys we already know: `epics.key` + keys in Jira links (`…/browse/ABC-123`) in `entries.link` (to-dos are entries)
- one batched `GET {base}/rest/api/3/search/jql?jql=key in (…)&fields=summary,status` (Basic auth email:API token), on start + every 15 min + the same "Refresh" button; cache table `jira_issues(key PK, summary, status, fetched_at)` (migration 003), upsert by key; unknown/forbidden keys just stay uncached
- shown as display only: epic list/page shows `summary · status` under the key when our `title` is empty ("No title yet" stays editable); Jira link chips read `ABC-123 · summary`
- config `data/config.toml`: `jira_base_url`, `jira_email`, `jira_token` (all optional; any missing → feature off, no callout noise); token is a credential, never logged or in git
- never writes to Jira; never creates epics/todos from Jira; tests use a fixture JSON, no network

**Delete + edit (added 2026-11-12)** — notes, meetings and topics can be deleted with the same Undo toast as log lines; an epic can be deleted only while it has no lines (else "Mark it Done or Dropped instead."); a log line can be edited inline and is re-parsed with the capture grammar (id, time and author kept). Deleting a meeting keeps its lines; deleting a topic keeps its `?` lines (back to the Learn inbox). Agents can't delete or edit.

**Looks (added 2026-11-12)** — `/settings` picks one look: Sumi (washi, default) · Sora (blue) · Wakaba (green) · Glass. It sets a soft mesh wallpaper behind every page, so the floating glass chrome (backdrop blur) shows it, the favicon, and in the Mac app the Dock icon (Option I, docs/design/icon/options.html) plus window vibrancy. Stored in a `settings` table, not config.toml (no credential). Reduced transparency → plain background.

## UX decisions (2026-11-11, from the canvas)

1. Sidebar = page list + active epics (meeting series / topics / notes on their pages); breadcrumbs on top
2. After a line is logged: "Saved as impact on OPS-101 · Undo" (undo = delete)
3. Epic key autocomplete = native `<datalist>` of known keys
4. Today page order: meeting banner · this-week to-dos (pinned plans) · open plans (toggle) · "Needs you" flags · log grouped by epic · floating "Type a line…" bar
5. Untitled keys get an inline title box on `/epics`
6. Epic / topic / meeting = page with big title + inline properties, autosaved on change
7. `/report` shows exactly what Copy gives; charts in a toggle, not copied
8. Day nav `← prev · next →` + date picker for backfill
9. Notes: Tiptap editor (type-to-format, `/` menu, `[[` picker), markdown storage; agent notes in their own sidebar group
10. Sidebar order: Today · To-dos · Calendar · Epics · Report · Meetings · Learn · Notes

## Error handling

- DB open/migrate fail → exit non-zero with message (`anyhow` in main only)
- form validation errors → re-render page with message, input kept
- handler DB error → 500 + log line; no retries (local single user)
- 2nd active meeting (unique index violation) → message "end {name} first"

## Testing

All `cargo test`; DB tests use in-memory SQLite (`Connection::open_in_memory`).
- `parse.rs` tests: table-driven — each prefix incl `>`, no key, key + no prefix, `?` with/without topic, `xml` not blocker, empty, prefix-only, link anywhere, `by fri` / `by 20 Nov` / bad date
- `report.rs` tests: in-memory SQLite — date-range filter, done-in-range, plan hit rate (done/open/dropped), depth "as of month end"
- `flags.rs` tests (phase 2): stale / at risk / overdue against fixed "today"
- `links.rs` tests (phase 4): `[[X]]` resolution order + unknown → plain text
- no UI tests
- LLM layer: no automated tests (prompts, not code); check by running each command once on real data

## Out of scope

auth · Jira sync (beyond P6 read-only titles) · blocker resolve · notifications · multi-device sync · charts lib · calendar write-back / Google API OAuth (P5b is read-only iCal) · task priority / subtasks · recurring meeting templates · time tracking · attendee tracking · Notion-style blocks/nesting/DB views · LLM API calls from the binary · embeddings/RAG · LLM writing topics/depth/entries · markdown wiki files (SQLite stays the single source).
Add when: Jira sync → typing titles annoys; sync → second machine; Google API → iCal address disabled by admin and calendar is missed.

## Layout

Vertical slices + pure core (decided 2026-11-11, option 4 of: full Clean Architecture · hexagonal-lite · layered · vertical slices).
Rules: `domain/` imports nothing from features · features never import each other (shared → `domain/`) · SQL only in a feature's `queries.rs` · HTML only in `templates/` · each htmx action returns one partial · folders arrive with their phase · `cargo fmt` + `cargo clippy -D warnings` + `cargo test` before every commit.
Not used: workspace / multiple crates, ORM, repository traits with one impl, JS build.

```
personal-tracker/
  Cargo.toml
  migrations/001_init.sql
  src/
    main.rs  app.rs (router + state)  config.rs  error.rs (AppError → response)  db.rs (open + migrate only)
    domain/            pure, unit-tested: entry.rs · parse.rs · dates.rs (`by fri`) · links.rs (P4) · flags.rs (P2) · depth.rs (P3)
    today/             mod.rs (routes) · handlers.rs · queries.rs (SQL) · view.rs (askama structs)
    epics/  report/    same 4 files
    todos/  meetings/  (P2)   learn/ (P3)   notes/ (P4)   calendar/ (P5)
  templates/
    base.html          sidebar + breadcrumb + page sheet
    <feature>/         one folder per feature: pages
    partials/          entry_row · plan_row · link_chip · flag · toast · property
  static/
    css/ tokens.css · base.css · layout.css · components.css
    js/  htmx.min.js (vendored, pinned) · copy.js
  tests/               HTTP tests per feature, in-memory SQLite
  CLAUDE.md            schema + LLM write rules (P4)
  .claude/commands/{query,lint,ingest}.md   (P4)
  data/                gitignored: atharu.db, config.toml
```
