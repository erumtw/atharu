# Worklog — design

2026-09-28 · status: approved in chat, pending spec review

## Goal

Personal work log → prove impact at OKR/perf review + track real understanding vs AI-surface knowledge.
Jobs: ownership · progress · planning · meetings · impact evidence · deep learning · notes.

Success:
- capture ≤ 2 min/day, one line per thing
- paste-ready weekly note / self-review / OKR draft from one page, no hand-writing summaries
- plan hit rate + depth history show growth over time

Rule: never store what Jira already knows. Store plans, decisions, impact, learning only.

## Assumptions

- single user, localhost, no auth, no hosting
- Jira = key + link only (no API)
- SQLite = single source of truth (no markdown files)
- not a Notion clone: no block editor, nesting, DB views, drag-drop

## Phases

Each phase shippable + used daily before next starts.

| Phase | Scope | Why this order |
|---|---|---|
| 1 | capture + grammar · epics · report | core loop, usable day 1 |
| 2 | planning `>` · progress flags · meeting mode | builds on `entries` |
| 3 | learn pages + depth heatmap | needs weeks of `?` data first |
| 4 | notes + `[[links]]` | only if Obsidian/Notion not enough |

Phase 1 grammar already parses all prefixes (`>` and `?` saved as entries); their pages arrive in later phases.

## Architecture

- one Go binary `worklog`, serves `http://localhost:7070`
- stdlib `net/http` + `html/template`, templates via `embed`
- deps: `modernc.org/sqlite` (pure Go, no cgo) · `github.com/yuin/goldmark` (phase 4 only, notes markdown)
- server-rendered pages, plain `<form>` POSTs; JS only for copy-to-clipboard
- heatmap / sparkline = CSS grid, no chart lib
- DB path: `./data/worklog.db` (flag `-db` overrides), created + migrated on start
- `data/` gitignored — DB holds work data (epic keys, metrics); never pushed

Subcommands:
- `worklog` → serve
- `worklog dump` → SQL dump to `data/dump.sql` (backup; stays local unless user opts in)

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
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  CHECK ((kind = 'plan') = (plan_state IS NOT NULL))
);
CREATE INDEX entries_day     ON entries(day);
CREATE INDEX entries_epic    ON entries(epic_key);
CREATE INDEX entries_meeting ON entries(meeting_id);

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

CREATE TABLE notes (
  id         INTEGER PRIMARY KEY,
  title      TEXT NOT NULL,
  body       TEXT NOT NULL DEFAULT '',      -- markdown
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

Depth: 1 heard of · 2 can explain · 3 can build w/o AI · 4 can teach.
All `CREATE ... IF NOT EXISTS`, `PRAGMA foreign_keys=ON`.
No attendee field on meetings — privacy (no personal data) + YAGNI. Team names go in notes/text if needed.

## Line grammar (parser) — phase 1

`[EPIC-KEY] [prefix] text` — pure func `Parse(line string) (Entry, error)`.

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

**Planning** (on `/`)
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
**`/notes/{id}`** — textarea + rendered preview (goldmark, raw HTML disabled).
- `[[X]]` in body → link at render: epic key regex → `/epics/X` · topic slug exists → `/learn/X` · meeting name exists → `/meetings?name=X` · else plain text
- backlinks: epic / topic / meeting pages show "mentioned in notes" = `body LIKE '%[[X]]%'`, no link table

## Error handling

- DB open/migrate fail → exit non-zero with message
- form validation errors → re-render page with message, input kept
- handler DB error → 500 + log line; no retries (local single user)
- 2nd active meeting (unique index violation) → message "end {name} first"

## Testing

- `parse_test.go`: table-driven — each prefix incl `>`, no key, key + no prefix, `?` with/without topic, `xml` not blocker, empty, prefix-only
- `report_test.go`: in-memory SQLite — date-range filter, done-in-range, plan hit rate (done/open/dropped), depth "as of month end"
- `flags_test.go` (phase 2): stale / at risk / overdue against fixed "today"
- `links_test.go` (phase 4): `[[X]]` resolution order + unknown → plain text
- no UI tests

## Out of scope

auth · Jira sync · entry edit · blocker resolve · notifications · multi-device sync · charts lib · calendar sync · recurring meeting templates · time tracking · attendee tracking · Notion-style blocks/nesting/DB views.
Add when: Jira sync → typing titles annoys; edit → delete+retype annoys; sync → second machine; calendar → meeting start typing annoys.

## Layout

```
personal-tracker/
  main.go          serve + dump subcommand, routes
  db.go            open, migrate, queries
  parse.go         line grammar
  report.go        report queries
  flags.go         progress flags (phase 2)
  links.go         [[X]] resolve (phase 4)
  schema.sql
  templates/*.html
  static/app.css
  *_test.go
  data/            gitignored
```
