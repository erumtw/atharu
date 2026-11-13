// /settings glass controls: preview on <html> as they move. Saving is htmx on the form (change / preset submit);
// this file only mirrors what the server will render after the save.
const root = document.documentElement
const form = document.querySelector<HTMLFormElement>('#glass')
const range = form?.querySelector<HTMLInputElement>('input[type=range]')
const all = form?.querySelector<HTMLInputElement>('input[name=glass_all]')
const textBlur = form?.querySelector<HTMLInputElement>('input[name=text_blur]')
const out = form?.querySelector<HTMLOutputElement>('output')

if (form && range && all && textBlur && out) {
  // The slider shows the true 0–100 scale; below this look's AA floor is hatched (a warning, still reachable).
  const floor = Number(range.dataset.floor ?? 0)
  const alpha = () => {
    root.style.setProperty('--glass-a', `${range.value}%`)
    range.style.setProperty('--v', range.value)
    out.value = `${range.value}%`
  }
  range.addEventListener('input', alpha)
  all.addEventListener('change', () => {
    if (all.checked) root.dataset.glass = 'all'
    else delete root.dataset.glass
  })
  textBlur.addEventListener('change', () => {
    if (textBlur.checked) root.dataset.blur = 'text'
    else delete root.dataset.blur
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
