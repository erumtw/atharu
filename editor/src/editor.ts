// Notes editor (spec P4): Tiptap with markdown as the stored format. Mounted on #editor;
// the hidden #note-md textarea carries the markdown, so htmx autosave and no-JS posts are the same form.
import { Editor, Extension, InputRule, Node, createBlockMarkdownSpec, mergeAttributes } from '@tiptap/core'
import type { ChainedCommands } from '@tiptap/core'
import StarterKit from '@tiptap/starter-kit'
import { TaskItem, TaskList } from '@tiptap/extension-list'
import { Details, DetailsContent, DetailsSummary } from '@tiptap/extension-details'
import { Markdown } from '@tiptap/markdown'
import Suggestion from '@tiptap/suggestion'
import type { SuggestionKeyDownProps, SuggestionOptions, SuggestionProps } from '@tiptap/suggestion'
import { PluginKey } from '@tiptap/pm/state'

declare module '@tiptap/core' {
  interface Commands<ReturnType> {
    callout: { setCallout: () => ReturnType }
  }
}

// `:::callout` … `:::` in markdown, the same fenced-block syntax Details uses (server renders both).
const Callout = Node.create({
  name: 'callout',
  group: 'block',
  content: 'block+',
  defining: true,
  parseHTML: () => [{ tag: 'div[data-type="callout"]' }],
  renderHTML: ({ HTMLAttributes }) => ['div', mergeAttributes(HTMLAttributes, { 'data-type': 'callout', class: 'callout' }), 0],
  ...createBlockMarkdownSpec({ nodeName: 'callout' }),
  addCommands() {
    return { setCallout: () => ({ commands }) => commands.wrapIn(this.name) }
  },
})

// `[[X]]` stays literally `[[X]]` in markdown: backlinks and the LLM layer grep for it.
const PAGE = /^\[\[([^[\]\n]+)\]\]/
const PageLink = Node.create({
  name: 'pageLink',
  group: 'inline',
  inline: true,
  atom: true,
  addAttributes: () => ({ id: { default: '' } }),
  parseHTML: () => [{ tag: 'span[data-page]', getAttrs: (el: HTMLElement) => ({ id: el.getAttribute('data-page') }) }],
  renderHTML: ({ node }) => ['span', { 'data-page': node.attrs.id, class: 'chip page' }, node.attrs.id],
  renderText: ({ node }) => `[[${node.attrs.id}]]`,
  markdownTokenizer: {
    name: 'pageLink',
    level: 'inline',
    start: (src: string) => src.indexOf('[['),
    tokenize: (src: string) => {
      const m = PAGE.exec(src)
      if (m?.[1]) return { type: 'pageLink', raw: m[0], id: m[1].trim() }
    },
  },
  parseMarkdown: (token, h) => h.createNode('pageLink', { id: token.id }),
  renderMarkdown: node => `[[${node.attrs?.id ?? ''}]]`,
  // Typing `[[X]]` by hand becomes a chip too. Not nodeInputRule: it would keep the brackets around the node.
  addInputRules() {
    return [new InputRule({
      find: /\[\[([^[\]\n]+)\]\]$/,
      handler: ({ state, range, match }) => { state.tr.replaceWith(range.from, range.to, this.type.create({ id: (match[1] ?? '').trim() })) },
    })]
  },
})

// Drawing block (D23/D24): `![Drawing](data:image/svg+xml;base64,…)` on its own line, scene embedded in the SVG.
// Markdown.rs shows exactly that form as an <img>, so the note reads (and works without JS) with no Excalidraw at all.
const DRAWING_LINE = /^!\[Drawing\]\((data:image\/svg\+xml;base64,[A-Za-z0-9+/=]+)?\)[ \t]*(?:\n|$)/
// A variable, not a literal: esbuild leaves import(x) alone, so the island stays out of this iife.
const ISLAND = '/static/js/drawing/drawing.js'
// Set by the `/` menu so the block it inserts opens the canvas at once (DESIGN.md §6 Insert).
let openNew = false

