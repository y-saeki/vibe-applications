// Frontend build script. `node build.mjs` produces dist/ for `tauri build`;
// `node build.mjs --serve` watches and serves it for `tauri dev`.
import * as esbuild from 'esbuild'
import { cpSync, mkdirSync, rmSync } from 'node:fs'

const serve = process.argv.includes('--serve')
const outdir = 'dist'
const port = 1420

rmSync(outdir, { recursive: true, force: true })
mkdirSync(outdir, { recursive: true })
cpSync('src/index.html', `${outdir}/index.html`)
cpSync('src/style.css', `${outdir}/style.css`)

// PDF.js's worker and the data files it fetches at run time (src/deck.ts). The
// legacy build matches the one src/deck.ts imports: it also runs on the older
// WKWebView of macOS 11.
const pdfjs = 'node_modules/pdfjs-dist'
mkdirSync(`${outdir}/pdfjs`, { recursive: true })
cpSync(`${pdfjs}/legacy/build/pdf.worker.min.mjs`, `${outdir}/pdfjs/pdf.worker.min.mjs`)
for (const dir of ['cmaps', 'standard_fonts', 'wasm', 'iccs']) {
  cpSync(`${pdfjs}/${dir}`, `${outdir}/pdfjs/${dir}`, {
    recursive: true,
    // QuickJS runs a document's own JavaScript, which pecha-player never does.
    filter: (path) => !path.includes('quickjs'),
  })
}

/** @type {esbuild.BuildOptions} */
const options = {
  entryPoints: ['src/main.ts'],
  bundle: true,
  format: 'esm',
  outdir,
  // WKWebView on macOS 11+ and WebView2 (evergreen Chromium) both run ES2020 natively.
  target: 'es2020',
  minify: !serve,
  sourcemap: serve ? 'inline' : false,
  legalComments: 'none',
  logLevel: 'info',
}

if (serve) {
  const ctx = await esbuild.context(options)
  await ctx.watch()
  await ctx.serve({ servedir: outdir, host: '127.0.0.1', port })
  console.log(`serving http://127.0.0.1:${port}`)
} else {
  await esbuild.build(options)
}
