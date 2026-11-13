// Note mode of a capture box, shared by the menu-bar popover (D53) and Today's bar (0.2.6): `n ` at the start of the line
// (D55) or ⌘N turns the box into a note (first line = title, the rest = markdown body), Enter saves, Shift+Enter is a new
// line. The caller owns the markup (`show` switches the two boxes); the server's no-JS path (a 307 to /notes/new) is the same.
export interface NoteBox {
  line: HTMLInputElement
  text: HTMLTextAreaElement
  hint: HTMLElement | null
  /** Show the note box (true) or the line box (false). */
  show: (on: boolean) => void
  /** ⌘N counts only inside this element (default: anywhere on the page). */
  scope?: Element
  /** Esc in the note box goes back to the line, its text moving with it (Today); the popover's Esc hides the window. */
  escBack?: boolean
}

/** Wires the note box; returns the mode switch for the caller's own buttons. */
export function wireNoteBox(o: NoteBox): (on: boolean) => void {
  const { line, text, hint } = o
  const noteMode = (on: boolean) => {
    o.show(on)
    if (on) {
      // The line's preview says nothing about a note; a half-typed line becomes the note's first line.
      hint?.replaceChildren()
      if (line.value.trim() && !text.value.trim()) {
        text.value = line.value
        line.value = ''
      }
    }
    const box = on ? text : line
    box.focus()
    box.setSelectionRange(box.value.length, box.value.length)
  }
  // `n ` at the start of the line = a note, the rest moves over. Stopped here so htmx's preview doesn't paint a "done" hint
  // over note mode (callers wire this before htmx binds its own input listener).
  line.addEventListener('input', ev => {
    const m = /^\s*n (.*)$/s.exec(line.value)
    if (!m) return
    ev.stopImmediatePropagation()
    line.value = m[1] ?? ''
    noteMode(true)
  })
  // ⌘N; a second press keeps note mode rather than toggling it off.
  document.addEventListener('keydown', ev => {
    if (!ev.metaKey || ev.key.toLowerCase() !== 'n') return
    if (o.scope && !o.scope.contains(ev.target as Node)) return
    ev.preventDefault()
    noteMode(true)
  })
  // Enter saves, Shift+Enter is a new line (an IME's Enter only commits the word).
  text.addEventListener('keydown', ev => {
    if (ev.key === 'Escape' && o.escBack) {
      ev.preventDefault()
      if (!line.value.trim()) line.value = text.value.replace(/\s*\n\s*/g, ' ').trim()
      text.value = ''
      noteMode(false)
      return
    }
    if (ev.key !== 'Enter' || ev.shiftKey || ev.isComposing) return
    ev.preventDefault()
    text.form?.requestSubmit()
  })
  // Saved: the page already shows "✓ Saved note · Open" (swapped out of band); the box goes back to the line.
  text.form?.addEventListener('htmx:afterRequest', ev => {
    // A 422 counts as `successful` (it swaps into the hint), so check the status, as the line form does.
    const d = (ev as CustomEvent<{ elt: Element; xhr: XMLHttpRequest }>).detail
    if (d.elt !== text.form || d.xhr.status >= 300) return
    text.value = ''
    noteMode(false)
  })
  return noteMode
}
