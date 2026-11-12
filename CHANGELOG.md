# Changelog

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
