// One-off bundle of the TypeScript sources → static/js/*.js (committed; not part of cargo build).
// Run after changing src/ or package versions: `npm ci && npm run check && node build.mjs` from editor/.
// esbuild only strips types; `npm run check` (tsc) is what type-checks.
import { build } from 'esbuild'
import { fileURLToPath } from 'node:url'

const entries = {
  editor: '/* atharu notes editor, built by editor/build.mjs. Bundles Tiptap, ProseMirror, marked, Floating UI (all MIT). */',
  copy: '/* atharu copy buttons, built by editor/build.mjs from editor/src/copy.ts. */',
  glass: '/* atharu glass preview on /settings, built by editor/build.mjs from editor/src/glass.ts. */',
}

// One build per entry: esbuild's banner is per build, and each file names what it bundles.
await Promise.all(Object.entries(entries).map(([name, banner]) => build({
  absWorkingDir: fileURLToPath(new URL('.', import.meta.url)),
  entryPoints: [`src/${name}.ts`],
  outfile: `../static/js/${name}.js`,
  bundle: true,
  minify: true,
  format: 'iife',
  target: 'es2022',
  legalComments: 'none',
  banner: { js: banner },
})))
