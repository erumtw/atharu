// Calendar (0.2.3 D3): with no view in the URL, N days follow the window (≥1200 → 7 · ≥900 → 5 · ≥640 → 3 · phone → 1).
// The server renders the N in the URL (7 by default), so this swaps only on first load and only when the width wants
// another N. It runs from <head> without defer, before the first paint; location.replace keeps Back working.
const url = new URL(location.href)
const w = window.innerWidth
const n = w >= 1200 ? 7 : w >= 900 ? 5 : w >= 640 ? 3 : 1
if (url.pathname === '/calendar' && !url.searchParams.has('view') && String(n) !== (url.searchParams.get('n') ?? '7')) {
  if (n === 7) url.searchParams.delete('n')
  else url.searchParams.set('n', String(n))
  // Leaving mid cross-document view transition rejects its promises ("Transition was skipped"): skip it quietly.
  addEventListener('pagereveal', e => {
    const vt = (e as Event & { viewTransition?: ViewTransition | null }).viewTransition
    vt?.ready.catch(() => {})
    vt?.skipTransition()
  })
  // Nothing of the 7-day page shows while the right N loads.
  document.documentElement.style.visibility = 'hidden'
  location.replace(url)
} else {
  // The hour grid opens scrolled to 08:00.
  addEventListener('DOMContentLoaded', () => {
    const cg = document.querySelector<HTMLElement>('.cg[data-h0]')
    const bs = cg?.querySelector<HTMLElement>('.bs')
    if (!cg || !bs) return
    const px = parseFloat(getComputedStyle(cg).getPropertyValue('--px')) || 46
    bs.scrollTop = Math.max(0, 8 - Number(cg.dataset.h0)) * px
  })
}
