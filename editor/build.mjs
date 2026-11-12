// One-off bundle of the notes editor → static/js/editor.js (committed; not part of cargo build).
// Run after changing src/ or package versions: `npm ci && node build.mjs` from editor/.
import { build } from 'esbuild'
import { fileURLToPath } from 'node:url'

await build({
  absWorkingDir: fileURLToPath(new URL('.', import.meta.url)),
  entryPoints: ['src/editor.js'],
  outfile: '../static/js/editor.js',
  bundle: true,
  minify: true,
  format: 'iife',
  target: 'es2022',
  legalComments: 'none',
  banner: {
    js: '/* atharu notes editor, built by editor/build.mjs. Bundles Tiptap, ProseMirror, marked, Floating UI (all MIT). */',
  },
})
