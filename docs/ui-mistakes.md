# UI mistakes log

Every UI bug the owner had to find gets a row here, with the rule it taught and the automatic check that now catches it.
Rule (CLAUDE.md "Long real data"): **a reported UI bug isn't fixed until a check exists that fails on the old code** (a
`scripts/visual-check.mjs` rule or a test), proven failing before the fix. Read this before UI work; add the row in the
same commit as the fix.

| Date · version | What the owner saw | Cause | Rule it taught | Check that catches it now |
|---|---|---|---|---|
| 2026-11-13 · 0.2.0 | To-do text squeezed, Meetings table overflowing, Notes padding jumping | Verified only on the short demo DB | Check UI on real data before release | `scripts/visual-check.sh` (every page × 1180/900/390 on a real-DB copy) |
| 2026-11-13 · 0.2.1 | Hover toolbar covered the note button | Toolbar placed over the row's own controls | Floating actions never cover the row's targets | none (manual) |
| 2026-11-14 · 0.2.4 | A release screenshot showed real coworker names | Demo DB carried the real Settings › Names | Release shots only from a fresh seed, looked at for real names | CLAUDE.md release-page rule (manual look) |
| 2026-11-14 · 0.2.5 | Grey empty strip down every page and the popover | `html{scrollbar-gutter:stable}` with "always show scroll bars" | No page-wide scrollbar gutter | none (needs macOS always-on scroll bars; manual) |
| 2026-11-14 · 0.2.5 | GitHub ↗ chip taller than tag/key chips | Row rule stretched non-`.chip` items to row height | A chip is a chip: one height for every pill | none (manual) |
| 2026-11-14 · 0.2.5 | Start-meeting popover hid the series list under its own text | Native datalist inside a popover | Start field lives in the page, not a popover | none (manual) |
| 2026-11-14 · 0.2.6 | Assigned rows: status chips at different x | Flex rows sized per row | Repeated rows use fixed grid columns | Epics tests assert the grid cells; manual measure |
| 2026-11-14 · 0.2.7 | Long series name after "via" ran past the page | Chip inside `.via > a` had nothing shrinking it; `.pg` clip hid it from the check | Text in a fixed slot ends in `…` (`.clip`) + `title` | visual-check: element past the sheet's right edge |
| 2026-11-14 · 0.2.7 | Meetings: series name printed over two columns | Fixed table, cell didn't clip | Same as above, for table cells | visual-check: text out of its `td` |
| 2026-11-14 · 0.2.7 | Epics titles crushed to "[Dat…" | Fixed columns too wide for 1180 | A table states its widths and which column gives way | visual-check: name clipped to a sliver (< 56 px) |
| 2026-11-14 · 0.2.7 | Status chip sat mid-row with a gap | Empty due column + wide action column between | Related items sit together (Gestalt proximity) | manual measure (gap ≤ 16 px) |
| 2026-11-14 · 0.2.7 | Epics status chip: fixed-width pill, empty gap, other labels cut | Chip in the Title cell with a min-width | Chips hug their text; status gets its own column | 0.2.8 T3c check |
| 2026-11-14 · 0.2.7 | To-do text one letter per line | Jira link chip showed the whole Jira title (606 px), text at 0 px; the check skipped 0-px text | Link chips clip; line text keeps ≥ 40 % | visual-check: squeezed line text incl. 0 px (0.2.8 T3d) |
| 2026-11-14 · 0.2.7 | Blank sidebar column on long pages | Sidebar `position:static` at ≥ 1024 px | The sidebar stays pinned | visual-check: `.side` at the viewport top after scrolling (0.2.8 T3d) |
| 2026-11-14 · design | A design review proposed a legend row DESIGN.md forbids | Reviewer used memory of DESIGN.md | Read CLAUDE.md + DESIGN.md in the run, quote the rule per decision | `/design-evidence` skill: "Project rules" line per decision |
