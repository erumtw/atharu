// Image block in notes (0.2.10): `![alt](src)` alone on a line. Paste (⌘V) or drop an image file → POST /notes/<id>/images →
// an image node with the returned `/attachments/notes/<id>/<name>` url. The server (domain::markdown) renders the same line
// as figure.image, so a note reads without JS; agents write `data:image/…;base64,` lines, which this node round-trips.
import { Image } from '@tiptap/extension-image'
import type { Editor } from '@tiptap/core'
import { Plugin } from '@tiptap/pm/state'
import type { EditorView } from '@tiptap/pm/view'

const OK_TYPES = ['image/png', 'image/jpeg', 'image/gif', 'image/webp']
const REFUSED = 'Paste a PNG, JPEG, GIF or WebP image.'

// Both editors (a note, a meeting's notes) post to /notes/<id>; that id is the one the upload belongs to.
const noteId = () => /^\/notes\/(\d+)$/.exec(document.getElementById('note')?.getAttribute('action') ?? '')?.[1]

// The autosave slot: the crumb's `#saved`. The next autosave swaps its text, and htmx's swap also clears the error colour.
function status(msg: string, err = false): void {
  const s = document.getElementById('saved')
  if (!s) return
  s.textContent = msg
  s.classList.toggle('err', err)
  if (err) s.addEventListener('htmx:afterSwap', () => s.classList.remove('err'), { once: true })
}

async function upload(editor: Editor, file: File, pos: number | null): Promise<void> {
  const id = noteId()
  if (!id) return status('Can’t attach an image here.', true)
  status('Uploading image…')
  try {
    const r = await fetch(`/notes/${id}/images`, { method: 'POST', headers: { 'Content-Type': file.type }, body: file })
    if (!r.ok) return status(r.status === 413 ? 'Image is too big (25 MB at most).' : (await r.text()) || 'Couldn’t upload the image.', true)
    const { data } = await r.json() as { data: { image: { url: string } } }
    editor.chain().focus().insertContentAt(pos ?? editor.state.selection.to, { type: 'image', attrs: { src: data.image.url, alt: 'image' } }).run()
    // The insert fires onUpdate → the form's `input` event → autosave, which replaces this with "✓ saved".
    status('Image added')
  } catch {
    status('Couldn’t upload · try again', true)
  }
}

// One handler for paste and drop: every image/* file is claimed (a dropped file the editor ignores would make the browser open it),
// the ones we can't store are named in the status, and anything else (text, a link, a note's own blocks) takes the default path.
function take(editor: Editor, files: FileList | undefined | null, event: Event, pos: number | null): boolean {
  const images = Array.from(files ?? []).filter(f => f.type.startsWith('image/'))
  if (!images.length) return false
  event.preventDefault()
  void (async () => {
    let at = pos
    for (const f of images) {
      if (!OK_TYPES.includes(f.type)) { status(REFUSED, true); continue }
      await upload(editor, f, at)
      at = null // the next one lands after the previous
    }
  })()
  return true
}

export const NoteImage = Image.configure({ inline: false, allowBase64: true }).extend({
  // Pasted HTML (copying from a web page) keeps only our own forms: a remote <img> would make every view of the note fetch it.
  parseHTML: () => [{ tag: 'img[src^="/attachments/notes/"], img[src^="data:image/"]' }],
  addProseMirrorPlugins() {
    const editor = this.editor
    return [new Plugin({
      props: {
        handlePaste: (_view: EditorView, event: ClipboardEvent) => take(editor, event.clipboardData?.files, event, null),
        handleDrop: (view: EditorView, event: DragEvent, _slice, moved: boolean) =>
          !moved && take(editor, event.dataTransfer?.files, event, view.posAtCoords({ left: event.clientX, top: event.clientY })?.pos ?? null),
      },
    })]
  },
})
