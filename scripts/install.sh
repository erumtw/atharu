#!/usr/bin/env bash
# Installs or updates atharu from a release tag (D40), built from source in a temp worktree: never
# the working tree, which agents edit mid-task. Safe to re-run.
#   scripts/install.sh                   newest release tag vX.Y.Z → ~/.cargo/bin/atharu
#   scripts/install.sh --version vX.Y.Z  that tag instead (pre-releases only this way)
#   scripts/install.sh --dev             the last commit instead (uncommitted edits are left out)
#   --app also replaces /Applications/Atharu.app (its data dir is untouched), --docker rebuilds the container
# Never edits shell rc files: it prints the lines to add instead.
set -euo pipefail
# All output is for humans; stdout stays free for `atharu update`'s JSON.
exec 1>&2

usage="usage: scripts/install.sh [--version vX.Y.Z | --dev] [--app] [--docker]"
app=0 docker=0 dev=0 want=
while [ $# -gt 0 ]; do
  case $1 in
    --app) app=1 ;;
    --docker) docker=1 ;;
    --dev) dev=1 ;;
    --version)
      [ $# -ge 2 ] || { echo "$usage"; exit 2; }
      want=v${2#v}
      shift
      ;;
    -h | --help) sed -n '2,8p' "$0"; exit 0 ;;
    *) echo "unknown option $1"; echo "$usage"; exit 2 ;;
  esac
  shift
done
if [ "$dev" = 1 ] && [ -n "$want" ]; then
  echo "--dev and --version don't mix"; echo "$usage"
  exit 2
fi

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

if [ "$dev" = 1 ]; then
  ref=HEAD
  label="dev build of $(git rev-parse --short HEAD)"
  if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
    echo "note: uncommitted changes are not included"
  fi
else
  git fetch --tags --quiet 2>/dev/null || echo "warning: can't fetch tags (offline?); using local tags"
  if [ -n "$want" ]; then
    if ! git rev-parse -q --verify "refs/tags/$want^{commit}" >/dev/null; then
      echo "no tag $want in $repo (git tag --list 'v*' lists the releases)"
      exit 1
    fi
    ref=$want
  else
    # Pre-releases (vX.Y.Z-rc1) are opt-in: only --version installs one.
    ref=$(git tag --list 'v[0-9]*' --sort=-v:refname | grep -v -- - | head -n 1 || true)
    if [ -z "$ref" ]; then
      echo "no release tags in $repo yet: scripts/release.sh X.Y.Z cuts one; --dev builds the last commit"
      exit 1
    fi
  fi
  label=$ref
fi

# A detached worktree, not `git archive`: build.rs reads the sha and tag from git.
tmp=$(mktemp -d "${TMPDIR:-/tmp}/atharu-install.XXXXXX")
src=$tmp/src
trap 'git -C "$repo" worktree remove --force "$src" >/dev/null 2>&1 || true; rm -rf "$tmp"; git -C "$repo" worktree prune' EXIT
git worktree add --quiet --detach "$src" "$ref"
# The repo's own target/ keeps the build incremental across temp worktrees.
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$repo/target}
# Baked in for `atharu update`: the temp worktree is gone by the time it runs.
export ATHARU_REPO=$repo

bin=${CARGO_INSTALL_ROOT:-${CARGO_HOME:-$HOME/.cargo}}/bin
# A binary from before `version` existed exits non-zero: that reads as "none".
old=$("$bin/atharu" version 2>/dev/null || echo none)
echo "building atharu $label from $repo …"
cargo install --path "$src" --locked --quiet
new=$("$bin/atharu" version)
echo "atharu: $old → $new"

if [ "$app" = 1 ]; then
  if ! cargo tauri --version >/dev/null 2>&1; then
    echo 'tauri-cli missing: cargo install tauri-cli --version "^2" --locked'
    exit 1
  fi
  (cd "$src/src-tauri" && cargo tauri build --bundles app)
  built=$CARGO_TARGET_DIR/release/bundle/macos/Atharu.app
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
  # Build context = the worktree; ./data bind and project name stay the repo's: running compose from
  # the worktree would start on an empty ./data.
  printf 'services:\n  atharu:\n    build:\n      context: %s\n' "$src" >"$tmp/build.yaml"
  docker compose --project-directory "$repo" -f "$repo/compose.yaml" -f "$tmp/build.yaml" up -d --build
fi

case ":$PATH:" in
  *":$bin:"*) ;;
  *) echo "add to ~/.zshrc: export PATH=\"$bin:\$PATH\"" ;;
esac
# The installed binary resolves data/atharu.db against the current dir; pin it to this checkout's DB.
if [ -z "${ATHARU_DB:-}" ]; then
  echo "add to ~/.zshrc: export ATHARU_DB=\"$repo/data/atharu.db\""
fi
