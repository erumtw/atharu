#!/usr/bin/env bash
# Cuts release vX.Y.Z (D40, D41): bumps the three version fields + Cargo.lock, prepends a CHANGELOG.md
# entry, commits `chore: release vX.Y.Z`, tags it, then asks before pushing and creating the GitHub Release.
#   scripts/release.sh X.Y.Z            from main; release/X.Y is created or fast-forwarded to the release
#   scripts/release.sh --hotfix X.Y.Z   from release/X.Y (fix committed there first); prints the cherry-pick for main
# ATHARU_COAUTHOR="Name <email>" adds a Co-Authored-By trailer (agents set it, the owner doesn't).
set -euo pipefail

usage="usage: scripts/release.sh [--hotfix] X.Y.Z"
hotfix=0 ver=
for a in "$@"; do
  case $a in
    --hotfix) hotfix=1 ;;
    -h | --help) sed -n '2,6p' "$0"; exit 0 ;;
    -*) echo "unknown option $a" >&2; echo "$usage" >&2; exit 2 ;;
    *) [ -z "$ver" ] || { echo "$usage" >&2; exit 2; }; ver=${a#v} ;;
  esac
done
[ -n "$ver" ] || { echo "$usage" >&2; exit 2; }
die() { echo "$*" >&2; exit 1; }

[[ $ver =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z.-]+)?$ ]] ||
  die "$ver is not semver X.Y.Z"
line=${BASH_REMATCH[1]}.${BASH_REMATCH[2]}
tag=v$ver
branch=release/$line

repo=$(cd "$(dirname "$0")/.." && pwd)
cd "$repo"
files=(Cargo.toml src-tauri/Cargo.toml src-tauri/tauri.conf.json Cargo.lock CHANGELOG.md)

[ -z "$(git status --porcelain --untracked-files=no)" ] ||
  die "uncommitted changes: commit or stash them first"$'\n'"$(git status --short --untracked-files=no)"
here=$(git symbolic-ref -q --short HEAD || echo "a detached HEAD")
if [ "$hotfix" = 1 ]; then
  [ "$here" = "$branch" ] || die "--hotfix $ver runs on $branch, not $here"
else
  [ "$here" = main ] || die "releases are cut from main, not $here (a patch on $branch: --hotfix)"
fi
git fetch --tags --quiet 2>/dev/null || echo "warning: can't fetch tags (offline?); using local tags" >&2
! git rev-parse -q --verify "refs/tags/$tag" >/dev/null || die "tag $tag exists already"

# Greater than every release so far; a hotfix only within its own line (v0.3.0 may exist beside v0.2.1).
pattern='v[0-9]*'
[ "$hotfix" = 0 ] || pattern="v$line.*"
newest=$(git tag --list "$pattern" | sed 's/^v//' | sort -V | tail -n 1)
if [ -n "$newest" ]; then
  [ "$(printf '%s\n%s\n' "$newest" "$ver" | sort -V | tail -n 1)" = "$ver" ] && [ "$newest" != "$ver" ] ||
    die "$ver is not greater than v$newest"
fi
if [ "$hotfix" = 0 ] && git rev-parse -q --verify "refs/heads/$branch" >/dev/null; then
  git merge-base --is-ancestor "$branch" HEAD ||
    die "$branch has commits main lacks (a hotfix?): cherry-pick them to main, or cut $ver from $branch with --hotfix"
fi

# Changelog since the nearest tag on this branch (the whole history for the first release).
prev=$(git describe --tags --abbrev=0 --match 'v[0-9]*' 2>/dev/null || true)
subjects=$(git log --format=%s ${prev:+"$prev..HEAD"} | grep -v '^chore: release v' || true)
conv='(\([^)]*\))?!?: '
group() { # heading, type regex: subjects of that type, prefix dropped, first letter capitalised (perl: BSD sed has no \u)
  local items
  items=$(printf '%s\n' "$subjects" | grep -E "^($2)$conv" | sed -E "s/^[a-z]+$conv/- /" | perl -pe 's/^- (.)/- \u$1/' || true)
  [ -z "$items" ] || printf '\n### %s\n\n%s\n' "$1" "$items"
}
notes=$(git rev-parse --absolute-git-dir)/release-notes-$tag.md
# The draft: CHANGELOG.md keeps it as is; the GitHub release page gets it rewritten with Highlights (CLAUDE.md "Releases").
{
  printf '## %s — %s\n' "$tag" "$(date +%Y-%m-%d)"
  group Features feat
  group Fixes fix
  group Performance perf
  group 'Under the hood' 'refactor|docs|build|ci|test|style|chore'
  other=$(printf '%s\n' "$subjects" | grep -vE "^(feat|fix|perf|refactor|docs|build|ci|test|style|chore)$conv" | grep . | sed 's/^/- /' || true)
  [ -z "$other" ] || printf '\n### Other\n\n%s\n' "$other"
  [ -n "$subjects" ] || printf '\n(no changes)\n'
} >"$notes"

# The first line matching $2 gets its version value replaced: [package] comes first in both manifests.
bump() {
  awk -v v="$ver" -v re="$2" '!done && $0 ~ re { sub(/"[0-9][^"]*"/, "\"" v "\""); done = 1 } 1' "$1" >"$1.tmp"
  mv "$1.tmp" "$1"
}
bump Cargo.toml '^version = "'
bump src-tauri/Cargo.toml '^version = "'
bump src-tauri/tauri.conf.json '^  "version": "'
# What cargo itself writes for the two workspace members; `cargo install --locked` refuses a stale lock.
awk -v v="$ver" '/^name = "atharu(-app)?"$/ { hit = 1; print; next } hit && /^version = / { $0 = "version = \"" v "\""; hit = 0 } 1' Cargo.lock >Cargo.lock.tmp
mv Cargo.lock.tmp Cargo.lock
{
  echo "# Changelog"
  echo
  cat "$notes"
  [ ! -f CHANGELOG.md ] || tail -n +2 CHANGELOG.md
} >CHANGELOG.md.tmp
mv CHANGELOG.md.tmp CHANGELOG.md

fixes_end=$(git rev-parse --short HEAD)
msg="chore: release $tag"
[ -z "${ATHARU_COAUTHOR:-}" ] || msg+=$'\n\n'"Co-Authored-By: $ATHARU_COAUTHOR"
git add "${files[@]}"
git commit --quiet -m "$msg"
git tag -a "$tag" -m "atharu $tag"
refs=("$branch" "$tag")
if [ "$hotfix" = 0 ]; then
  # The release commit stays on main too, so main's version and CHANGELOG follow the releases.
  git branch -f "$branch" HEAD
  refs=(main "${refs[@]}")
fi
echo "released $tag on $branch ($(git rev-parse --short HEAD)); notes: $notes"

push=(git push origin "${refs[@]}")
release=(gh release create "$tag" --title "atharu $tag" --notes-file "$notes" --verify-tag)
ans=n
if [ -t 0 ]; then
  read -r -p "Push ${refs[*]} and create the GitHub Release? [y/N] " ans
fi
if [ "$ans" = y ] || [ "$ans" = Y ]; then
  "${push[@]}"
  "${release[@]}"
else
  echo "not pushed; when ready:"
  echo "  ${push[*]}"
  echo "  gh release create $tag --title \"atharu $tag\" --notes-file $notes --verify-tag"
fi
if [ "$hotfix" = 1 ]; then
  echo "bring the fix back to main: git checkout main && git cherry-pick ${prev:+$prev..}$fixes_end"
fi
