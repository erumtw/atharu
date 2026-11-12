// /settings glass controls: preview on <html> as they move. Saving is htmx on the form (change / preset submit);
// this file only mirrors what the server will render after the save.
const root = document.documentElement
const form = document.querySelector<HTMLFormElement>('#glass')
const range = form?.querySelector<HTMLInputElement>('input[type=range]')
const all = form?.querySelector<HTMLInputElement>('input[name=glass_all]')
const out = form?.querySelector<HTMLOutputElement>('output')

if (form && range && all && out) {
  const alpha = () => {
    root.style.setProperty('--glass-a', `${range.value}%`)
    out.value = `${range.value}%`
  }
  range.addEventListener('input', alpha)
  all.addEventListener('change', () => {
    if (all.checked) root.dataset.glass = 'all'
    else delete root.dataset.glass
  })
  // Same rule as settings::save_glass: Clear = this look's floor (the slider's min), Tinted = 85.
  form.addEventListener('click', ev => {
    const btn = (ev.target as Element | null)?.closest<HTMLButtonElement>('button[name=preset]')
    if (!btn) return
    const tinted = btn.value === 'tinted'
    range.value = tinted ? String(Math.max(85, +range.min)) : range.min
    alpha()
    if (tinted) root.dataset.tint = 'tinted'
    else delete root.dataset.tint
    for (const b of form.querySelectorAll('button[name=preset]')) b.setAttribute('aria-pressed', String(b === btn))
  })
}
