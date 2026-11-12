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
