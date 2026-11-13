// Launch animation (DESIGN.md §6 "Motion", 2026-11-13). The look's icon is inlined so its paths can be masked (the
// ensō is brushed on in one stroke) and its ink dot split (the dot is thrown); launch.css owns every timing.
// Runs full-window on /launch (then replaces the page with Today) and as the small preview on /settings.
export {}

const NS = 'http://www.w3.org/2000/svg'
type Variant = 'ink' | 'glass'

const reduce = () => matchMedia('(prefers-reduced-motion: reduce)').matches
const variant = (el: HTMLElement): Variant => (el.dataset.launch === 'glass' ? 'glass' : 'ink')

const icons = new Map<string, Promise<string>>()
const icon = (look: string) => {
  let p = icons.get(look)
  if (!p) {
    // A missing icon must not strand the app on a blank screen: the animation just plays without it.
    p = fetch(`/static/icon/${look}.svg`).then(r => (r.ok ? r.text() : '')).catch(() => '')
    icons.set(look, p)
  }
  return p
}

// The ensō gets a mask: a fat butt-capped circle along the ring, dashed so CSS can unroll it like a brush. One stroke
// runs head → tail through the ring's gap (a round cap would reach back across it and light the tail early).
const brush = (svg: SVGSVGElement) => {
  const paths = [...svg.querySelectorAll<SVGPathElement>('path')].filter(p => !p.closest('defs'))
  if (!paths.length) return
  let x0 = Infinity
  let y0 = Infinity
  let x1 = -Infinity
  let y1 = -Infinity
  for (const p of paths) {
    const b = p.getBBox()
    x0 = Math.min(x0, b.x)
    y0 = Math.min(y0, b.y)
    x1 = Math.max(x1, b.x + b.width)
    y1 = Math.max(y1, b.y + b.height)
  }
  const cx = (x0 + x1) / 2
  const cy = (y0 + y1) / 2
  const R = Math.max(x1 - x0, y1 - y0) / 2
  // Not laid out (hidden): leave the icon whole rather than masked to nothing.
  if (!(R > 0)) return
  // Where the ink actually is around the ring (1° steps, a few radii): the largest run of empty degrees is the gap
  // between the tail and the head.
  const pt = new DOMPoint()
  const ink: boolean[] = []
  for (let a = 0; a < 360; a++) {
    const t = (a * Math.PI) / 180
    let hit = false
    for (let k = 0.62; k <= 1.0 && !hit; k += 0.04) {
      pt.x = cx + Math.cos(t) * R * k
      pt.y = cy + Math.sin(t) * R * k
      hit = paths.some(p => p.isPointInFill(pt))
    }
    ink.push(hit)
  }
  let best = 0
  let bestEnd = 0
  let run = 0
  for (let i = 0; i < 720; i++) {
    if (ink[i % 360]) run = 0
    else if (++run > best) {
      best = run
      bestEnd = i % 360
    }
  }
  const head = (bestEnd + 1) % 360
  const sweep = Math.min(360, 360 - best + 6)
  const defs = svg.querySelector('defs') ?? svg.insertBefore(document.createElementNS(NS, 'defs'), svg.firstChild)
  const mask = document.createElementNS(NS, 'mask')
  mask.id = 'launch-brush'
  mask.setAttribute('maskUnits', 'userSpaceOnUse')
  for (const [k, v] of [['x', 0], ['y', 0], ['width', 1024], ['height', 1024]] as const) mask.setAttribute(k, String(v))
  const c = document.createElementNS(NS, 'circle')
  c.setAttribute('class', 'brush')
  for (const [k, v] of [['cx', cx], ['cy', cy], ['r', R * 0.82], ['stroke-width', R * 0.7], ['pathLength', 1]] as const) {
    c.setAttribute(k, String(v))
  }
  c.setAttribute('fill', 'none')
  c.setAttribute('stroke', '#fff')
  c.setAttribute('stroke-linecap', 'butt')
  c.style.setProperty('--len', (sweep / 360).toFixed(4))
  // SVG circles start at 3 o'clock and run clockwise: turn the start onto the head.
  c.setAttribute('transform', `rotate(${head - 3} ${cx} ${cy})`)
  mask.append(c)
  defs.append(mask)
  for (const p of paths) p.setAttribute('mask', 'url(#launch-brush)')
}

