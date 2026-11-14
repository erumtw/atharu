#!/bin/bash
# Visual check on real data (T4b): run before every release.sh. Copies data/atharu.db with sqlite3 .backup into a
# temp dir (the live file is only read), serves the copy on a free port, and runs scripts/visual-check.mjs over
# every main page at 1180 / 900 / 390 px. Exit 1 = layout problems, one line each. Screenshots stay in the temp
# dir it prints: they hold work data, never commit them.
#   scripts/visual-check.sh [path/to/atharu.db]      (no argument: data/atharu.db, the real data)
#   scripts/visual-check.sh --db <file>              a seeded demo / long-data DB (scripts/long-seed.sh) instead; its copy also gets a
#                                                    throwaway Jira config pointing at a closed local port, so Jira chips render
set -euo pipefail
cd "$(dirname "$0")/.."
# Raw colours outside tokens.css and the per-look blocks fail the run before anything is served (0.2.9, D3).
scripts/color-lint.sh
FAKE_JIRA=0
if [ "${1:-}" = --db ]; then
  [ -n "${2:-}" ] || { echo "usage: scripts/visual-check.sh --db <file>" >&2; exit 2; }
  SRC=$2 FAKE_JIRA=1
else
  SRC=${1:-data/atharu.db}
fi
[ -f "$SRC" ] || { echo "No database at $SRC" >&2; exit 2; }
TMP=$(mktemp -d "${TMPDIR:-/tmp}/atharu-visual-check.XXXXXX")
DB="$TMP/atharu.db"
sqlite3 "$SRC" ".backup '$DB'"
# Chips only render with Jira configured (base URL + email + token); port 9 is closed, so nothing is ever contacted.
if [ "$FAKE_JIRA" = 1 ]; then
  printf 'jira_base_url = "http://127.0.0.1:9"\njira_email = "demo@example.invalid"\njira_token = "demo"\n' >"$TMP/config.toml"
fi
mkdir -p "$TMP/shots"

export PATH=/opt/homebrew/opt/rustup/bin:$PATH
cargo build -q
PORT=$(python3 -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])')
ATHARU_HOST=127.0.0.1 ATHARU_PORT=$PORT target/debug/atharu --db "$DB" >"$TMP/server.log" 2>&1 &
SERVER=$!
# The copy holds work data: it goes with the server; shots/ stays for the person reading the report.
trap 'kill $SERVER 2>/dev/null || true; wait $SERVER 2>/dev/null || true; rm -f "$DB" "$DB"-wal "$DB"-shm' EXIT
for _ in $(seq 40); do curl -sf "http://127.0.0.1:$PORT/settings" >/dev/null && break; sleep 0.25; done

# One of each entity page, read from the copy; a kind with no rows is skipped.
q() { sqlite3 "$DB" "$1"; }
enc() { python3 -c 'import sys, urllib.parse; print(urllib.parse.quote(sys.argv[1]))' "$1"; }
PAGES=(/ /todos "/todos?group=tag" /calendar "/calendar?view=day" "/calendar?view=week" "/calendar?view=month"
  /epics "/epics?group=status" "/epics?group=parent" "/epics?group=tag" /quick /meetings /notes "/notes?by=mine" "/notes?by=agent" /learn /tags /report /settings)
EPIC=$(q "SELECT key FROM epics WHERE status = 'active' ORDER BY key LIMIT 1")
[ -n "$EPIC" ] && PAGES+=("/epics/$EPIC")
MEETING=$(q "SELECT id FROM meetings ORDER BY id DESC LIMIT 1")
[ -n "$MEETING" ] && PAGES+=("/meetings/$MEETING")
SERIES=$(q "SELECT name FROM meetings ORDER BY id DESC LIMIT 1")
[ -n "$SERIES" ] && PAGES+=("/meetings?name=$(enc "$SERIES")")
NOTE=$(q "SELECT id FROM notes ORDER BY updated_at DESC LIMIT 1")
[ -n "$NOTE" ] && PAGES+=("/notes/$NOTE")
TOPIC=$(q "SELECT slug FROM topics ORDER BY slug LIMIT 1")
[ -n "$TOPIC" ] && PAGES+=("/learn/$(enc "$TOPIC")")
TAG=$(q "SELECT tag FROM taggings GROUP BY tag ORDER BY COUNT(*) DESC, tag LIMIT 1")
[ -n "$TAG" ] && PAGES+=("/tags/$(enc "$TAG")")
# A tag a meeting series carries: its hub lists the series by name (long names live there).
STAG=$(q "SELECT tag FROM taggings WHERE item_type = 'series' ORDER BY tag LIMIT 1")
[ -n "$STAG" ] && PAGES+=("/tags/$(enc "$STAG")")

node scripts/visual-check.mjs "http://127.0.0.1:$PORT" "$TMP/shots" "${PAGES[@]}"