function button(label: string, cls: string, onClick: () => void): HTMLButtonElement {
  const b = document.createElement('button')
  b.type = 'button'
  b.className = cls
  b.textContent = label
  b.addEventListener('click', onClick)
  return b
}

async function editDrawing(start: string, set: (src: string) => void): Promise<void> {
  if (!document.getElementById('excalidraw-css')) {
    const l = document.createElement('link')
    l.id = 'excalidraw-css'
    l.rel = 'stylesheet'
    l.href = '/static/excalidraw/index.css'
    document.head.append(l)
  }
  ;(window as { EXCALIDRAW_ASSET_PATH?: string }).EXCALIDRAW_ASSET_PATH = '/static/excalidraw/'
  const sheet = document.createElement('div')
  sheet.className = 'drawsheet'
  const host = document.createElement('div')
  host.className = 'drawhost'
  const saved = document.createElement('span')
  saved.className = 'saved small'
  saved.setAttribute('role', 'status')
  const form = document.getElementById('note')
  const md = document.getElementById('note-md')
  let session: import('./drawing').Session | null = null
  let timer = 0
  let saving = false
  let closed = false
  const status = (text: string, err = false) => { saved.textContent = text; saved.classList.toggle('err', err) }
  // The form's own htmx autosave posts the note; this only reports how the post went while the sheet is open.
  const after = (e: Event) => {
    if (!saving) return
    if ((e as CustomEvent<{ successful: boolean }>).detail.successful) { saving = false; status('✓ saved just now') }
    else {
      status('Couldn’t save · retrying', true)
      setTimeout(() => md?.dispatchEvent(new Event('input', { bubbles: true })), 3000)
    }
  }
  form?.addEventListener('htmx:afterRequest', after)
  const close = (src: string) => {
    if (closed) return
    closed = true
    clearTimeout(timer)
    // Done never waits on the network: the node changes now, the form posts it like any edit.
    set(src)
    session?.close()
    sheet.remove()
    form?.removeEventListener('htmx:afterRequest', after)
  }
  const bar = document.createElement('div')
  bar.className = 'drawbar'
  const cancel = button('Cancel', 'btn ghost', () => close(start))
  cancel.title = 'Discard changes made since you opened it'
  bar.append(saved, cancel, button('Done', 'btn', async () => close(session ? await session.src(true) : start)))
  sheet.append(host, bar)
  // Keys must not reach ProseMirror under the sheet: with the block selected, a typed letter would replace it.
  ;(document.activeElement as HTMLElement | null)?.blur()
  document.body.append(sheet)
  const mod = await import(ISLAND) as typeof import('./drawing')
  session = await mod.open(host, start, () => {
    status('Saving…')
    clearTimeout(timer)
    timer = window.setTimeout(async () => {
      if (!session || closed) return
      const src = await session.src(false)
      if (closed) return
      saving = true
      set(src)
    }, 1000)
  })
}

const PENCIL = 'M4 20h4L19 9l-4-4L4 16zM13.5 6.5l4 4'

