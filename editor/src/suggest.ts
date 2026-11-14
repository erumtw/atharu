// Our menu in place of the browser's own <datalist> dropdown (DESIGN.md §6 "Suggest menu"): that one can't be
// styled or widened, so long names were cut ("cms-customer-") and it looked like nothing else here. Any input with
// `list=` gets it; the capture bar (#line) and the popover start box (#qname) run their own menus. Without JS the
// datalist still works. Delegated from document: htmx swaps replace these inputs.
// A module (own scope): the other bundles are scripts sharing tsc's global names (`input`, `close`).
export {}

const OWN = new Set(['line', 'qname'])
const MAX = 12
let input: HTMLInputElement | null = null
let names: string[] = []
let sel = -1
let menu: HTMLDivElement | null = null

const close = () => {
  menu?.remove()
  menu = null
  sel = -1
  input?.setAttribute('aria-expanded', 'false')
  input?.removeAttribute('aria-activedescendant')
}

const place = () => {
  if (!menu || !input) return
  // The rounded field around the input (it also holds e.g. the Start button), else the input itself.
  const r = (input.closest('.fld') ?? input).getBoundingClientRect()
  // The box's own width (owner: the native list was narrower than its field). Tiny boxes ("Filter by tag", 160)
  // still get room for a name.
  const w = Math.max(r.width, 220)
  menu.style.left = `${Math.max(8, Math.min(r.left, innerWidth - w - 8))}px`
  menu.style.top = `${r.bottom + 6}px`
  menu.style.width = `${w}px`
}

const draw = () => {
  if (!input) return
  const q = input.value.trim().toLowerCase()
  const hits = names.filter(n => n.toLowerCase().includes(q) && n !== input?.value).slice(0, MAX)
  if (!hits.length) return close()
  if (!menu) {
    menu = document.createElement('div')
    menu.className = 'menu sugm'
    menu.id = 'sugm'
    menu.setAttribute('role', 'listbox')
    document.body.append(menu)
    input.setAttribute('aria-controls', 'sugm')
    input.setAttribute('aria-expanded', 'true')
  }
  menu.replaceChildren(
    ...hits.map((n, i) => {
      const o = document.createElement('div')
      o.setAttribute('role', 'option')
      o.id = `sugm-${i}`
      o.title = n
      o.dataset.name = n
      o.setAttribute('aria-selected', String(i === sel))
      const t = document.createElement('span')
      t.className = 'clip'
      t.textContent = n
      o.append(t)
      return o
    }),
  )
  place()
}

const opts = () => Array.from(menu?.querySelectorAll<HTMLElement>('[role=option]') ?? [])
const select = (i: number) => {
  sel = i
  opts().forEach((o, k) => o.setAttribute('aria-selected', String(k === i)))
  const o = opts()[i]
  if (o) {
    input?.setAttribute('aria-activedescendant', o.id)
    o.scrollIntoView({ block: 'nearest' })
  }
}

// A pick fills the box and lets its form react as if typed: the tag-filter GET and the meeting start submit,
// an autosaving form (series tags) hears `change`.
const pick = (name: string) => {
  const el = input
  if (!el) return
  el.value = name
  close()
  el.dispatchEvent(new Event('input', { bubbles: true }))
  el.dispatchEvent(new Event('change', { bubbles: true }))
  const f = el.form
  if (f && (f.method === 'get' || f.id === 'start-meeting')) f.requestSubmit()
  el.focus()
}

document.addEventListener('focusin', ev => {
  const el = ev.target
  if (!(el instanceof HTMLInputElement) || OWN.has(el.id)) return
  const id = el.getAttribute('list') ?? el.dataset.list
  if (!id) return
  // Keep the id for later swaps; drop `list` so the native dropdown doesn't open on top of ours.
  el.dataset.list = id
  el.removeAttribute('list')
  el.setAttribute('role', 'combobox')
  el.setAttribute('aria-autocomplete', 'list')
  input = el
  names = Array.from(document.getElementById(id)?.querySelectorAll('option') ?? [], o => o.value).filter(Boolean)
  sel = -1
  draw()
})
document.addEventListener('input', ev => {
  if (ev.target === input && ev.isTrusted) {
    sel = -1
    draw()
  }
})
document.addEventListener('focusout', ev => {
  if (ev.target === input) setTimeout(() => document.activeElement !== input && close(), 150)
})
document.addEventListener('mousedown', ev => {
  const o = (ev.target as Element).closest<HTMLElement>('#sugm [role=option]')
  if (!o?.dataset.name) return
  ev.preventDefault()
  pick(o.dataset.name)
})
document.addEventListener(
  'keydown',
  ev => {
    if (ev.target !== input || !menu) return
    const n = opts().length
    if (ev.key === 'ArrowDown' || ev.key === 'ArrowUp') {
      ev.preventDefault()
      select(ev.key === 'ArrowDown' ? (sel + 1) % n : sel <= 0 ? n - 1 : sel - 1)
    } else if ((ev.key === 'Enter' || ev.key === 'Tab') && sel >= 0) {
      // Only an arrowed-to option is picked: Enter on a typed new value keeps that value.
      ev.preventDefault()
      pick(opts()[sel]?.dataset.name ?? '')
    } else if (ev.key === 'Escape') {
      ev.preventDefault()
      ev.stopImmediatePropagation()
      close()
    }
  },
  true,
)
addEventListener('resize', place)
document.addEventListener('scroll', place, true)
