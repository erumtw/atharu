import { byMenu } from './bymenu'
import { wireNoteBox } from './notebox'

// Capture bar tags (D29, DESIGN.md §6 "Tag chip"). The server renders the `#` suggestions (#tagsug, out-of-band
// with each /parse-preview) and the "Tag as #x?" offer (#hint .tagask); this file only moves the selection and
// edits the input. Without JS the bar still saves whatever `#tag` was typed. #tagsug is swapped innerHTML, so the
// element found here stays the live one.
const input = document.getElementById('line') as HTMLInputElement | null
const box = document.getElementById('tagsug')
const skip = document.getElementById('notag') as HTMLInputElement | null

// `n ` / ⌘N in Today's bar (0.2.6): the bar turns into a note box (#tnote), same as the popover's. Wired before the menus below
// and htmx's own input listener, so a note's text never paints a "done" preview.
const tnote = document.getElementById('tnote') as HTMLFormElement | null
const ttext = tnote?.querySelector('textarea')
const lineForm = input?.closest('form')
if (input && tnote && ttext && lineForm) {
  wireNoteBox({
    line: input,
    text: ttext,
    hint: document.getElementById('hint'),
    show: on => {
      tnote.hidden = !on
      lineForm.hidden = on
    },
    scope: tnote.parentElement ?? undefined,
    escBack: true,
  })
}

// The grammar in the hint pill (0.2.8, owner's design review): while the line is focused and empty (or one character), the pill above the
// capsule (#hint, where "→ will save as …" appears later) shows the prefixes. The placeholder's example vanishes on the first keystroke and
// recognition beats recall; DESIGN.md says no legend row under the floating bar, so it is the existing pill, and the capsule never moves.
const hint = document.getElementById('hint')
if (input && hint && document.querySelector('.capture')) {
  const GRAMMAR =
    '<span class="gram"><span class="mono">!</span> decision · <span class="mono">x</span> blocker · <span class="mono">+</span> impact · <span class="mono">?</span> learn · <span class="mono">&gt;</span> to-do · <span class="mono">n</span> note</span>'
  const show = () => {
    if (document.activeElement === input && input.value.trim().length <= 1 && !hint.textContent?.trim()) hint.innerHTML = GRAMMAR
  }
  const clear = () => hint.querySelector('.gram')?.remove()
  input.addEventListener('focus', show)
  input.addEventListener('blur', clear)
  // Past one character the preview (or nothing) is the hint's to say; back to empty, the grammar returns.
  input.addEventListener('input', () => (input.value.trim().length > 1 ? clear() : show()))
  document.body.addEventListener('htmx:afterSwap', ev => {
    if ((ev.target as Element | null)?.id === 'hint') show()
  })
  show()
}

if (input && box && skip) {
  const by = byMenu(input, document.getElementById('bysug'), document.getElementById('bydate') as HTMLInputElement | null)
  const opts = () => Array.from(box.querySelectorAll<HTMLElement>('[role=option]'))
  const picked = () => opts().findIndex(o => o.getAttribute('aria-selected') === 'true')

  // Combobox state follows whatever the last swap left in the box.
  const sync = () => {
    if (by.open()) return
    const o = opts()
    const sel = o[picked()]
    input.setAttribute('aria-controls', 'tagsug')
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
    if (by.key(ev)) return
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


// Floating capture bar (T78). Phone keyboard: iOS keeps the layout viewport when the keyboard opens,
// so a sticky bottom bar would sit under it; lift it by the covered height (responsive.css reads --kb and .kb). Chrome on
// Android with `interactive-widget=resizes-content` in the viewport meta shrinks the layout viewport itself, so kb is 0 there.
const cap = document.querySelector<HTMLElement>('.capture')
const vv = window.visualViewport
if (cap && vv) {
  const ride = () => {
    // Pinch zoom also shrinks the visual viewport: only a keyboard counts.
    const kb = vv.scale > 1.01 ? 0 : Math.max(0, Math.round(window.innerHeight - vv.height - vv.offsetTop))
    cap.style.setProperty('--kb', `${kb}px`)
    cap.classList.toggle('kb', kb > 0)
  }
  vv.addEventListener('resize', ride)
  vv.addEventListener('scroll', ride)
}
