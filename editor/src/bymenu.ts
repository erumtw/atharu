// `by ` date menu, shared: the capture bar (capture.ts) and any to-do field marked `data-by`.
// `by ` date menu (T80, D44): on a `>` line, typing `by ` offers Today · Tomorrow · the next weekday this week ·
// Next Mon · Pick a date…, built here from the client's date. Nothing is preselected, so Enter still saves the line
// as typed; only arrowing in makes Enter pick. Without JS, `by fri` / `by 20 Nov` typed by hand parse the same.
// `plan`: the field only takes to-dos (To-dos box, follow-up), so no `>` is needed. `rest`: the aria-controls to hand
// back on close (the capture bar's # menu), else none.
export function byMenu(input: HTMLInputElement, box: HTMLElement | null, carrier: HTMLInputElement | null, plan = false, rest: string | null = 'tagsug') {
  const MON = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']
  const DAY = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday']
  // The key is case-sensitive like the parser's; `by` needs some text before it.
  const TRIGGER = plan
    ? /^(?:[A-Z][A-Z0-9]*-\d+\s+)?(?:>\s+)?(?:\S+\s+)+[Bb][Yy]\s+([A-Za-z]*)$/
    : /^(?:[A-Z][A-Z0-9]*-\d+\s+)?>\s+(?:\S+\s+)+[Bb][Yy]\s+([A-Za-z]*)$/
  const id = box?.id ?? 'bysug'
  type Opt = { label: string, keys: string[], date?: Date }
  let shown: Opt[] = []
  let active = -1
  let dismissed = ''

  const at = (n: number) => { const d = new Date(); d.setHours(0, 0, 0, 0); d.setDate(d.getDate() + n); return d }
  const short = (d: Date) => `${DAY[d.getDay()]!.slice(0, 3)} ${d.getDate()} ${MON[d.getMonth()]}`
  const iso = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
  // `20 Nov` reads as the next 20 Nov (parse_when), so a date a year or more out is written in full.
  const words = (d: Date) => {
    const limit = at(0)
    limit.setFullYear(limit.getFullYear() + 1)
    return d < limit ? `${d.getDate()} ${MON[d.getMonth()]}` : iso(d)
  }

  const options = (): Opt[] => {
    const day = (d: Date) => [DAY[d.getDay()]!.toLowerCase()]
    const o: Opt[] = [
      { label: 'Today', keys: ['today', ...day(at(0))], date: at(0) },
      { label: 'Tomorrow', keys: ['tomorrow', ...day(at(1))], date: at(1) },
    ]
    // Monday-first week (D4): the day after tomorrow, while it is still this week.
    const wd = (d: Date) => (d.getDay() + 6) % 7
    if (wd(at(2)) > wd(at(0))) o.push({ label: DAY[at(2).getDay()]!, keys: day(at(2)), date: at(2) })
    const mon = at(7 - wd(at(0)))
    o.push({ label: 'Next Mon', keys: ['next', 'monday'], date: mon })
    o.push({ label: 'Pick a date…', keys: ['pick', 'date'] })
    return o
  }

  const open = () => !!box?.firstChild
  const close = () => {
    box?.replaceChildren()
    shown = []
    active = -1
    input.removeAttribute('aria-activedescendant')
    if (rest) input.setAttribute('aria-controls', rest)
    else input.removeAttribute('aria-controls')
    input.setAttribute('aria-expanded', 'false')
  }

  const draw = () => {
    if (!box) return
    const m = document.createElement('div')
    m.className = 'menu up'
    m.setAttribute('role', 'listbox')
    m.setAttribute('aria-label', 'Due date')
    m.innerHTML = '<div class="mh" role="presentation">Due date</div>' + shown.map((o, i) =>
      `<div role="option" id="${id}-${i}" aria-selected="${i === active}"><span>${o.label}</span>` +
      // The weekday row already names its day: its date goes without it.
      `${o.date ? `<span class="n">${o.label === DAY[o.date.getDay()] ? short(o.date).slice(4) : short(o.date)}</span>` : ''}</div>`).join('') +
      '<div class="mf m-hide" role="presentation">↑↓ pick · Tab insert · ↵ saves as typed</div>'
    box.replaceChildren(m)
    input.setAttribute('aria-controls', id)
    input.setAttribute('aria-expanded', 'true')
    if (active >= 0) input.setAttribute('aria-activedescendant', `${id}-${active}`)
    else input.removeAttribute('aria-activedescendant')
  }

  const update = () => {
    const m = TRIGGER.exec(input.value)
    if (!m) { dismissed = ''; return close() }
    if (dismissed && input.value.startsWith(dismissed)) return close()
    const q = (m[1] ?? '').toLowerCase()
    shown = options().filter(o => !q || o.keys.some(k => k.startsWith(q)))
    // No match: close and leave the text alone (the parser judges it on save).
    if (!shown.length) return close()
    active = Math.min(active, shown.length - 1)
    draw()
  }

  const insert = (d: Date) => {
    input.value = input.value.replace(/[Bb][Yy]\s+[A-Za-z]*$/, `by ${words(d)}`)
    close()
    dismissed = input.value
    input.focus()
    input.setSelectionRange(input.value.length, input.value.length)
    input.dispatchEvent(new Event('input', { bubbles: true }))
  }

  const choose = (o: Opt | undefined) => {
    if (!o) return
    if (o.date) return insert(o.date)
    if (!carrier) return
    // The shared date popover (datepicker.js) opens on a date input's click; this hidden one carries the pick.
    close()
    carrier.min = iso(at(0))
    carrier.value = ''
    carrier.click()
  }
  carrier?.addEventListener('change', () => {
    const [y, mo, d] = carrier.value.split('-').map(Number)
    if (y && mo && d) insert(new Date(y, mo - 1, d))
  })

  input.addEventListener('input', update)
  box?.addEventListener('mousedown', ev => {
    const el = (ev.target as Element).closest<HTMLElement>('[role=option]')
    if (!el) return
    ev.preventDefault()
    choose(shown[Number(el.id.slice(3))])
  })

  const key = (ev: KeyboardEvent): boolean => {
    if (!open()) return false
    if (ev.key === 'ArrowDown' || ev.key === 'ArrowUp') {
      const n = shown.length
      active = ev.key === 'ArrowDown' ? (active + 1) % n : (active <= 0 ? n - 1 : active - 1)
      draw()
    } else if (ev.key === 'Tab' && !ev.shiftKey) {
      choose(shown[Math.max(active, 0)])
    } else if (ev.key === 'Enter' && active >= 0) {
      choose(shown[active])
    } else if (ev.key === 'Escape') {
      dismissed = input.value
      close()
    } else {
      // Enter with nothing arrowed in: the form saves the line as typed.
      if (ev.key === 'Enter') close()
      return false
    }
    ev.preventDefault()
    return true
  }

  return { key, open }
}
