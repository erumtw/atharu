#!/bin/bash
# Colour lint (0.2.9, D3): a raw colour (#hex, rgb()/rgba()/hsl(), white, black) in static/css is only allowed in tokens.css and between
# the `color-lint:off` / `color-lint:on` markers in looks.css (the per-look blocks). Everywhere else a colour is a role token from
# tokens.css, so a look (Yoru) is one block of values. Comments are ignored. Exit 1 lists the hits. visual-check.sh runs it first.
# False positive: an id selector that is a valid hex (#add, #bad): rename the id.
cd "$(dirname "$0")/.." && python3 -I - <<'PY'
import glob, re, sys
HIT = re.compile(r'#[0-9a-fA-F]{3,8}\b|\b(?:rgba?|hsla?)\(|(?<![-\w])(?:white|black)(?![-\w])')
bad = []
for path in sorted(glob.glob('static/css/*.css')):
    if path.endswith('/tokens.css'):
        continue
    src = open(path).read()
    # blank the comments but keep line numbers; the markers are read from the raw text first
    off = False
    marks = {}
    for i, line in enumerate(src.split('\n'), 1):
        if 'color-lint:off' in line: off = True
        marks[i] = off
        if 'color-lint:on' in line: off = False
    code = re.sub(r'/\*.*?\*/', lambda m: re.sub(r'[^\n]', '', m.group(0)), src, flags=re.S)
    for i, line in enumerate(code.split('\n'), 1):
        if marks.get(i) and path.endswith('/looks.css'):
            continue
        m = HIT.search(line)
        if m:
            bad.append(f'{path}:{i}: raw colour "{m.group(0)}": use a token from tokens.css: {line.strip()[:120]}')
print('\n'.join(bad))
print(f'color-lint: {len(bad)} raw colour line(s)')
sys.exit(1 if bad else 0)
PY
