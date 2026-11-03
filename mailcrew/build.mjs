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

/** @type {esbuild.BuildOptions} */
const options = {
  entryPoints: ['src/main.ts'],
  bundle: true,
  format: 'esm',
  splitting: true,
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
