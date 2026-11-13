// Visual check: load every main page at 1180 / 900 / 390 px in headless Chrome (CDP, no dependencies) and fail
// on layout bugs real data shows and the demo DB hides. Run through scripts/visual-check.sh, which serves a COPY
// of data/atharu.db; screenshots hold work data, so they go to a temp dir and are never committed.
//
//   node scripts/visual-check.mjs <base-url> <shots-dir> <path>...   one line per problem, exit 1 if any
//   node scripts/visual-check.mjs --self-test                        the checks against two fixture pages
//
// Checks: horizontal overflow (an element past its scroll container or the viewport), a line's text
// (`.blk.ln .ltext`) narrower than 200 px on desktop / 120 px on phone, the `.doc` left edge moving between
// pages at one width (compared per `.doc` variant: `.doc.wide`, `.doc.set` are meant to differ), any console error or uncaught exception.
import { spawn } from 'node:child_process'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const WIDTHS = [[1180, 900], [900, 900], [390, 844]]
const CHROME = process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'

// Runs in the page; returns { problems: string[], doc: {left, wide} | null }.
const CHECK = `(() => {
  const vw = document.documentElement.clientWidth
  const phone = vw < 640
  const out = []
  const name = el => el.tagName.toLowerCase() + (el.id ? '#' + el.id : '') +
    [...el.classList].slice(0, 3).map(c => '.' + c).join('')
  if (document.documentElement.scrollWidth > vw + 1)
    out.push('page scrolls sideways: ' + document.documentElement.scrollWidth + ' > ' + vw + ' px')
  // The nearest ancestor that scrolls sideways, else the viewport. Clipping (overflow hidden / clip) is
  // deliberate (ellipsis, rounded cards), so only scrolling containers and the viewport count.
  const box = el => {
    for (let p = el.parentElement; p; p = p.parentElement) {
      const o = getComputedStyle(p).overflowX
      if (o === 'hidden' || o === 'clip') return null
      if (o === 'auto' || o === 'scroll') return p.getBoundingClientRect()
    }
    return { left: 0, right: vw }
  }
  const flagged = new Set()
  for (const el of document.querySelectorAll('body *')) {
    const r = el.getBoundingClientRect()
    if (r.width === 0 || r.height === 0 || getComputedStyle(el).position === 'fixed') continue
    let p = el.parentElement
    while (p && !flagged.has(p)) p = p.parentElement
    if (p) continue
    const b = box(el)
    if (b && (r.right > b.right + 1 || r.left < b.left - 1)) {
      flagged.add(el)
      out.push('overflow: ' + name(el) + ' spans ' + Math.round(r.left) + '–' + Math.round(r.right) +
        ' px, its box ' + Math.round(b.left) + '–' + Math.round(b.right))
    }
  }
  const min = phone ? 120 : 200
  for (const t of document.querySelectorAll('.blk.ln .ltext')) {
    const w = t.getBoundingClientRect().width
    if (w > 0 && w < min) out.push('squeezed line text: ' + Math.round(w) + ' px < ' + min + ': "' + t.textContent.trim().slice(0, 40) + '"')
  }
  const doc = document.querySelector('.doc')
  return { problems: out.slice(0, 8), doc: doc && { left: Math.round(doc.getBoundingClientRect().left), kind: doc.className } }
})()`

