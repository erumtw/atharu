# Changelog

## v0.2.8 — 2026-11-14

### Features

- Epics gets a Status column, the capture hint shows the grammar, long names show two lines on a phone
- A to-do's due date sits right after its title and tags (inline, no separator); the phone keeps it on the meta line
- The Update row shows a progress bar with the step and the time left
- Update progress model (per-step estimate from the last run, front-loaded curve, never backwards); install.sh also times source and swap
- Install.sh prints one "▸ step N/M name" line per phase (the progress bar's markers)

### Fixes

- Update ETA counts the later steps when one overruns; a run that ends mid-poll renders its end state, not 0 %
- A Jira link chip clips instead of squeezing the to-do text to 0 px, the sidebar stays pinned, and the check no longer skips 0-px text
- The update bar accelerates (0.97 x real^1.6, slow start and fast finish) instead of front-loading 60 %
- A screenshot from the popover hides Atharu, so the main window doesn't jump over what you capture

### Under the hood

- V0.2.8 release screenshot shows "Updating to v0.2.9" (was a v0.9.0 placeholder tag)
- V0.2.8 release screenshot (Settings › Update mid-install, fresh demo DB, no names or paths)
- Ui-mistakes root cause R7 (layout structure), fix planned for 0.3.0
- Ui-mistakes log names the root causes (R1–R6) behind each row
- UI mistakes log + rule (every reported UI bug gets a row and a check that fails on the old code)
- The update bar walks the four steps of a fake install, never back, accelerating

## v0.2.7 — 2026-11-14

### Features

- Long-data guard (scripts/long-seed.sh, visual-check --db, shared .clip class + titles on every fixed slot, DESIGN rule) and the fixes the audit found
- Live filter on /epics (hides rows and empty groups as you type, counts stay true, Esc clears)
- Epics view row (filter box, Group by, Show) in Today's control-row markup, Hide tracked and a folded assigned list
- Epics lists filter and group (OKR, Jira status, parent, tag, none), Show active / done & dropped / all, Hide tracked on assigned

### Fixes

- Install.sh ignores an exported lock-test seam function
- Gate and real-data look fixes (suggest menus use .clip, D4 header clips, Epics empty states, stale-lock pid re-check, live filter covers Done & dropped and Unnamed, assigned parents keyed by key, header / label / phone to-do alignment)
- Epics title column keeps its room (new widths, chip yields), Hide tracked is a switch, assigned rows put due before status, long via chips and series names clip; visual-check flags text past the page edge or its cell
- Install.sh reclaims a stale lock by renaming it away (only one run wins), not rm + mkdir

### Under the hood

- V0.2.7 release screenshots (Epics filtered, assigned grouped by status) from a fresh fictional demo copy
- UI rules for long real data (ellipsis in fixed slots, long-data seed, every-page check before release)
- One pass builds the Epics candidates (no second query or stub for the filter's group totals)
- README Epics row mentions the filter, Group by, Show and Hide tracked

## v0.2.6 — 2026-11-14

### Features

- Settings › Update in the Mac app (check, Update, progress, Restart); shared src/updater.rs
- N-space / ⌘N in Today's capture bar saves a note (shared note box with the popover)
- Popover to-dos are five flat rows with the quiet due date, no inner scroll
- Popover meeting section is one row (name, mic toggle, Start; running: dot, name, time, camera, mic, End)
- Popover capture is one field (kind menu inside, Log inside, legend, header note button)

### Fixes

- One install at a time (install.sh lock dir with pid; updater busy flag held until the process ends; timed-out row says it is still running)
- In-app update no longer quits its own app (install.sh flag, log file not pipes, 20 min deadline); no-JS n-note lands on Today; to-do cap counts the mic's consent line
- Popover capture section ends right under the hint (no extra gap); one row less in the tight Glass everywhere states
- Assigned-to-you rows on fixed columns so the status chips line up; phone = key · status · action, title under

### Under the hood

- Rustfmt tests/jira.rs
- V0.2.6 release screenshots (popover at rest, a meeting recording, Today's note box) from a fresh fictional demo DB
- Simplify the 0.2.6 popover and update code (dead pill CSS, derived kind fields, one update-row helper)
- Quick popover section matches the compact layout (header, keyboard order, recording row, AA); README capture paragraph
- Src/updater.rs in the layout notes (shared update logic)
- Line grammar notes `n ` = a note in the capture box (Today and popover)
- Release screenshots only from a freshly seeded demo DB, checked for real names

## v0.2.5 — 2026-11-14

### Features

- Report lists what Jira resolved this period, under each parent (D3)
- Epic and story pages follow Jira (Done banner, Tickets, story head, "Also move in Jira?")
- Epics reads Jira (assigned to you, parent groups, Jira title, status by category, Jira flags, dead token)
- Move a Jira issue's status from its chip, with Undo (D1, D8)
- Jira data for 0.2.5 (type, status category, parent, due, resolved, updated, assigned to me, dead token)

### Fixes

- No page-wide scrollbar gutter (an empty grey strip on every page and the menu-bar popover with always-shown scroll bars)
- Jira gate fixes (live status for menu + Undo, serialised hx-vals, whole-chip swap, 401 vs 403, epic level, mark_done guard)
- Jira design parity (one-pill status chip, menu header/pills/footer, inline chip, assigned rows, one-line story head, bold banner key; shots)
- Simplify + verify for 0.2.5 (one Jira chip partial, status menu inside the epic head, ticket keys; screenshots)
- A Mac sync writes nothing once the switch is off, even if it went off during the access dialog
- Start meeting is a compact row in the page again; the popover hid the series list under its own text
- Link chip is the same 20px pill as tag and key chips (was stretched to the row height)

### Under the hood

- Spec entry for Epics × Jira (status moves on press only, D1–D10) and README Jira paragraph

## v0.2.4 — 2026-11-14

### Features

- Every to-do row shows its due date again, quietly (last column, muted, red when late)
- Calendar entitlement for the signed (hardened runtime) Mac app (D6)
- The Mac app reads macOS Calendar (EventKit) into the Calendar page
- Mac calendars on the server side (settings row, replace_mac_events)
- Migration 012 lets events come from macOS Calendar (source 'mac')

### Fixes

- Final-gate findings for 0.2.4 (Mac all-day history, autorelease pool, switch off, hub label, Day view fold, DST, row poll)
- Link chips join the grey pill family (blue text + ↗); epic head stays one line with the Jira title ending in …
- Calendar real-data fixes (N-days label, column alignment, short and crowded events, 08:00 label)
- Switch highlight re-measures when its buttons resize (picked label turning semibold left it short)
- Today's tag filter and Group by back as an always-visible row (no View menu)

### Under the hood

- Day view never folds a cluster
- Events.source includes mac
- V0.2.4 release screenshots (demo DB: Calendar with Mac events, Settings › Mac calendars)
- Mac calendars in DESIGN.md (Settings row, Calendar sources) and README Configuration

## v0.2.3 — 2026-11-13

### Features

- Scripts/visual-check.sh, a real-data layout check before every release
- Calendar interactions (N days by width, event cards, empty slot → + Event)
- Google-Calendar-style calendar page (bar, hour grid, all-day row, month cells)
- Calendar model for day / N days / week / month with an hour grid and overlap columns

### Fixes

- Final-gate findings (multi-day events, visual-check cleanup, auto-N flash, tag filter view, out-of-range add)
- List tables fit a phone; simplify pass; v0.2.3 calendar screenshots
- Jira Refresh shows that it ran (updated N min ago, Refreshing…, ✓ Refreshed from Jira)
- A tag or series rename leaves [[old]] alone while it still resolves (P4)
- To-do hover toolbar sits above the row's corner, not over its note button
- View menu's Group by switch keeps its track and highlight; its tag field keeps its box
- Notes tag-filter test follows the row markup (no inline style since v0.2.0)
- Real-data layout bugs in 0.2.0 (to-do text squeezed, Meetings table overflow, page jump on Notes filter, note rows run together)

### Under the hood

- Name db.rs as the shared-SQL exception
- Run scripts/visual-check.sh before every release
- Agents never read data/config.toml (credentials)

### Other

- merge: release/0.2 hotfixes v0.2.1, v0.2.2 back into main

## v0.2.2 — 2026-11-13

### Fixes

- To-do hover toolbar sits above the row's corner, not over its note button

## v0.2.1 — 2026-11-13

### Fixes

- View menu's Group by switch keeps its track and highlight; its tag field keeps its box
- Real-data layout bugs in 0.2.0 (to-do text squeezed, Meetings table overflow, page jump on Notes filter, note rows run together)

### Under the hood

- Notes list rows match the nrow block

## v0.2.0 — 2026-11-13

### Features

- Phone rules for the redesign; DESIGN.md catalogues the 0.2.0 components
- Today head with Focus and Start pill; to-do rows quieter; @agent chips; Notes as one list
- Fixed sidebar (titles first, "unnamed" stubs) and the Epics table with Jira ↗ chips
- Series page and Archive (migration 011); Meetings list is one row per series
- Relations both ways (meeting chip, note Meeting, Same name, linked via/topic chips, [[ tags, dead [[KEY]])
- One page template on epic, meeting, topic, tag hub and note pages
- Page-template components (.pghead, .addprop, .pmenu, .refs, @mention chip); Linked references grouped by source
- One Linked references panel (mentions, #tags, notes on lines); renames carry [[links]]

### Fixes

- Final-gate findings (archive back path, start un-archives, exact [[X]] match, URL-only to-dos, docs)
- Agent to-dos carry their @chip; Epics Impact cell no longer runs into Last log
- Mac app pages mark themselves (UA) so the narrow-window menu button clears the window buttons

### Under the hood

- Series_archive table; [[KEY]] links only an existing epic
- V0.2.0 release screenshots (demo DB)
- Simplify pass over 0.2.0 (one [[X]] resolver, one Copy link item, one popover recipe)
- Note --links reports deadLinks (D13)

## v0.1.13 — 2026-11-13

### Fixes

- Settings switches' sliding highlight keeps the macOS segment shape

### Under the hood

- V0.1.13 release screenshots

## v0.1.12 — 2026-11-13

### Features

- Launch animation for the Mac app (Ink · Liquid Glass · Off); marking an epic done stamps its date

### Under the hood

- README rewritten for open source, with screenshots from a demo DB
- Release pages get Highlights + demo-DB screenshots; release.sh groups Performance / Under the hood

## v0.1.11 — 2026-11-13

### Other

- perf: transcript lines saved in one write; Mac app keeps DB reads off the main thread; recorder retries 5xx

## v0.1.10 — 2026-11-13

### Fixes

- the `by ` date menu works in the follow-up and the To-dos box

## v0.1.9 — 2026-11-13

### Features

- ticking a to-do asks for its follow-up; glass opacity may go below the AA floor

## v0.1.8 — 2026-11-13

### Other

- perf: recording captures 1 screen frame per 10 s, reads Others audio in chunks at Stop

## v0.1.7 — 2026-11-12

### Features

- agents can draw an Excalidraw drawing into their notes

## v0.1.6 — 2026-11-12

### Fixes

- a hidden menu stays hidden (popover series menu was always open)

### Other

- Merge pull request #7 from erumtw/feat/worklog
- refactor: one line-row module, entry_row.html + plan_row.html the only row markup
- refactor: one meeting-state module behind recording::MeetingState

## v0.1.5 — 2026-11-12

### Fixes

- line note button draws in the Mac app and is findable

### Other

- Merge pull request #6 from erumtw/feat/worklog

## v0.1.4 — 2026-11-12

### Features

- motion, To-dos Due|Tag switch, Tag view and dropdown fixes (0.1.4)

### Other

- Merge pull request #5 from erumtw/feat/worklog

## v0.1.3 — 2026-11-12

### Fixes

- our own suggestion menu instead of the browser's datalist dropdown

### Other

- Merge pull request #4 from erumtw/feat/worklog

## v0.1.2 — 2026-11-12

### Features

- popover picks a meeting series from a menu, not pills
- Today groups lines by type too

### Other

- Merge pull request #3 from erumtw/feat/worklog
- merge: main (v0.1.1 tag) back into feat/worklog
- wip: T91 UX review on real data: line rows, wrapping titles, sidebar clips

## v0.1.1 — 2026-11-12

### Features

- a meeting's notes are a real note (T90, migration 010)
- rename a meeting from its title (T92)
- Names list so transcripts and /meeting get people right
- atharu edit lets an agent fix lines it wrote
- screenshots into a meeting from the banner or popover (T89)

### Fixes

- agent badge and epic key chips never clip
- long meeting / tag names end in … instead of breaking rows
- popover clips a long meeting name instead of running under Stop

### Other

- Merge pull request #2 from erumtw/feat/worklog
- merge: T56 speaker split for Others (0.1.1)
- wip: T56 speaker split for "Others"
- test: popover meeting link carries its full name as title

## v0.1.0 — 2026-11-12

### Features

- End meeting leaves a link to the meeting it ended
- meetings list shows each meeting's time and transcript size
- atharu meeting + /meeting writes notes from a transcript (T54)
- recording time beside the menu-bar icon, Record audio on popover start
- install prints build time per step and total
- menu-bar icon shows meeting / recording, popover can record
- quick note by typing n + space or Cmd+N in the popover (T87, D55)
- meeting recording + local transcription (T53)
- quick note in the menu-bar popover (T86)
- Mac app shares the main database (T85)
- line notes, the note is the detail page (T84)
- menu-bar quick popover (T81)
- floating capture bar (T78)
- weeks show Sunday to Saturday in calendar and date picker (T83)
- by-date menu, soft date hint (T80)
- version in Settings (T82)
- shared date picker (T75)
- tags step 2 (T73)
- install and update follow release tags (T77)
- atharu help, install refuses a dirty tree (T76)
- tags step 1 (T72)
- Excalidraw drawings in notes (T42)
- sidebar icons, text-box blur setting (T33)
- atharu install/update by CLI
- phase 7 — Mac app, settings and nav redesign, components, Liquid Glass
- phase 6 — Jira titles, CLI note/evidence, looks, deletes, TypeScript, responsive, glass
- phase 5 — calendar with Google iCal read-only
- phase 4 — notes with Tiptap, LLM layer and agent CLI
- phase 3 — learn pages with depth ratings; rename to atharu
- phase 2 — to-dos, progress flags, meetings; rename to athar
- phase 1 — scaffold, Docker, domain parser, today/epics/report slices

### Fixes

- Thai transcripts: drop whisper's made-up labels and the mic's echo
- ask for Screen Recording when Record audio is switched on
- recording banner buttons no longer flash a spinner every second
- popover to-do text lost the transcript box's border
- main window follows the popover; recording time in red
- sign Atharu.app with a stable local identity so recording grants stick
- hide the mic's echo of the call in transcripts
- glass opacity drag to the far left saves in the Mac app
- menu-bar mark follows the real Dry brush logo
- a #tag on a note's title line tags the note (D54)
- quick popover follows appearance settings, spacing
- Today's log refreshes after a to-do edit or delete in This week
- tags verify, [[X]] lands on the hub, learn phone overflow (T74)
- to-do moves to its new group after a due date, pin or state change
- drawing parity, live blur preview (T43)
- to-do check is an Apple-style circle with an accent tick
- 8px gap between to-do rows with date, Pin and Drop
- usage line lists version and update
- design parity (T31 audit)

### Docs

- agents read DESIGN.md before any UI change
- DESIGN.md meeting recording section (T51)
- spec adds planning, progress flags, meetings, notes · 4 phases
- worklog design spec

### Other

- Merge pull request #1 from erumtw/feat/worklog
- test: popover to-do rows are .qtx now
- perf: install --dev reuses its build dir and skips fat LTO
- test: Today log refresher also listens to plans-changed
- wip: T84 line notes design
- wip: T81 quick popover design
- wip: T78 floating capture bar design
- wip: T75 date picker design
- wip: T70 tags design
- wip: T51 meeting recording design
- wip: T41 drawing design
