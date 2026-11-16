// Opens each note in the web editor, types one character, waits for the autosave and asserts the stored markdown changed by
// exactly that character: an edit must never rewrite or drop what the editor merely carries (0.2.10: it deleted tables).
//   node scripts/editor-roundtrip.mjs <base url> <db file> <note id>...      (scripts/editor-roundtrip.sh drives it)
// RT_MODE=render (real notes): the editor's serializer may normalise text it can't keep verbatim (`a_b` → `a\_b`, blank lines), so
// compare what the read view SHOWS before and after instead of the bytes. Default (fixture): the stored markdown, byte for byte.
import { spawn, execSync } from 'node:child_process'
import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const [base, db, ...ids] = process.argv.slice(2)
if (!base || !db || !ids.length) { console.error('usage: editor-roundtrip.mjs <base url> <db> <note id>...'); process.exit(2) }
const CHROME = process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
const sleep = ms => new Promise(r => setTimeout(r, ms))
const body = id => execSync(`sqlite3 "${db}" "select body from notes where id=${id}"`, { maxBuffer: 1 << 28 }).toString().replace(/\n+$/, '')

const port = 9400 + Math.floor(Math.random() * 400)
const profile = mkdtempSync(join(tmpdir(), 'atharu-rt-chrome-'))
const proc = spawn(CHROME, ['--headless=new', `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`, 'about:blank'], { stdio: 'ignore' })
let tabs
for (let i = 0; i < 60 && !tabs; i++) { await sleep(250); try { tabs = await (await fetch(`http://127.0.0.1:${port}/json`)).json() } catch {} }
if (!tabs) { console.error(`Chrome did not start (CHROME=${CHROME})`); process.exit(2) }
const ws = new WebSocket(tabs.find(t => t.type === 'page').webSocketDebuggerUrl)
await new Promise(r => (ws.onopen = r))
let n = 0
const wait = new Map()
ws.onmessage = e => { const m = JSON.parse(e.data); if (m.id) { wait.get(m.id)?.(m); wait.delete(m.id) } }
const cdp = (method, params = {}) => new Promise(r => { const i = ++n; wait.set(i, r); ws.send(JSON.stringify({ id: i, method, params })) })
const ev = async expression => (await cdp('Runtime.evaluate', { expression, returnByValue: true })).result.result.value
await cdp('Emulation.setFocusEmulationEnabled', { enabled: true })

const RENDER = process.env.RT_MODE === 'render'
// The read view's note body, minus the typed letter (a data: URI holds Z too, so strip them all from both sides).
const shown = async id => ((await (await fetch(`${base}/notes/${id}`)).text()).match(/<div id="editor"[^>]*>([\s\S]*?)<\/div>\s*<textarea/)?.[1] ?? '').replaceAll('Z', '')

const problems = []
for (const id of ids) {
  const before = body(id)
  const shownBefore = RENDER ? await shown(id) : ''
  await cdp('Page.navigate', { url: `${base}/notes/${id}` })
  for (let i = 0; i < 40 && !(await ev("!!document.querySelector('.ProseMirror')")); i++) await sleep(250)
  await sleep(500)
  // A captioned drawing must open as a DRAWING (editable, caption shown), not fall back to a plain image that merely round-trips.
  const want = Number(process.env.RT_DRAWING_CAPTIONS ?? 0)
  const got = await ev("document.querySelectorAll('.ProseMirror figure.drawing figcaption').length")
  if (got !== want) problems.push(`note ${id}: ${got} captioned drawings in the editor, want ${want}`)
  await ev("document.querySelector('.ProseMirror').focus()")
  await cdp('Input.insertText', { text: 'Z' })
  await sleep(3500) // htmx's 1 s autosave debounce + the round trip
  const after = body(id)
  if (RENDER) {
    const now = await shown(id)
    if (!shownBefore || now !== shownBefore) problems.push(`note ${id}: the read view changed after one typed character (${shownBefore.length} → ${now.length} chars of HTML)`)
    continue
  }
  let i = 0
  while (i < before.length && before[i] === after[i]) i++
  if (after.length !== before.length + 1 || after.slice(0, i) + after.slice(i + 1) !== before) {
    const lines = s => s.split('\n')
    const [b, a] = [lines(before), lines(after)]
    const k = b.findIndex((l, j) => l !== a[j] && !(j === 0 && a[0] === 'Z' + l))
    problems.push(`note ${id}: not one typed character (${before.length} → ${after.length} chars); first changed line ${k + 1}:\n  was: ${b[k]?.slice(0, 120)}\n  now: ${a[k]?.slice(0, 120)}`)
  }
}
ws.close()
const gone = new Promise(r => proc.once('exit', r))
proc.kill()
await Promise.race([gone, sleep(3000)])
rmSync(profile, { recursive: true, force: true })
if (process.env.RT_SHOW) for (const id of ids) console.log(JSON.stringify(body(id)))
console.log(problems.length ? problems.join('\n') : `editor round trip ok (${ids.length} note${ids.length > 1 ? 's' : ''})`)
process.exit(problems.length ? 1 : 0)