async function chrome() {
  const port = 9400 + Math.floor(Math.random() * 400)
  const profile = mkdtempSync(join(tmpdir(), 'atharu-vc-chrome-'))
  const proc = spawn(CHROME, ['--headless=new', `--remote-debugging-port=${port}`,
    `--user-data-dir=${profile}`, '--hide-scrollbars', 'about:blank'])
  const sleep = ms => new Promise(r => setTimeout(r, ms))
  let tabs
  for (let i = 0; i < 60 && !tabs; i++) {
    await sleep(250)
    try { tabs = await (await fetch(`http://127.0.0.1:${port}/json`)).json() } catch {}
  }
  if (!tabs) throw new Error(`Chrome did not start (CHROME=${CHROME})`)
  const ws = new WebSocket(tabs.find(t => t.type === 'page').webSocketDebuggerUrl)
  await new Promise(r => (ws.onopen = r))
  let id = 0
  const wait = new Map()
  const errors = []
  ws.onmessage = e => {
    const m = JSON.parse(e.data)
    if (m.id) { wait.get(m.id)?.(m); wait.delete(m.id); return }
    if (m.method === 'Runtime.exceptionThrown') errors.push(m.params.exceptionDetails.exception?.description ?? m.params.exceptionDetails.text)
    if (m.method === 'Runtime.consoleAPICalled' && m.params.type === 'error') errors.push(m.params.args.map(a => a.value ?? a.description).join(' '))
    if (m.method === 'Log.entryAdded' && m.params.entry.level === 'error') errors.push(`${m.params.entry.text} ${m.params.entry.url ?? ''}`.trim())
  }
  const cdp = (method, params = {}) => new Promise(r => { const i = ++id; wait.set(i, r); ws.send(JSON.stringify({ id: i, method, params })) })
  await cdp('Runtime.enable')
  await cdp('Log.enable')
  return {
    errors,
    sleep,
    cdp,
    async load(url, w, h) {
      await cdp('Emulation.setDeviceMetricsOverride', { width: w, height: h, deviceScaleFactor: 1, mobile: w < 640 })
      errors.length = 0
      await cdp('Page.navigate', { url })
      await sleep(900)
      const r = await cdp('Runtime.evaluate', { expression: CHECK, returnByValue: true })
      return r.result.result.value
    },
    // The profile caches the pages it loaded (work data): kill Chrome, wait for it, then remove the profile.
    async close() {
      ws.close()
      const gone = new Promise(r => proc.once('exit', r))
      proc.kill()
      await Promise.race([gone, sleep(3000)])
      rmSync(profile, { recursive: true, force: true })
    },
  }
}

async function selfTest() {
  const c = await chrome()
  const page = body => 'data:text/html,' + encodeURIComponent(`<!doctype html><meta name=viewport content="width=device-width"><body style="margin:0">${body}`)
  const bad = await c.load(page('<main class="doc"><div style="width:2000px">wide</div></main>'), 900, 600)
  const ok = await c.load(page('<main class="doc"><div style="overflow:hidden"><div style="width:2000px">clipped on purpose</div></div></main>'), 900, 600)
  await c.close()
  const fail = []
  if (!bad.problems.some(p => p.startsWith('overflow: div'))) fail.push('missed an overflowing div: ' + JSON.stringify(bad.problems))
  if (ok.problems.length) fail.push('flagged a clipped div: ' + JSON.stringify(ok.problems))
  console.log(fail.length ? fail.join('\n') : 'self-test ok')
  process.exit(fail.length ? 1 : 0)
}

if (process.argv[2] === '--self-test') await selfTest()

const [base, shots, ...paths] = process.argv.slice(2)
if (!base || !shots || !paths.length) {
  console.error('usage: node scripts/visual-check.mjs <base-url> <shots-dir> <path>...  |  --self-test')
  process.exit(2)
}
const c = await chrome()
const problems = []
try {
  for (const [w, h] of WIDTHS) {
    const left = {}
    for (const path of paths) {
      const r = await c.load(base + path, w, h)
      const at = `${w}px ${path}`
      for (const p of r.problems) problems.push(`${at}: ${p}`)
      for (const e of c.errors) problems.push(`${at}: console error: ${e}`)
      if (r.doc) {
        const k = r.doc.kind
        left[k] ??= [r.doc.left, path]
        if (Math.abs(left[k][0] - r.doc.left) > 1) problems.push(`${at}: .doc left edge ${r.doc.left} px, ${left[k][0]} px on ${left[k][1]}`)
      }
      const shot = await c.cdp('Page.captureScreenshot', { format: 'png' })
      writeFileSync(join(shots, `${w}-${path.replace(/[^a-z0-9]+/gi, '_')}.png`), Buffer.from(shot.result.data, 'base64'))
    }
  }
} finally {
  await c.close()
}
for (const p of problems) console.log(p)
console.log(`${problems.length} problem${problems.length === 1 ? '' : 's'} · ${paths.length} pages × ${WIDTHS.length} widths · screenshots: ${shots}`)
process.exit(problems.length ? 1 : 0)
