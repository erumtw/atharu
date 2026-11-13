#!/bin/bash
# Visual check on real data (T4b): run before every release.sh. Copies data/atharu.db with sqlite3 .backup into a
# temp dir (the live file is only read), serves the copy on a free port, and runs scripts/visual-check.mjs over
# every main page at 1180 / 900 / 390 px. Exit 1 = layout problems, one line each. Screenshots stay in the temp
# dir it prints: they hold work data, never commit them.
#   scripts/visual-check.sh [path/to/atharu.db]
set -euo pipefail
cd "$(dirname "$0")/.."
SRC=${1:-data/atharu.db}
[ -f "$SRC" ] || { echo "No database at $SRC" >&2; exit 2; }
TMP=$(mktemp -d "${TMPDIR:-/tmp}/atharu-visual-check.XXXXXX")
DB="$TMP/atharu.db"
sqlite3 "$SRC" ".backup '$DB'"
mkdir -p "$TMP/shots"

export PATH=/opt/homebrew/opt/rustup/bin:$PATH
cargo build -q
PORT=$(python3 -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])')
ATHARU_HOST=127.0.0.1 ATHARU_PORT=$PORT target/debug/atharu --db "$DB" >"$TMP/server.log" 2>&1 &
SERVER=$!
trap 'kill $SERVER 2>/dev/null || true' EXIT
for _ in $(seq 40); do curl -sf "http://127.0.0.1:$PORT/settings" >/dev/null && break; sleep 0.25; done

# One of each entity page, read from the copy; a kind with no rows is skipped.
q() { sqlite3 "$DB" "$1"; }
enc() { python3 -c 'import sys, urllib.parse; print(urllib.parse.quote(sys.argv[1]))' "$1"; }
PAGES=(/ /todos "/todos?group=tag" /calendar "/calendar?view=day" "/calendar?view=week" "/calendar?view=month"
  /epics /meetings /notes "/notes?by=mine" "/notes?by=agent" /learn /tags /report /settings)
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

node scripts/visual-check.mjs "http://127.0.0.1:$PORT" "$TMP/shots" "${PAGES[@]}"
