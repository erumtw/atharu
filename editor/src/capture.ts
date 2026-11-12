// Capture bar tags (D29, DESIGN.md §6 "Tag chip"). The server renders the `#` suggestions (#tagsug, out-of-band
// with each /parse-preview) and the "Tag as #x?" offer (#hint .tagask); this file only moves the selection and
// edits the input. Without JS the bar still saves whatever `#tag` was typed. #tagsug is swapped innerHTML, so the
// element found here stays the live one.
const input = document.getElementById('line') as HTMLInputElement | null
const box = document.getElementById('tagsug')
const skip = document.getElementById('notag') as HTMLInputElement | null

if (input && box && skip) {
  const opts = () => Array.from(box.querySelectorAll<HTMLElement>('[role=option]'))
  const picked = () => opts().findIndex(o => o.getAttribute('aria-selected') === 'true')

  // Combobox state follows whatever the last swap left in the box.
  const sync = () => {
    const o = opts()
    const sel = o[picked()]
    input.setAttribute('aria-expanded', String(o.length > 0))
    if (sel) input.setAttribute('aria-activedescendant', sel.id)
    else input.removeAttribute('aria-activedescendant')
  }
  const close = () => {
    box.replaceChildren()
    sync()
  }
  // Re-run the live hint (htmx listens for `input`), so the preview and offers follow the edit.
  const changed = () => input.dispatchEvent(new Event('input', { bubbles: true }))

  // The menu is for the token being typed at the end of the line.
  const insert = (tag: string) => {
    input.value = input.value.replace(/#[A-Za-z0-9-]*$/, `#${tag} `)
    close()
    input.focus()
    changed()
  }

  const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  // "Tag as": the word as typed becomes `#tag` where it stands; the rest of the line is untouched.
  const accept = (ask: HTMLElement) => {
    const { word, tag } = ask.dataset
    if (!word || !tag) return
    input.value = input.value.replace(new RegExp(`(^|\\s)${escape(word)}(?=[\\s.,;:!?)]|$)`), `$1#${tag}`)
    input.focus()
    changed()
  }
  // ×: not a tag. The word rides along in a hidden field, so the server stops offering it for this line.
  const dismiss = (ask: HTMLElement) => {
    const { word } = ask.dataset
    if (!word) return
    skip.value = skip.value ? `${skip.value},${word}` : word
    // The line didn't change, so htmx's `changed` filter won't re-ask the server: drop the offer here.
    ask.closest('.ask')?.remove()
    input.focus()
  }

  input.addEventListener('keydown', ev => {
    const o = opts()
    if (o.length) {
      const i = picked()
      if (ev.key === 'ArrowDown' || ev.key === 'ArrowUp') {
        ev.preventDefault()
        const n = (i + (ev.key === 'ArrowDown' ? 1 : -1) + o.length) % o.length
        o.forEach((x, j) => x.setAttribute('aria-selected', String(j === n)))
        sync()
        return
      }
      const sel = o[i]
      if ((ev.key === 'Tab' || ev.key === 'Enter') && sel) {
        ev.preventDefault()
        insert(sel.dataset.tag ?? '')
        return
      }
      if (ev.key === 'Escape') {
        ev.preventDefault()
        close()
        return
      }
    }
    const ask = document.querySelector<HTMLElement>('#hint .tagask')
    if (ask && ev.key === 'Tab' && !ev.shiftKey) {
      ev.preventDefault()
      accept(ask)
    }
  })

  // mousedown, not click: keeps focus in the input.
  box.addEventListener('mousedown', ev => {
    const o = (ev.target as Element).closest<HTMLElement>('[role=option]')
    if (!o) return
    ev.preventDefault()
    insert(o.dataset.tag ?? '')
  })

  document.addEventListener('click', ev => {
    const t = ev.target as Element
    const ask = t.closest<HTMLElement>('#hint .tagask')
    if (!ask) return
    if (t.closest('.x')) dismiss(ask)
    else accept(ask)
  })

  document.body.addEventListener('htmx:afterSettle', sync)
}
