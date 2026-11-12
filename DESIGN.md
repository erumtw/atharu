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
| `--hatch-a` / `--hatch-b` | `#c7c7cc` / `#e5e5ea` | hatched slider range that can't be reached; `--hatch-a` = switch hover |
| `--accent-2` | `#0055b3` | primary hover/pressed, switch-on hover, the tinted `--accent` (white on it 7.1:1) |
| `--accent-wash` | `#e8f1fb` | meeting events, selected menu option |
| `--accent-soft` | `#4d8fd1` | bars, "done" OKR segment (never under text) |
| `--accent-on-dark` | `#2997ff` | link, dot, End button on `--dark` (4.9:1, so `--dark` stays opaque) |
| `--rec-on-dark` | `#ff5c52` | the recording dot on `--dark` only (4.90:1; `--err` is 2.28 there). Never text, never on white: the word "Recording" carries the state |
| `--heat-1` / `--heat-2` / `--heat-4` | `#e3effc` / `#bcd6f2` / `#003d7a` | depth heat steps 1, 2, 4 (3 = `--accent`) |
| `--dark` / `--on-dark-muted` | `#272729` / `#cccccc` | meeting banner, impact callout / muted text on it (9.3:1) |
| `--fill-3` / `--now` | `#fafafc` / `#f5f9fe` | table hover, calendar heads, toast / today's calendar column |
| `--faint` | `#b0b0b5` | dashed rules (inline edit, own events), the "other" OKR segment; never text |
| `--ctl-edge` | `#8e8e93` | unchecked control rings, e.g. the to-do check (3.3:1 on white, the 3:1 floor for non-text UI) |
| `--tick` | SVG mask (`url(data:…)`) | the check's tick, drawn as a mask so it takes the fill colour |
| `--chrome-bg` | `#eef0f5` | behind the sheet when no look is set |
| `--muted-look` / `--muted-tint` / `--muted-hc` | `#5e5e63` / `#48484d` / `#3a3a3c` | `--muted` on a look / with Tinted glass / under Increase Contrast |
| `--err-tint` | `#a32019` | `--err` with Tinted glass (`#b3261e` falls to 4.48:1 on the Glass look's tinted sheet) |

Colour literals live only in `tokens.css`, with three exemptions: the looks' wallpaper stops in `looks.css` (§5, brand), the SVG icons, and
material that isn't a palette colour: `#fff` (sheet, knob, text on blue/ink) and `rgba()` black/white for shadows, glass edges and hover tints
(plus the iOS segmented track `rgba(120,120,128,.12)`).

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
- One alpha drives all of them: `--glass-a` (inline on `<html>`, clamped per look by `domain::look::glass_alpha` to keep AA). Each surface adds a fixed step `--s`: crumb 0, sidebar +5%, capture bar +10% (floor 70%), its hint/toast pills +25% (floor 85%), glass pills +10% (hover +25%, pressed +35%), fields and text boxes +12%, menus +25%.
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

Coloured meshes sit under a 45% white veil; sidebar text over the worst spot stays ≥ 4.6:1.

## 6. Components (`components.css`)

### Control rules (canvas "Controls" + "Components · every state", T28)

- **One height per context**: 32 in chrome (`.crumb`), 36 in the page, 44 on phone (< 640). An action inside a field is one step smaller than the field: an action inside a 36 field is 28 (a 44 field holds 36). Labels are centred with flex + `line-height:1`, never padding guesswork.
- **One blue primary** per view (`.btn`): Log ↵, Add ↵, Copy as markdown. Everything else is a **glass pill** (`.btn.ghost`); `.s` = the 32 size for row actions (Pin, Drop, Save, Delete).
- **Single-line fields are glass pills with their action inside** on the right (`.fld`): meeting start, search, add evidence; the capture bar `.newline` is the 52 px version holding the primary. Inputs that stand alone (dates, event form) take `.fld` on the input itself.
- **Day nav** = one glass capsule `.dnav`: Today | ‹ | date ▾ | ›. The date is a native `<input type=date>` laid over its label; a click opens the Atharu date popover (`.dp`, below), never the browser's picker; without JS the native picker still works. Today shows only when not on today. Calendar uses the same capsule with the range as its label.
- **States**, every control: default · `:hover` · `:focus-visible` (2px accent ring) · `:active` (pressed: primary darker + `scale(.96)`, glass pill swells `scale(1.04)` + glow) · selected (`.on` / `aria-pressed` / `:checked`) · `:disabled` (opacity .45, `not-allowed`) · error (`.bad` = red ring, input kept, message under it) · loading (`.htmx-request` on the form: its submit dims and shows a spinner; a to-do label dims) · empty (placeholder in `--muted-tint`).
- Datalist inputs hide Chrome's ▼ (`input[list]`); suggestions still open as you type. Legend chips sit on one baseline and wrap as whole items.
- Reduce Transparency: same shapes, solid white, no rim. Increase Contrast: ink borders, no rim, `--muted-hc`.

### Catalogue

| Component | Class | Notes |
|---|---|---|
| Primary button | `.btn` | blue pill, white text, 36 (32 in chrome, 44 phone); hover/pressed `--accent-2`. One per view. |
| Glass secondary | `.btn.ghost` | glass pill, ink text; `.s` = 32; `.dng` = destructive (err text). On `.callout.dark`: `--accent-on-dark` outline. |
| Icon button | `.ib` | 32 round (44, 12 radius on phone); always an `aria-label`; `.ib.more` opens a row's `.acts` |
| Link button | `.linkbtn` | POST form that reads as a link: Undo, re-open; underline on hover |
| Row actions | `.acts` | the row's buttons inline on desktop; a bottom action sheet (popover) on phone, 48 px rows |
| Delete × | `.x` | 28, revealed on row hover / focus (always shown on touch) |
| Field | `.fld`, `.fld.m`, `.fld.s` | glass pill 44 / 36 / 32 with the action inside; `.bad` = 422 |
| Capture bar | `.capture` > `.newline` + `.hint` + `.legend` | 52 glass pill, primary inside; `.newline.bad` on a parse error; hint = live parse preview in accent |
| Toast | `.toast` | above the capture bar, `role=status`; `:empty` hides it; `.tick` ✓ in accent |
| Day nav | `.dnav` (`.i` = icon segment) | see rules |
| Text box | `.ta`, `.note-md` | multi-line glass box, 12 radius; autosaves (Save only in `<noscript>`) |
| Inline input | `.pin`, `.pin.dash`, `.pin.bad` | borderless props input/select; `.dash` = dashed "type here" rule (inline edit); `.bad` = red ring |
| Line edit | `.edit-line` | log text as a link; dotted underline on hover; opens `entry_edit` in place |
| Save status | `.saved` | 12 px muted in the crumb: "Changes save as you type · ✓ saved"; `.err` on failure |
| Error | `.err` | 13 px `--err` text under the field it explains |
| Line kind square | `.k` | 22px mono square: `!` `x` `+` `?` `>`; impact = blue (`.k.imp`), blocker = ink (`.k.bl`) |
| Chip | `.chip`, `.chip.key` | epic keys, topics; `.chip.llm` = ink outline badge for agent-written rows; `.chip.tag` (`.inh`, `.fl`) = see "Tag chip" below |
| Link chip | `.lchip` | source link on a line; the only blue chip |
| Flag | `.flag.stale/.risk/.over` | pill; ink weight, no colour |
| Due | `.due`, `.due.late` | muted / red bold |
| Callout | `.callout`, `.callout.dark` | grey (info) / `--dark` (meeting banner with `.dot`, impact line with `.num` / `.num.up`) |
| Card | `.card` | white, line border, 12 radius |
| Stat tile | `.stat` | 28/600 number over a muted label; glass +10% with Glass everywhere |
| Properties | `.props` | Notion inline `dt/dd` grid, hover = editable, autosave on change |
| Block row | `.blk` | log line, 32 tall; hover reveals delete `.x` |
| To-do check | `.check` + `.done-text` | Apple Reminders circle: 20px (24 phone), 1.5px `--ctl-edge` ring (3.3:1), checked = accent fill + white tick; native checkbox under appearance:none, posts on change; done = struck + muted |
| Segmented | `.seg`, `.seg.mac` | pill track 36 / 30 segments (32/26 in chrome), `rgba(120,120,128,.12)` (iOS); `.mac` = 8-radius macOS control for settings |
| Grouped list | `.sec` + `.grp` > `.gi` | Apple System Settings inset group (`--fill`, hairline inset 16px); Notion bold label + one-line help left, one control right; `.gi.col` stacks |
| Switch | `.sw` | native checkbox `role=switch` (posts without JS), 38×22, 51×31 on phone; pressed stretches the knob |
| Slider | `.sl` > `.rng` + `.val` | native range on the true 0–100 scale; below `--f` hatched and clamped, `--v` = value |
| Look tile / status | `.lk`, `.pill`, `.pill.ok` | app-icon tile, accent ring when picked (dashed on focus); status pill grey / solid ink (On) |
| Preview tile | `.prev` | the look's wallpaper behind a mini glass sidebar + sheet, same alpha math as the real chrome |
| Disclosure | `.chev`, `details > summary` | settings row value in muted / toggles that fold secondary things (count + hint inline, 400 weight) |
| Meter | `.meter` | depth 0–4 as five 14×6 pills, on = accent |
| Bars | `.bars` | activity / impact bars in `--accent-soft` |
| OKR bar / heat | `.obar` (`.active/.done/.other`), `.heat1`–`.heat4` | report charts: status by weight; depth steps `--heat-*` (from `report::view::heat_class`) |
| Sidebar count / label | `.cnt`, `.side .lbl` | muted count, not a badge / 12/600 section label (a hairline in the rail) |
| Calendar | `.cal`, `.dh`, `.dc`, `.ev`, `.logdot`, `.evadd` | meetings = blue left rule on `--accent-wash`; `.ev.todo` outline (`.late` red text), `.ev.tgt` grey, `.ev.own` dashed; `.logdot` = lines logged; `.evadd` = "+ Event" popover form; weeks Sun → Sat (week range, month grid, phone list; D48) |
| Menus | `.menu [role=option]` | glass +25%, 32 px options, selected `--accent-wash`; `/` block menu and `[[` picker |
| Date picker | `.menu.dp` | shared date popover on every `input[type=date]`; glass = `.menu` with an 85% floor; Sunday first (D48); phone = bottom sheet. See "Date picker" below |
| Notes body | `.note-body` | Tiptap content as the board draws it (code on `--fill`, task lists, toggles, `.page` links) |
| Drawing | `.drawing`, `.drawing.empty`, `.drawsheet` + `.drawbar` | see "Drawing block" below |
| Tables | `table th td` | head 12/500 muted on a line rule, rows hairline, hover `--fill-3`; scroll in `.tw` |
| Recording banner | `.callout.dark.recb` > `.dot.rec` + `.rect` + `.lvl` ×2 | meeting banner while recording: pulsing dot, elapsed, one level meter per stream, Stop recording + End meeting |
| Transcript | `.tx` > `.txhead` + `.txbody` > `.txrow` | white 12-radius box (content, no glass); search `.fld.s` + Copy in the head |
| Progress | `progress.prog` | native `<progress>`, 6 px, `--accent` on `--sep` (4.6:1) |
| Command chip | `.cmd` | a copyable CLI / slash command in a white pill with a Copy button inside |

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
- **Transcript** section on the meeting page, after Notes: `.tx` with a head (meta in muted: "Live · about 30 s behind · th · N
  lines" / "Recorded 1 h 02 min · 812 lines · large-v3-turbo · th"), search (`GET ?q=`, server filters rows and wraps hits in
  `<mark>` on `--accent-wash`; works without JS) and **Copy** (clipboard; a `.toast` "Copied N lines"). Rows: mono timestamp ·
  **Me** / **Others** (12/600, repeated speaker hidden) · text. Live tail line: "Listening · the next lines arrive about 30 s after
  they're said". The body scrolls in its own 440 px box and follows the bottom while live.
- **Finishing**: after Stop / End, a `role=status` row under the last lines: "Transcribing the last part… about N min left · you can
  close this page, it keeps going" + `progress.prog` (≈ 4–5 min per hour of meeting).
- **Done**: a grey `.callout` above the transcript: "Ask Claude for notes: run `/meeting` in Claude Code…" + `.cmd` holding
  `/meeting <id>` with Copy. No API key, no button that calls a model.
- **Interrupted**: a grey callout "Atharu closed during this meeting. The transcript is saved up to hh:mm:ss; the rest of the audio
  is gone."
- **Permissions** (first Record): a grey callout with numbered `.steps`: Microphone (`.pill.ok` Allowed) → Screen & System Audio
  Recording (path in 500 weight + **Open System Settings** glass pill) → **Quit & Reopen** (the view's one blue). The meeting starts
  anyway, banner says "not recording yet". Denied → "Atharu can't hear this meeting", the path, the same two buttons. Mic dialog copy
  (`NSMicrophoneUsageDescription`): "Atharu records your side of a meeting and turns it into text on this Mac. Audio is deleted once
  it's transcribed."
- **Settings › Transcription** (between Connections and Accessibility): Speech model (Download 574 MB = primary · downloading:
  `progress.prog` + "212 of 574 MB · about 1 min left" + Cancel · "Checking the file…" · Ready `.pill.ok` + Delete `.dng` · checksum
  failed `.err` + Try again) · Language `.seg.mac` Auto | Thai | English (Thai default; help says Auto can translate) · Vocabulary
  `.pill` Automatic (meeting name + active epics as hints) · Permissions `.pill.ok` On · Audio "Deleted as soon as it's text".
- RT / IC: the banner is already opaque `--dark`; `.tx` is content; glass bits (search field, Copy pill, crumb) follow the usual rules.

### Tag chip (canvas row "Tags · one grouping layer", T70; D29)

- **`.chip.tag`** = the `.chip` family (`--fill-2`, 6 radius, 12px, `1px 8px`), text `--ink-2` (11.09:1); a `::before` **`#`** in
  `--accent` is the only colour (4.88:1 on `--fill-2`; Tinted `--accent` #0055b3 = 6.25). The fill is opaque, so the numbers hold on
  every look, over glass and in menus. Hover = `--sep` fill, and the `#` steps to `--accent-2` (`--accent` is 4.35 on `--sep`, fails;
  `--accent-2` 5.57, text 9.88). Focus = 2px accent ring. Increase Contrast: text `--ink` (14.77), `#` `--accent-2`.
- Not to be confused with its siblings: `.chip.key` = mono epic key, `.lchip` = the blue-outline source link (still the only blue chip),
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
- Sections in this order, each `h3` + muted count: Open to-dos · **Decisions, blockers, impact** (the highlighted block: `--fill`
  12-radius panel, `.hi`) · Recent lines (newest first, `details` for older) · Epics | Notes and Meetings | Topics (two columns
  `.cols`, one on phone) · Coming up (own events with the tag; Google events matched by a title word, marked "matched by word" in
  `--muted`, never stored). Inherited lines carry "via `OPS-140`" (`.via`).
- **One name, one hub**: a tag, topic and meeting series with the same name share the page; a `.callout` says so and the topic
  (depth meter, own words) and series rows sit in the props. `[[X]]` resolves epic → topic → series → tag (so a shared name lands
  on the topic first; the hub's callout says where).
- **Tags list** under Track (sidebar `#` icon after Report): Find field, `.seg` Most used | Recent | A–Z, table Tag · Lines · Open
  to-dos · Last used · Tagged by; counts include inherited lines.
- **Filter row** (GET `?tag=`, any-of for a comma list) on To-dos, Today, Notes, Calendar, Report: "Tag" label + `.chip.tag.fl` chips +
  a `.fld.s` "+ tag"; Today and Report add **Group by** `.seg` Epic | Tag (a line with two tags shows under both; Report copies
  `## #tag` headings).
- Epics, meeting series and topics have a **Tags** prop (`partials/tag_field.html`, `.tagf`): chips + an inline `.pin` with a
  datalist; comma / Enter adds, Backspace on empty removes; autosaves. Each chip carries a small × (a submit button, as on the
  `.fl` filter chip) so removing works without JS; an off-screen first submit button keeps Enter from ever hitting a ×.
  Series tags sit on the meeting page ("Series tags", one field for every meeting of the series), topic tags under its title.
- **Hub ⋯ menu**: a native popover anchored under the crumb's ⋯ (`.acts-menu`, CSS anchor positioning); each item opens its confirm
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
  under the bar (it would float over content). It opens a `.menu` panel as wide as the capsule, 8px above it (CSS anchor
  positioning on `.newline`, fixed 12px-gutter fallback).
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
  A pick writes readable text, `by 20 Nov` (ISO when a year or more out). "Pick a date…" opens this popover (min = today) anchored to
  the capture bar. A trailing `by` + words that aren't a date save as text with a soft `--muted` note in the hint ("‘by hand’ isn't
  a date · saved as text"); an impossible date (`by 32 Nov`, `by 2026-13-01`) is still an error.
- **RT / IC**: nothing of its own under Reduce Transparency (`.menu` goes solid white + `--line` hairline, no lens). Increase Contrast:
  ink edge (from `.menu`), weekday and out-of-month `--muted-hc` (9.67), today ring 2 px.
- **AA** (T75, each look at its glass floor, Clear + Tinted, over a half-covered ink pixel / the darkest veiled wallpaper, desktop and
  phone sheet): ink 13.58 · `--muted-tint` 7.34 · `--accent` text 4.57 · white on `--accent` 5.57 (7.12 tinted) · hover ink 12.18,
  out-of-month 6.58, today `--accent-2` 5.15 · today and focus rings 4.57 (≥ 3).

## 7. UX patterns (from Notion)

1. **Sidebar** = page list, each row icon + label (icon `--muted`, `--ink` on the selected `.on` row; 16 px desktop, 20 px rail/drawer) grouped by the user's loops (D18): an unlabelled daily block Today · To-dos · Calendar · Meetings, then **Track** Epics · Report, then **Grow** Learn · Notes, then the active epics, and Settings pinned to the bottom (spacer). Same order in the phone drawer; the rail draws the section labels as thin dividers. Breadcrumbs on top.
2. **Every thing is a page**: epic, meeting, topic, note → big title + inline properties, autosaved. No Save buttons anywhere.
3. **One input**: the floating "Type a line…" bar takes the line grammar; a live hint above it says what it will save as.
4. **Feedback**: toast above the bar, "Saved as impact on OPS-101 · Undo", `role=status`.
5. **Toggles** fold secondary things (open plans, report charts, raw numbers).
6. **Notes**: Tiptap; type-to-format markdown shortcuts, `/` block menu (glass popover), `[[` page picker.
7. **Empty states name the next action** ("No epics. Start a line with a key, e.g. OPS-101."). No tutorials: the UI must explain itself.

## 8. Responsive (`responsive.css`)

| Width | Layout |
|---|---|
| ≥ 1024 | sidebar 220–248px + sheet |
| 640–1023 | 64px icon rail (labels become tooltips, section labels become dividers) |
| < 640 | sheet full width with top-rounded corners; nav in a popover; inputs ≥ 16px (no iOS zoom); the capture bar floats 12px inside the edges above the home indicator and rides the keyboard |

## 9. Rules

- Glass on chrome, never on content (unless the user opts into `data-glass=all`).
- One blue. A new colour needs a reason written here.
- Every text/surface pair passes WCAG AA, measured over the worst wallpaper spot (and, for glass over content, a half-covered ink pixel behind); record the number in a CSS comment.
- No `url()` filter reaches WebKit: anything Chromium-only goes behind `@supports (-webkit-app-region:drag)`.
- Honour `prefers-reduced-transparency` and `prefers-contrast`.
- New tokens go in `tokens.css`; components in `components.css`. Inline `style=` in templates only for one-off layout (gap, margin), never colour or type.
