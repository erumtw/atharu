#!/bin/bash
# The web editor must carry what the read view renders (0.2.10: opening a note with a markdown table and typing one character deleted
# the table). Types one character into a note in headless Chrome and asserts the stored markdown changed by that character only.
#   scripts/editor-roundtrip.sh                                  a fresh fixture note: table (agent style), bare URLs, image, drawing, callout
#   RT_DB=data/atharu.db RT_NOTES="14 8" scripts/editor-roundtrip.sh   those notes of a .backup COPY of a real DB (the source is only read);
#                                                                     compares what the read view shows, not the bytes (RT_MODE=render)
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH=/opt/homebrew/opt/rustup/bin:$PATH
cargo build -q
TMP=$(mktemp -d "${TMPDIR:-/tmp}/atharu-roundtrip.XXXXXX")
DB="$TMP/atharu.db"
if [ -n "${RT_DB:-}" ]; then
  sqlite3 "$RT_DB" ".backup '$DB'"
  IDS=${RT_NOTES:?set RT_NOTES to the note ids to open}
  export RT_MODE=render
else
  cat >"$TMP/fixture.md" <<'MD'
Intro with a bare link https://example.com/a/b?c=1 and `code` and **bold** and [[OPS-101]].

### Findings

| PR | Status |
|---|---|
| BE https://example.com/pull/47 | waiting for review (second round) |
| **FE** | `approved`, merge pending |

![Architecture](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==)

![Drawing](data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciLz4=)

- first point
- second point
MD
  target/debug/atharu --db "$DB" note "Round trip fixture" --file "$TMP/fixture.md" --as claude >/dev/null
  IDS=1
fi
PORT=$(python3 -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])')
ATHARU_HOST=127.0.0.1 ATHARU_PORT=$PORT target/debug/atharu --db "$DB" >"$TMP/server.log" 2>&1 &
SERVER=$!
trap 'kill $SERVER 2>/dev/null || true; wait $SERVER 2>/dev/null || true; rm -rf "$TMP"' EXIT
for _ in $(seq 40); do curl -sf "http://127.0.0.1:$PORT/settings" >/dev/null && break; sleep 0.25; done
# shellcheck disable=SC2086
node scripts/editor-roundtrip.mjs "http://127.0.0.1:$PORT" "$DB" $IDS
