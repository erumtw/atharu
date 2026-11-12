// Shared date popover (D38, DESIGN.md §6 "Date picker"). It replaces only the browser's picker UI: every
// `input[type=date]` stays the value carrier and the no-JS fallback, so forms post unchanged. Delegated from the
// document because plan rows and the event form are swapped in by htmx.
export {}

const MONTHS = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December']
// Monday first (D4).
const DAYS = ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday']
const ICON = (d: string) => `<svg class="ic" viewBox="0 0 24 24" aria-hidden="true"><path d="${d}"/></svg>`

// Dates as UTC midnights: day arithmetic never meets a DST jump.
const parse = (s: string): Date | null => /^\d{4}-\d{2}-\d{2}$/.test(s) ? new Date(`${s}T00:00:00Z`) : null
const iso = (d: Date) => d.toISOString().slice(0, 10)
const addDays = (d: Date, n: number) => new Date(d.getTime() + n * 864e5)
// Same day of month, clamped (31 Jan + 1 month = 28/29 Feb).
const addMonths = (d: Date, n: number) => {
  const y = d.getUTCFullYear(), m = d.getUTCMonth() + n
  const last = new Date(Date.UTC(y, m + 1, 0)).getUTCDate()
  return new Date(Date.UTC(y, m, Math.min(d.getUTCDate(), last)))
}
const weekday = (d: Date) => (d.getUTCDay() + 6) % 7
const todayIso = () => {
  const n = new Date()
  return iso(new Date(Date.UTC(n.getFullYear(), n.getMonth(), n.getDate())))
}

const phone = matchMedia('(max-width: 639px)')

// One panel for the whole page, built on first use.
let dp: HTMLElement | null = null
let input: HTMLInputElement | null = null
let view = new Date() // first of the shown month
let focusDay = new Date()
let picked = '' // the sheet's selection before Done; on desktop a pick commits at once
let swallow = false

const panel = (): HTMLElement => {
  if (dp) return dp
  const p = document.createElement('div')
  p.className = 'menu dp'
  p.id = 'dp'
  // Manual, not auto: an auto popover would light-dismiss the ⋯ sheet a due field lives in when it opens.
  p.setAttribute('popover', 'manual')
  p.setAttribute('role', 'dialog')
  p.setAttribute('aria-modal', 'false')
  p.setAttribute('aria-labelledby', 'dp-m')
  p.innerHTML = '<div class="dph"><h2 id="dp-m" aria-live="polite"></h2>' +
    `<button class="ib" type="button" data-m="-1" aria-label="Previous month">${ICON('M15 6l-6 6 6 6')}</button>` +
    `<button class="ib" type="button" data-m="1" aria-label="Next month">${ICON('M9 6l6 6-6 6')}</button></div>` +
    '<table role="grid" aria-labelledby="dp-m"><thead><tr>' +
    DAYS.map(d => `<th scope="col" abbr="${d}">${d.slice(0, 2)}</th>`).join('') + '</tr></thead><tbody></tbody></table>' +
    '<div class="dpf"><button class="linkbtn clear" type="button">Clear</button><button class="linkbtn today" type="button">Today</button>' +
    '<button class="btn done" type="button">Done</button></div>'
  p.addEventListener('click', onPanelClick)
  p.addEventListener('keydown', onPanelKey)
  // Swipe down closes the sheet (no change), like the ⋯ sheet.
  let y0 = 0
  p.addEventListener('touchstart', e => { y0 = e.touches[0]?.clientY ?? 0 }, { passive: true })
  p.addEventListener('touchend', e => { if ((e.changedTouches[0]?.clientY ?? 0) - y0 > 80) close(false) })
  document.body.append(p)
  dp = p
  return p
}

const inRange = (s: string) => !!input && !(input.min && s < input.min) && !(input.max && s > input.max)

