// One-off bundle of the TypeScript sources → static/js/*.js (committed; not part of cargo build).
// Run after changing src/ or package versions: `npm ci && npm run check && node build.mjs` from editor/.
// esbuild only strips types; `npm run check` (tsc) is what type-checks.
import { build } from 'esbuild'
import { cpSync, rmSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

const dir = fileURLToPath(new URL('.', import.meta.url))
const common = { absWorkingDir: dir, bundle: true, minify: true, target: 'es2022', legalComments: 'none' }

const entries = {
  editor: '/* atharu notes editor, built by editor/build.mjs. Bundles Tiptap, ProseMirror, marked, Floating UI (all MIT). */',
  copy: '/* atharu copy buttons, built by editor/build.mjs from editor/src/copy.ts. */',
  glass: '/* atharu glass preview on /settings, built by editor/build.mjs from editor/src/glass.ts. */',
  capture: '/* atharu capture-bar tag suggestions, built by editor/build.mjs from editor/src/capture.ts. */',
}

// One build per entry: esbuild's banner is per build, and each file names what it bundles.
await Promise.all(Object.entries(entries).map(([name, banner]) => build({
  ...common,
  entryPoints: [`src/${name}.ts`],
  outfile: `../static/js/${name}.js`,
  format: 'iife',
  banner: { js: banner },
})))

// Drawing island (D24): ESM + splitting, because one iife would inline every lazy chunk (8.5 MB). Hashed chunk names
// change with versions, so the folder is rebuilt from empty rather than left to collect stale files.
rmSync('../static/js/drawing', { recursive: true, force: true })
await build({
  ...common,
  entryPoints: ['src/drawing.ts'],
  outdir: '../static/js/drawing',
  format: 'esm',
  splitting: true,
  chunkNames: 'c-[hash]',
  conditions: ['production'],
  define: { 'process.env.NODE_ENV': '"production"', 'import.meta.env': '{}' },
  // Mermaid-to-Excalidraw is ~3.6 MB of lazy chunks for a dialog we never open.
  alias: { '@excalidraw/mermaid-to-excalidraw': './src/mermaid-stub.js' },
  banner: { js: '/* atharu drawing editor, built by editor/build.mjs. Bundles Excalidraw, React (MIT). */' },
})

// Excalidraw's own assets, served from EXCALIDRAW_ASSET_PATH so nothing comes from its CDN. Xiaolai (CJK, 12.7 MB) is
// left out: without it CJK text falls back to a system font.
const x = 'node_modules/@excalidraw/excalidraw/dist/prod'
rmSync('../static/excalidraw', { recursive: true, force: true })
cpSync(`${x}/index.css`, '../static/excalidraw/index.css')
cpSync(`${x}/fonts`, '../static/excalidraw/fonts', { recursive: true, filter: p => !p.includes('Xiaolai') })
