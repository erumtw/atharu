# Design sources

Canvas "Atharu Design": https://claude.ai/artifact/SJY9VrvtZb5r6iZzRzLjSZ (rules in `DESIGN.md`; Apple look + Liquid Glass
chrome, Notion UX patterns).

`boards/` is a copy of the canvas's `project/` folder (synced at canvas version 33; page boards' CSS re-synced to the T30 glass):

- `canvas.json` = the canvas index: each board's frame (`x`, `y`, `w`, `h`, `title`), board `order`, and the title / sticky
  notes. Edit it only through the canvas, or keep every key you don't change.
- `*.dc.html` = one board each, a self-contained Design Component page: `<helmet><style>` for CSS, markup inside
  `<x-dc>`, a `data-props` `$preview` with the board size. No scripts, every attribute quoted.
- Page and phone boards (Today, To-dos, Calendar, Calendar.Month, Meetings, Epics, Epic, Report, Learn, Topic, Notes,
  Settings, States, Mobile.*) are generated from the running app with made-up sample data: the served HTML with
  scripts dropped and `ui.svg` icons inlined, plus `static/css/*.css` verbatim with `html` / `:root` scoped to a
  `.dcroot` wrapper. A board gets no real viewport, so desktop boards pin the ≥1024 rules and phone boards (390×844) pin
  the <640 rules, drawing an open popover (drawer, ⋯ sheet) with an `.open` class. States tiles are the app's real
  responses for each state (422s, toasts, empty pages).
- Hand-drawn: Flows, Sitemap, Controls, Components.* (the T28 component sheet; a T30 block at the end of each brings it to
  Liquid Glass). LiquidGlass = every glass surface in Clear / Tinted, states, RT / IC, built from `static/css` verbatim.
- Drawing.* + Mobile.Drawing* (T41): `static/css` verbatim (Reduce Transparency forced on Drawing.RT) + the proposed drawing CSS
  in each board's `<helmet>`, the Notes chrome, and a real Excalidraw 0.18.1 `exportToSvg` of a made-up scene; Excalidraw's UI is
  redrawn in HTML (themed to `--accent`).

Source of truth is `templates/` + `static/css/`; when a board and the app differ, the app wins and the board is
regenerated. `boards/app.css` is the original wireframe stylesheet that `tokens.css` was transcribed from; no board uses it now.