const render = (slide = 0) => {
  if (!dp || !input) return
  const y = view.getUTCFullYear(), m = view.getUTCMonth()
  dp.querySelector('h2')!.textContent = `${MONTHS[m]} ${y}`
  const today = todayIso()
  const focus = iso(focusDay)
  // Always 6 rows from the Monday on or before the 1st: the panel never changes height.
  let d = addDays(view, -weekday(view))
  let rows = ''
  for (let r = 0; r < 6; r++) {
    rows += '<tr>'
    for (let c = 0; c < 7; c++, d = addDays(d, 1)) {
      const s = iso(d)
      const label = `${DAYS[c]} ${d.getUTCDate()} ${MONTHS[d.getUTCMonth()]} ${d.getUTCFullYear()}`
      rows += `<td class="d${d.getUTCMonth() === m ? '' : ' out'}" data-d="${s}" aria-label="${label}" tabindex="${s === focus ? 0 : -1}"` +
        `${s === today ? ' aria-current="date"' : ''}${inRange(s) ? '' : ' aria-disabled="true"'} aria-selected="${s === picked}">${d.getUTCDate()}</td>`
    }
    rows += '</tr>'
  }
  const old = dp.querySelector('tbody')!
  const body = document.createElement('tbody')
  body.innerHTML = rows
  if (slide) body.className = slide > 0 ? 'nx' : 'pv'
  old.replaceWith(body)
  // Clear only where empty is a valid value.
  dp.querySelector<HTMLElement>('.clear')!.hidden = input.required
}

// Move the roving focus; crossing a month edge re-renders that month.
const moveTo = (d: Date, focusCell = true) => {
  const slide = (d.getUTCFullYear() - view.getUTCFullYear()) * 12 + d.getUTCMonth() - view.getUTCMonth()
  focusDay = d
  if (slide) {
    view = new Date(Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), 1))
    render(slide)
  } else {
    dp?.querySelectorAll('td.d').forEach(td => td.setAttribute('tabindex', td.getAttribute('data-d') === iso(d) ? '0' : '-1'))
  }
  if (focusCell) dp?.querySelector<HTMLElement>(`td[data-d="${iso(d)}"]`)?.focus()
}

const place = () => {
  if (!dp || !input) return
  if (phone.matches) {
    dp.style.top = dp.style.left = ''
    return
  }
  // The day nav's input is laid over one segment: anchor to the whole capsule, end edge to end edge.
  const nav = input.closest('.dnav')
  const r = (nav ?? input).getBoundingClientRect()
  const w = dp.offsetWidth, h = dp.offsetHeight
  const left = nav || r.left + w > innerWidth - 8 ? r.right - w : r.left
  const below = r.bottom + 6
  dp.style.left = `${Math.max(8, left)}px`
  dp.style.top = `${below + h > innerHeight - 8 && r.top - 6 - h > 8 ? r.top - 6 - h : below}px`
}

const open = (el: HTMLInputElement) => {
  const p = panel()
  input = el
  picked = el.value
  focusDay = parse(el.value) ?? parse(todayIso())!
  view = new Date(Date.UTC(focusDay.getUTCFullYear(), focusDay.getUTCMonth(), 1))
  render()
  p.showPopover()
  place()
  p.querySelector<HTMLElement>('td[tabindex="0"]')?.focus()
}

const close = (refocus = true) => {
  if (!dp?.matches(':popover-open')) return
  dp.hidePopover()
  const el = input
  input = null
  // On a phone, focusing the field again can bring up the OS picker over the page.
  if (refocus && !phone.matches) el?.focus({ preventScroll: true })
}

// The existing listeners (htmx triggers, the epic autosave, the day nav's submit) all hang off `change`.
const commit = (value: string) => {
  const el = input
  close()
  if (!el || el.value === value) return
  el.value = value
  // A plan row's `change` swaps the row out (outerHTML): find the same field in the new row afterwards.
  const row = el.closest('[id]')?.id
  if (row && !phone.matches) refind = `#${CSS.escape(row)} input[name="${el.name}"]`
  el.dispatchEvent(new Event('input', { bubbles: true }))
  el.dispatchEvent(new Event('change', { bubbles: true }))
}

let refind = ''
document.addEventListener('htmx:afterSettle', () => {
  if (refind && (document.activeElement === document.body || !document.activeElement)) {
    document.querySelector<HTMLElement>(refind)?.focus({ preventScroll: true })
  }
  refind = ''
})

