// Live filter on /epics (0.2.7): typing in the Filter box hides the epic rows, the "assigned to you" rows and the folded Done & dropped
// list's rows whose `data-q`
// (lowercased key, title, Jira title, tags) doesn't contain the text, hides a group once all its rows are hidden, and keeps
// every count ("· 2 of 5 active") true. The server renders the same filter from `?q=` (Enter submits the GET form), so this is
// only the speed of typing; without JS the form still works.
const input = document.querySelector<HTMLInputElement>('form[role=search] input[name=q]')
const ROWS = '.etab tr[data-q], .arow[data-q], .blocks .blk[data-q]'

if (input) {
  const rows = Array.from(document.querySelectorAll<HTMLElement>(ROWS))
  const count = (d: Element) => d.querySelector<HTMLElement>(':scope > summary > .muted.normal')
  // "2 of 5 active" / "3 open" / "7": the number, an optional "of M", and the unit that follows.
  const parse = (t: string) => /^·\s*(\d+)(?: of (\d+))?(.*)$/.exec(t.trim())
  // A group's own rows: the rows under it that aren't under a nested group. Header rows (`.jhead`) aren't epics.
  const own = (d: Element) =>
    Array.from(d.querySelectorAll<HTMLElement>(ROWS)).filter(r => r.closest('details') === d && !r.classList.contains('jhead'))
  // The rendered size of each group, taken once: a server-side filter has already narrowed it, and "of M" keeps that M.
  const total = new Map<Element, number>()
  const groups = Array.from(document.querySelectorAll('main details')).filter(d => d.querySelector(ROWS))
  for (const d of groups) {
    const c = count(d)
    const m = c ? parse(c.textContent ?? '') : null
    total.set(d, m ? Number(m[2] ?? m[1]) : own(d).length)
  }
  const show = (d: Element, n: number) => {
    const c = count(d)
    const m = c ? parse(c.textContent ?? '') : null
    if (!c || !m) return
    const of = total.get(d) ?? n
    c.textContent = `· ${n}${n === of ? '' : ` of ${of}`}${m[3] ?? ''}`
  }

  const apply = () => {
    const q = input.value.trim().toLowerCase()
    for (const r of rows) r.hidden = q !== '' && !(r.dataset.q ?? '').includes(q)
    for (const d of groups) {
      const mine = own(d)
      const n = mine.filter(r => !r.hidden).length
      // The assigned section keeps its summary ("0 of 12 open"); its groups and the epic groups go when empty.
      const nested = d.querySelector('details')
      if (nested) {
        const sum = Array.from(d.querySelectorAll<HTMLElement>('.arow')).filter(r => !r.hidden).length
        show(d, sum)
      } else {
        ;(d as HTMLElement).hidden = mine.length > 0 && n === 0
        show(d, n)
      }
    }
  }

  let timer = 0
  input.addEventListener('input', () => {
    window.clearTimeout(timer)
    timer = window.setTimeout(apply, 120)
  })
  // Esc clears the box and shows everything (Enter is the form's own submit: the same filter, in the URL).
  input.addEventListener('keydown', ev => {
    if (ev.key !== 'Escape' || !input.value) return
    ev.preventDefault()
    input.value = ''
    apply()
  })
}
