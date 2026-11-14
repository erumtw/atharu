#!/usr/bin/env bash
# Installs or updates atharu from a release tag (D40), built from source in a temp worktree: never
# the working tree, which agents edit mid-task. Safe to re-run.
#   scripts/install.sh                   newest release tag vX.Y.Z → ~/.cargo/bin/atharu
#   scripts/install.sh --version vX.Y.Z  that tag instead (pre-releases only this way)
#   scripts/install.sh --dev             the last commit instead (uncommitted edits are left out)
#   --app also replaces /Applications/Atharu.app (its data dir is untouched), --docker rebuilds the container
# Never edits shell rc files: it prints the lines to add instead.
set -euo pipefail
# The lock test's seam (tests/update.rs) must never come in from an exported parent-shell function.
unset -f install_lock_seam 2>/dev/null || true
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
# Step markers for Settings › Update's progress bar (0.2.8): one line when a phase starts, "▸ step N/M name". With --app there are
# four phases (source, cli, app, swap), without it two (source, cli). The bar reads the last one from update.log.
steps=2
if [ "$app" = 1 ]; then steps=4; fi
marker() { echo "▸ step $1/$steps $2"; }
# Build time per step + total on stderr (bash's SECONDS), so a slow run shows where the minutes went. The bar's estimate is made of
# these lines: ⏱ source, ⏱ cli, ⏱ app build, ⏱ swap (update-times.txt keeps the last run's).
t0=$SECONDS t_step=$SECONDS
mins() { printf '%dm%02ds' $(($1 / 60)) $(($1 % 60)); }
step() { echo "⏱ $1: $(mins $((SECONDS - t_step)))"; t_step=$SECONDS; }

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

# One install at a time: Settings › Update, `atharu update --app` and a retry can all start one, and they share target/install-src,
# CARGO_TARGET_DIR and /Applications/Atharu.app.new. The lock is a directory (mkdir is atomic) holding the owner's pid; a dead
# owner (crash, reboot) leaves a stale lock, which the next run reclaims.
lock=$repo/target/install.lock
take_lock() {
  local holder
  mkdir -p "$(dirname "$lock")"
  if ! mkdir "$lock" 2>/dev/null; then
    holder=$(cat "$lock/pid" 2>/dev/null || true)
    # The owner may be between its mkdir and writing the pid: give it a moment before calling that stale.
    if [ -z "$holder" ]; then sleep 1; holder=$(cat "$lock/pid" 2>/dev/null || true); fi
    if [ -n "$holder" ] && kill -0 "$holder" 2>/dev/null; then
      echo "another install is running (pid $holder)"
      exit 1
    fi
    # A caller can define install_lock_seam to run something right here (the tests race a new owner in); never defined for real.
    ! declare -F install_lock_seam >/dev/null || install_lock_seam
    # Reclaim by renaming it away: of several runs that see the same stale lock only one wins the rename, the others stop here.
    mv "$lock" "$lock.stale.$$" 2>/dev/null || { echo "another install is running"; exit 1; }
    # Between our read and the rename a faster run may have replaced the lock with its own: what we hold must still be the one we
    # judged stale. If not, put it back (unless yet another run has taken the name) and give way.
    if [ "$(cat "$lock.stale.$$/pid" 2>/dev/null || true)" != "$holder" ]; then
      [ -e "$lock" ] || mv "$lock.stale.$$" "$lock" 2>/dev/null || true
      echo "another install is running"
      exit 1
    fi
    rm -rf "$lock.stale.$$"
    mkdir "$lock" 2>/dev/null || { echo "another install is running"; exit 1; }
  fi
  echo $$ >"$lock/pid"
}
take_lock
# Only the owner reaches here; every EXIT trap below also lets the lock go (a second `trap … EXIT` replaces the first).
trap 'rm -rf "$lock"' EXIT

marker 1 source
# A detached worktree, not `git archive`: build.rs reads the sha and tag from git.
# Same path every run (inside target/, gitignored and kept out of Spotlight): cargo keys a local crate's
# cache by its path, so a fresh temp dir each time rebuilt atharu + the app from scratch.
src=$repo/target/install-src
git -C "$repo" worktree remove --force "$src" >/dev/null 2>&1 || rm -rf "$src"
git -C "$repo" worktree prune
trap 'git -C "$repo" worktree remove --force "$src" >/dev/null 2>&1 || true; git -C "$repo" worktree prune; rm -rf "$lock"' EXIT
git worktree add --quiet --detach "$src" "$ref"
# The repo's own target/ keeps the build incremental across runs.
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$repo/target}
if [ "$dev" = 1 ]; then
  # --dev is for trying a change: skip the release profile's fat LTO + 1 codegen unit (most of the build
  # time, for a binary a little bigger and no slower to use). Release tags keep the full profile.
  export CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 CARGO_PROFILE_RELEASE_INCREMENTAL=true