const Drawing = Node.create({
  name: 'drawing',
  group: 'block',
  atom: true,
  draggable: true,
  addAttributes: () => ({ src: { default: '' } }),
  parseHTML: () => [{ tag: 'figure.drawing', getAttrs: (el: HTMLElement) => ({ src: el.querySelector('img')?.getAttribute('src') ?? '' }) }],
  renderHTML: ({ node }) => ['figure', { class: 'drawing' }, ['img', { src: node.attrs.src, alt: 'Drawing' }]],
  markdownTokenizer: {
    name: 'drawing',
    level: 'block',
    start: (src: string) => src.indexOf('![Drawing]('),
    tokenize: (src: string) => {
      const m = DRAWING_LINE.exec(src)
      if (m) return { type: 'drawing', raw: m[0], src: m[1] ?? '' }
    },
  },
  parseMarkdown: (token, h) => h.createNode('drawing', { src: token.src }),
  renderMarkdown: node => `![Drawing](${node.attrs?.src ?? ''})`,
  addNodeView() {
    return ({ node, getPos, editor }) => {
      const dom = document.createElement('figure')
      let src = ''
      const set = (next: string) => {
        const pos = getPos()
        if (typeof pos === 'number') editor.view.dispatch(editor.state.tr.setNodeAttribute(pos, 'src', next))
      }
      const edit = () => void editDrawing(src, set)
      const draw = (next: string) => {
        src = next
        dom.className = src ? 'drawing' : 'drawing empty'
        if (src) {
          const img = document.createElement('img')
          img.src = src
          img.alt = 'Drawing'
          dom.replaceChildren(img, button('Edit drawing', 'btn ghost s edit', edit))
        } else {
          const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg')
          svg.setAttribute('class', 'ic')
          svg.setAttribute('viewBox', '0 0 24 24')
          svg.setAttribute('aria-hidden', 'true')
          const path = document.createElementNS('http://www.w3.org/2000/svg', 'path')
          path.setAttribute('d', PENCIL)
          svg.append(path)
          const label = document.createElement('span')
          label.className = 'small muted'
          label.textContent = 'Empty drawing'
          dom.replaceChildren(svg, label, button('Draw', 'btn ghost s', edit))
        }
      }
      draw(node.attrs.src as string)
      if (openNew && !src) { openNew = false; queueMicrotask(edit) }
      return {
        dom,
        update: n => {
          if (n.type.name !== 'drawing') return false
          if (n.attrs.src !== src) draw(n.attrs.src as string)
          return true
        },
        selectNode: () => dom.classList.add('sel'),
        deselectNode: () => dom.classList.remove('sel'),
        // The buttons are ours; ProseMirror would turn their clicks into a node selection.
        stopEvent: e => e.target instanceof HTMLButtonElement,
        ignoreMutation: () => true,
      }
    }
  },
})

// One listbox renderer for both menus; text goes in via textContent, so page names can't inject markup.
// `k` = the block's glyph tile; `group` = the `.mh` header drawn above each run of items.
type Item = { title: string, hint?: string, k?: string, group: string }

function menu<I extends Item>(label: string, empty?: string): NonNullable<SuggestionOptions<I, I>['render']> {
  return () => {
    let el: HTMLDivElement | null = null
    let unmount: (() => void) | undefined
    let items: I[] = []
    let sel = 0
    let pick: (it: I) => void = () => {}
    const draw = () => {
      if (!el) return
      const kids: HTMLElement[] = []
      items.forEach((it, i) => {
        if (it.group !== items[i - 1]?.group) {
          const h = document.createElement('div')
          h.className = 'mh'
          h.setAttribute('role', 'presentation')
          h.textContent = it.group
          kids.push(h)
        }
        const o = document.createElement('div')
        o.setAttribute('role', 'option')
        o.setAttribute('aria-selected', String(i === sel))
        if (it.k) {
          const k = document.createElement('span')
          k.className = 'k'
          k.setAttribute('aria-hidden', 'true')
          k.textContent = it.k
          o.append(k)
        }
        o.append(it.title)
        if (it.hint) {
          const s = document.createElement('span')
          s.className = 'small muted'
          s.textContent = ' ' + it.hint
          o.append(s)
        }
        // mousedown, not click: click would blur the editor first and close the menu.
        o.addEventListener('mousedown', e => { e.preventDefault(); pick(it) })
        kids.push(o)
      })
      // Say "no match" rather than vanish, so `/` mid-sentence reads as plain text on purpose.
      if (!items.length && empty) {
        const o = document.createElement('div')
        o.setAttribute('role', 'option')
        o.setAttribute('aria-disabled', 'true')
        o.className = 'muted'
        o.textContent = empty
        kids.push(o)
      }
      el.replaceChildren(...kids)
      el.hidden = kids.length === 0
    }
    const update = (p: SuggestionProps<I, I>) => { items = p.items; pick = p.command; sel = 0; draw() }
    return {
      onStart: p => {
        const box = document.createElement('div')
        box.className = 'menu'
        box.setAttribute('role', 'listbox')
        box.setAttribute('aria-label', label)
        el = box
        update(p)
        unmount = p.mount(box)
        // Tiptap's mount sizes the popup to max-content; the menu's width is the CSS's 260.
        box.style.width = ''
      },
      onUpdate: update,
      onKeyDown: ({ event }: SuggestionKeyDownProps) => {
        if (!items.length) return false
        if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
          sel = (sel + (event.key === 'ArrowDown' ? 1 : items.length - 1)) % items.length
          draw()
          return true
        }
        const it = items[sel]
        if (event.key === 'Enter' && it) { pick(it); return true }
        return false
      },
      onExit: () => { unmount?.(); el = null },
    }
  }
}