// Desktop: a pick commits. Phone sheet: a tap only selects, Done commits.
const pick = (s: string) => {
  if (!inRange(s)) return
  if (!phone.matches) return commit(s)
  picked = s
  moveTo(parse(s)!, false)
  dp?.querySelectorAll('td.d').forEach(td => td.setAttribute('aria-selected', String(td.getAttribute('data-d') === s)))
}

function onPanelClick(e: MouseEvent) {
  const t = e.target as Element
  const cell = t.closest<HTMLElement>('td.d')
  if (cell?.dataset.d) return pick(cell.dataset.d)
  const btn = t.closest<HTMLButtonElement>('button')
  if (!btn) return
  if (btn.dataset.m) {
    const n = Number(btn.dataset.m)
    focusDay = addMonths(focusDay, n)
    view = new Date(Date.UTC(focusDay.getUTCFullYear(), focusDay.getUTCMonth(), 1))
    render(n)
  } else if (btn.classList.contains('clear')) commit('')
  else if (btn.classList.contains('today')) pick(todayIso())
  else if (btn.classList.contains('done')) commit(picked)
}

function onPanelKey(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    e.preventDefault()
    return close()
  }
  if (e.key === 'Tab' && dp) {
    // Keep Tab inside the panel: ‹ › → grid → footer → ‹.
    const stops = Array.from(dp.querySelectorAll<HTMLElement>('button:not([hidden]), td[tabindex="0"]')).filter(x => x.offsetParent)
    const i = stops.indexOf(document.activeElement as HTMLElement)
    const n = e.shiftKey ? (i <= 0 ? stops.length - 1 : i - 1) : (i + 1) % stops.length
    e.preventDefault()
    return stops[n]?.focus()
  }
  if (!(e.target as Element).matches('td.d')) return
  const steps: Record<string, () => Date> = {
    ArrowLeft: () => addDays(focusDay, -1),
    ArrowRight: () => addDays(focusDay, 1),
    ArrowUp: () => addDays(focusDay, -7),
    ArrowDown: () => addDays(focusDay, 7),
    PageUp: () => addMonths(focusDay, e.shiftKey ? -12 : -1),
    PageDown: () => addMonths(focusDay, e.shiftKey ? 12 : 1),
    Home: () => addDays(focusDay, -weekday(focusDay)),
    End: () => addDays(focusDay, 6 - weekday(focusDay)),
  }
  const step = steps[e.key]
  if (step) {
    e.preventDefault()
    moveTo(step())
  } else if (e.key === 'Enter' || e.key === ' ') {
    e.preventDefault()
    // Keyboard Enter commits on the sheet too: there is no tap to confirm.
    const s = iso(focusDay)
    if (inRange(s)) commit(s)
  }
}

const dateInput = (t: EventTarget | null): HTMLInputElement | null =>
  t instanceof HTMLInputElement && t.type === 'date' && !t.disabled && !t.readOnly ? t : null

// Any press outside closes without a change. On the phone sheet the press lands on the dimmed page (the
// popover's ::backdrop lets it through), so its click is swallowed rather than following a link underneath.
document.addEventListener('pointerdown', e => {
  const el = dateInput(e.target)
  if (el) e.preventDefault() // no focus, so no OS picker; the click below opens ours
  if (!dp?.matches(':popover-open') || dp.contains(e.target as Node) || el === input) return
  swallow = phone.matches
  close(false)
}, true)

document.addEventListener('click', e => {
  if (swallow) {
    swallow = false
    e.preventDefault()
    e.stopPropagation()
    return
  }
  const el = dateInput(e.target)
  if (!el) return
  e.preventDefault()
  if (el === input) close()
  else open(el)
}, true)

document.addEventListener('keydown', e => {
  const el = dateInput(e.target)
  if (!el || el === input) return
  // Enter would also submit the field's form (report range): here it opens the picker instead.
  if (e.key === 'Enter' || e.key === ' ' || (e.altKey && e.key === 'ArrowDown')) {
    e.preventDefault()
    open(el)
  }
})

// The panel is fixed to the viewport: follow the field while the page scrolls, close if the layout changes.
addEventListener('scroll', () => { if (input) place() }, true)
addEventListener('resize', () => close(false))
