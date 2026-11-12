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
  // A pointer pick goes back to typing; arrow keys stay on the tiles (←→ picks).
  tiles.addEventListener('click', ev => { if (ev.detail > 0) line.focus() })
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
