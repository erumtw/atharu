// Notes editor (spec P4): Tiptap with markdown as the stored format. Mounted on #editor;
// the hidden #note-md textarea carries the markdown, so htmx autosave and no-JS posts are the same form.
import { Editor, Extension, InputRule, Node, createBlockMarkdownSpec, mergeAttributes } from '@tiptap/core'
import StarterKit from '@tiptap/starter-kit'
import { TaskItem, TaskList } from '@tiptap/extension-list'
import { Details, DetailsContent, DetailsSummary } from '@tiptap/extension-details'
import { Markdown } from '@tiptap/markdown'
import Suggestion from '@tiptap/suggestion'
import { PluginKey } from '@tiptap/pm/state'

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
  parseHTML: () => [{ tag: 'span[data-page]', getAttrs: el => ({ id: el.getAttribute('data-page') }) }],
  renderHTML: ({ node }) => ['span', { 'data-page': node.attrs.id, class: 'chip page' }, node.attrs.id],
  renderText: ({ node }) => `[[${node.attrs.id}]]`,
  markdownTokenizer: {
    name: 'pageLink',
    level: 'inline',
    start: src => src.indexOf('[['),
    tokenize: src => {
      const m = PAGE.exec(src)
      if (m) return { type: 'pageLink', raw: m[0], id: m[1].trim() }
    },
  },
  parseMarkdown: (token, h) => h.createNode('pageLink', { id: token.id }),
  renderMarkdown: node => `[[${node.attrs.id}]]`,
  // Typing `[[X]]` by hand becomes a chip too. Not nodeInputRule: it would keep the brackets around the node.
  addInputRules() {
    return [new InputRule({
      find: /\[\[([^[\]\n]+)\]\]$/,
      handler: ({ state, range, match }) => { state.tr.replaceWith(range.from, range.to, this.type.create({ id: match[1].trim() })) },
    })]
  },
})

// One listbox renderer for both menus; text goes in via textContent, so page names can't inject markup.
function menu(label) {
  return () => {
    let el, unmount, items = [], sel = 0, pick
    const draw = () => {
      el.replaceChildren(...items.map((it, i) => {
        const o = document.createElement('div')
        o.setAttribute('role', 'option')
        o.setAttribute('aria-selected', String(i === sel))
        o.textContent = it.title
        if (it.hint) {
          const s = document.createElement('span')
          s.className = 'small muted'
          s.textContent = ' ' + it.hint
          o.append(s)
        }
        // mousedown, not click: click would blur the editor first and close the menu.
        o.addEventListener('mousedown', e => { e.preventDefault(); pick(it) })
        return o
      }))
      el.hidden = items.length === 0
    }
    const update = p => { items = p.items; pick = p.command; sel = 0; draw() }
    return {
      onStart: p => {
        el = document.createElement('div')
        el.className = 'menu'
        el.setAttribute('role', 'listbox')
        el.setAttribute('aria-label', label)
        update(p)
        unmount = p.mount(el)
      },
      onUpdate: update,
      onKeyDown: ({ event }) => {
        if (!items.length) return false
        if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
          sel = (sel + (event.key === 'ArrowDown' ? 1 : items.length - 1)) % items.length
          draw()
          return true
        }
        if (event.key === 'Enter') { pick(items[sel]); return true }
        return false
      },
      onExit: () => { unmount?.(); el = null },
    }
  }
}

const BLOCKS = [
  { title: 'Heading', hint: '#', run: c => c.setNode('heading', { level: 3 }) },
  { title: 'To-do list', hint: '[]', run: c => c.toggleTaskList() },
  { title: 'Toggle', run: c => c.setDetails() },
  { title: 'Callout', run: c => c.setCallout() },
  { title: 'Code block', hint: '```', run: c => c.setCodeBlock() },
  { title: 'Divider', hint: '---', run: c => c.setHorizontalRule() },
  { title: 'Link a page', hint: '[[', run: c => c.insertContent('[[') },
]

const Menus = Extension.create({
  name: 'menus',
  addProseMirrorPlugins() {
    return [
      Suggestion({
        editor: this.editor,
        pluginKey: new PluginKey('slash'),
        char: '/',
        items: ({ query }) => BLOCKS.filter(b => b.title.toLowerCase().includes(query.toLowerCase())),
        command: ({ editor, range, props }) => props.run(editor.chain().focus().deleteRange(range)).run(),
        render: menu('Insert block'),
      }),
      Suggestion({
        editor: this.editor,
        pluginKey: new PluginKey('pages'),
        char: '[[',
        items: async ({ query, signal }) => {
          const r = await fetch('/api/pages?q=' + encodeURIComponent(query), { signal })
          const { data } = await r.json()
          return data.pages.map(p => ({ id: p.id, title: p.name ? `${p.id} · ${p.name}` : p.id, hint: p.type }))
        },
        command: ({ editor, range, props }) =>
          editor.chain().focus().deleteRange(range)
            .insertContent([{ type: 'pageLink', attrs: { id: props.id } }, { type: 'text', text: ' ' }]).run(),
        render: menu('Link a page'),
      }),
    ]
  },
})

const el = document.getElementById('editor')
const md = document.getElementById('note-md')
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
