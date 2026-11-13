import { byMenu } from './bymenu'

// Plain to-do fields (To-dos box, the follow-up after a tick) opt in with `data-by`: bound on first focus, so a field
// htmx swaps in later gets it too. The menu is fixed on <body> under the field: the To-dos box is glass (.newline's
// backdrop-filter is a Backdrop Root, so a menu inside it would blur only the capsule) and a row is too short to hold it.
const bound = new WeakSet<HTMLInputElement>()
let n = 0
document.addEventListener('focusin', ev => {
  const input = ev.target
  if (!(input instanceof HTMLInputElement) || !input.matches('[data-by]') || bound.has(input)) return
  bound.add(input)
  n += 1
  // datepicker.js anchors the "Pick a date…" popover on the carrier's data-for id.
  if (!input.id) input.id = `byin-${n}`
  const box = document.createElement('div')
  box.id = `bysug-${n}`
  box.style.cssText = 'position:fixed;z-index:20'
  const carrier = document.createElement('input')
  carrier.type = 'date'
  carrier.hidden = true
  carrier.tabIndex = -1
  carrier.dataset.for = input.id
  carrier.setAttribute('aria-label', 'Due date')
  document.body.append(box, carrier)
  const place = () => {
    const r = input.getBoundingClientRect()
    box.style.left = `${r.left}px`
    box.style.top = `${r.bottom + 4}px`
  }
  input.addEventListener('input', place)
  const by = byMenu(input, box, carrier, true, null)
  // Runs after the field's own hx-on keydown, which leaves Esc alone while the menu is open (aria-expanded).
  input.addEventListener('keydown', e => { if (by.key(e)) e.stopImmediatePropagation() })
  // The field went away (follow-up answered, list re-rendered): its menu goes too.
  new MutationObserver((_, o) => {
    if (input.isConnected) return
    box.remove()
    carrier.remove()
    o.disconnect()
  }).observe(document.body, { childList: true, subtree: true })
})
