# Atharu design system

Three sources, one rule each:

| Source | What we take | What we leave |
|---|---|---|
| **Apple** ([DESIGN.md](https://getdesign.md/apple/design-md)) | Look: SF type, near-black ink, one blue, pill buttons, quiet surfaces | Marketing-page hero scale, product photography |
| **Apple Liquid Glass** ([HIG](https://developer.apple.com/documentation/technologyoverviews/liquid-glass)) | Material: glass **only on floating chrome**, content on an opaque sheet | Glass on content, glass-on-glass stacks |
| **Notion** | UX: sidebar page list, breadcrumbs, everything is a page with a big title + inline properties, lines behave like blocks, to-dos, toggles, `/` menu, `[[` links | Drag-drop, custom DB views, multi-column layouts |

Source of truth for values is `static/css/` (this file explains it; when they differ, the CSS wins and this file gets fixed).
Approved wireframes: canvas https://claude.ai/artifact/SJY9VrvtZb5r6iZzRzLjSZ, HTML in `docs/design/boards/`.

## 1. Colour (`tokens.css`)

| Token | Value | Use |
|---|---|---|
| `--ink` | `#1d1d1f` | text, blocker key, overdue flag |
| `--ink-2` | `#333333` | secondary text, chips |
| `--muted` | `#6e6e73` (`#5e5e63` on a look, `#48484d` tinted) | meta, labels, breadcrumbs |
| `--accent` | `#0066cc` (`#0055b3` tinted) | **the only colour**: links, primary button, impact key, focus, counts |
| `--line` | `#e0e0e0` | borders, table head rule |
| `--fill` / `--fill-2` | `#f5f5f7` / `#f0f0f0` | sidebar, callouts / chips, key squares |
| `--err` | `#b3261e` | errors, late due dates only |

No second accent. State is shown with ink weight, not colour: stale = grey fill, at risk = ink outline, overdue = solid ink.

## 2. Type (`layout.css`, `base.css`)

- Stack: `system-ui, -apple-system, BlinkMacSystemFont, 'Inter', sans-serif` (SF on Apple, Inter elsewhere). Mono: `ui-monospace, 'SF Mono', Menlo`.
- Body 15px / 1.47, letter-spacing −0.2px (Apple's tight tracking).

| Role | Size / weight |
|---|---|
| Page title `.ptitle` | 40 / 600, −0.5px (30 on phone) |
| h3 section | 20 / 600 |
| h4 group | 15 / 600 |
| Body | 15 / 400 |
| Small, meta `.small` | 13 |
| Labels, chips, table head | 12 / 500–600 |

## 3. Shape and space

- Radii: page sheet + sidebar **22**, cards + callouts **12**, rows + inputs **8**, chips + key squares **6**, buttons + flags + capture bar **9999** (pill).
- Doc column: max 860px (`.doc.wide` 1080), padding 24/48/72; gap 14 between blocks.
- Hit targets ≥ 32px desktop, 44px rail/phone.

## 4. Liquid Glass (`looks.css`)

Glass surfaces, and nothing else: **sidebar `.side`, sticky breadcrumb `.crumb`, floating capture bar `.newline`, menus/popovers `.menu` `.acts`**.

- Recipe: `backdrop-filter: blur(20–24px) saturate(180%)`, white 70% edge, `inset 0 1px 0 #fff` top highlight, soft 8–32px shadow.
- One alpha drives all of them: `--glass-a` (inline on `<html>`, clamped per look by `domain::look::glass_alpha` to keep AA). Each surface adds a fixed step `--s`: crumb 0, sidebar +5%, capture bar +10%, menus +25%.
- The page sheet `.pg` is opaque white. Opt-in `data-glass=all` makes it glass too (−5%), with cards/callouts +10% on top.
- `data-tint=tinted`: the look's colour mixed into the glass, darker edges, full-ink chrome text, darker muted/links (measured AA).
- **Reduced transparency → every glass surface opaque, mesh becomes one flat colour.** Not optional.

## 5. Looks (wallpaper)

`/settings` picks one; it sets a fixed mesh behind every page so the glass has something to blur (also favicon, Mac Dock icon).

| Look | Base |
|---|---|
| Sumi (default) | washi `#f6f1e6` |
| Sora | blue `#63b6ff` mesh |
| Wakaba | green `#94f7b6` mesh |
| Glass | deep blue `#2466ea` gradient |

Coloured meshes sit under a 45% white veil; sidebar text over the worst spot stays ≥ 4.6:1.

## 6. Components (`components.css`)

| Component | Class | Notes |
|---|---|---|
| Primary button | `.btn` | blue pill, white text, 36px. One per view. |
| Ghost button | `.btn.ghost` | transparent, blue text |
| Utility button / input | `.util` | 28px, 8px radius, line border |
| Line kind square | `.k` | 22px mono square: `!` `x` `+` `?` `>`; impact = blue (`.k.imp`), blocker = ink (`.k.bl`) |
| Chip | `.chip`, `.chip.key` | epic keys, topics; `.chip.llm` = ink outline badge for agent-written rows |
| Link chip | `.lchip` | source link on a line |
| Flag | `.flag.stale/.risk/.over` | pill; ink weight, no colour |
| Callout | `.callout`, `.callout.dark` | grey (info) / near-black (meeting banner) |
| Card | `.card` | white, line border, 12 radius |
| Properties | `.props` | Notion inline `dt/dd` grid, hover = editable, autosave on change |
| Block row | `.blk` | log line; hover reveals delete `.x` (always visible on touch) |
| Segmented | `.seg` | pill track, `rgba(120,120,128,.12)` (iOS) |
| Capture bar | `.newline` | sticky bottom pill, glass, `/`-free: types the line grammar |
| Due | `.due`, `.due.late` | muted / red bold |
| Calendar | `.cal`, `.ev`, `.ev.meet` | meetings = blue left rule on light-blue fill |

## 7. UX patterns (from Notion)

1. **Sidebar** = page list (Today · To-dos · Calendar · Epics · Report · Meetings · Learn · Notes) + active epics; breadcrumbs on top.
2. **Every thing is a page**: epic, meeting, topic, note → big title + inline properties, autosaved. No Save buttons anywhere.
3. **One input**: the floating "Type a line…" bar takes the line grammar; a live hint underneath says what it will save as.
4. **Feedback**: toast above the bar, "Saved as impact on OPS-101 · Undo", `role=status`.
5. **Toggles** fold secondary things (open plans, report charts, raw numbers).
6. **Notes**: Tiptap; type-to-format markdown shortcuts, `/` block menu (glass popover), `[[` page picker.
7. **Empty states name the next action** ("No epics. Start a line with a key, e.g. OPS-101."). No tutorials: the UI must explain itself.

## 8. Responsive (`responsive.css`)

| Width | Layout |
|---|---|
| ≥ 1024 | sidebar 220–248px + sheet |
| 640–1023 | 64px icon rail (labels become tooltips) |
| < 640 | sheet full width with top-rounded corners; nav in a popover; inputs ≥ 16px (no iOS zoom) |

## 9. Rules

- Glass on chrome, never on content (unless the user opts into `data-glass=all`).
- One blue. A new colour needs a reason written here.
- Every text/surface pair passes WCAG AA, measured over the worst wallpaper spot; record the number in a CSS comment.
- Honour `prefers-reduced-transparency` and `prefers-contrast`.
- New tokens go in `tokens.css`; components in `components.css`. Inline `style=` in templates only for one-off layout (gap, margin), never colour or type.
