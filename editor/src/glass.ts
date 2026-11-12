// /settings glass controls: preview on <html> as they move. Saving is htmx on the form (change / preset submit);
// this file only mirrors what the server will render after the save.
const root = document.documentElement
const form = document.querySelector<HTMLFormElement>('#glass')
const range = form?.querySelector<HTMLInputElement>('input[type=range]')
const all = form?.querySelector<HTMLInputElement>('input[name=glass_all]')
const out = form?.querySelector<HTMLOutputElement>('output')

if (form && range && all && out) {
  // The slider shows the true 0–100 scale; below this look's AA floor is hatched and unreachable.
  const floor = Number(range.dataset.floor ?? 0)
  const alpha = () => {
    if (+range.value < floor) range.value = String(floor)
    root.style.setProperty('--glass-a', `${range.value}%`)
    range.style.setProperty('--v', range.value)
    out.value = `${range.value}%`
  }
  range.addEventListener('input', alpha)
  all.addEventListener('change', () => {
    if (all.checked) root.dataset.glass = 'all'
    else delete root.dataset.glass
  })
  // Same rule as settings::save_glass: Clear = this look's floor, Tinted = 85.
  form.addEventListener('click', ev => {
    const btn = (ev.target as Element | null)?.closest<HTMLButtonElement>('button[name=preset]')
    if (!btn) return
    const tinted = btn.value === 'tinted'
    range.value = String(tinted ? Math.max(85, floor) : floor)
    alpha()
    if (tinted) root.dataset.tint = 'tinted'
    else delete root.dataset.tint
    for (const b of form.querySelectorAll('button[name=preset]')) b.setAttribute('aria-pressed', String(b === btn))
  })
}
