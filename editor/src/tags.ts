// Tag helpers outside the capture bar (D29, DESIGN.md §6): "Make tag" on a selected word in a line's edit box,
// and the Tags prop's keyboard (comma adds, Backspace on empty removes the last chip). Both only drive the
// server's own forms; without JS a typed `#word` still tags and the prop still works by Enter and ×.
export {}

// One word as the tag grammar allows it (a-z 0-9 -, not all digits); typed case is folded by the server.
const WORD = /^[A-Za-z0-9][A-Za-z0-9-]*$/

let menu: HTMLElement | null = null
const close = () => {
  menu?.remove()
  menu = null
}

// Edit boxes are swapped in by htmx, so listen on the document rather than on each input.
const editInput = (t: EventTarget | null): HTMLInputElement | null =>
  t instanceof HTMLInputElement && t.name === 'line' && t.form?.id.startsWith('entry-') ? t : null

let measure: CanvasRenderingContext2D | null = null
const textWidth = (input: HTMLInputElement, text: string) => {
  measure ??= document.createElement('canvas').getContext('2d')
  if (!measure) return 0
  measure.font = getComputedStyle(input).font
  return measure.measureText(text).width
}

const offer = (input: HTMLInputElement) => {
  close()
  const a = input.selectionStart ?? 0
  const b = input.selectionEnd ?? 0
  const word = input.value.slice(a, b)
  const before = input.value[a - 1]
  const after = input.value[b]
  // Whole word only, not already a tag: `#` before it or letters running on would make a different token.
  if (!WORD.test(word) || /^\d+$/.test(word) || (before !== undefined && !/\s/.test(before)) ||
      (after !== undefined && /[A-Za-z0-9_-]/.test(after))) return
  const form = input.form
  if (!form) return
  const tag = word.toLowerCase()
  const m = document.createElement('div')
  m.className = 'menu'
  m.setAttribute('role', 'menu')
  m.setAttribute('aria-label', 'Selection')
  m.style.width = '240px'
  m.style.top = `${input.offsetTop + input.offsetHeight + 6}px`
  m.style.left = `${input.offsetLeft + Math.max(0, textWidth(input, input.value.slice(0, a)) - 12)}px`
  const item = document.createElement('div')
  item.setAttribute('role', 'menuitem')
  item.setAttribute('aria-selected', 'true')
  item.tabIndex = 0
  item.innerHTML = '<span class="k" aria-hidden="true">#</span>Make tag <span class="chip tag"></span>'
  item.querySelector('.chip')!.textContent = tag
  const make = () => {
    input.value = `${input.value.slice(0, a)}#${tag}${input.value.slice(b)}`
    const made = document.createElement('input')
    made.type = 'hidden'
    made.name = 'made'
    made.value = word
    form.append(made)
    close()
    form.requestSubmit()
  }
  item.addEventListener('mousedown', e => e.preventDefault())
  item.addEventListener('click', make)
  item.addEventListener('keydown', e => {
    if (e.key === 'Enter') { e.preventDefault(); make() }
    if (e.key === 'Escape') { e.preventDefault(); close(); input.focus() }
  })
  m.append(item)
  form.style.position = 'relative'
  form.append(m)
  menu = m
}

document.addEventListener('mouseup', e => {
  const input = editInput(e.target)
  if (input) setTimeout(() => offer(input))
  else if (menu && !menu.contains(e.target as Node)) close()
})
document.addEventListener('keyup', e => {
  const input = editInput(e.target)
  if (input && e.shiftKey) offer(input)
})
document.addEventListener('keydown', e => {
  const input = editInput(e.target)
  // ↓ moves into the menu; any other key goes on editing and the offer goes away.
  if (input && menu) {
    if (e.key === 'ArrowDown') { e.preventDefault(); menu.querySelector<HTMLElement>('[role=menuitem]')?.focus() }
    else if (!e.shiftKey) close()
  }
})

// Tags prop: the add box is `[data-tagf]` inside a form whose × buttons are `.rm`.
let refocus: string | null = null
document.addEventListener('keydown', e => {
  const box = e.target
  if (!(box instanceof HTMLInputElement) || !box.hasAttribute('data-tagf') || !box.form) return
  if (e.key === ',' ) {
    e.preventDefault()
    if (box.value.trim()) { refocus = box.id; box.form.requestSubmit() }
  } else if (e.key === 'Backspace' && box.value === '') {
    const chips = box.closest('dd')?.querySelectorAll<HTMLButtonElement>('.rm')
    const last = chips?.[chips.length - 1]
    if (last) { e.preventDefault(); refocus = box.id; last.click() }
  } else if (e.key === 'Enter') {
    refocus = box.id
  }
})
// The prop is re-rendered after each save: put the caret back in the new add box.
document.addEventListener('htmx:afterSettle', () => {
  if (!refocus) return
  document.getElementById(refocus)?.focus()
  refocus = null
})
