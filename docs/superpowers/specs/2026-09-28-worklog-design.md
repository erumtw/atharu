# Worklog — design

2026-09-28 · status: approved in chat, pending spec review

## Goal

Personal work log → prove impact at OKR/perf review + track real understanding vs AI-surface knowledge.
Four jobs: ownership · progress · impact evidence · deep learning.

Success:
- capture ≤ 2 min/day, one line per thing
- paste-ready weekly note / self-review / OKR draft from one page, no hand-writing summaries
- depth history per topic shows growth over time

Rule: never store what Jira already knows. Store decisions, impact, learning only.

## Assumptions

- single user, localhost, no auth, no hosting
- Jira = key + link only (no API) in v1
- SQLite = single source of truth (no markdown files)

## Architecture

- one Go binary `worklog`, serves `http://localhost:7070`
- stdlib `net/http` + `html/template`, templates via `embed`
- only dep: `modernc.org/sqlite` (pure Go, no cgo)
- server-rendered pages, plain `<form>` POSTs; JS only for copy-to-clipboard
- heatmap = CSS grid, no chart lib
- DB path: `./data/worklog.db` (flag `-db` overrides), created + migrated on start
- `data/` gitignored — DB holds Freshket work data (epic keys, metrics); never pushed

Subcommands:
- `worklog` → serve
- `worklog dump` → `sqlite3`-style SQL dump to `data/dump.sql` (backup; stays local unless user opts in)

## Data model

```sql
CREATE TABLE entries (
  id         INTEGER PRIMARY KEY,
  day        TEXT NOT NULL,                 -- YYYY-MM-DD
  epic_key   TEXT REFERENCES epics(key),
  kind       TEXT NOT NULL CHECK (kind IN ('done','decision','blocker','impact','learn')),
  topic      TEXT,                          -- set only for kind='learn' with "slug:"
  text       TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX entries_day  ON entries(day);
CREATE INDEX entries_epic ON entries(epic_key);

CREATE TABLE epics (
  key    TEXT PRIMARY KEY,                  -- e.g. FK-123
  title  TEXT NOT NULL DEFAULT '',
  role   TEXT NOT NULL DEFAULT 'owner' CHECK (role IN ('owner','contributor')),
  okr    TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','done','dropped')),
  start  TEXT, target TEXT, done_on TEXT,
  metric TEXT NOT NULL DEFAULT '', before TEXT NOT NULL DEFAULT '', after TEXT NOT NULL DEFAULT ''
);

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
```

Depth: 1 heard of · 2 can explain · 3 can build w/o AI · 4 can teach.
Migration: single `schema.sql` embedded, `CREATE TABLE IF NOT EXISTS`, `PRAGMA foreign_keys=ON`. No migration framework until schema changes.

## Line grammar (parser)

`[EPIC-KEY] [prefix] text` — pure func `Parse(line string) (Entry, error)`.

| prefix | kind |
|---|---|
| none | done |
| `!` | decision |
| `x` | blocker |
| `+` | impact |
| `?` | learn |

- key = first token matching `^[A-Z][A-Z0-9]*-\d+$`; optional, must be first token
- prefix = next token if exactly one of `! x + ?`
- `?` with `slug: rest` → topic = slug (lowercased, `[a-z0-9-]+`), text = rest; no colon → topic empty
- empty/whitespace-only or empty text after prefix → error, line stays in input box
- unknown key → stub epic row inserted (title empty) → listed as "unfiled"
- `x` only a prefix when followed by space (so `xml parser fixed` = done)

## Pages

**`/` Today** — autofocus input, Enter submits, date field (default today) for backfill · today's entries grouped by epic · delete per entry (no edit; delete + retype) · parse error → inline, input kept.

**`/epics`** — grouped by OKR · status + target · unfiled stubs on top.
**`/epics/{key}`** — edit form (title, role, OKR, status, dates, metric, before, after) · add/delete evidence links · entry timeline for key · impact line `Did {title} → {metric} {before}→{after} → {evidence urls}` + copy button.

**`/learn`** — inbox: distinct `entries.topic` with no `topics` row, + learn entries with empty topic · topic list: latest depth + last-rated day, stale (> 30 days since rating) highlighted.
**`/learn/{slug}`** — edit summary / own words / resources · rate 1–4 (appends row) · rating history · learn entries that raised it.

**`/report?from=&to=`** — presets: this week (Mon start) / month / quarter · default = this week.
- visual: epic status counts by OKR · impact entries per week in range · heatmap topics × months (cell = latest depth as of month end, blank if none yet)
- text: decisions / blockers / impact grouped by epic · epics with `done_on` in range + impact line · depth changes in range (first → last rating)
- "copy as markdown": same content rendered via markdown template into hidden textarea

Blockers = plain entries, no resolve state. Close by logging a follow-up line. Add toggle only if report noise proves it.

## Error handling

- DB open/migrate fail → exit non-zero with message
- form validation errors → re-render page with message, input kept
- handler DB error → 500 + log line; no retries (local single user)

## Testing

- `parse_test.go`: table-driven — each prefix, no key, key + no prefix, `?` with/without topic, `xml` not blocker, empty, prefix-only
- `report_test.go`: in-memory SQLite — date-range filter, depth "as of month end", done-in-range
- no UI tests

## Out of scope (v1)

auth · Jira sync · entry edit · blocker resolve · notifications · multi-device sync · charts lib.
Add when: Jira sync → typing titles gets annoying; edit → delete+retype annoys; sync → second machine.

## Layout

```
personal-tracker/
  main.go          serve + dump subcommand, routes
  db.go            open, migrate, queries
  parse.go         line grammar
  report.go        report queries
  schema.sql
  templates/*.html
  static/app.css
  *_test.go
  data/            gitignored
```
