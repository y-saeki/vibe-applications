// A tiny PDF writer for the tests: `pages` 16:9 pages, each showing its own
// page number large enough to read in a screenshot, on a colour that changes
// from page to page. Written by hand so the suite needs no PDF library.

export function makePdf(pages: number): Uint8Array {
  const objects: string[] = []
  const add = (body: string): number => {
    objects.push(body)
    return objects.length
  }

  const catalog = add('') // filled in below, once the page tree exists
  const pageTree = add('')
  const font = add('<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold >>')
  const kids: number[] = []
  for (let n = 1; n <= pages; n++) {
    const hue = (n * 47) % 360
    const [r, g, b] = hsl(hue, 0.45, 0.82)
    const label = n === 1 ? 'COVER' : String(n - 1)
    const stream = [
      `${r} ${g} ${b} rg 0 0 960 540 re f`,
      `0.1 0.1 0.1 rg BT /F1 200 Tf ${480 - label.length * 58} 200 Td (${label}) Tj ET`,
    ].join('\n')
    const contents = add(`<< /Length ${stream.length} >>\nstream\n${stream}\nendstream`)
    kids.push(
      add(
        `<< /Type /Page /Parent ${pageTree} 0 R /MediaBox [0 0 960 540] /Resources << /Font << /F1 ${font} 0 R >> >> /Contents ${contents} 0 R >>`,
      ),
    )
  }
  objects[catalog - 1] = `<< /Type /Catalog /Pages ${pageTree} 0 R >>`
  objects[pageTree - 1] = `<< /Type /Pages /Kids [${kids.map((k) => `${k} 0 R`).join(' ')}] /Count ${pages} >>`

  let out = '%PDF-1.4\n'
  const offsets: number[] = []
  objects.forEach((body, i) => {
    offsets.push(out.length)
    out += `${i + 1} 0 obj\n${body}\nendobj\n`
  })
  const xref = out.length
  out += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`
  for (const offset of offsets) out += `${String(offset).padStart(10, '0')} 00000 n \n`
  out += `trailer\n<< /Size ${objects.length + 1} /Root ${catalog} 0 R >>\nstartxref\n${xref}\n%%EOF\n`
  return new TextEncoder().encode(out)
}

function hsl(h: number, s: number, l: number): [string, string, string] {
  const k = (n: number) => (n + h / 30) % 12
  const a = s * Math.min(l, 1 - l)
  const f = (n: number) => l - a * Math.max(-1, Math.min(k(n) - 3, Math.min(9 - k(n), 1)))
  return [f(0), f(8), f(4)].map((v) => v.toFixed(3)) as [string, string, string]
}
