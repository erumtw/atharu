// Copy buttons: <button data-copy="#target">. Copies the target's value (textarea) or text,
// then shows "✓ Copied" for 2 s (States board copy).
document.addEventListener("click", async (ev) => {
  const btn = ev.target.closest("[data-copy]");
  if (!btn) return;
  const el = document.querySelector(btn.dataset.copy);
  if (!el) return;
  await navigator.clipboard.writeText(el.value ?? el.textContent);
  const label = btn.textContent;
  btn.textContent = "✓ Copied";
  setTimeout(() => (btn.textContent = label), 2000);
});
