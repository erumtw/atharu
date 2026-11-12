#!/bin/bash
# The Mac app's speaker split (T56, D37) links sherpa-onnx's static lib. Its own build.rs would download the
# archive with no hash check; .cargo/config.toml points SHERPA_ONNX_LIB_DIR here instead, so that never runs.
# Fetches the pinned archive once (cached), checks its SHA-256 and unpacks it under target/. Idempotent.
set -euo pipefail
v=2.0.1
name=sherpa-onnx-v$v-osx-arm64-static-lib
sha=d7a9b14f337958e7373c8c650f046df33d210bbaff2f33cb8322dda663ee5e1c
url=https://github.com/k2-fsa/sherpa-onnx/releases/download/v$v/$name.tar.bz2
root=$(cd "$(dirname "$0")/.." && pwd)
dest=$root/target/sherpa-onnx
[ -f "$dest/$name/lib/libsherpa-onnx-c-api.a" ] && exit 0
cache=${XDG_CACHE_HOME:-$HOME/Library/Caches}/atharu
mkdir -p "$cache" "$dest"
tarball=$cache/$name.tar.bz2
ok() { [ -f "$1" ] && [ "$(shasum -a 256 -b "$1" | cut -d' ' -f1)" = "$sha" ]; }
if ! ok "$tarball"; then
  echo "fetching $name (20 MB) …"
  curl -sSfL --retry 2 -o "$tarball.part" "$url"
  if ! ok "$tarball.part"; then
    rm -f "$tarball.part"
    echo "sherpa-onnx: SHA-256 mismatch, nothing kept" >&2
    exit 1
  fi
  mv "$tarball.part" "$tarball"
fi
tar -xjf "$tarball" -C "$dest"
