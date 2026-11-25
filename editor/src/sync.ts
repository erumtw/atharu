// Page sync (0.2.13): ask /api/sync on tab focus and every 15 s while visible. Something changed elsewhere (CLI,
// agents, the other server, another tab or the phone) → fire `atharu-sync`, which every live part already re-fetches
// on; a page with none, or a note changed under its editor, shows "Updated elsewhere · Reload" and never reloads itself.
const tab = Math.random().toString(36).slice(2)
// Our own writes carry the page id, so the server can tell them from everyone else's.
document.body.addEventListener('htmx:configRequest', ev => {
  ;(ev as CustomEvent<{ headers: Record<string, string> }>).detail.headers['X-Atharu-Tab'] = tab
})

declare const htmx: { ajax(verb: string, path: string, ctx: { target: string; select: string; swap: string }): Promise<void> }

type Answer = { version: number; seq: number; other: boolean; note: { id: number; updatedAt: string } | null }
const noteId = document.querySelector<HTMLFormElement>('form#note')?.getAttribute('action')?.match(/^\/notes\/(\d+)$/)?.[1]
let last: { version: number; seq: number; noteAt?: string } | null = null
let asking = false
let askedAt = 0
let pending = false
let stale = false

const listeners = () => [...document.querySelectorAll<HTMLElement>('[hx-trigger*="atharu-sync"]')]

// A re-fetch swaps its target whole: wait while the caret is in one (an inline line edit), go on focus-out.
const typingInLive = () => {
  const a = document.activeElement
  if (!a?.matches('input, textarea, select, [contenteditable]')) return false
  return listeners().some(el => {
    const sel = el.getAttribute('hx-target')
    return (sel ? document.querySelector(sel) : el)?.contains(a)
  })
}

function refresh() {
  pending = typingInLive()
  if (!pending) document.body.dispatchEvent(new CustomEvent('atharu-sync'))
}
document.addEventListener('focusout', () => pending && setTimeout(refresh))

// The sidebar's To-dos count and Active epics: this page's own copy of #side, swapped whole. Not while the phone
// drawer is open (the swap would close it under the finger); the next change catches up.
function side() {
  if (!document.querySelector('#side:popover-open')) htmx.ajax('GET', location.href, { target: '#side', select: '#side', swap: 'outerHTML' })
}

function offer() {
  stale = true
  const at = document.getElementById(noteId ? 'saved' : 'jtoast')
  if (!at || at.querySelector('.sync')) return
  const pill = document.createElement('span')
  pill.className = noteId ? 'sync' : 'jtoast-in jsug sync'
  const reload = document.createElement('button')
  reload.type = 'button'
  reload.className = 'linkbtn'
  reload.textContent = 'Reload'
  reload.onclick = () => location.reload()
  pill.append('Updated elsewhere · ', reload)
  // The save status is ours to replace; a Jira toast already in its spot stays beside the offer.
  if (noteId || !at.children.length) at.replaceChildren(pill)
  else at.append(pill)
}
// An autosave (or a Jira toast) rewrites the spot: put the offer back until the page reloads.
document.body.addEventListener('htmx:afterSwap', () => stale && offer())

async function check() {
  if (asking || document.hidden || Date.now() - askedAt < 2000) return
  asking = true
  askedAt = Date.now()
  try {
    const q = new URLSearchParams({ tab })
    if (last) q.set('since', String(last.seq))
    if (noteId) q.set('note', noteId)
    const r = await fetch('/api/sync?' + q)
    if (!r.ok) return
    const a = (await r.json()) as Answer
    const prev = last
    last = { version: a.version, seq: a.seq, noteAt: a.note?.updatedAt }
    // seq going back = the server restarted and its write log is gone: assume something changed.
    if (!prev || (a.version === prev.version && !a.other && a.seq >= prev.seq)) return
    // ponytail: our own autosave landing in the same 15 s as an unrelated outside write reads as "note changed";
    // rare, and the cost is one needless Reload offer. Send the save's updatedAt back if it ever bites.
    side()
    if (noteId && prev.noteAt !== last.noteAt) offer()
    if (listeners().length) refresh()
    else if (!noteId) offer()
  } catch {
    // Server restarting or the laptop offline: the next tick asks again.
  } finally {
    asking = false
  }
}

setInterval(check, 15_000)
document.addEventListener('visibilitychange', check)
addEventListener('focus', check)
check()
