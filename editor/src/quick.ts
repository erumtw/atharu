// Menu-bar quick popover (T81, DESIGN.md §6 "Quick popover"). capture.ts already runs the #/by menus; this adds the
// kind tiles and Esc. Without JS the tiles are a plain radio group and the server applies the picked prefix.
const line = document.getElementById('line') as HTMLInputElement | null
const tiles = document.querySelector<HTMLFieldSetElement>('.kinds')

if (line && tiles) {
  // A typed prefix wins on the server; mirroring it here keeps the tile from ever disagreeing with the line.
  const PREFIX = /^\s*(?:[A-Z][A-Z0-9]*-\d+\s+)?([!x+?>])(?:\s|$)/
  line.addEventListener('input', () => {
    const p = PREFIX.exec(line.value)?.[1]
    if (!p) return
    const r = tiles.querySelector<HTMLInputElement>(`input[value="${CSS.escape(p)}"]`)
    if (r) r.checked = true
  })
  // The line didn't change, so the preview's `changed` filter wouldn't fire: ask for it by name.
  tiles.addEventListener('change', () => line.dispatchEvent(new Event('kindpick')))
  // A pointer pick goes back to typing; arrow keys stay on the tiles (←→ picks). The Note tile focuses its own box.
  tiles.addEventListener('click', ev => {
    if (ev.detail > 0 && !(ev.target as Element).closest('.nt')) line.focus()
  })
}

// Note tile (D53): without JS a GET to /quick?mode=note; here the box switches in place. The kind radio keeps its
// check, so after saving the box is back on the kind it had.
const qcap = document.querySelector<HTMLElement>('.qcap')
const nt = qcap?.querySelector<HTMLButtonElement>('.kc.nt')
const text = document.getElementById('qtext') as HTMLTextAreaElement | null
if (qcap && nt && text && line && tiles) {
  const noteMode = (on: boolean) => {
    qcap.dataset.mode = on ? 'note' : 'line'
    nt.setAttribute('aria-pressed', String(on))
    if (on) {
      // The line's preview says nothing about a note; a half-typed line becomes the note's first line.
      document.getElementById('hint')?.replaceChildren()
      if (line.value.trim() && !text.value.trim()) {
        text.value = line.value
        line.value = ''
      }
    }
    const box = on ? text : line
    box.focus()
    box.setSelectionRange(box.value.length, box.value.length)
  }
  nt.addEventListener('click', ev => {
    ev.preventDefault()
    noteMode(qcap.dataset.mode !== 'note')
  })
  tiles.addEventListener('click', ev => {
    if (qcap.dataset.mode === 'note' && !nt.contains(ev.target as Node)) noteMode(false)
  })
  // D55: `n ` at the start of the line = a note, the rest moves over (the server's no-JS path does the same). Stopped
  // here so htmx's preview doesn't paint a "done" hint over note mode (this listener is added before htmx's).
  line.addEventListener('input', ev => {
    const m = /^\s*n (.*)$/s.exec(line.value)
    if (!m) return
    ev.stopImmediatePropagation()
    line.value = m[1] ?? ''
    noteMode(true)
  })
  // ⌘N anywhere in the popover; a second press keeps note mode rather than toggling it off.
  document.addEventListener('keydown', ev => {
    if (!ev.metaKey || ev.key.toLowerCase() !== 'n') return
    ev.preventDefault()
    noteMode(true)
  })
  // Enter saves, Shift+Enter is a new line (an IME's Enter only commits the word).
  text.addEventListener('keydown', ev => {
    if (ev.key !== 'Enter' || ev.shiftKey || ev.isComposing) return
    ev.preventDefault()
    text.form?.requestSubmit()
  })
  // Saved: the hint already reads "✓ Saved note · Open" (swapped out of band); the box goes back to the line.
  text.form?.addEventListener('htmx:afterRequest', ev => {
    // A 422 counts as `successful` (it swaps into the hint), so check the status, as the line form does.
    const d = (ev as CustomEvent<{ elt: Element; xhr: XMLHttpRequest }>).detail
    if (d.elt !== text.form || d.xhr.status >= 300) return
    text.value = ''
    noteMode(false)
  })
}

// Esc with no menu open (capture.ts claims it while one is) hides the popover. Only in Atharu.app: the window's
// navigation hook sees /quick/close and hides instead of loading it, so no IPC is needed.
document.addEventListener('keydown', ev => {
  if (ev.key !== 'Escape' || ev.defaultPrevented) return
  if (document.documentElement.dataset.app === 'mac') location.assign('/quick/close')
})

// Meeting forms post without htmx's HX-Request, so the meetings slice answers its plain 303 back to /quick (with the
// header it would answer Today's banner partial); the fresh #qmeet is swapped in. Without JS they are plain forms.
document.addEventListener('submit', async ev => {
  const form = ev.target as HTMLFormElement
  // A to-do's "Add a note": a plain POST would land the new note in the popover. Fetched instead, its HX-Redirect
  // becomes a navigation, which the app's hook hands to the main window (as it does for an existing note's link).
  if (form.closest('.qt') && form.action.endsWith('/note')) {
    ev.preventDefault()
    const res = await fetch(form.action, { method: 'POST', headers: { 'HX-Request': 'true' } })
    const to = res.headers.get('HX-Redirect')
    if (to) location.assign(to)
    return
  }
  if (!form.closest('#qmeet')) return
  ev.preventDefault()
  const body = new URLSearchParams(new FormData(form, ev.submitter) as unknown as Record<string, string>)
  const res = await fetch(form.action, { method: 'POST', body })
  const next = new DOMParser().parseFromString(await res.text(), 'text/html').getElementById('qmeet')
  if (next) document.getElementById('qmeet')?.replaceWith(next)
})