fi
# Baked in for `atharu update`: the build worktree is gone by the time it runs.
export ATHARU_REPO=$repo
step source

bin=${CARGO_INSTALL_ROOT:-${CARGO_HOME:-$HOME/.cargo}}/bin
# A binary from before `version` existed exits non-zero: that reads as "none".
old=$("$bin/atharu" version 2>/dev/null || echo none)
# One EXIT trap (a second `trap … EXIT` replaces the first): clean the worktree, then the total.
trap 'git -C "$repo" worktree remove --force "$src" >/dev/null 2>&1 || true; git -C "$repo" worktree prune; rm -rf "$lock"; echo "⏱ total: $(mins $((SECONDS - t0)))"' EXIT
echo "building atharu $label from $repo …"
marker 2 cli
cargo install --path "$src" --locked --quiet
new=$("$bin/atharu" version)
echo "atharu: $old → $new"
step cli

if [ "$app" = 1 ]; then
  if ! cargo tauri --version >/dev/null 2>&1; then
    echo 'tauri-cli missing: cargo install tauri-cli --version "^2" --locked'
    exit 1
  fi
  # A stable signature keeps macOS's Microphone / Screen Recording grants across installs (ad-hoc loses
  # them every build). scripts/dev-cert.sh makes the identity once.
  if security find-identity -v -p codesigning 2>/dev/null | grep -q '"Atharu Local Signing"'; then
    export APPLE_SIGNING_IDENTITY="Atharu Local Signing"
  else
    echo "note: unsigned build, recording permissions reset on every install: run scripts/dev-cert.sh once"
  fi
  marker 3 app
  # Speaker split (T56): the sha-pinned sherpa-onnx static lib; tags before 0.1.1 have no script and no need.
  if [ -x "$src/scripts/sherpa-onnx.sh" ]; then "$src/scripts/sherpa-onnx.sh"; fi
  (cd "$src/src-tauri" && cargo tauri build --bundles app)
  step "app build"
  built=$CARGO_TARGET_DIR/release/bundle/macos/Atharu.app
  # ATHARU_APP_DEST: where the app is replaced (the tests point it at a temp dir so a fake run never touches /Applications).
  dest=${ATHARU_APP_DEST:-/Applications/Atharu.app}
  # Settings › Update (the app's own button) sets ATHARU_UPDATE_FROM_APP: this script runs as a child of the very app it is about
  # to replace, and the server inside it is what shows the progress and the Restart button, so quitting it here would kill the
  # install and its page. The running app is left alone: moving a running .app's bundle aside (mv) is safe on macOS, and the
  # owner restarts into the new copy from the page. From a Terminal (`atharu update --app`) the old app is quit first, as before.
  if [ -z "${ATHARU_UPDATE_FROM_APP:-}" ]; then
    # "if running" first: a bare `quit app` launches the app just to quit it.
    osascript -e 'if application "Atharu" is running then tell application "Atharu" to quit'
    for _ in $(seq 20); do
      [ "$(osascript -e 'application "Atharu" is running')" = true ] || break
      sleep 0.5
    done
  fi
  marker 4 swap
  # Copy beside the old app, swap only once the copy is whole: a failed copy leaves the old one working.
  rm -rf "$dest.new"
  ditto "$built" "$dest.new"
  if [ -e "$dest" ]; then mv "$dest" "$dest.old"; fi
  mv "$dest.new" "$dest"
  rm -rf "$dest.old"
  step swap
  echo "Mac app replaced: $dest (open it again; data stays in ~/Library/Application Support/dev.atharu.desktop)"
fi

if [ "$docker" = 1 ]; then
  # Build context = the worktree; ./data bind and project name stay the repo's: running compose from
  # the worktree would start on an empty ./data.
  printf 'services:\n  atharu:\n    build:\n      context: %s\n' "$src" >"$repo/target/install-build.yaml"
  docker compose --project-directory "$repo" -f "$repo/compose.yaml" -f "$repo/target/install-build.yaml" up -d --build
  step docker
fi

case ":$PATH:" in
  *":$bin:"*) ;;
  *) echo "add to ~/.zshrc: export PATH=\"$bin:\$PATH\"" ;;
esac
# The installed binary resolves data/atharu.db against the current dir; pin it to this checkout's DB.
if [ -z "${ATHARU_DB:-}" ]; then
  echo "add to ~/.zshrc: export ATHARU_DB=\"$repo/data/atharu.db\""
fi