// สาดสี: the biggest circle of the ink dot is the blob that slaps the paper; the others fly out of it to where the
// icon has them; a seeded spray (the same splash every play) scatters past and fades. Sumi / Sora / Wakaba keep the
// dot in a `-dab` filtered group; the Glass icon has it as plain white circles in a `-glass` filtered group.
const DAB = 'g[filter$="-dab)"]:has(> circle), g[filter$="-glass)"][fill="rgba(255,255,255,.94)"]'
const splash = (svg: SVGSVGElement) => {
  const dab = svg.querySelector<SVGGElement>(DAB)
  const circles = [...(dab?.querySelectorAll<SVGCircleElement>('circle') ?? [])]
  const [first] = circles
  if (!dab || !first) return
  const num = (c: Element, a: string) => Number(c.getAttribute(a))
  const blob = circles.reduce((a, b) => (num(b, 'r') > num(a, 'r') ? b : a), first)
  const bx = num(blob, 'cx')
  const by = num(blob, 'cy')
  const br = num(blob, 'r')
  dab.classList.add('dab')
  blob.classList.add('blob')
  circles
    .filter(c => c !== blob)
    .forEach((c, j) => {
      c.classList.add('drop')
      c.style.setProperty('--j', String(j))
      c.style.setProperty('--dx', `${bx - num(c, 'cx')}px`)
      c.style.setProperty('--dy', `${by - num(c, 'cy')}px`)
    })
  let seed = 7
  const rnd = () => (seed = (seed * 16807) % 2147483647) / 2147483647
  for (let j = 0; j < 14; j++) {
    const a = rnd() * Math.PI * 2
    const d = br * (1.3 + rnd() * 1.6)
    const c = document.createElementNS(NS, 'circle')
    c.setAttribute('class', 'spray')
    c.setAttribute('cx', String(bx + Math.cos(a) * br * 0.6))
    c.setAttribute('cy', String(by + Math.sin(a) * br * 0.6))
    c.setAttribute('r', (2 + rnd() * 5).toFixed(1))
    c.style.setProperty('--j', String(j))
    c.style.setProperty('--tx', `${(Math.cos(a) * d).toFixed(0)}px`)
    c.style.setProperty('--ty', `${(Math.sin(a) * d).toFixed(0)}px`)
    dab.append(c)
  }
}

// Fills .logo with the current look's icon. One inline copy per page at a time, so the file's own ids stay unique.
const mount = async (el: HTMLElement, motion: boolean) => {
  const logo = el.querySelector<HTMLElement>('.logo')
  if (!logo) return
  const svg = await icon(document.documentElement.dataset.look ?? 'sumi')
  logo.innerHTML = `<span class="glow"></span>${svg}<span class="sheen"></span><span class="ring"></span><span class="ring r2"></span>`
  const mark = logo.querySelector('svg')
  if (!mark) return
  mark.classList.add('mark')
  mark.setAttribute('aria-hidden', 'true')
  // Reduce Motion plays the finished mark: no brush, no splash.
  if (motion) {
    brush(mark)
    splash(mark)
  }
}

const prepare = async (el: HTMLElement, v: Variant, motion: boolean) => {
  el.classList.remove('play', 'still')
  el.classList.toggle('a', v === 'ink')
  el.classList.toggle('b', v === 'glass')
  await mount(el, motion)
}

// The finished mark at rest, no animation.
const rest = async (el: HTMLElement, v: Variant) => {
  await prepare(el, v, false)
  el.classList.add('still')
}

// Resolves when CSS animation `name` ends on `el`. The timer is for a window that never gets to run it (hidden at
// launch): the app must still open.
const ended = (el: HTMLElement, name: string) =>
  new Promise<void>(done => {
    const t = setTimeout(done, 3600)
    el.addEventListener('animationend', e => {
      if (e.animationName !== name) return
      clearTimeout(t)
      done()
    })
  })

// Plays once and resolves when the sheet has cleared (Reduce Motion: when the mark has faded in). A `.keep` preview
// never clears, so it resolves at once.
const run = async (el: HTMLElement, v: Variant) => {
  const quiet = reduce()
  await prepare(el, v, !quiet)
  const last = el.querySelector<HTMLElement>(quiet ? '.logo' : '.plate')
  const end = last && !el.classList.contains('keep') ? ended(last, quiet ? 'ln-fade' : 'ln-plate-out') : Promise.resolve()
  if (quiet) el.classList.add('still')
  void el.offsetWidth // restart the animations on a replay
  el.classList.add('play')
  await end
}

const page = document.querySelector<HTMLElement>('main.launch')
if (page) {
  // Today isn't cacheable (no validators), so this only wakes the server and DB; the view-transition dissolve hides the rest.
  void fetch('/').catch(() => {})
  run(page, variant(page))
    .catch(console.error)
    .then(() => location.replace(page.dataset.then ?? '/'))
}

// Settings › Launch animation: the preview sits at rest on the finished mark; picking a variant or clicking it plays.
const form = document.querySelector<HTMLFormElement>('form#launch')
const prev = form?.querySelector<HTMLElement>('.launch.mini')
if (form && prev) {
  const row = prev.closest<HTMLElement>('.lprow')
  const play = () => run(prev, variant(prev)).catch(console.error)
  if (prev.dataset.launch !== 'off') void rest(prev, variant(prev)).catch(console.error)
  prev.addEventListener('click', () => void play())
  form.addEventListener('click', ev => {
    const btn = (ev.target as Element | null)?.closest<HTMLButtonElement>('button[name=launch]')
    if (!btn) return
    for (const b of form.querySelectorAll('button[name=launch]')) b.setAttribute('aria-pressed', String(b === btn))
    prev.dataset.launch = btn.value
    if (row) row.hidden = btn.value === 'off'
    if (btn.value !== 'off') void play()
  })
}
