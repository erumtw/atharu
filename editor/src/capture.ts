// Capture bar tags (D29, DESIGN.md §6 "Tag chip"). The server renders the `#` suggestions (#tagsug, out-of-band
// with each /parse-preview) and the "Tag as #x?" offer (#hint .tagask); this file only moves the selection and
// edits the input. Without JS the bar still saves whatever `#tag` was typed. #tagsug is swapped innerHTML, so the
// element found here stays the live one.
const input = document.getElementById('line') as HTMLInputElement | null
const box = document.getElementById('tagsug')
const skip = document.getElementById('notag') as HTMLInputElement | null

if (input && box && skip) {
  const by = byMenu(input)
  const opts = () => Array.from(box.querySelectorAll<HTMLElement>('[role=option]'))
  const picked = () => opts().findIndex(o => o.getAttribute('aria-selected') === 'true')

  // Combobox state follows whatever the last swap left in the box.
  const sync = () => {
    if (by.open()) return
    const o = opts()
    const sel = o[picked()]
    input.setAttribute('aria-controls', 'tagsug')
    input.setAttribute('aria-expanded', String(o.length > 0))
    if (sel) input.setAttribute('aria-activedescendant', sel.id)
    else input.removeAttribute('aria-activedescendant')
  }
  const close = () => {
    box.replaceChildren()
    sync()
  }
  // Re-run the live hint (htmx listens for `input`), so the preview and offers follow the edit.
  const changed = () => input.dispatchEvent(new Event('input', { bubbles: true }))

  // The menu is for the token being typed at the end of the line.
  const insert = (tag: string) => {
    input.value = input.value.replace(/#[A-Za-z0-9-]*$/, `#${tag} `)
    close()
    input.focus()
    changed()
  }

  const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  // "Tag as": the word as typed becomes `#tag` where it stands; the rest of the line is untouched.
  const accept = (ask: HTMLElement) => {
    const { word, tag } = ask.dataset
    if (!word || !tag) return
    input.value = input.value.replace(new RegExp(`(^|\\s)${escape(word)}(?=[\\s.,;:!?)]|$)`), `$1#${tag}`)
    input.focus()
    changed()
  }
  // ×: not a tag. The word rides along in a hidden field, so the server stops offering it for this line.
  const dismiss = (ask: HTMLElement) => {
    const { word } = ask.dataset
    if (!word) return
    skip.value = skip.value ? `${skip.value},${word}` : word
    // The line didn't change, so htmx's `changed` filter won't re-ask the server: drop the offer here.
    ask.closest('.ask')?.remove()
    input.focus()
  }

  input.addEventListener('keydown', ev => {
    if (by.key(ev)) return
    const o = opts()
    if (o.length) {
      const i = picked()
      if (ev.key === 'ArrowDown' || ev.key === 'ArrowUp') {
        ev.preventDefault()
        const n = (i + (ev.key === 'ArrowDown' ? 1 : -1) + o.length) % o.length
        o.forEach((x, j) => x.setAttribute('aria-selected', String(j === n)))
        sync()
        return
      }
      const sel = o[i]
      if ((ev.key === 'Tab' || ev.key === 'Enter') && sel) {
        ev.preventDefault()
        insert(sel.dataset.tag ?? '')
        return
      }
      if (ev.key === 'Escape') {
        ev.preventDefault()
        close()
        return
      }
    }
    const ask = document.querySelector<HTMLElement>('#hint .tagask')
    if (ask && ev.key === 'Tab' && !ev.shiftKey) {
      ev.preventDefault()
      accept(ask)
    }
  })

  // mousedown, not click: keeps focus in the input.
  box.addEventListener('mousedown', ev => {
    const o = (ev.target as Element).closest<HTMLElement>('[role=option]')
    if (!o) return
    ev.preventDefault()
    insert(o.dataset.tag ?? '')
  })

  document.addEventListener('click', ev => {
    const t = ev.target as Element
    const ask = t.closest<HTMLElement>('#hint .tagask')
    if (!ask) return
    if (t.closest('.x')) dismiss(ask)
    else accept(ask)
  })

  document.body.addEventListener('htmx:afterSettle', sync)
}

// `by ` date menu (T80, D44): on a `>` line, typing `by ` offers Today · Tomorrow · the next weekday this week ·
// Next Mon · Pick a date…, built here from the client's date. Nothing is preselected, so Enter still saves the line
// as typed; only arrowing in makes Enter pick. Without JS, `by fri` / `by 20 Nov` typed by hand parse the same.
function byMenu(input: HTMLInputElement) {
  const box = document.getElementById('bysug')
  const carrier = document.getElementById('bydate') as HTMLInputElement | null
  const MON = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']
  const DAY = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday']
  // The key is case-sensitive like the parser's; `by` needs some text before it.
  const TRIGGER = /^(?:[A-Z][A-Z0-9]*-\d+\s+)?>\s+(?:\S+\s+)+[Bb][Yy]\s+([A-Za-z]*)$/
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
    input.setAttribute('aria-controls', 'tagsug')
    input.setAttribute('aria-expanded', 'false')
  }

  const draw = () => {
    if (!box) return
    const m = document.createElement('div')
    m.className = 'menu up'
    m.setAttribute('role', 'listbox')
    m.setAttribute('aria-label', 'Due date')
    m.innerHTML = '<div class="mh" role="presentation">Due date</div>' + shown.map((o, i) =>
      `<div role="option" id="by-${i}" aria-selected="${i === active}"><span>${o.label}</span>` +
      // The weekday row already names its day: its date goes without it.
      `${o.date ? `<span class="n">${o.label === DAY[o.date.getDay()] ? short(o.date).slice(4) : short(o.date)}</span>` : ''}</div>`).join('') +
      '<div class="mf m-hide" role="presentation">↑↓ pick · Tab insert · ↵ saves as typed</div>'
    box.replaceChildren(m)
    input.setAttribute('aria-controls', 'bysug')
    input.setAttribute('aria-expanded', 'true')
    if (active >= 0) input.setAttribute('aria-activedescendant', `by-${active}`)
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
