// Copy buttons: <button data-copy="#target">. Copies the target's value (textarea) or text,
// then shows "✓ Copied" for 2 s (States board copy).
document.addEventListener('click', async ev => {
  const btn = (ev.target as Element | null)?.closest<HTMLElement>('[data-copy]')
  if (!btn?.dataset.copy) return
  const el = document.querySelector<HTMLElement>(btn.dataset.copy)
  if (!el) return
  await navigator.clipboard.writeText(el instanceof HTMLTextAreaElement ? el.value : (el.textContent ?? ''))
  const label = btn.textContent
  btn.textContent = '✓ Copied'
  setTimeout(() => (btn.textContent = label), 2000)
})
