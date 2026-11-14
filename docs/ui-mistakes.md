# UI mistakes log

Every UI bug the owner had to find gets a row here: what they saw, the direct cause in the code, the **root cause** (why it
was written that way and why nothing caught it before release), the rule it taught and the automatic check that now
catches it. Rule (CLAUDE.md "Long real data"): **a reported UI bug isn't fixed until a check exists that fails on the old
code** (a `scripts/visual-check.mjs` rule or a test), proven failing before the fix. Read this before UI work; add the row
in the same commit as the fix.

## Root causes (the patterns behind the rows)

| Id | Root cause |
|---|---|
| R1 | **Short demo data.** Designs, boards and checks used the seeded demo (short names, 4 tickets); the owner's data has 60-char series names, 80–90-char Jira titles, 30 assigned tickets, Thai text. Long text never existed where we looked. |
| R2 | **Blind checks.** `visual-check` measured only the page edge, but `.pg{overflow:clip}` hides anything that overflows inside the sheet; it skipped 0-px text (`w > 0`); it never scrolled. It said "0 problems" while the bug was on screen. |
| R3 | **Per-component widths, no shared pattern.** Each table/row/chip picked its own px widths and its own ellipsis rule (or none); nothing forced "text in a fixed slot clips with a title". |
| R4 | **Reviewed against the wrong reference.** Parity checks compared to boards drawn with short text; a design review used memory of DESIGN.md instead of reading it. Matching the reference proved nothing. |
| R5 | **Environment not covered.** Only checked in headless Chrome with overlay scroll bars and a browser UA; the owner runs the Mac app with "always show scroll bars". |
| R6 | **Pace over looking.** Five releases in two days; the master looked at the changed pages' screenshots, not every page on real data. |
| R7 | **Layout structure.** Rows are flex boxes steered by exception lists (`.blk.ln>:not(.ltext,…,.chip,.lchip,.k)`: a new chip type missing from the list broke the row), 172 inline `style="…"` in templates, 114 hard px widths, and one `components.css` appended per release where rule order decides winners. Planned fix: 0.3.0 (row and table primitives, CSS layers, inline styles out). |

## Log

| Date · version | What the owner saw | Direct cause | Root cause | Rule it taught | Check that catches it now |
|---|---|---|---|---|---|
| 2026-11-13 · 0.2.0 | To-do text squeezed, Meetings table overflowing, Notes padding jumping | Layout built and verified on the demo DB only | R1, R6 | Check UI on real data before release | `scripts/visual-check.sh` (every page × 1180/900/390 on a real-DB copy) |
| 2026-11-13 · 0.2.1 | Hover toolbar covered the note button | Toolbar positioned over the row's own controls | R4 (board showed no note button there) | Floating actions never cover the row's targets | none (manual) |
| 2026-11-14 · 0.2.4 | A release screenshot showed real coworker names | The demo DB used for the shot carried the real Settings › Names | R1 inverse: "demo" DB not freshly seeded; R6 | Release shots only from a fresh seed, looked at for real names | CLAUDE.md release-page rule (manual look) |
| 2026-11-14 · 0.2.5 | Grey empty strip down every page and the popover | `html{scrollbar-gutter:stable}` added to stop a 1-scrollbar jump | R5 | No page-wide scrollbar gutter | none (needs macOS always-on scroll bars; manual) |
| 2026-11-14 · 0.2.5 | GitHub ↗ chip taller than tag/key chips | A row rule stretched every non-`.chip` item to the row height; `.lchip` wasn't in its exception list | R3 (two chip classes, no shared pill) | A chip is a chip: one height for every pill | none (manual) |
| 2026-11-14 · 0.2.5 | Start-meeting popover hid the series list under its own text | Native `<datalist>` drop-down rendered under the popover's top layer | R5 (only checked in Chrome, not the app's WebKit) | Start field lives in the page, not a popover | none (manual) |
| 2026-11-14 · 0.2.6 | Assigned rows: status chips at different x | Flex rows, each sized by its own content | R3, R1 (demo had 4 similar rows) | Repeated rows use fixed grid columns | Epics tests assert the grid cells; manual measure |
| 2026-11-14 · 0.2.7 | Long series name after "via" ran past the page | Chip inside `.via > a`, which nothing shrank | R1, R2 (`.pg` clip hid it), R3 | Text in a fixed slot ends in `…` (`.clip`) + `title` | visual-check: element past the sheet's right edge |
| 2026-11-14 · 0.2.7 | Meetings: series name printed over two columns | Fixed-layout table, first cell didn't clip | R1, R2, R3 | Same, for table cells | visual-check: text out of its `td` |
| 2026-11-14 · 0.2.7 | Epics titles crushed to "[Dat…" | Five fixed columns summed to 650 px of a ~730 px table | R1, R3 | A table states its widths and which column gives way | visual-check: name clipped to a sliver (< 56 px) |
| 2026-11-14 · 0.2.7 | Status chip sat mid-row with a gap | Empty due column + 132 px action column between status and action | R1 (demo rows all had due dates), R4 | Related items sit together (Gestalt proximity) | manual measure (gap ≤ 16 px) |
| 2026-11-14 · 0.2.7 | Epics status chip: fixed-width pill, empty gap, other labels cut | Chip inside the Title cell with an 88 px min-width | R3, R1 (demo statuses were short) | Chips hug their text; status gets its own column | tests/jira.rs: chip inside `<td class="stc">` (failed on the old templates) |
| 2026-11-14 · 0.2.7 | To-do text one letter per line | Jira link chip shows the whole Jira title (606 px), line text squeezed to 0 px | R1, R2 (check skipped `w == 0`), R3 | Link chips clip; line text keeps ≥ 40 % | visual-check: squeezed line text incl. 0 px (0.2.8 T3d) |
| 2026-11-14 · 0.2.7 | Blank sidebar column on long pages | `.side` sticky only in the 640–1023 rail; `position:static` at ≥ 1024 | R2 (check never scrolled), R1 (demo pages short) | The sidebar stays pinned | visual-check: `.side` at the viewport top after scrolling (0.2.8 T3d) |
| 2026-11-14 · design | A design review proposed a legend row DESIGN.md forbids | Reviewer used memory of DESIGN.md | R4 | Read CLAUDE.md + DESIGN.md in the run, quote the rule per decision | `/design-evidence` skill: "Project rules" line per decision |
