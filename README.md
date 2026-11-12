# atharu (أثر)

Personal work log on localhost: one line per thing, epics, report. Spec: `docs/superpowers/specs/2026-09-28-atharu-design.md`.

## Run

```sh
export PATH=/opt/homebrew/opt/rustup/bin:$PATH   # rustup is keg-only on this Mac
cargo run                # serves http://127.0.0.1:7070, creates + migrates data/atharu.db
cargo run -- dump        # writes data/dump.sql (plain SQL backup)
cargo run -- --db x.db   # another DB file
cargo run -q -- log "OPS-101 shipped it" --as claude   # agent CLI: log · epic · today · show (see CLAUDE.md)
```

Docker: `docker compose up -d --build` → http://localhost:7070 (published on 127.0.0.1 only; DB in `./data`), `docker compose down` to stop.

Mac app (Tauri 2, system WebView): `cargo install tauri-cli --version "^2" --locked` once, then `cd src-tauri && cargo tauri build` → unsigned `target/release/bundle/macos/Atharu.app` + `bundle/dmg/*.dmg`. It serves the same pages on a free 127.0.0.1 port, keeps its DB in `~/Library/Application Support/dev.atharu.desktop/` (`ATHARU_DB` still wins), sits in the menu bar, and **Ctrl+Opt+Space** (`CAPTURE` in `src-tauri/src/lib.rs`) opens it on the capture bar. The window has no title bar: the traffic lights sit in the sidebar and the brand row / breadcrumb drag the window. The Dock icon follows the look picked in /settings, and the look is also written onto the .app itself (unsigned builds only) so it stays after Quit.

Builds before T27 used the bundle id `dev.atharu.app`; to keep that data, quit the app and `mv ~/Library/Application\ Support/dev.atharu.app ~/Library/Application\ Support/dev.atharu.desktop` before the first start of the new build.

Env: `ATHARU_HOST` (127.0.0.1) · `ATHARU_PORT` (7070) · `ATHARU_DB` (data/atharu.db) · `ATHARU_AGENT` (CLI writer, e.g. claude).

Optional `data/config.toml` (next to the DB, read at start): `ical_url = "…"` = Google Calendar's secret iCal address, shown read-only on `/calendar` and refreshed every 15 min. It is a credential: keep it out of git and logs.

Frontend sources are strict TypeScript in `editor/src/*.ts`; `cd editor && npm ci && npm run check && node build.mjs`
type-checks them and rebuilds the committed `static/js/{editor,copy}.js` (browsers load the .js; `htmx.min.js` is vendored).
`data/` is gitignored: it holds work data, never commit it.

## Check

```sh
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```
