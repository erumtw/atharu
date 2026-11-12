# atharu (أثر)

Personal work log on localhost: one line per thing, epics, report. Spec: `docs/superpowers/specs/2026-09-28-atharu-design.md`.

## Run

```sh
export PATH=/opt/homebrew/opt/rustup/bin:$PATH   # rustup is keg-only on this Mac
cargo run                # serves http://127.0.0.1:7070, creates + migrates data/atharu.db
cargo run -- dump        # writes data/dump.sql (plain SQL backup)
cargo run -- --db x.db   # another DB file
```

Docker: `docker compose up -d --build` → http://localhost:7070 (published on 127.0.0.1 only; DB in `./data`), `docker compose down` to stop.

Env: `ATHARU_HOST` (127.0.0.1) · `ATHARU_PORT` (7070) · `ATHARU_DB` (data/atharu.db).
`data/` is gitignored: it holds work data, never commit it.

## Check

```sh
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```
