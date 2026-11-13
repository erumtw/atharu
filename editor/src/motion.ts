// Motion that CSS can't do alone (DESIGN.md §6 "Motion", D61): rows that moved glide from where they were (FLIP),
// rows that are new fade in, the meeting banner grows instead of jumping, and every segmented switch gets one white
// thumb that slides. htmx replaces whole sections, so "before" is measured at htmx:beforeSwap and "after" at
// htmx:afterSettle. Reduce Motion: nothing here moves (CSS keeps its short fades).
export {}

const reduce = () => matchMedia('(prefers-reduced-motion: reduce)').matches
const ROWS = '[id^="entry-"], [id^="plan-"]'
const DUR = 250
const EASE = 'cubic-bezier(.2, .8, .2, 1)'

let before = new Map<string, DOMRect>()
let bannerH: number | null = null
// Set by a real swap: the first row on an empty day has nothing "before" and must still fade in.
let armed = false

document.addEventListener('htmx:beforeSwap', () => {
  before = new Map(Array.from(document.querySelectorAll<HTMLElement>(ROWS), el => [el.id, el.getBoundingClientRect()]))
  bannerH = document.getElementById('meeting')?.getBoundingClientRect().height ?? null
  armed = true
})

document.addEventListener('htmx:afterSettle', () => {
  segs()
  if (reduce() || !armed) return finish()
  for (const el of document.querySelectorAll<HTMLElement>(ROWS)) {
    const was = before.get(el.id)
    if (!was) {
      el.classList.add('fresh')
      el.addEventListener('animationend', () => el.classList.remove('fresh'), { once: true })
      continue
    }
    const now = el.getBoundingClientRect()
    const dx = was.left - now.left
    const dy = was.top - now.top
    // Same place (a re-render that changed nothing visible, e.g. a poll): leave it alone.
    if (Math.abs(dx) < 2 && Math.abs(dy) < 2) continue
    el.animate([{ transform: `translate(${dx}px, ${dy}px)` }, { transform: 'none' }], { duration: DUR, easing: EASE })
  }
  const m = document.getElementById('meeting')
  if (m && bannerH !== null) {
    const h = m.getBoundingClientRect().height
    // The banner polls every second; only a real change of size (start / record / end) animates.
    if (Math.abs(h - bannerH) > 2) {
      m.animate([{ height: `${bannerH}px`, overflow: 'hidden' }, { height: `${h}px`, overflow: 'hidden' }], {
        duration: DUR,
        easing: EASE,
      })
    }
  }
  finish()
})

const finish = () => {
  before = new Map()
  bannerH = null
  armed = false
}

// Segmented switches: one thumb per .seg under the picked button; a click slides it there at once (the page may
// navigate right after; it reopens with the thumb already in place and the view transition cross-fades the rest).
const active = (seg: HTMLElement) => seg.querySelector<HTMLElement>(':scope > :is(.on, [aria-pressed="true"])')
const put = (seg: HTMLElement, to: HTMLElement | null, animate: boolean) => {
  const t = seg.querySelector<HTMLElement>(':scope > .thumb')
  if (!t) return
  if (!to) {
    t.hidden = true
    return
  }
  t.hidden = false
  t.classList.toggle('still', !animate)
  t.style.transform = `translateX(${to.offsetLeft}px)`
  t.style.width = `${to.offsetWidth}px`
}
// The picked label turns semibold after the thumb was measured (and fonts or a closed popover can change sizes too):
// the thumb then stopped short of the label ("Liquid Gl|ass"). Re-measure whenever a switch's buttons change size.
const resized = new ResizeObserver(entries => {
  for (const e of entries) {
    const seg = (e.target as Element).closest<HTMLElement>('.seg.has-thumb')
    if (seg) put(seg, active(seg), false)
  }
})
const segs = () => {
  for (const seg of document.querySelectorAll<HTMLElement>('.seg')) {
    if (!seg.querySelector(':scope > .thumb')) {
      for (const b of seg.querySelectorAll<HTMLElement>(':scope > :is(button,a)')) resized.observe(b)
      const t = document.createElement('span')
      t.className = 'thumb still'
      t.setAttribute('aria-hidden', 'true')
      seg.prepend(t)
      seg.classList.add('has-thumb')
    }
    put(seg, active(seg), false)
  }
}
document.addEventListener('click', ev => {
  const b = (ev.target as Element).closest<HTMLElement>('.seg > button')
  const seg = b?.parentElement
  if (!b || !seg?.classList.contains('has-thumb')) return
  put(seg, b, !reduce())
})
// A switch inside a closed popover measures 0 wide at load: place its thumb when the popover opens.
document.addEventListener('toggle', ev => {
  const pop = ev.target as HTMLElement
  if ((ev as ToggleEvent).newState !== 'open') return
  for (const seg of pop.querySelectorAll<HTMLElement>('.seg.has-thumb')) put(seg, active(seg), false)
}, true)
// glass.js flips aria-pressed in place: follow it. Not `class`: the thumb's own class flips would loop back here.
new MutationObserver(ms => {
  for (const m of ms) {
    const seg = (m.target as Element).closest?.('.seg') as HTMLElement | null
    if (seg?.classList.contains('has-thumb')) put(seg, active(seg), !reduce())
  }
}).observe(document.documentElement, { subtree: true, attributes: true, attributeFilter: ['aria-pressed'] })

if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', segs)
else segs()
addEventListener('resize', segs)
