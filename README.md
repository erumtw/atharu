# worklog

Personal work log on localhost: one line per thing, epics, report. Spec: `docs/superpowers/specs/2026-09-28-worklog-design.md`.

## Run

```sh
export PATH=/opt/homebrew/opt/rustup/bin:$PATH   # rustup is keg-only on this Mac
cargo run                # serves http://127.0.0.1:7070, creates + migrates data/worklog.db
cargo run -- dump        # writes data/dump.sql (plain SQL backup)
cargo run -- --db x.db   # another DB file
```

Docker: `docker compose up -d --build` → http://localhost:7070 (published on 127.0.0.1 only; DB in `./data`), `docker compose down` to stop.

Env: `WORKLOG_HOST` (127.0.0.1) · `WORKLOG_PORT` (7070) · `WORKLOG_DB` (data/worklog.db).
`data/` is gitignored: it holds work data, never commit it.

## Check

```sh
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```
