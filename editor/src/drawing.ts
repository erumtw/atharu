// Drawing editor island (D24): React + Excalidraw, loaded by editor.ts only when a drawing is opened, so a note
// page without one pays 0 bytes. No React app: one createRoot on the sheet, unmounted on Done/Cancel.
import { createElement } from 'react'
import { createRoot } from 'react-dom/client'
import { Excalidraw, exportToSvg, getSceneVersion, loadFromBlob } from '@excalidraw/excalidraw'
import type { ExcalidrawImperativeAPI } from '@excalidraw/excalidraw/types'

export interface Session {
  /** `data:image/svg+xml;base64,…` with the scene embedded, or '' when nothing is drawn. */
  src: (inlineFonts: boolean) => Promise<string>
  close: () => void
}

const PREFIX = 'data:image/svg+xml;base64,'

function toBase64(s: string): string {
  const bytes = new TextEncoder().encode(s)
  let bin = ''
  // Chunked: String.fromCharCode(...all) overflows the call stack on a big drawing.
  for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000))
  return btoa(bin)
}

export async function open(host: HTMLElement, src: string, changed: () => void): Promise<Session> {
  // The scene rides inside the stored SVG (Excalidraw's own embed-scene format), so the note keeps one block, not two.
  const scene = src.startsWith(PREFIX)
    ? await loadFromBlob(new Blob([Uint8Array.from(atob(src.slice(PREFIX.length)), c => c.charCodeAt(0))], { type: 'image/svg+xml' }), null, null)
    : null
  let api: ExcalidrawImperativeAPI | null = null
  let version = scene ? getSceneVersion(scene.elements) : 0
  const root = createRoot(host)
  root.render(createElement(Excalidraw, {
    excalidrawAPI: (a: ExcalidrawImperativeAPI) => { api = a },
    autoFocus: true,
    initialData: { elements: scene?.elements ?? [], files: scene?.files, appState: { viewBackgroundColor: '#ffffff' }, scrollToContent: true },
    UIOptions: {
      canvasActions: { loadScene: false, saveToActiveFile: false, export: false, saveAsImage: false, toggleTheme: false },
      // Off (D24): an image would land base64 in the note body.
      tools: { image: false },
    },
    // onChange also fires on scroll and selection; only a new scene version is an edit worth saving.
    onChange: elements => {
      const v = getSceneVersion(elements)
      if (v !== version) { version = v; changed() }
    },
  }))
  return {
    src: async inlineFonts => {
      if (!api) return src
      const elements = api.getSceneElements()
      if (!elements.length) return ''
      const svg = await exportToSvg({
        elements,
        appState: { exportBackground: false, exportWithDarkMode: false, exportEmbedScene: true },
        files: api.getFiles(),
        // Inlining pulls Excalidraw's 1.8 MB font-subsetting chunk, so autosaves skip it and Done pays it once.
        ...(inlineFonts ? {} : { skipInliningFonts: true as const }),
      })
      return PREFIX + toBase64(svg.outerHTML)
    },
    close: () => root.unmount(),
  }
}
