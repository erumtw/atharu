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
| `--sep` / `--off` | `#e3e3e8` / `#d1d1d6` | grouped-list hairlines + grey pills / switch and slider track off |
| `--hatch-a` / `--hatch-b` | `#c7c7cc` / `#e5e5ea` | hatched slider range below the look's AA floor; `--hatch-a` = switch hover |
| `--accent-2` | `#0055b3` | primary hover/pressed, switch-on hover, the tinted `--accent` (white on it 7.1:1) |
| `--accent-wash` | `#e8f1fb` | meeting events, selected menu option |
| `--accent-soft` | `#4d8fd1` | bars, "done" OKR segment (never under text) |
| `--accent-on-dark` | `#2997ff` | link, dot, End button on `--dark` (4.9:1, so `--dark` stays opaque) |
| `--rec-on-dark` | `#ff5c52` | the recording dot on `--dark` only (4.90:1; `--err` is 2.28 there). Never text, never on white: the word "Recording" carries the state |
| `--heat-1` / `--heat-2` / `--heat-4` | `#e3effc` / `#bcd6f2` / `#003d7a` | depth heat steps 1, 2, 4 (3 = `--accent`) |
| `--dark` / `--on-dark-muted` | `#272729` / `#cccccc` | meeting banner, impact callout / muted text on it (9.3:1) |
| `--ok` | `#18753a` | a done-category status in the Jira move menu only (5.06:1 on `--fill-2`) |
| `--fill-3` / `--now` | `#fafafc` / `#f5f9fe` | table hover, calendar heads, toast / today's calendar column |
| `--faint` | `#b0b0b5` | dashed rules (inline edit, own events), the "other" OKR segment; never text |
| `--ctl-edge` | `#8e8e93` | unchecked control rings, e.g. the to-do check (3.3:1 on white, the 3:1 floor for non-text UI) |
| `--tick` | SVG mask (`url(data:…)`) | the check's tick, drawn as a mask so it takes the fill colour |
| `--chrome-bg` | `#eef0f5` | behind the sheet when no look is set |
| `--muted-look` / `--muted-tint` / `--muted-hc` | `#5e5e63` / `#48484d` / `#3a3a3c` | `--muted` on a look / with Tinted glass / under Increase Contrast |
| `--err-tint` | `#a32019` | `--err` with Tinted glass (`#b3261e` falls to 4.48:1 on the Glass look's tinted sheet) |

**Role tokens (0.2.9, the CUBE token layer).** Every colour a component uses is a token named by its role, so a look is one block of values and no
component restates a colour. Light values are the exact colours the components hard-coded before (proved: computed colours of every element
on 22 pages in 10 look variants identical before and after).

| Token | Light | Role |
|---|---|---|
| `--sheet` | `#fff` | the page sheet `.pg`, the drawing sheet, the preview tile's sheet |
| `--surface` | `#fff` | raised content: cards, controls, pills, rings, calendar grid, the glass base `--glass-c` |
| `--on-accent` / `--on-ink` / `--on-dark` | `#fff` | text, ticks and strokes on `--accent`·`--rec`·`--heat-4` / on an `--ink` fill / on `--dark` (and fills-by-alpha on it) |
| `--paper` | `#fff` | drawing paper: scenes are drawn dark ink on white, so it stays white in every look |
| `--seg-on` / `--thumb` | `#fff` | the selected segment's thumb / switch and slider knobs (light in every look) |
| `--hi` | `#fff` | highlights, with an alpha: the lit top edge of glass, hover lifts on glass |
| `--rim` | `rgba(255,255,255,.7)` | the glass edge (`--glass-edge`) |
| `--shadow-c` | `#000` | shadows, with an alpha |
| `--tint-c` | `#000` | hover / pressed / hairline overlays, with an alpha: darkens a light surface, lightens a dark one |
| `--overlay` · `--wall-veil` · `--chrome-glow` · `--track` · `--rec` | `#14141e` · `rgba(255,255,255,.45)` · `#0066cc` · `rgba(120,120,128,.12)` · `#ff3b30` | phone-sheet scrim · the wash between wallpaper and glass · the glows behind the sheet with no look · segmented track · microphone on |

A colour with an alpha is written `color-mix(in srgb, var(--hi) 60%, transparent)`, never `rgba()`. **`scripts/color-lint.sh` (run first by
`scripts/visual-check.sh`) fails on any `#hex`, `rgb()/rgba()/hsl()`, `white` or `black` in `static/css` outside `tokens.css` and the per-look
blocks of `looks.css` (between `color-lint:off` / `color-lint:on`; those hold a look's own wallpaper and base colours).** Exempt by nature:
SVG icons (`currentColor`) and data colours that come from the user's calendars (`style="background: {{ c.color }}"` on the calendar dot).

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

Glass surfaces, and nothing else: **sidebar `.side`, sticky breadcrumb `.crumb`, floating capture bar `.newline`, menus/popovers `.menu` `.acts`**,
plus the controls that sit on them or float like them (canvas "Controls", T28): glass pills `.btn.ghost`, fields `.fld`, day nav `.dnav`,
text boxes `.ta`/`.note-md`. On the solid sheet (Glass everywhere off) those controls render white with a `--line` border.
One more: the **launch sheet** (`.launch .plate`, §6 "Launch animation"), a full-window pane over the wallpaper that clears before any content shows (rim + light, no shadow, no text on it).

Text boxes come two ways (D25, Settings › Appearance › **Blur text boxes**): **off** (default) = tint only, no backdrop filter;
**on** (`<html data-blur=text>`) = they take `--lg-filter` like the chrome. AA with it on, each look at its floor over its darkest
wallpaper: ink ≥ 13.61, placeholder ≥ 7.35 (Glass Tinted; T33). Reduce Transparency drops the blur either way.

- Recipe (T30, D22; canvas "Liquid Glass"): Apple's three layers, all plain CSS so WebKit gets the whole look. Tokens in `looks.css`:
  - **Material**: `--lg-filter` = `blur(var(--lg-blur)) saturate(var(--lg-sat)) brightness(1.04)`. **Clear** (default) = 4px / 140%, so what is
    behind stays readable; **Tinted** (`data-tint=tinted`) = Apple's Regular: 12px / 180%, look-tinted glass, darker text set. The Settings
    Clear | Tinted switch is the only variant control.
  - **Highlight**: `--lg-rim`, a gradient in a transparent border (bright top-left and bottom-right), not a flat white line.
  - **Illumination**: `--lg-light`, inset glow + light lower lip; the selected sidebar row is lit from inside.
  - **Shadow**: `--lg-shadow`, contact + soft 24px; the floating capture bar alone takes `--lg-float` (contact + 32px .16).
- **Floating layers** get all three: sidebar `.side`, capture bar `.newline` (a sticky capsule over the sheet, §6 "Floating capture bar"; its hint and toast pills take the same three layers at the menus' step), menus `.menu`, the ⋯ sheet `.acts:popover-open`, and the
  Glass-everywhere sheet (rim + light, no shadow). The crumb is a bar flush with the sheet: material + lit top edge only.
- **Never glass on glass**: pills, fields, day nav and text boxes inside any glass layer stay flat glass (no rim, no lens, small shadow).
- **Over content** (crumb, capture bar, menus, ⋯ sheet, phone drawer): text behind a 4px blur is half-sharp, so these never drop below
  `--lg-min` (60%) (the capture bar 70%, the pills and menus above it 85%: they pass over the dark meeting banner) and take `--muted-tint` / `--err-tint`. Placeholders on every glass field are `--muted-tint` (`--muted-look` measures 4.40 there).
- **Press**: glass pills swell to `scale(1.04)` and glow; the blue primary still sinks to `.96`.
- **Lensing** (Chromium only): SVG filter `#lg-lens` (defined once in `templates/base.html`) is prepended to the floating layers'
  `backdrop-filter` under `@supports (-webkit-app-region:drag)`. WebKit (Safari, the Mac app's WKWebView) accepts `url()` in
  `backdrop-filter` and then drops the **whole** filter, blur included (measured T30), so it must never see it; `-webkit-app-region` parses
  only in Chromium. Off under Reduce Transparency.
- AA at each look's floor (T30): worst 4.70 (Wakaba Tinted, error text over content); sidebar labels ≥ 5.61 Clear, ≥ 6.78 Tinted.
- One alpha drives all of them: `--glass-a` (inline on `<html>`, the slider value as set: each look's AA floor is a marked baseline the owner may go below, 2026-11-13). Each surface adds a fixed step `--s`: crumb 0, sidebar +5%, capture bar +10% (floor 70%), its hint/toast pills +25% (floor 85%), glass pills +10% (hover +25%, pressed +35%), fields and text boxes +12%, menus +25%.
- The page sheet `.pg` is opaque white. Opt-in `data-glass=all` makes it glass too (−5%), with cards/callouts +10% on top.
- `data-tint=tinted`: the look's colour mixed into the glass, darker edges, full-ink chrome text, darker muted/links (measured AA).
- **Reduced transparency → every glass surface opaque, mesh becomes one flat colour, no rim or lens, hairline border back.** Not optional.
- Mac app (`<html data-app="mac">`, set by the shell only): no title bar; the traffic lights float over the sidebar's top-left (row above the brand; rail and phone crumb make the same room), and the brand row + `.crumb` are window drag regions. The browser view is unchanged.

## 5. Looks (wallpaper)

`/settings` picks one; it sets a fixed mesh behind every page so the glass has something to blur (also favicon, Mac Dock icon).

| Look | Base |
|---|---|
| Sumi (default) | washi `#f6f1e6` |
| Sora | blue `#63b6ff` mesh |
| Wakaba | green `#94f7b6` mesh |
| Glass | deep blue `#2466ea` gradient |
| Yoru (夜, 0.2.9, dark) | night `#0d1222` mesh: indigo `#1f2c6e`, teal `#0f4a5a`, violet `#33285e`, under a 35 % black veil |
| Auto (自動, 0.2.9) | Sumi by day, Yoru by night: follows macOS |

Coloured meshes sit under a 45% white veil (`--wall-veil`); sidebar text over the worst spot stays ≥ 4.6:1.
Each look also carries `--bloom` (looks.css), the glow behind the icon in the launch animation: Sumi `#e9d9b6`, Sora `#bfe6ff`, Wakaba `#d6ffe2`, Glass `#cfe6ff`, Yoru `#cfd8ff`.

**Auto (0.2.9)** has no values of its own: a head script (`base.html`, `/quick`, `/launch`) resolves `data-look="auto"` to `sumi` or `yoru` before first paint and again
whenever the system flips (`matchMedia('(prefers-color-scheme: dark)')`; `data-pref="auto"` keeps the choice), so every rule stays one source per look; without JS the page is
Sumi (`looks.css` lists `auto` with Sumi). Favicon: two `<link rel="icon">` with `media="(prefers-color-scheme: …)"` (sumi.svg / yoru.svg); the picker shows `auto.svg`
(Sumi's day half, Yoru's night half). In Atharu.app the window and popover follow the system (`set_theme(None)`), and the Dock icon swaps with `ThemeChanged`. The glass floor is
the higher of the two (38 %). The Look picker is an auto-fit grid (84 px minimum), six looks in a row on desktop.

**Yoru (0.2.9)** is the one dark look and it is ONE token block (`html[data-look=yoru]` in looks.css, `color-scheme: dark`): ink `#ececf1`, sheet `#15181f`, surface `#1c2030`,
muted `#a3a6b4`, accent `#5aa2ff`, with the role tokens of §1 (`--on-accent`/`--on-ink` become the dark `#0d1222`, `--seg-on` the elevated grey `#3a3f4d`, never white;
`--hi` and `--tint-c` flip to white alphas). Every pair is measured in a comment above the block (ink 15.08, muted 7.33, accent 6.81, err 7.03, ring 4.52; text on
the glass over the brightest veiled wallpaper stop ≥ 5.37 even at 0 %). Dark glass is the Apple way, not a grey slab: a near-black tint `#05070d` at a low alpha so
the mesh shows, saturate 185 %, brightness .92, a thin bright top edge (`--lg-light` 22 %), a deeper shadow; the floating layers keep `--lg-min` 65 % (a half-covered ink pixel
behind them: muted 5.28, accent 4.90). Glass floor 38 % (the board's Clear; the whole slider is AA over the wallpaper). Reduce Transparency → flat `--base`;
Increase Contrast → ink `#fff`, lighter lines. The one component exception: `.callout.dark` gets a `--rim` edge (a dark banner on a dark sheet). Icons: `static/icon/yoru.svg`
(flat `#0d1222`, ensō `#f1ede4`) and `src-tauri/icons/looks/yoru.png` (night mesh, moon ensō, glass rim). Each look has its own glass-opacity default (`Look.glass_default`): the light looks 60 %, Yoru and Auto 38 %, so the night mesh shows through the near-black glass; a stored slider value always wins.

## 6. Components (`components.css`)

### Control rules (canvas "Controls" + "Components · every state", T28)

- **One height per context**: 32 in chrome (`.crumb`), 36 in the page, 44 on phone (< 640). An action inside a field is one step smaller than the field: an action inside a 36 field is 28 (a 44 field holds 36). Labels are centred with flex + `line-height:1`, never padding guesswork.
- **One blue primary** per view (`.btn`): Log ↵, Add ↵, Copy as markdown. Everything else is a **glass pill** (`.btn.ghost`); `.s` = the 32 size for row actions (Pin, Drop, Save, Delete).
- **Single-line fields are glass pills with their action inside** on the right (`.fld`): meeting start, search, add evidence; the capture bar `.newline` is the 52 px version holding the primary. Inputs that stand alone (dates, event form) take `.fld` on the input itself.
- **Day nav** = one glass capsule `.dnav`: Today | ‹ | date ▾ | ›. The date is a native `<input type=date>` laid over its label; a click opens the Atharu date popover (`.dp`, below), never the browser's picker; without JS the native picker still works. Today shows only when not on today. Calendar uses the capsule for ‹ › only, with Today as a pill before it and the range as the page title (0.2.3).
- **States**, every control: default · `:hover` · `:focus-visible` (2px accent ring) · `:active` (pressed: primary darker + `scale(.96)`, glass pill swells `scale(1.04)` + glow) · selected (`.on` / `aria-pressed` / `:checked`) · `:disabled` (opacity .45, `not-allowed`) · error (`.bad` = red ring, input kept, message under it) · loading (`.htmx-request` on the submitting form only, never a polling container: its submit dims and shows a spinner; a to-do label dims) · empty (placeholder in `--muted-tint`).
- Datalist inputs hide Chrome's ▼ (`input[list]`); suggestions still open as you type. Legend chips sit on one baseline and wrap as whole items.
- Reduce Transparency: same shapes, solid white, no rim. Increase Contrast: ink borders, no rim, `--muted-hc`.

### Catalogue

| Component | Class | Notes |
|---|---|---|
| Primary button | `.btn` | blue pill, white text, 36 (32 in chrome, 44 phone); hover/pressed `--accent-2`. One per view. |
| Glass secondary | `.btn.ghost` | glass pill, ink text; `.s` = 32; `.dng` = destructive (err text). On `.callout.dark`: `--accent-on-dark` outline. |
| Icon button | `.ib` | 32 round (44, 12 radius on phone); always an `aria-label`; `.ib.more` opens a row's `.acts` |
| Link button | `.linkbtn` | POST form that reads as a link: Undo, re-open; underline on hover |
| Row actions | `.acts` | the row's buttons inline on desktop, shown on row hover / keyboard focus (their room kept, so nothing shifts); a bottom action sheet (popover) on phone, 48 px rows |
| Delete × | `.x` | 28, revealed on row hover / focus (always shown on touch) |
| Field | `.fld`, `.fld.m`, `.fld.s` | glass pill 44 / 36 / 32 with the action inside; `.bad` = 422 |
| Capture bar | `.capture` > `.newline` + `.hint` + `.legend` | 52 glass pill, primary inside; `.newline.bad` on a parse error; hint = live parse preview in accent |
| Toast | `.toast` | above the capture bar, `role=status`; `:empty` hides it; `.tick` ✓ in accent |
| Day nav | `.dnav` (`.i` = icon segment) | see rules |
| Text box | `.ta`, `.note-md` | multi-line glass box, 12 radius; autosaves (Save only in `<noscript>`) |
| Inline input | `.pin`, `.pin.dash`, `.pin.bad` | borderless props input/select; `.dash` = dashed "type here" rule (inline edit); `.bad` = red ring; a page title (note, epic) is a one-row `textarea.pin` that grows, so a long title wraps instead of clipping; Enter leaves it (T91) |
| Follow-up | `.blk.follow` (`todos/follow.html`) | under a to-do just ticked done: `↳` in the check's column · the carried epic key + #tag chips · a `.pin.dash` "Next step? Enter adds it · Esc skips" · Add (`.btn.ghost.s`) · Skip (`.linkbtn`). Asks, never blocks (no dialog): the row keeps its place until it's answered, then the list regroups once. Focused on a fine pointer only (no phone keyboard popping on a tick); a refused line stays with `.bad` + `.err` |
| Line edit | `.edit-line` | log text as a link; dotted underline on hover; opens `entry_edit` in place |
| Save status | `.saved` | 12 px muted in the crumb: "Changes save as you type · ✓ saved"; `.err` on failure |
| Error | `.err` | 13 px `--err` text under the field it explains |
| Line kind square | `.k` | 22px mono square: `!` `x` `+` `?` `>`; impact = blue (`.k.imp`), blocker = ink (`.k.bl`) |
| Chip | `.chip`, `.chip.key` | epic keys, topics; `.chip.tag.at` = the agent @mention (below); `.chip.tag` (`.inh`, `.fl`) = see "Tag chip" below. One line, never wider than its container: a long name ends in "…" with the full name in the link's `title` (also crumbs, sidebar series via `.ell`) |
| Epics × Jira | `#assigned` section > `.arow`, `tr.jhead` + `tr.child`, `.ettl`, `.lchip.jc-{new,indeterminate,done}` | 0.2.5: "In Jira, assigned to you · N open" as hairline `.arow` rows: `.jtype` · key · Jira title (ellipsis), then the quiet due, the status ▾ and the action (the chip sits right next to its action, 10 px; Epic → "+ Track as epic", others → "+ To-do" filled `KEY > title link by due`, "✓ tracked"), on fixed grid columns (type 52 · key 76 · title · due 64 · status 172, right-aligned · action 124, left-aligned; 0.2.7) so the status chips share one right edge whatever the type, due or action; on a phone key · status · action on one line, the title under it, two lines at most (D5, D6); the table's title cell `.ettl` keeps title · "from Jira" on one line and the Jira status has its own **Status** column (0.2.8, `.stc`; owner screenshot: the chip sat in a fixed pill beside an empty gap while "Waiting f…" was cut): columns Key 88 · Title (the rest, ≥ 220 px at 1180) · Status 132 (chip ≤ 112, as wide as its words, ellipsis only past that) · To-dos 60 · Impact 96 · Last log 68 · Flag 124; on a phone the Status column is hidden and the title cell carries a copy of the chip (`.m-only`) that yields to the title first; stories under a `--fill-3` header row for an untracked Jira parent ("Jira epic · not tracked" + Track), indented 28 px (D4); no typed title → Jira's title + muted "from Jira" (D7); the chip's look follows the status category, never its name (new `--ink-2`, in progress accent, done muted + ✓); flags "due dates differ" / "Jira moved, no log" only when no flag of ours speaks; a dead token (401 only, and only while Jira is set up) is one `.err` line with Make a new one ↗ (D9). "+ Track as epic" goes by Jira's epic level (`hierarchyLevel` 1), the type name "Epic" only when the level is unknown (D16); a parent tracked here in any group or status is never a "not tracked" header. Today shows one line "N Jira tickets assigned to you →" |
| Epics view row | `.row.epview` > `form.fld.s` + `form.seg` ×2 (Today's control row, §6 "Logged today": same classes, no new token) | 0.2.7, under the page head, for both lists on `/epics`: a **Filter epics** box (200, `type=search`; a case-insensitive substring of key, title, Jira title and tags; assigned rows: key + Jira title), **Group by** OKR · Status · Parent · Tag · None, **Show** Active · Done & dropped · All. All state is the URL (`?q=&group=&show=&hide_tracked=1`, plain GET forms, each carrying the other params), so it works without JS and survives a reload. Groups are `details open` with "· N" ("· N of M" while filtering): OKR is the page as it was (with the D4 parent header rows); Status = Jira status, in progress first, then not started, then done, then "Not in Jira"; Parent = "title · KEY" then "No parent"; Tag = one group per tag (an epic with two appears in both) then "No tag"; None = one table, no header. Show = Done & dropped puts those epics in the table as rows with their `done 20 Oct` / `dropped` chip (the folded "Done & dropped" list only exists under Active). "In Jira, assigned to you" takes the same filter, is grouped by Jira status (by Jira parent when Group by = Parent), has a **Hide tracked** `.sw` switch in its summary, and starts folded when it holds more than 10 rows (open while filtering). The phone stacks the row: filter, Group by, Show, each `.seg` scrolling inside itself. Empty views say why: "No epics match ‘q’. Clear the filter" (the link keeps Group by / Show), "No assigned ticket matches ‘q’.", "Nothing done or dropped yet.", "Everything assigned to you is already tracked."; the live filter also reads the folded Done & dropped list and keeps the Unnamed count true; the table header says "To-dos" (title "Open to-dos") so it never runs into Impact; assigned parent groups are keyed and sorted by the parent's key |
| Epic page × Jira | `.callout.jdone`, Tickets `details.tg`, `.jsug` in `#jtoast` | 0.2.5: while Jira says done and the epic is active, "Jira says KEY is Done on <date>" + Mark done (Jira's resolution date becomes done on) / Not yet (hidden until that date changes) (D2); "Tickets · N in Jira · M logged here" lists Jira children (type, key → its page when tracked else Jira, title, status ▾, "N lines" / "not logged") (D3); a story's head is one line "Story · KEY · status ▾ · under PARENT title" (its page, or Jira ↗; `.under` gives way first); the banner names the key in bold text, not a key chip; no typed title → Jira's title in the head and crumb (D7). Right after Atharu marks an epic or a Jira-linked to-do done / dropped, `#jtoast` asks "Also move KEY in Jira?" with the matching moves only (done → done-category, not cancel; dropped → Won't do / Cancelled …) + Skip; no match → no prompt (D2, D10) |
| Jira status menu | `.lchip.st` (one pill "In review ▾", a button) + `.menu.jmenu` popover (`partials/jira_chip.html` → `partials/jira_menu.html`, `jira/moves.html`); a to-do's link chip keeps `.jchip` = ↗ link + `.jmv` ▾ | 0.2.5 D1/D8 (board Redesign.Jira.Status): every Jira status chip (Epics, epic page, tickets, assigned) is one pill that opens the menu (its category class sits on the `.jst-KEY` status span, the pill follows via `:has`); to-do link chips get a ▾. Each open re-reads Jira (one call: the issue's live status + its allowed moves; the cache takes that status). The menu: header "Move KEY in Jira", the issue's allowed moves read live, each a small status pill coloured by category (to do `--ink-2`, in progress accent, done `--ok`) + the transition's name when it differs, footer "Only the moves your workflow allows from “X”", last Open in Jira ↗. Nothing is sent to Jira until a pick; a pick is one transition. Without JS the popover still offers Open in Jira. The moves are htmx buttons (not forms) with their own `hx-target`: the menu often sits inside a page's form (the epic head) |
| Jira move toast | `#jtoast` > `.jtoast-in` (`jira/toast.html`) | fixed under the crumb on every page, 5 s then fades (`steps` under Reduce Motion): "Moved KEY to Done in Jira · Undo" (Undo = the move back to Jira's live status before the move, by a transition needing no field when there is one, else "Can't undo in Jira: open KEY ↗"); a move that needs fields hands over ("Jira needs Resolution for “Done”. Open in Jira ↗"), a refusal with only Jira's sentence shows it ("Jira says: …"); a 401 names the dead token with Make a new one ↗ (D9), a 403 says the account may not move it. The page's chips of that key take the new status span whole (text + category) out-of-band (`.jst-KEY`) |
| Quiet due date | `.qdue` (`partials/plan_row.html`, `Plan::due_quiet`) | every open to-do's date (owner 2026-11-14): on a desktop **right after the title and its tags**, inline at the end of the (wrapped) text, 6 px off it, no separator character, so it never wraps away from the title and the right side keeps only the chips and the note button (0.2.8); on a phone the old meta-line date (`.m-only`; the inline one is `.m-hide`). `Today` (`--accent`) · `Tomorrow` · weekday within 6 days · `Wed 18 Nov` this year · ISO beyond; late in `--err` keeping its date; 13px muted, tabular figures (left-aligned inline on a desktop, right-aligned on the phone's meta line) |
| Link chip | `.lchip` | a link out with ↗: a line's source link (its URL then leaves the row text, the chip carries it), an epic's Jira status → `{base}/browse/KEY`. Same grey pill as `#tag` / `@agent` / key chips (`--fill-2`, no border), blue text + ↗ say "opens outside" (owner 2026-11-14: the white outlined box read as a button). In a line row (`.jchip`, 0.2.8) a Jira link's label is the whole Jira title, so it clips (`.clip`, full label in `title`) at 32 % of the row / 15 rem, the ↗ and ▾ never clip, and the line text keeps ≥ 200 px (≥ 120 on a phone, where the chip rides the meta line at ≤ 100 % − 90 px) |
| Flag | `.flag.stale/.risk/.over` | pill; ink weight, no colour |
| Due | `.due`, `.due.late` | muted / red bold |
| Callout | `.callout`, `.callout.dark` | grey (info) / `--dark` (meeting banner with `.dot`, impact line with `.num` / `.num.up`) |
| Card | `.card` | white, line border, 12 radius |
| Stat tile | `.stat` | 28/600 number over a muted label; glass +10% with Glass everywhere |
| Properties | `.props` | Notion inline `dt/dd` grid, hover = editable, autosave on change |
| Block row | `.blk` | log line, 32 tall; hover reveals delete `.x` |
| Line row | `.blk.ln` | a log line, to-do or decision (Today, To-dos, meeting, epic, tag hub): the text takes the room and wraps; chips + actions keep their width in one right-hand column, top-aligned to the first text line (`--lh` 32, 44 phone), so a wrapped row never squeezes the text or shifts Pin / Drop. A series chip is capped at min(35%, 16rem) with "…" + full name on hover. Phone: check/kind + text + ⋯ on line one, chips wrap under it (T91) |
| To-do check | `.check` + `.done-text` | Apple Reminders circle: 20px (24 phone), 1.5px `--ctl-edge` ring (3.3:1), checked = accent fill + white tick; native checkbox under appearance:none, posts on change; done = struck + muted |
| Segmented | `.seg`, `.seg.mac` | pill track 36 / 30 segments (32/26 in chrome), `rgba(120,120,128,.12)` (iOS); `.mac` = 8-radius macOS control for settings |
| Grouped list | `.sec` + `.grp` > `.gi` | Apple System Settings inset group (`--fill`, hairline inset 16px); Notion bold label + one-line help left, one control right; `.gi.col` stacks |
| Switch | `.sw` | native checkbox `role=switch` (posts without JS), 38×22, 51×31 on phone; pressed stretches the knob |
| Slider | `.sl` > `.rng` + `.val` | native range on the true 0–100 scale; below `--f` hatched (the look's AA floor: a warning, still reachable), `--v` = value |
| Look tile / status | `.lk`, `.pill`, `.pill.ok` | app-icon tile, accent ring when picked (dashed on focus); status pill grey / solid ink (On) |
| Preview tile | `.prev` | the look's wallpaper behind a mini glass sidebar + sheet, same alpha math as the real chrome |
| Launch animation | `form#launch` > `.gi` + `.gi.lprow` > `.launch.mini` | `.seg.mac` (Ink · Liquid Glass · Off, submit buttons, htmx 204) + a 96 px preview of the real launch (`launch.css`, §6 "Launch animation"); the preview row is hidden while Off |
| Disclosure | `.chev`, `details > summary` | settings row value in muted / toggles that fold secondary things (count + hint inline, 400 weight) |
| Meter | `.meter` | depth 0–4 as five 14×6 pills, on = accent |
| Bars | `.bars` | activity / impact bars in `--accent-soft` |
| OKR bar / heat | `.obar` (`.active/.done/.other`), `.heat1`–`.heat4` | report charts: status by weight; depth steps `--heat-*` (from `report::view::heat_class`) |
| Sidebar count / label | `.cnt`, `.side .lbl` | muted count, not a badge / 12/600 section label (a hairline in the rail) |
| Calendar (0.2.3) | `.cgbar`, `.cg` (`.hd`, `.ad`, `.bs` > `.bd`, `.col`, `.tev`, `.nowl`), `.mo` > `.mc`, `.ev`, `.evcard`, `.slot`, `.more`, `.evadd` | Google-Calendar layout in Atharu's look (boards Redesign.Calendar + .Views). Bar: Today · ‹ › (`.dnav`) · range title · `.seg` Day · N days · Week · Month · "+ Event" (the one primary). Hour grid 07–21 (widened to fit, `--px` = one hour), timed items placed by start / end, overlaps side by side (`domain::timegrid`); all-day row = due dates, epic targets, date-only events; today = `--now` column, accent day number, red `.nowl`; the day header opens the Day view, "N lines" opens that day's log. Month cells and all-day cells show 3 items, else 2 + "+N more" (to the Day view). Timed blocks are at least one line tall (26 min ≈ 20 px), so a 5-minute call keeps a readable title; under 40 min a block shows the title only (never a time cut mid-line). A cluster wider than the day fits (columns under ~60 px: 2 in a week, 3 in 5 days, …) shows its first columns + a `.tmore` "+N" that opens the Day view; the Day view itself never folds (0.2.4). The "N days" button says its number ("5 days"). The header rows keep the grid's scrollbar gutter, so columns line up when scrollbars always show. A meeting or event chip opens its card `.evcard` (native popover anchored under the chip, centred without anchor positioning): Open meeting · Open series · Day's log for a meeting; Start meeting (+ Delete on an own event) · Day's log for an event. An empty hour `.slot` is a plain link that opens "+ Event" with that day and hour (dashed accent on hover, `--accent` on `--accent-wash` 4.9:1). N days follow the window (`calendar.ts`: ≥1200 → 7 · ≥900 → 5 · ≥640 → 3 · phone → 1, first load only, D3); the grid opens scrolled to 08:00. Meetings, Google (iCal) and Mac calendar (EventKit, source `mac`, 0.2.4) events = blue left rule on `--accent-wash` (0.2.3 D2); events read from outside are read-only (their card starts a meeting, no Delete); `.ev.todo` outline (`.late` red text), `.ev.tgt` grey, `.ev.own` dashed. No legend, no setup text (iCal lives in Settings › Connections). Weeks Sun → Sat (D48); 7 days is the week |
| Menus | `.menu [role=option]` | glass +25%, 32 px options, selected `--accent-wash`; `/` block menu and `[[` picker |
| Date picker | `.menu.dp` | shared date popover on every `input[type=date]`; glass = `.menu` with an 85% floor; Sunday first (D48); phone = bottom sheet. See "Date picker" below |
| Notes body | `.note-body` | Tiptap content as the board draws it (code on `--fill`, task lists, toggles, `.page` links) |
| Drawing | `.drawing`, `.drawing.empty`, `.drawsheet` + `.drawbar` | see "Drawing block" below |
| Tables | `table th td` | head 12/500 muted on a line rule, rows hairline, hover `--fill-3`; scroll in `.tw`. A long first-column name (series, topic, tag) clips with an ellipsis and carries its full text as the link's `title`, never printing over the next column; a "via <series>" chip on a line clips the same way inside its row |
| Recording banner | `.callout.dark.recb` > `.dot.rec` + `.rect` + `.lvl` ×2 | meeting banner while recording: pulsing dot, elapsed, one level meter per stream, Stop recording + End meeting |
| Transcript | `.tx` > `.txhead` (+ `.txspk`) + `.txbody` > `.txrow` | white 12-radius box (content, no glass); search `.fld.s` + Copy in the head; `.txspk` names split voices |
| Meeting notes | `#notes` > `form#note` > `#editor.note-body` | the Notes editor in place on the meeting page (T90): same blocks, `[[links]]`, #tags, drawings; "saves as you type" + **Open as note**; a meeting without one shows **Start notes** (ghost pill) |
| Meeting title | `.ttl` > `h1.ptitle` > `textarea` | the series name, editable in place: looks like the h1, `--fill-2` on hover/focus + accent ring; Enter or leaving saves (renames every meeting in the series + its tags), Esc restores; refusal in `#ttl-err` (T92) |
| Suggest menu | `.menu.sugm` (suggest.ts) | every `list=` input (tag filter, meeting start, series tags, tag merge): a glass menu fixed under the box, exactly its width (220 min), compact 28 px rows like the native list, up to 12, substring filter, ↑↓ then ↵/Tab/click picks (nothing preselected); replaces the unstylable native datalist dropdown. #line and the popover keep their own menus |
| Screenshots | `#shots` > `.shots` > `a` > `img` + caption | meeting page under Notes: 160×100 cover thumbnails (8 radius, `--line` border), `HH:MM` caption, click = full PNG; **Take screenshot** ghost pill (app, meeting running); the camera `.btn.ghost` on the banner and the popover card runs macOS's picker (T89) |
| Progress | `progress.prog` | native `<progress>`, 6 px, `--accent` on `--sep` (4.6:1) |
| Command chip | `.cmd` | a copyable CLI / slash command in a white pill with a Copy button inside |
| Agent @chip | `.chip.tag.at` | who wrote it when it isn't the owner: the `#tag` chip with an `@` (`::before`) instead of the `#` (0.2.0 D1). On lines, to-dos, notes, refs and the note's Written by; replaced the outlined `.chip.llm` badge |
| Page head | `header.pghead` > `.pgk` + `h1.ptitle` + `p.fact` | the one head of every entity page and list (epic, meeting, series, topic, tag, Today, Epics, Meetings): kind glyph or key chip (+ Jira `.lchip`) in `.pgk`, the 40/600 title, then **one read-only fact line** (`2 meetings · last met today`); editing lives in the props |
| Page menu | `.menu.pmenu#pmenu` + `partials/pmenu_btn.html` | the ⋯ at the crumb's right end (`.crumb .pend`, after `.saved`): Copy link · page actions (Open in Jira ↗, Archive, Rename…) · **Delete…** in `--err` last, a form button styled as a menu item (plain POST without JS, Undo toast after). Native popover anchored under the ⋯; fixed fallback where anchor positioning is missing. Replaced the hub's `.acts-menu` and every page-bottom Delete button |
| Add property | `details.addp > summary.addprop` | empty props are hidden (0.2.0 D2); "+ Add property · OKR dates metric" names what's missing and reveals them in place. JS-free: the empty fields stay inside the page's autosave form, so a PATCH still carries them |
| Toggled section | `details.tg > summary` | a page section at h3 size with its count inline (`Open to-dos · 2`, `Timeline · 6 days`, `Evidence · 1`); open by default, closed when it would only say "none" |
| Linked references | `section.refs` (`partials/refs.html`) | last on every entity page: notes that point here, grouped by how (`h4` Mentioned in · On its lines · Tagged), one `.ref` row each with the @chip; empty = one line naming how to link (`[[name]]`). A note's own refs are its lines (`#lnh`, the white `.lnh` box inside) |
| Same name | `partials/same_name.html` | a prop on topic, meeting and series pages linking the tag hub / topic / series of the same name, so "one name, one hub" reads both ways |
| Start meeting row | `#meeting:has(>#start-meeting)` | Today and Meetings (0.2.5, back in the page as before 0.2.0; the 0.2.0 crumb pill + popover hid the series list under its own text): one row = name field (series list below it, help in its `title`) · Record audio switch; the consent line and errors wrap under it. A running meeting's banner swaps in at the same place |
| Log view row | `.row` + tag filter + Group by `.seg` | under "Logged today", always visible (no menu, owner 2026-11-13): tag filter left, Group by Epic · Tag · Type right, the same row as To-dos |
| Epics table | `table.etab` (`epics/row.html`) | fixed layout Key · Title (+ Jira ↗) · Open to-dos · Impact · Last log · Flag, one table per OKR; unnamed stubs last, titled in place; the one `.doc.wide` list besides Calendar |

### Drawing block (canvas row "Drawings · Excalidraw in notes", T41; D23, D24)

- **Stored as** one markdown line in the note body, `![Drawing](data:image/svg+xml;base64,…)`: the exported SVG with the
  Excalidraw scene embedded in it (`exportEmbedScene`), so one block is both the picture and the editable scene; `![Drawing]()`
  = an empty drawing. The server sanitiser (`domain::markdown`) turns only that exact form, alone on its line with a base64
  payload, into the figure; anything else stays an ordinary markdown image.
- **Read view** `.drawing`: a figure in `.note-body`, white, `--line` border, 12 radius, 16 padding; the stored SVG centred,
  `max-width:100%`, `max-height:480px`. It is content, so no glass. The SVG is shown as an `<img>` (no script, no fetches) and works
  without JS. **Edit drawing** = `.btn.ghost.s` top-right, shown on hover / focus-within (always on touch).
- **Selected** (one click; Backspace deletes it like any block): 2px `--accent` outline, offset 2.
- **Empty** `.drawing.empty`: dashed `--faint` border, 160 tall, pencil icon + "Empty drawing" in `--muted` + a **Draw** glass pill.
- **Insert**: `/` menu → **Drawing** (`✎` key tile, hint "opens the canvas"), which opens the editor at once.
- **Editing** `.drawsheet`: Excalidraw full-screen over the note on an opaque white sheet (content, not chrome). Our controls sit in
  one floating glass capsule `.drawbar` top-right, where Excalidraw's Library button was (hidden): `.saved` status + **Cancel**
  (glass pill, discards changes made since opening) + **Done** (the view's one blue primary). 44 tall, buttons 36; phone: full-width
  bar at the top, Cancel left, Done right, 44 buttons. Glass recipe = `.menu` (`--s` 25%, rim + light + shadow, `--lg-min` floor,
  `--muted-tint` / `--err-tint`); Reduce Transparency → white + `--line` hairline.
- **Autosave**: the scene saves into the note 1 s after a change; the bar says "Saving…" → "✓ saved just now" (same copy as the
  crumb) or "Couldn’t save · retrying" in `--err`; Done never waits on the network.
- **Excalidraw is themed, not restyled**: on its container set `--color-primary` `--accent`, `--color-primary-darker` and
  `--color-on-primary-container` `--accent-2`, `--color-primary-light` `--accent-wash`, `--color-selection` `--accent` (the canvas
  reads it), and the UI font to the system stack (so its Assistant webfont is never fetched). Hint text takes `--muted` for AA.
  The colours *inside* a drawing (Excalidraw's stroke palette) are the user's content, not UI, so the one-blue rule doesn't apply there.
- Text in a drawing uses Excalifont (vendored, inlined into the stored SVG at Done), so the read view needs no webfont.

### Meeting recording (canvas row "Meeting recording", T51; D26, D27)

- **Mac app only.** The browser and phone never capture; they show transcripts, and a running recording as "Recording on the Mac"
  (no Stop). Settings › Transcription in the browser is one line saying so.
- **Start**: under the meeting field, a `.sw` **Record audio** switch with a mic icon; **off by default every time** (recording is a
  per-meeting choice). Switched on, the help line becomes the consent reminder in ink, not muted: "Tell people you're recording
  before you start." No speech model yet → switch disabled + "Needs the speech model (574 MB). Download it in Settings".
- **Banner while recording** (`#meeting`, also shown on the meeting page while it runs): `.dot.rec` (the only red; pulses, still under
  `prefers-reduced-motion`) · **Recording** + elapsed in tabular mono · two level meters, **Me** (mic) and **Others** (system audio),
  lit bars `--accent-on-dark`, so a muted mic shows in the first seconds · **Stop recording** (meeting keeps running) · **End meeting**
  (also stops). The word "Recording" carries the state; the dot is decoration.
  Meter bars: unlit `--ctl-edge` (4.6:1 on `--dark`), lit `--accent-on-dark` (4.9:1); the separator is `--ctl-edge` too (T53).
- **Not recording** (Mac app, model ready, meeting running: after Stop, or after Quit & Reopen for a permission): the plain
  banner gets a ghost **Record audio** next to End meeting (T53); the consent reminder is its tooltip.
- **Transcript** section on the meeting page, after Notes: `.tx` with a head (meta in muted: "Live · about 30 s behind · th · N
  lines" / "Recorded 1 h 02 min · 812 lines · large-v3-turbo · th"), search (`GET ?q=`, server filters rows and wraps hits in
  `<mark>` on `--accent-wash`; works without JS) and **Copy** (clipboard; a `.toast` "Copied N lines"). Rows: mono timestamp ·
  **Me** / **Others** (12/600, 84 px column, a longer name cut with its full text in `title`; repeated speaker hidden) · text. Live tail line: "Listening · the next lines arrive about 30 s after
  they're said". The body scrolls in its own 440 px box and follows the bottom while live.
- **Finishing**: after Stop / End, a `role=status` row under the last lines: "Transcribing the last part… about N min left · you can
  close this page, it keeps going" + `progress.prog` (≈ 4–5 min per hour of meeting, plus ≈ 6 min per hour for the speaker
  split).
- **Speakers** (T56; D34, D37): after the tail, the Mac app splits Others by voice (local sherpa-onnx diarization) and the
  Others lines come back as **Speaker 1..N**; live lines say "Others" until then. A done recording with split voices gets a
  `.txspk` row under the head: one muted help line ("Voices on the call, told apart after the meeting. Name one and every line
  follows.") and one `.fld.s` (140 px) per voice holding its name, autosaved on change (no Save; Enter posts the plain form).
  Naming two voices the same merges them (the split cut one person in two); blank / Me / Others are ignored. The name lives in
  each line's `speaker`, per recording; no voice print outlives the pass, so nothing carries to the next meeting.
- **Done**: a grey `.callout` above the transcript: "Ask Claude for notes: run `/meeting` in Claude Code…" + `.cmd` holding
  `/meeting <id>` with Copy. No API key, no button that calls a model.
- **Interrupted**: a grey callout "Atharu closed during this meeting. The transcript is saved up to hh:mm:ss; the rest of the audio
  is gone."
- **Permissions** (first Record): a grey callout with numbered `.steps`: Microphone (`.pill.ok` Allowed) → Screen & System Audio
  Recording (path in 500 weight + **Open System Settings** glass pill) → **Quit & Reopen** (the view's one blue). The meeting starts
  anyway, banner says "not recording yet". Denied → "Atharu can't hear this meeting", the path, the same two buttons. Mic dialog copy
  (`NSMicrophoneUsageDescription`): "Atharu records your side of a meeting and turns it into text on this Mac. Audio is deleted once
  it's transcribed."
- **Settings › Connections › Mac calendars** (0.2.4, `settings/maccal.html`): in the Mac app a `.sw` switch row, then one `label.gi`
  per Calendar.app calendar (`.cdot` in the calendar's own colour, title, account muted, checkbox); the form autosaves on change (plain
  POST 303 without JS) and each save makes the app sync. States: waiting for macOS · access off (`.err` "Calendar access is off. Open
  System Settings › Privacy & Security › Calendars.") · no calendars yet. In a browser: one status row ("Set up in the Mac app" + last
  sync, `.pill`).
- **Settings › About › Update** (0.2.6, `settings/update.html`): the browser keeps the Terminal row ("Run in Terminal" + the `.cmd` chip
  `atharu update` + Copy). In Atharu.app a second `.gi` row (`#update`, `.mac-only`) walks one update: **Check for updates** (ghost) →
  "✓ v0.2.6 is the newest" + Check again, or "v0.2.7 is available" + the one primary **Update** → "Updating to v0.2.7 · 1 min" with a **progress bar** (`.upd-bar`, 6 px,
  `--accent` on `--fill-2` 4.88:1 non-text, gliding 1.9 s between the 2 s polls, still under Reduce Motion), "Step 3 of 4 · Building the Mac app ·
  about 2 min left" and the install's last line under it (12 px mono `--muted`, one line, ellipsis; the row polls itself every 2 s only in this state) → "✓ v0.2.7
  is installed" + primary **Restart Atharu** (the app's `/app/restart`). A failure is an `.err` title + the reason + "Log: …/update.log" +
  Try again; a failed check says "Couldn’t check for updates". It runs `scripts/install.sh --version <tag> --app` (fixed arguments, the tag
  from `git tag --list` that passed semver; no request value reaches the command line), one at a time, output written straight to `update.log` next to the
  database (a file, not a pipe: the progress line is that file's tail, read when the row polls). The app runs it with
  `ATHARU_UPDATE_FROM_APP=1`, so install.sh does not quit the app it runs inside (the page and its Restart button live there); a
  Terminal `atharu update --app` still quits it first. The bar is real per step and shaped for the eye: install.sh prints `▸ step N/4 source · cli · app · swap` (2 steps without `--app`) and
  `⏱ <step>: XmYYs`; each step's share of the bar is how long it took the LAST whole run (`update-times.txt` next to the log; defaults
  10 / 60 / 90 / 5 s, and no "left" text while there is no history), the bar accelerates (slow start, fast finish: `0.97 × real^1.6`, because an
  accelerating bar is judged faster than a steady one and an end slow-down worst), capped at 97 %, it never moves backwards and is 100 % only on the Done row; "about N min left" ≥ 90 s, "less than a
  minute left" 15–89 s, "taking longer than usual" past 1.5× the step's time. Past 20 minutes the row says "Took longer than 20 min; still running in the
  background, see the log": the install is left running, no Try again is offered and the row keeps polling until it ends (then it
  says how it went). One install at a time: `updater::start` is refused while one is alive, and install.sh itself holds
  `target/install.lock` (a directory with its pid; a dead owner's lock is reclaimed), so a second one from the Terminal or after an
  app restart exits "another install is running (pid N)", which the row shows as "Another install is running (pid N). Nothing was changed." Update from a browser answers 403 "Update from Terminal: atharu update". Plain sentences, no exclamation marks, no new token.
- **Settings › Transcription** (between Connections and Accessibility): Speech model (Download 574 MB = primary · downloading:
  `progress.prog` + "212 of 574 MB · about 1 min left" + Cancel · "Checking the file…" · Ready `.pill.ok` + Delete `.dng` · checksum
  failed `.err` + Try again) · Language `.seg.mac` Auto | Thai | English (Thai default; help says Auto can translate) · Vocabulary
  `.pill` Automatic (names + meeting name + active epics as hints) · **Names** `.ta` (both views, autosaves): one person per line `แฟรง = Frank, VP Strategy`, fed to the speech model and to /meeting · Speakers `.pill` Automatic (split after Stop; the 30 MB voice models
  are fetched sha-pinned with the first recording, no button; Delete of the speech model removes them too) · Permissions
  `.pill.ok` On · Audio "Deleted as soon as it's text and its speakers are told apart" (Others sits in a temp file until the
  split ends; a crash leaves it for the next launch to delete).
- RT / IC: the banner is already opaque `--dark`; `.tx` is content; glass bits (search field, Copy pill, crumb) follow the usual rules.

### Tag chip (canvas row "Tags · one grouping layer", T70; D29)

- **`.chip.tag`** = the `.chip` family (`--fill-2`, 6 radius, 12px, `1px 8px`), text `--ink-2` (11.09:1); a `::before` **`#`** in
  `--accent` is the only colour (4.88:1 on `--fill-2`; Tinted `--accent` #0055b3 = 6.25). The fill is opaque, so the numbers hold on
  every look, over glass and in menus. Hover = `--sep` fill, and the `#` steps to `--accent-2` (`--accent` is 4.35 on `--sep`, fails;
  `--accent-2` 5.57, text 9.88). Focus = 2px accent ring. Increase Contrast: text `--ink` (14.77), `#` `--accent-2`.
- Not to be confused with its siblings: `.chip.key` = mono epic key, `.lchip` = the source link: grey pill, blue text + ↗,
  plain `.chip` = meeting name / Google label.
- **In the text**: a line's `#expense` renders as the chip where it was typed. The line text is split into `.ltext` segments: plain
  runs stay the `.edit-line` link, each tag is its own `<a class="chip tag">` to the hub (no link inside a link). Phone: `.blk>.ltext`
  takes the row (`flex:1 1 0`), like `.edit-line`.
- **Inherited** (`.chip.tag.inh`): white + 1px dashed `--ctl-edge` ring (3.3:1, non-text floor): the tag comes from the line's epic or
  meeting series and is not in its text. **Active filter** (`.chip.tag.fl`): `--accent-wash` fill (= selected, as in menus; text
  11.07, `#` 4.88) + a × link that removes it.
- **Capture bar**: `#` opens tag suggestions in the `.menu`, upward from the bar (`.menu.up`), most used first, count right in
  `--muted` (menus take `--muted-tint`), last option "New tag #ex", footer "↑↓ pick · Tab or ↵ insert · Esc close"; the input is a
  combobox (`aria-activedescendant`). A typed word that is already a tag (case-insensitive, plural) gets an offer in the hint row:
  `.tagask` = white 28px pill, dashed `--ctl-edge` ring, "Tag as `#expense`? `Tab`" + × (not a tag). Never applied by itself.
- **Make tag**: selecting one word in a line edit shows a small `.menu` "Make tag `#expense`"; the toast then offers
  "Tagged `#expense` · 4 older lines mention ‘expense’ · **Tag them** · Undo"; Tag them is one POST, one Undo.

### Tag hub (canvas "Tags.Hub", "Tags.HubMerged", "Tags.List")

- **`/tags/<tag>`** is a page like any other: crumb Tags / #tag with ⋯ (Rename… · Merge into another tag… · Delete tag… in `--err`;
  rename/merge rewrite the `#word` in the sources, delete keeps the word and drops the `#`). Title `.ptitle` with the `#` in `--accent`
  (`.hash`), props: Used (counts, last used) · Tagged by (epics, series, topics whose tag field holds it).
- Head: `.pghead` (tag glyph, `#tag`, the Used counts as its fact line). Sections in this order, each a `details.tg` + muted count: Open to-dos · **Decisions, blockers, impact** (the highlighted block: `--fill`
  12-radius panel, `.hi`) · Recent lines (newest first, `details` for older) · Epics · Meetings | Topics (two columns
  `.cols`, one on phone) · Coming up (own events with the tag; Google events matched by a title word, marked "matched by word" in
  `--muted`, never stored), then Linked references (the notes with `#tag`, `[[tag]]` or on a tagged line; 0.2.0 dropped the Notes column
  for it). Inherited lines carry "via `OPS-140`" / "via `series`" (`.via`), each a link to its epic / series page.
- **One name, one hub**: a tag, topic and meeting series with the same name share the page; a `.callout` says so and the topic
  (depth meter, own words) and series rows sit in the props. `[[X]]` resolves epic → tag → topic → series (D45: a shared name lands
  on the hub; the callout says so).
- **Tags list** under Track (sidebar `#` icon after Report): Find field, `.seg` Most used | Recent | A–Z, table Tag · Lines · Open
  to-dos · Last used · Tagged by; counts include inherited lines.
- **Filter row** (GET `?tag=`, any-of for a comma list) on To-dos, Today, Notes, Calendar, Report: "Tag" label + `.chip.tag.fl` chips +
  a `.fld.s` "+ tag"; Today and Report add **Group by** `.seg` Epic | Tag (a line with two tags shows under both; Report copies
  `## #tag` headings).
- Epics, meeting series and topics have a **Tags** prop (`partials/tag_field.html`, `.tagf`): chips + an inline `.pin` with a
  datalist; comma / Enter adds, Backspace on empty removes; autosaves. Each chip carries a small × (a submit button, as on the
  `.fl` filter chip) so removing works without JS; an off-screen first submit button keeps Enter from ever hitting a ×.
  Series tags sit on the meeting page ("Series tags", one field for every meeting of the series), topic tags under its title.
- **Hub ⋯ menu**: the page menu (`.pmenu`, see the catalogue) with Copy link first; each item opens its confirm
  step on the hub itself (`?act=rename|merge|delete`, a `.callout` form), so it works as plain GET + POST. Done = the `<main>` becomes
  the toast "Renamed #a to #b · N places rewritten · Undo · Open #b" (one transaction, one Undo, like a deleted page).
- Report: Group by `.seg` Epic | Tag sits at the right of the filter row (the range row already holds the from/to dates).

### Floating capture bar (canvas row "Floating capture bar", T78; D42)

The Today capture bar is a Liquid Glass capsule that floats over the page at every width, like iOS 26's bottom bars;
the log scrolls under it.

- **Placement**: `position: sticky; bottom: 16px` as the last child of `.doc`, not `fixed`. It stays inside the sheet,
  centred on the doc column at every width with no sidebar maths, and it stays in flow, so at scroll end it lands 16px
  above the sheet's edge and the last line is never under it. `.doc{flex:1 0 auto}` + `margin-top:auto` keep it at the bottom
  on a short day. Sheet stays the scroll context: no top-layer bar, no z-index fight with menus.
- **Size**: width = the doc column (764 at 1280: long lines need the room); phone = full width minus 12px gutters,
  `bottom: 12px + safe-area-inset-bottom`. Height 52, 44 Log ↵ inside (44 targets on phone).
- **Contents**: input · `?` · Log ↵. The `?` stays in the capsule at every width; the grammar legend is no longer a row
  under the bar (it would float over content). While the input is focused and holds ≤ 1 character, the `#hint` pill above it shows the
  grammar (`! decision · x blocker · + impact · ? learn · > to-do · n note`, 12 px `--muted`, marks mono `--ink-2`; capture.ts); the
  first real character swaps it for the live preview / error, blur removes it. No new surface: it is the hint pill, so the capsule never moves. It opens a `.menu` panel as wide as the capsule, 8px above it (CSS anchor
  positioning on `.newline`, fixed 12px-gutter fallback).
- **Note mode** (0.2.6, D55): `n ` (n + space) at the start of the line, or ⌘N while focus is in the bar, swaps the capsule for the
  same glass pill grown to three lines (`.newline.note`, `#tnote`: 22 radius, a 24/16 textarea, **Save note ↵** bottom right, the
  line's surfaces and tokens, nothing new to measure), placeholder "Note title, then the body on the next lines"; the typed rest
  moves in with the caret at its end (`nginx fixed` stays a line). Enter saves a note (first line = title, the rest = markdown body),
  Shift+Enter is a new line, Esc goes back to the line with the text. Saved → the toast "✓ Saved note · **Open**" (`/notes/<id>`),
  the bar is the line again. The hint for `n …` reads "→ will save as a **note** · “title”". Without JS `n …` in the line posts to
  `/entries`, which re-sends (307) to `/notes/new`; the note's page opens. Shared with the popover (editor/src/notebox.ts).
- **Above the bar**, out of flow, bottom-up: hint pill (nearest) · `#` / `by ` menu · toast; 6px apart, 8px clear of the
  capsule, so nothing ever sits on it (**never glass on glass**). The capsule never moves when they appear. The bar
  reserves 40px above itself (pointer-events none) so at scroll end one pill never covers the last line.
  `#tagsug` and the `by ` menu (`#bysug`) live outside the form: `.newline`'s backdrop-filter is a Backdrop Root, so a
  menu inside it blurred only the capsule. The open legend takes the pills' slot, so they hide meanwhile; on phone the menus
  take the full width.
- **Hint + toast pills**: their own small glass layer (the `.menu` recipe: +25%, rim, light, `--lg-shadow`), 32px,
  pill radius; hint text `--accent-2`, errors `--err-tint`.
- **Shadow**: the capsule takes `--lg-float` (1px contact + `0 12px 32px rgba(0,0,0,.16)`): `--lg-shadow`'s 24px .12
  got lost over busy text.
- **Floors** (it now floats over the whole sheet, dark banner included): capsule 70%, pills + menus + legend above it
  85%. Measured (t78/aa78.py: every look × Clear/Tinted at its floor; backdrop = a half-covered ink pixel, or the
  `--dark` meeting banner fully behind): placeholder `--muted-tint` ≥ 4.75 (Wakaba Tinted over the banner; 3.83 at the
  old 60%), hint `--accent-2` ≥ 5.12 (`--accent` was 4.17 there).
- **Phone keyboard**: `capture.ts` reads `visualViewport` and sets `--kb` + `.kb` on `.capture`; the bar rides 8px above
  the keyboard (no safe-area inset then, the keyboard covers the home indicator). The viewport meta adds
  `interactive-widget=resizes-content`, so Chrome on Android resizes the layout viewport itself and `--kb` stays 0.
- **Reduce Transparency**: capsule, pills, menu and legend solid white with a `--line` hairline, no blur, rim or lens.
  **Increase Contrast**: ink edge on the capsule and pills.

### Date picker (canvas row "Date picker · one shared popover", T75; D38, D4)

- **One popover for every date field**: day nav, to-do due (`.fld.s`), report from/to, event date, epic start/target/done on. It replaces
  only the browser's picker UI: the `input[type=date]` stays in the form as the value carrier and the no-JS fallback, so forms post
  unchanged. Opens on click, Enter, Space or Alt+↓; typing a date into a visible field still works.
- **Panel** = `<div class="menu dp" popover role="dialog" aria-labelledby>`: the `.menu` glass recipe whole (rim, light, shadow, lens,
  `--muted-tint`, RT/IC), 18 radius, 10 padding, but `--lg-min: 85%` instead of 60%: a month is 42 small numbers plus an accent ring,
  and at 60% `--accent` measured 3.18. Anchored 6 px under the field (end edge to the day nav; start edge to a field, end edge when the
  start would leave the sheet); flips above when there is no room.
- **Header**: month + year 15/600 (`aria-live=polite`) left, ‹ › `.ib` right (flat inside the glass, no glass on glass).
  **Weekday row** S M T W T F S (full names in `abbr` / `aria-label`), **Sunday first** (D48; report weeks stay
  Monday, D4), 11/600 `--muted`. **Grid** `table role=grid`, 7 × 6 always from the Sunday on or before the 1st (the panel never
  changes height), 36 px round cells, 14 px tabular numbers in `--ink`; Home / End = Sunday / Saturday.
- **Day states**: today = 1.5 px inset `--accent` ring + accent 600 number (`aria-current=date`) · value = `--accent` fill, white 600
  (`aria-selected`; wins over today) · hover = `rgba(0,0,0,.05)` (a white lift is invisible at 85%; today's number steps to
  `--accent-2` while hovered) · keyboard focus = 2px `--accent` outline, offset 1 (shows around the fill too) · out of month = `--muted`,
  clickable (picks the day, flips the month); never `--faint` (1.74) · disabled (`aria-disabled`, e.g. an epic target before its start)
  = opacity .45, `not-allowed`.
- **Footer**: hairline, then **Clear** (only on a field that is not `required`: due, epic dates; not day nav, report range or event date) left and **Today** right,
  both `.linkbtn` 13 px.
- **Keyboard**: ←→ day · ↑↓ week · PgUp/PgDn month · Shift+PgUp/PgDn year · Home/End week start/end · Enter/Space picks (writes the
  input, fires `change`, closes, focus back on the field) · Esc closes, nothing changes · Tab cycles ‹ › → grid → footer. Roving
  tabindex: the focused day has `tabindex=0`; it starts on the value, else today. Month switch slides the new grid 8 px in (160 ms), none
  under `prefers-reduced-motion`.
- **Phone (< 640)**: a bottom sheet like the ⋯ sheet: 8 px inset, 22 radius, grabber, 17 px month, 44 px day cells across the width,
  `::backdrop` dim; a tap selects, **Done** (the sheet's one blue) commits; backdrop tap / Esc / swipe down close without change.
- **`by ` menu** (T80, D44; not drawn on the canvas, described here): on a `>` line in the capture bar, typing `by ` opens a `.menu.up`
  listbox like the `#` menu ("Due date" head): Today · Tomorrow · the day after tomorrow by name while it is still this week ·
  Next Mon · Pick a date…, each with its date right in `--muted` (`Thu 12 Nov`). **Nothing is preselected**: Enter saves the line
  as typed unless ↑↓ moved into the menu; Tab or a click picks (Tab with nothing chosen takes the first); Esc closes it for this
  line; typing filters by word or weekday name (`fr` → the row that falls on Friday), no match closes it and leaves the text.
  The same menu (`bymenu.ts`) runs on to-do-only fields marked `data-by` (the To-dos box, the Follow-up line; `byfield.ts`):
  no `>` needed there, and it opens **down**, fixed under the field (`.newline` is a Backdrop Root, a row too short to hold it).
  A pick writes readable text, `by 20 Nov` (ISO when a year or more out). "Pick a date…" opens this popover (min = today) anchored to
  the capture bar. A trailing `by` + words that aren't a date save as text with a soft `--muted` note in the hint ("‘by hand’ isn't
  a date · saved as text"); an impossible date (`by 32 Nov`, `by 2026-13-01`) is still an error.
- **RT / IC**: nothing of its own under Reduce Transparency (`.menu` goes solid white + `--line` hairline, no lens). Increase Contrast:
  ink edge (from `.menu`), weekday and out-of-month `--muted-hc` (9.67), today ring 2 px.
- **AA** (T75, each look at its glass floor, Clear + Tinted, over a half-covered ink pixel / the darkest veiled wallpaper, desktop and
  phone sheet): ink 13.58 · `--muted-tint` 7.34 · `--accent` text 4.57 · white on `--accent` 5.57 (7.12 tinted) · hover ink 12.18,
  out-of-month 6.58, today `--accent-2` 5.15 · today and focus rings 4.57 (≥ 3).

### Quick popover (canvas row "Menu-bar quick popover", T81; D46)

- **What**: the tray item's left click (and Ctrl+Opt+Space) opens `/quick` in a small borderless window hung under the menu bar,
  macOS 26 menu-bar-extra style: no arrow, 18 radius, 6 px under the bar, left edge on the tray item, clamped 8 px inside the
  screen's right edge. Right click = a native menu (below). The browser can open `/quick` too; every action is a plain form POST.
- **Panel** = `<div class="menu quick" role="dialog">`: the `.menu` glass recipe whole (rim, light, shadow, `--muted-tint`,
  `--err-tint`, RT/IC), following **every appearance setting like the main window**: the look's wallpaper behind it, Clear/Tinted, the
  opacity slider (`--glass-a` + 5%, the sidebar's step, so Wakaba reads green), Glass everywhere (panel −5%, each section a 12-radius
  card at +10% with the `--glass-edge`, 8 apart), RT/IC. Blur text boxes has nothing to act on here (no `.ta`/`.note-md`), as on Today.
  In Atharu.app the native Popover material is the base layer, the look's wallpaper paints over it at .8 (as in the main window,
  rounded 18) and the glass on top; the CSS blur is dropped (a smooth mesh gains nothing). The window's material follows the look (0.2.9, `apply_theme` in src-tauri): Yoru dark, Auto the system, every other look light. App-only floor **35%** (`--lg-min`): the 20% left to the material is modelled as black, and Sumi's 18% floor would put the
  hint at 3.88 there. In the panel, links take `--accent-2` (`--accent` is 4.44
  over black) and the to-do check ring `--muted` (`--ctl-edge` is 2.12 there); fields and pills keep a `--line` edge.
- **Layout**, 360 × 480, 12 side padding, sections split by hairlines, in this order: **header** (40: dry-brush mark · **Atharu** ·
  date in `--muted` · **New note** `.ib` (aria-pressed in note mode) · **Open Atharu** `.ib`) · **capture** · **meeting** · **to-dos**. Desktop control heights at every width: the page
  loads tokens/base/components/looks + `quick.css`, never `responsive.css` (360 px must not trip the phone rules); a `(hover:none)`
  rule gives touch its 44.
- **Capture** (0.2.6, board Redesign.Quick): ONE field, 8 px between it, the legend and the hint (the section's gap). `.q2f` is 40 tall,
  12 radius, `--fill` on a `--ctl-edge` ring (3.3:1, the 3:1 control floor), focus-within = `--accent` ring + 3 px `--accent-wash`: the
  **kind button** `.q2k` inside on the left (mark in mono `--accent-2` + word + chevron; `--accent-2` 6.25:1 on `--fill-2`), the line, and
  **Log ↵** inside on the right (30, the view's one blue). The kind button opens `.q2menu`, the capture bar's opaque `.menu` (no glass on
  glass) under the field: six `menuitemradio` rows (mark · word · the typed key at the right: done, `!` decision, `x␣` blocker, `+` impact,
  `?` learned, `>` to-do) + the **note** row (`n␣`); the picked row = `--accent-wash` + 600. The rows are a hidden radio group posting
  `kind` (it still works without JS, where the menu stays shut and the line's own prefix picks). Keys: Enter / Space / ↓ on the button open
  it, ↑↓ move, Enter picks (back to the line), Esc closes the menu only (the next Esc hides the popover), a click outside closes. A typed
  prefix always wins and quick.ts mirrors it onto the button; a picked kind only prepends its prefix when the line has none. Under the
  field the **legend** (12, `--muted`, marks in mono `--ink-2`): "or type ! decision · x blocker · + impact · ? learn · > to-do · n note",
  hidden in note mode; then the live hint (13, `--accent-2`, 18 px reserved so nothing jumps). Enter saves → "✓ Saved as impact on
  `OPS-140` · Undo", the box clears, focus stays, the popover stays open. A hidden twin of Log ↵ comes first in the page, so Enter in the
  line is the default submit (the header's note button would otherwise be). **`#` and `by ` menus open down** inside the window over the
  sections below (the window can't grow up into the menu bar), opaque white (`--lg-min:100%`), same options, footers and keys as the capture
  bar (T70, T80).
- **Note** (T86, D53; board Quick.Note): the header's **New note** `.ib` (the `note` icon inlined, `aria-pressed`; `<use>` drew an empty
  square in WKWebView), and the menu's note row, are not a kind: a submit button that without JS GETs `/quick?mode=note` with the line
  form's fields, and quick.ts switches in place (⌘N does too). Pressed = `--accent-wash` + an inset `--accent` ring. The field becomes a
  3-line `.q2note` (12 radius, `--accent` edge + ring like the focused field, 14/20) with **Save note** bottom right, placeholder "Note
  title, then the body on the next lines"; a half-typed line moves into it. The kind radio keeps its check, so after saving the box is
  back on that kind. First non-blank line = title, the rest = markdown body (empty ok), saved as the owner via `POST /notes/new` (htmx even
  in the app: a plain POST would be a navigation); `#tags` anywhere, title line included, tag it (D54). Enter saves, Shift+Enter = new
  line. Saved → hint "✓ Saved note · **Open**" (opens `/notes/<id>` in the main window). Only whitespace → "Nothing to save. Type the note’s
  title first." in the hint, the text stays.
  **By typing (T87, D55):** `n ` (n + space) at the start of the line box switches to note mode in place, the rest moves into
  the note box with the caret at its end (`nginx fixed` stays a line, like `x `); ⌘N anywhere in the popover does the same. Without
  JS the line form's `n …` POST to `/entries` is re-sent (307) to `/notes/new` and saved as a note. The same on Today's bar (0.2.6, below): `n ` anywhere a line is captured is a note.
- **Meeting** (0.2.6, board Redesign.Quick): idle = ONE `.q2f` row, 40 tall like the capture field: the name box (placeholder "Start a
  meeting…", tooltip "e.g. oms-sync; reuse a name for a series"; focus opens the recent-series menu, up to 12, ↑↓ ↵ picks and starts) ·
  the **mic toggle** · a ghost **Start** (30). The "Start a meeting" heading stays as a visually hidden label. The mic (app only) is a
  30 icon toggle: crossed grey mic = not recording (off every time the popover opens, D31), white mic on `#ff3b30` = on (white on it
  3.55:1, non-text 3:1; `--muted` 5.07 on white); it is a label around a hidden checkbox, so `record=1` still posts without JS. On, a
  12 px `--ink` line "Recording on. Tell people you’re recording." shows under the row (CSS `:has`). No speech model → the toggle is
  disabled (40%) with one line "Recording needs the speech model · Settings". Running → ONE `.q2run` row in the banner's opaque
  `--dark` (min 40, 12 radius): dot (`.dot.rec` red only while capturing) · name link (ellipsis) · time (minutes, "24 min", or the
  recording's elapsed "04:12", tabular 600) · spacer · 📷 screenshot (app) · the dark mic toggle (28; app + model: pressed = live, click
  stops the recording and the meeting keeps running; not live, click starts one; blocked → disabled and the name row reads **Allow
  recording**, linking the meeting page in the main window; finishing → time "Transcribing…", no mic) · **End meeting** (text button,
  `rgba(255,255,255,.14)`). Ended → "✓ Ended **X** · Open meeting" above the idle row.
- **To-dos** (0.2.6): today + overdue only, overdue first. Header "To-dos **2 overdue** · 1 today" (the overdue part in `--err`, only
  when > 0). Rows are 32: check 20 · text on one line with ellipsis · the note button · the **quiet due date** on the right (the To-dos
  list's words: `Today` in `--accent-2` 6.2:1, a late one `--err` 600 with its day, `Fri 6 Nov`, else `--muted`). **No inner scroll**: at most
  **5** rows, the window's height comes from the content, and five rows fit the 480 at rest (idle, recording, and the no-speech-model
  line; measured at 360 × 480). Three things cost room: Glass everywhere's section cards (~17 px), "✓ Ended X" above the idle meeting row (~27) and, in the
  app, the line under it (~23: "Recording on…" once the mic is ticked, or "needs the speech model"; the cap counts it always, so
  ticking never pushes the last link off). The server shows one row less per 32 px over the 29 spare (`todo_cap`), so the window never
  scrolls and "+ N more" stays true; measured at 360 × 480 with the mic on: flat 468, Glass everywhere 453 (4 rows), + Ended 448 (3). "+ N more · Open Atharu" links
  `/todos`. Ticking posts at once; the struck row stays until the popover closes (untick = reopen). Empty: "Nothing due. Start a line with
  `>` to add one." + "N open · Open Atharu". Sections are flat: a `--line` hairline between them, 10 padding (the capture sits under the
  header with no rule); Glass everywhere keeps its cards (8/12 padding, 6 apart).
- **Keyboard** (0.2.6): opens focused in the line. Tab from the line: **Log ↵** → meeting name → mic toggle → **Start** (or, while a meeting
  runs: 📷 → mic → **End meeting**) → each to-do's check → "+ N more · Open Atharu"; Shift+Tab: the kind button → **New note** → **Open
  Atharu**. Enter saves (unless an option was arrowed to). Esc closes an open menu (the kind menu, `#`, `by `, series), else hides the
  popover; blur hides it; Ctrl+Opt+Space toggles it; ⌘N or `n ` = note. Any link opens in the main window and hides the popover.
- **Menu-bar icon states**: idle = the Dry brush mark (template, macOS tints it) · meeting running = the mark with a solid badge
  dot bottom-right, cut out of the ring (template; also while a recording finishes or is blocked) · capturing = the same in `#ff3b30` (not a template: red has to stay red; reads on
  light and dark bars), with the recording's elapsed time (`04:12`) as the item's title beside it. Tooltip names the state and
  meeting. The app polls every second, so a browser start/end shows too.
- **Recording in the meeting row** (app only, same rules as the banner; see Meeting above): the dark row's mic toggle is the whole
  control. Model ready + not recording → crossed grey mic (click starts); live → red mic + `.dot.rec` + the elapsed time, click stops
  (the meeting keeps running); blocked → disabled + "Allow recording"; finishing → "Transcribing…". The browser shows none of it.
- **Series menu** (T93): the start box opens the capture bar's opaque menu below it on focus, "Recent series" (up to 12),
  filtered as you type; ↑↓ selects (nothing preselected, so ↵ on a typed new name starts that name), ↵ / Tab / click picks
  and starts at once (the mic toggle rides along); names clip with the full one on hover. Replaced the 3 series pills (a long name took a row
  and pushed the to-dos out of the window). No JS: a `<datalist>`.
- **Right-click menu** (native NSMenu, follows system light/dark): **Open Atharu** · **Start meeting ›** the 3 recent series ·
  while one runs, **End meeting · name** instead · separator · **Quit Atharu ⌘Q**. "Quick capture" goes (left click is it now).
- **RT / IC**: Reduce Transparency → panel 100% white + `--line` hairline, no blur (the OS drops the material too). Increase Contrast →
  ink edge (from `.menu`), ink hairlines, the field and the note box edged in ink, the picked kind row ringed, `--muted-hc`, ink check ring.
- **AA** (`aa81b.py`, each look at its floor, weakest step −5%, darkest veiled stop, unblurred, Clear + Tinted + IC; app = wallpaper
  .8 over black + the 35% floor): worst text is always the hint/links `--accent-2`: browser Sumi 5.75 · Sora 5.34 · Wakaba 5.63 ·
  Glass 4.88; app Sumi 4.56 · Sora 4.68 · Wakaba 4.66 · Glass 4.57. Rings ≥ 3.59. Opaque pairs unchanged (white on `--accent` 5.57,
  dark callout as the banner: white 14.91, muted 9.28, rec dot 4.90). 0.2.6 adds only opaque pairs, measured in `quick.css`: the kind
  button and its menu (`--accent-2` 6.25 on `--fill-2`, 7.12 on white), the mic (`--muted` 5.07 on white, white on `#ff3b30` 3.55 as a
  non-text pair), the dark row's mic and 📷 (white on the row's `rgba(255,255,255,.14)` over `--dark`).
- **Canvas**: the boards "Next · 0.2.6 Menu-bar popover" (`Redesign.Quick`, `Redesign.Quick.States`, final version 64) are what the
  app draws; where they differ from the text above, the text is what shipped.

### Line notes (canvas row "Line notes · the note is the detail page", T84; D49)

A line (any kind) can have one note; many lines can share it. There is no line or to-do detail page: the note page is it,
and it lists its lines at the top.

- **Icon**: `ui.svg` gains `note` (a page with a folded corner and two text lines). One glyph, two states: its paths read
  `--note-fill` / `--note-line` (custom properties inherit into the `<use>` tree), so a row sets them and nothing is duplicated:
  `<symbol id="note" viewBox="0 0 24 24"><path d="M6 3h9l4 4v14H6z" style="fill:var(--note-fill,none)"/><path d="M15 3v4h4M9 12h6M9 16h4" style="stroke:var(--note-line,currentColor)"/></symbol>`
- **Row button `.ib.note`** on every list of lines: Today (log rows and to-dos), To-dos, tag hub (also inside `.hi`), epic
  timeline, quick popover to-dos (a click opens the main window on the note). 28 px like `.x`, so rows stay 32. It is the
  **first item of the row's right cluster** (plan_row's `span.row`; entry_row gets the same wrapper, `.end`), before the due
  field, Pin/Drop, ⋯ and ×, so filled icons line up in one column.
  - **Has a note**: `<a class="ib note has" href="/notes/12" aria-label="Open note: <title>" title="Open note">`, always shown
    (it tells you the line has more): `--accent` fill with white lines (a link, so the one blue); hover `--accent-2` + the `.ib` tint.
  - **No note**: `<form method="post" action="/entries/<id>/note"><button class="ib note" aria-label="Add a note to this line"
    title="Add a note">`: `--muted` outline at 35% opacity, full on row hover / focus (never fully hidden: the owner
    couldn't find a hover-only icon); the icon is inline SVG, not the sprite (WKWebView drew the sprite's version blank). One POST creates the note (title = the line text without prefix and key, author me),
    links it and 303s to `/notes/<id>`. Plain form, no JS needed.
  - **Phone (< 640)**: the add button leaves the row; the ⋯ sheet gets **Add a note** (or **Open note**) above Edit line. A
    line that has a note keeps its filled button in the row at 44.
- **Note lines `.lnh`** on `/notes/<id>`, inside its Linked references at the end of the page (0.2.0; it sat between the title
  and the props before): a white card with a `--line` hairline, 12 radius, under the group label "Line" / "Lines · 3" (the
  `.refs h4`). Content, so no glass; rows are the normal 32 `.blk` with no hover tint
  (only their controls react). A `--fill` panel was tried and failed AA: `--muted` 4.27 under the `.hi` row hover, the check
  ring `--ctl-edge` 2.99 on `--fill`.
  - Each line: kind square `.k` (a to-do takes the round `.check` instead of `>`, posting like its row) · text with tag chips ·
    epic `.chip.key` · a to-do's due `.fld.s` date field (in `form.due-f`, which the layout rules key on) (opens the shared date popover) or "done Thu 12 Nov" (`.when`) · last,
    **"logged Thu 12 Nov"** (`.day`, 12 px `--muted`, links to that day; "today" when it is), so the day links line up.
  - Ticked to-do: struck + `--muted`; untick reopens. Text is not an edit link here: lines are edited on their day.
  - **Line deleted**: while its Undo lives, a dashed empty `.k.gone`, "Line deleted · “old text”" in `--muted` and Undo
    (`.linkbtn`); Undo restores the line and its link. After that the link is gone (`ON DELETE SET NULL`); a note with no lines
    left loses the card and is an ordinary note.
  - More than 5 lines: the newest 5, the rest in a `details` ("3 more lines").
  - Phone: rows wrap; the day link drops under the text (indented 32).
- **Notes list**: a linked note shows the kind square of its first line before the title (`title="Linked to a to-do line"`)
  and "3 lines" (`.small.muted`) when there are more. Filter row gets the chip **Linked to lines** (`?linked=1`, a link with
  `aria-pressed`): off = plain `.chip`, on = the tag filter's `.fl` chip without the `#` (rename `.chip.tag.fl` → `.chip.fl`)
  with × to clear. The sidebar note list stays as it is.
- **RT / IC**: nothing of its own under Reduce Transparency (all content). Increase Contrast: card edge `--ink`; the add
  button, label, day link and deleted text in `--muted-hc`.
- **AA** (t84/aa84.py, sheet `#fff`, worst of plain / look / Tinted `--muted` and `--accent`): add button outline 4.66 on the
  `--fill` row hover (4.27 on the `.hi` hover, non-text floor 3) · filled button `--accent` 5.11, its white lines 5.57 · header
  label, day link, done and deleted text `--muted` 5.07 · Undo `--accent` 5.57 · check ring `--ctl-edge` 3.26 on the card ·
  list "3 lines" 4.66 on hover · filter chip text 9.88 off, 11.07 on.

### Motion (0.1.4; D61, grilled with the owner 2026-11-12)

Apple-subtle: motion says where something went, never performs (one exception, the launch animation below). Tokens in `tokens.css`: `--dur-1` 120 ms (menus, fades) ·
`--dur-2` 200 ms (rows, toast, switch thumb) · `--dur-3` 250 ms (glides, banner height, page cross-fade) · `--ease-out`
(fast start, soft settle) · `--ease-spring` (the to-do check only). Movement 4–8 px at most. CSS + `motion.ts`; no library.

| Where | Motion |
|---|---|
| Logged line / new to-do (any list htmx re-renders) | the rows that weren't there before fade in + rise 6 px (`.fresh`, `--dur-2`) |
| To-do tick / regroup (due, pin, state) | check: fill + tick spring (`--ease-spring`); rows that moved glide from their old place (FLIP, `--dur-3`) |
| Menus (`#`, `by`, suggest, date picker, popover series, `[popover]`) | fade + 4 px drop in (`--dur-1`, `@starting-style`); out immediately |
| Toast / Undo | rises 6 px + fades in (`--dur-2`) |
| Segmented switches (every `.seg`) | one white thumb slides to the picked option (`--dur-2`); the page behind it cross-fades |
| Meeting banner (start / record / end) | its height animates from old to new (`--dur-3`); the 1 s polls don't move it (same height) |
| Page to page | cross-document view transition: root cross-fade (`--dur-3`) |

**Reduce Motion**: no movement anywhere (no rise, glide, slide, height, spring); opacity fades stay, ≤ 100 ms. `motion.ts`
skips FLIP and height morphs entirely. Polling never animates (the banner every 1 s, transcripts every 5 s).

#### Launch animation (2026-11-13; owner decision: the one place motion may perform)

The Mac app's main window opens `/launch` instead of `/` (a browser still opens `/`). The look's icon is inlined and brushed onto a
full-window Liquid Glass sheet over the look's wallpaper (`--wall`, `--glass-a`, the sheet takes `--lg-filter` / `--lg-rim` / `--lg-light`),
then the sheet clears and the page does `location.replace('/')`. Both pages carry `@view-transition{navigation:auto}`, so the hand-over is
the usual `--dur-3` root dissolve over the same wallpaper, never a white flash. `/` is fetched while it plays to wake the server. `static/css/launch.css`
holds every timing; `launch.ts` only prepares the SVG. Setting `launch` = `ink` (default) · `glass` · `off` (`/launch` redirects to `/`).

| Variant | Beats (ms from start) | Ends |
|---|---|---|
| **A · Ink** | sheet fades in 60 → 760 · icon drops in with a spring + tilt (0 → 700) · the ensō is brushed in one stroke, head to tail, slow start / fast middle / lift-off (380 → 1200) · the dab is thrown at 1180: its biggest circle slaps (squash 1.32 × .72, rebound), the small circles fly out of it to where the icon has them, 14 seeded specks scatter and fade (→ 1800) · glow 200 → 1600 · ripples 1220 + 1340 · light sweeps 1450 → 2200 | lifts (blur, −10 px, 1.08×) and the sheet clears 2150 → 2470 |
| **B · Liquid Glass** | sheet as A · icon condenses out of a colour blur (0 → 800) · quicker brush 280 → 880 · dab thrown at 860 · glow 100 → 1300 · sweep 450 → 1350 · one ripple 700 · one breath 1000 → 1600 | lifts and clears 1750 → 2070 |

- **Reduce Motion**: nothing moves. The finished mark (no brush mask, no splash) fades in over 100 ms with the sheet, then hands over; the view transition is 100 ms.
- **Reduce Transparency**: the sheet is solid white (`--glass-a` is forced to 100%, no backdrop blur, hairline `--line` edge) over the flat wallpaper colour. **Increase Contrast**: ink edge, no rim.
- Brush: a mask circle along the ensō starting at the head, found by sampling `isPointInFill` around the ring for the largest empty gap (butt caps, `pathLength=1`, dash = the covered fraction), so any icon redraw keeps working. If the icon can't be fetched the animation plays without it and still hands over; a 3.6 s timer opens the app even if the window never runs the animation (hidden at start).
- Settings › Appearance › **Launch animation**: Ink · Liquid Glass · Off, autosaved. A 96 px preview (`.launch.mini.keep`, the same CSS and `launch.js`) rests on the finished mark, replays when a variant is picked or clicked, stops on the mark instead of clearing, and is hidden while Off.

## 7. UX patterns (from Notion)

1. **Sidebar** = one fixed page list, the same on every page (0.2.0 D4: no per-page Series / Notes / Topics lists), each row icon + label (icon `--muted`, `--ink` on the selected `.on` row; 16 px desktop, 20 px rail/drawer) grouped by the user's loops (D18): an unlabelled daily block Today · To-dos · Calendar · Meetings, then **Track** Epics · Report, then **Grow** Learn · Notes, then the active epics (title first, key muted on the right; a stub reads *unnamed*), and Settings pinned to the bottom (spacer). Same order in the phone drawer; the rail draws the section labels as thin dividers. Breadcrumbs on top.
2. **Every thing is a page**, on one template (0.2.0): crumb with save status + ⋯ page menu · `.pghead` (glyph/key, title, one
   fact line) · filled props only, the empty ones behind "+ Add property" · the page's body · toggled sections · Linked references.
   Relations read both ways (a line's meeting chip opens that meeting, a note shows the meeting it is the notes of, Same name).
   `.doc` is 860 everywhere; `.doc.wide` only for the Epics list and Calendar. Autosaved; no Save buttons anywhere.
3. **One input**: the floating "Type a line…" bar takes the line grammar; a live hint above it says what it will save as.
4. **Feedback**: toast above the bar, "Saved as impact on OPS-101 · Undo", `role=status`.
5. **Toggles** fold secondary things (open plans, report charts, raw numbers).
6. **Notes**: Tiptap; type-to-format markdown shortcuts, `/` block menu (glass popover), `[[` page picker.
7. **Empty states name the next action** ("No epics. Start a line with a key, e.g. OPS-101."). No tutorials: the UI must explain itself.
   An empty property is not an empty state to draw: it hides behind "+ Add property", whose label names what is missing (0.2.0 D2);
   an empty section folds away or is one line naming how to fill it ("No note points here yet. Write `[[OPS-101]]` in a note…").
8. **Today** (0.2.0 D3): head + fact line (`1 due today · 1 overdue · 6 lines logged · in oms-sync`) · the running meeting's banner ·
   **Focus** = the pinned top 3 + every overdue to-do, overdue first, each once, then "n more open in To-dos →" · **Needs you** =
   flagged epics only · Logged today with its tag filter + Group by row · the capture bar.
9. **Archive** (0.2.0 D4): a meeting series' ⋯ or row Archive hides it from Meetings and the Calendar (folded "Archived · n" under the
   list); its series and meeting pages stay, a rename carries it.
10. **Long text in a fixed slot** (0.2.7; CLAUDE.md "Long real data"): the owner's data is long (60-character series names, 90-character
   Jira titles, 30 assigned tickets, Thai), the demo data is short. Text in a fixed-width slot (a table cell, a grid column, a chip, a
   row) ends in `…` and carries its full text as `title`: put **`.clip`** (`base.css`, `.clip.i` for chips) on it, never a one-off
   ellipsis rule. Nothing prints over a neighbour or past its card. On a phone `.clip` shows **two lines** instead of one (a `title` tooltip can't be opened by touch). A new table or grid states its column widths and says which column
   gives way (Epics: the title; Series, Topics, Tags: the name; the assigned rows: the title), and the status chip is never the one
   that yields to nothing (88 px floor). Check a UI change on `scripts/long-seed.sh` (`scripts/visual-check.sh --db <file>`), whose
   rules flag anything past the sheet's right edge, text past its own cell, and a name clipped to a sliver, not only on the short demo.

## 8. Responsive (`responsive.css`)

| Width | Layout |
|---|---|
| ≥ 1024 | sidebar 220–248px, pinned (`position:sticky; top:12px`, scrolls inside itself when taller than the window) + sheet |
| 640–1023 | 64px icon rail (labels become tooltips, section labels become dividers) |
| < 640 | sheet full width with top-rounded corners; nav in a popover; inputs ≥ 16px (no iOS zoom); the capture bar floats 12px inside the edges above the home indicator and rides the keyboard |
| < 640, 0.2.0 | the first screen reaches content: an epic shows Status + Role, the filled rest behind "n more" (`.pmore` checkbox, no JS); Dates stack start / target / done on one per row; the Epics table drops Impact and Last log; no Start pill on Today (start from Meetings); to-do actions stay in the ⋯ sheet (on a pointer they show on row hover / focus) |
| < 640, 0.2.3 | Calendar: the bar wraps; the hour grid keeps its gutter at 40px and hides "N lines"; the month is 7 columns of day numbers with a dot (tap → Day view) |
| < 640, 0.2.3 | List tables fit the width, never scroll sideways: the identifying column + the one fact that matters, the rest stays on the row's page (Epics Key · Title · Flag; Meetings Series · Last met; Learn Topic · Depth; Tags Tag · Lines). `scripts/visual-check.sh` fails a table wider than its box |

## 9. Rules

- Glass on chrome, never on content (unless the user opts into `data-glass=all`).
- One blue. A new colour needs a reason written here.
- Every text/surface pair passes WCAG AA, measured over the worst wallpaper spot (and, for glass over content, a half-covered ink pixel behind); record the number in a CSS comment. Measured at each look's glass floor; an opacity the owner sets below it (hatched on the slider) is their call.
- No `url()` filter reaches WebKit: anything Chromium-only goes behind `@supports (-webkit-app-region:drag)`.
- Honour `prefers-reduced-transparency` and `prefers-contrast`.
- New tokens go in `tokens.css`; components in `components.css`. Inline `style=` in templates only for one-off layout (gap, margin), never colour or type.