type Block = Item & { run: (c: ChainedCommands) => ChainedCommands }
type Page = Item & { id: string }
type PagesJson = { data: { pages: { id: string, name?: string | null, type: string }[] } }

const group = 'Insert block'
const BLOCKS: Block[] = [
  { title: 'Heading', k: 'H', group, run: c => c.setNode('heading', { level: 3 }) },
  { title: 'To-do list', k: '☐', group, run: c => c.toggleTaskList() },
  { title: 'Toggle', k: '▸', group, run: c => c.setDetails() },
  { title: 'Callout', k: 'i', group, run: c => c.setCallout() },
  { title: 'Code block', k: '{}', group, run: c => c.setCodeBlock() },
  { title: 'Divider', k: '—', group, run: c => c.setHorizontalRule() },
  { title: 'Drawing', k: '✎', hint: 'opens the canvas', group, run: c => { openNew = true; return c.insertContent({ type: 'drawing' }) } },
  { title: 'Link a page', k: '[[', group, run: c => c.insertContent('[[') },
]
const PAGE_GROUP: Record<string, string> = { epic: 'Epics', topic: 'Topics', meeting: 'Meetings' }

const Menus = Extension.create({
  name: 'menus',
  addProseMirrorPlugins() {
    return [
      Suggestion<Block, Block>({
        editor: this.editor,
        pluginKey: new PluginKey('slash'),
        char: '/',
        items: ({ query }) => BLOCKS.filter(b => b.title.toLowerCase().includes(query.toLowerCase())),
        command: ({ editor, range, props }) => props.run(editor.chain().focus().deleteRange(range)).run(),
        render: menu<Block>('Insert block', 'No blocks match. Keep typing as text.'),
      }),
      Suggestion<Page, Page>({
        editor: this.editor,
        pluginKey: new PluginKey('pages'),
        char: '[[',
        items: async ({ query, signal }) => {
          const r = await fetch('/api/pages?q=' + encodeURIComponent(query), { signal })
          const { data } = await r.json() as PagesJson
          // The API returns pages grouped by type, so each kind gets one header.
          return data.pages.map(p => ({ id: p.id, title: p.name ? `${p.id} · ${p.name}` : p.id, group: PAGE_GROUP[p.type] ?? p.type }))
        },
        command: ({ editor, range, props }) =>
          editor.chain().focus().deleteRange(range)
            .insertContent([{ type: 'pageLink', attrs: { id: props.id } }, { type: 'text', text: ' ' }]).run(),
        render: menu<Page>('Link a page'),
      }),
    ]
  },
})

const el = document.getElementById('editor')
const md = document.getElementById('note-md') as HTMLTextAreaElement | null
if (el && md) {
  md.hidden = true
  el.replaceChildren()
  new Editor({
    element: el,
    extensions: [
      StarterKit.configure({
        heading: { levels: [1, 2, 3] },
        // No markdown form for underline, so it would not survive a save.
        underline: false,
        link: { openOnClick: false, autolink: true, linkOnPaste: true, defaultProtocol: 'https' },
      }),
      TaskList,
      TaskItem.configure({ nested: true }),
      Details,
      DetailsSummary,
      DetailsContent,
      Callout,
      PageLink,
      Drawing,
      Menus,
      Markdown,
    ],
    content: md.value,
    contentType: 'markdown',
    onUpdate: ({ editor }) => {
      md.value = editor.getMarkdown()
      // htmx's `input delay:1s` on the form is the debounce; this just tells it the body changed.
      md.dispatchEvent(new Event('input', { bubbles: true }))
    },
  })
}
