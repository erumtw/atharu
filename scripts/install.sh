#!/usr/bin/env bash
# Installs or updates atharu from this checkout (D28). Safe to re-run: every step replaces, never appends.
#   scripts/install.sh            CLI + server binary → ~/.cargo/bin/atharu
#   scripts/install.sh --app      also the Mac app → /Applications/Atharu.app (its data dir is untouched)
#   scripts/install.sh --docker   also rebuilds and restarts the compose service
# Never edits shell rc files: it prints the lines to add instead.
set -euo pipefail
# All output is for humans; stdout stays free for `atharu update`'s JSON.
exec 1>&2

app=0 docker=0
for a in "$@"; do
  case $a in
    --app) app=1 ;;
    --docker) docker=1 ;;
    -h | --help) sed -n '2,6p' "$0"; exit 0 ;;
    *) echo "unknown option $a (use --app, --docker)"; exit 2 ;;
  esac
done

repo=$(cd "$(dirname "$0")/.." && pwd)
cd "$repo"

# Homebrew's rustup is keg-only: cargo is often not on PATH yet.
if ! command -v cargo >/dev/null; then
  if [ -x /opt/homebrew/opt/rustup/bin/cargo ]; then
    export PATH=/opt/homebrew/opt/rustup/bin:$PATH
  else
    echo "cargo not found: brew install rustup && rustup default stable"
    exit 1
  fi
fi

bin=${CARGO_INSTALL_ROOT:-${CARGO_HOME:-$HOME/.cargo}}/bin
# A binary from before `version` existed exits non-zero: that reads as "none".
old=$("$bin/atharu" version 2>/dev/null || echo none)
echo "building atharu from $repo …"
cargo install --path . --locked --quiet
new=$("$bin/atharu" version)
echo "atharu: $old → $new"

if [ "$app" = 1 ]; then
  if ! cargo tauri --version >/dev/null 2>&1; then
    echo 'tauri-cli missing: cargo install tauri-cli --version "^2" --locked'
    exit 1
  fi
  (cd src-tauri && cargo tauri build --bundles app)
  built=$repo/target/release/bundle/macos/Atharu.app
  dest=/Applications/Atharu.app
  # "if running" first: a bare `quit app` launches the app just to quit it.
  osascript -e 'if application "Atharu" is running then tell application "Atharu" to quit'
  for _ in $(seq 20); do
    [ "$(osascript -e 'application "Atharu" is running')" = true ] || break
    sleep 0.5
  done
  # Copy beside the old app, swap only once the copy is whole: a failed copy leaves the old one working.
  rm -rf "$dest.new"
  ditto "$built" "$dest.new"
  if [ -e "$dest" ]; then mv "$dest" "$dest.old"; fi
  mv "$dest.new" "$dest"
  rm -rf "$dest.old"
  echo "Mac app replaced: $dest (open it again; data stays in ~/Library/Application Support/dev.atharu.desktop)"
fi

if [ "$docker" = 1 ]; then
  docker compose up -d --build
fi

case ":$PATH:" in
  *":$bin:"*) ;;
  *) echo "add to ~/.zshrc: export PATH=\"$bin:\$PATH\"" ;;
esac
# The installed binary resolves data/atharu.db against the current dir; pin it to this checkout's DB.
if [ -z "${ATHARU_DB:-}" ]; then
  echo "add to ~/.zshrc: export ATHARU_DB=\"$repo/data/atharu.db\""
fi
