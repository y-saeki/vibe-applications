// Keeps src/style.css built out of its own tokens.
//
// The chrome has no UI library behind it, so what holds its proportions
// together is that every size, spacing and color lives in the :root blocks of
// src/style.css and nowhere else. Two things can quietly undo that: writing a
// number where it is used instead of reaching for a token, and answering every
// new need with a new token until the "scale" is just a list. This checks both.
//
// Run with `pnpm lint:style`; the frontend CI job runs it too. Raising a
// budget below is a deliberate one-line change — that is the point of it.
//
// A check that stops checking passes everything, so the rules are run against
// the fixtures at the bottom on every invocation before the real stylesheet is
// read. If one of those stops being reported, this exits non-zero saying so.

import { readFileSync } from 'node:fs'

/** How many tokens each family may hold, and which values it accepts. */
const FAMILIES = [
  {
    name: 'type',
    prefix: '--font-size-',
    budget: 3,
    accepts: (px) => Number.isInteger(px) && px >= 11 && px <= 18,
    wants: 'a whole number of pixels between 11 and 18',
  },
  {
    name: 'spacing',
    prefix: '--space-',
    budget: 5,
    accepts: (px) => px > 0 && px % 4 === 0,
    wants: 'a multiple of 4px',
  },
  {
    name: 'corners',
    prefix: '--radius-',
    budget: 3,
    accepts: (px) => px > 0 && px <= 8 && px % 2 === 0,
    wants: 'an even number of pixels, 8 or under',
  },
]

const LIMITS = {
  families: FAMILIES,
  /** Everything else measured in pixels: control metrics and layout. */
  otherLengths: { budget: 12, accepts: (px) => px % 4 === 0, wants: 'a multiple of 4px' },
  /** The palette, including the two shadows and the tick and chevron images. */
  palette: 22,
  /** Hairlines, focus rings and optical nudges stay where they are used. */
  literalPx: 2,
}

// ---- the rules ---------------------------------------------------------

/**
 * Reports everything wrong with one stylesheet.
 *
 * @param css the stylesheet
 * @param usedIn other sources that may read its tokens, as text
 * @param label how to name the file in a message
 * @param limits budgets and value rules; the fixtures narrow these
 * @returns one string per problem, empty when there is none
 */
export function findProblems({ css, usedIn = [], label = 'style.css', limits = LIMITS }) {
  const problems = []

  /** Blanks out comments, keeping every offset and line break in place. */
  const bare = (() => {
    let out = ''
    for (let i = 0; i < css.length; i++) {
      if (css[i] === '/' && css[i + 1] === '*') {
        const close = css.indexOf('*/', i + 2)
        const stop = close === -1 ? css.length : close + 2
        for (let j = i; j < stop; j++) out += css[j] === '\n' ? '\n' : ' '
        i = stop - 1
        continue
      }
      out += css[i]
    }
    return out
  })()

  const at = (offset) => `${label}:${bare.slice(0, offset).split('\n').length}`
  const report = (where, message) => problems.push(`${where}: ${message}`)

  /** Every top-level rule, as its selector plus the text between the braces. */
  const rules = []
  {
    let depth = 0
    let selectorStart = 0
    let bodyStart = 0
    let selector = ''
    let quote = null
    for (let i = 0; i < bare.length; i++) {
      const c = bare[i]
      if (quote) {
        if (c === quote && bare[i - 1] !== '\\') quote = null
        continue
      }
      if (c === '"' || c === "'") {
        quote = c
        continue
      }
      if (c === '{') {
        if (depth === 0) {
          selector = bare.slice(selectorStart, i).trim()
          bodyStart = i + 1
        }
        depth++
      } else if (c === '}') {
        depth--
        if (depth === 0) {
          rules.push({ selector, body: bare.slice(bodyStart, i), offset: bodyStart })
          selectorStart = i + 1
        }
      }
    }
    if (depth !== 0) report(label, 'unbalanced braces; the rest of this check cannot be trusted')
  }

  const tokenBlocks = rules.filter((r) => r.selector.startsWith(':root'))
  const componentRules = rules.filter((r) => !r.selector.startsWith(':root'))

  // ---- the tokens themselves -------------------------------------------

  const declared = new Map()
  for (const rule of tokenBlocks) {
    for (const match of rule.body.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)) {
      const [, name, value] = match
      if (!declared.has(name)) declared.set(name, { value: value.trim(), offset: rule.offset + match.index })
    }
  }

  // A token declared on a component keeps a second inventory nobody reads.
  for (const rule of componentRules) {
    for (const match of rule.body.matchAll(/(--[\w-]+)\s*:/g)) {
      report(at(rule.offset + match.index), `${match[1]} is declared on "${rule.selector}"; tokens belong in the :root blocks`)
    }
  }

  const used = new Set()
  for (const source of [css, ...usedIn]) {
    for (const match of source.matchAll(/var\((--[\w-]+)/g)) used.add(match[1])
  }
  for (const [name, { offset }] of declared) {
    if (!used.has(name)) report(at(offset), `${name} is declared but never used; delete it`)
  }
  for (const name of used) {
    if (!declared.has(name)) report(label, `var(${name}) is used but never declared`)
  }

  // ---- token values and budgets ----------------------------------------

  const asPx = (value) => (/^\d+(\.\d+)?px$/.test(value) ? Number.parseFloat(value) : null)
  const inAFamily = new Set()

  for (const family of limits.families) {
    const members = [...declared].filter(([name]) => name.startsWith(family.prefix))
    for (const [name, { value, offset }] of members) {
      inAFamily.add(name)
      const px = asPx(value)
      if (px === null) report(at(offset), `${name} is in the ${family.name} family, so its value should be a length in px`)
      else if (!family.accepts(px)) report(at(offset), `${name} is ${value}; the ${family.name} family takes ${family.wants}`)
    }
    if (members.length > family.budget) {
      report(label, `the ${family.name} family holds ${members.length} tokens, over its budget of ${family.budget}. Reuse one, or raise the budget in lint-style.mjs on purpose`)
    }
  }

  const lengths = [...declared].filter(([name, { value }]) => !inAFamily.has(name) && asPx(value) !== null)
  for (const [name, { value, offset }] of lengths) {
    if (!limits.otherLengths.accepts(asPx(value))) {
      report(at(offset), `${name} is ${value}; a length outside the named families takes ${limits.otherLengths.wants}`)
    }
  }
  if (lengths.length > limits.otherLengths.budget) {
    report(label, `${lengths.length} control and layout lengths are declared, over the budget of ${limits.otherLengths.budget}. Reuse one, or raise the budget in lint-style.mjs on purpose`)
  }

  const palette = [...declared].filter(([name, { value }]) => !inAFamily.has(name) && asPx(value) === null && name !== '--ui-font')
  if (palette.length > limits.palette) {
    report(label, `the palette holds ${palette.length} tokens, over its budget of ${limits.palette}. Reuse one, or raise the budget in lint-style.mjs on purpose`)
  }

  // ---- numbers and colors written where they are used ------------------

  for (const rule of componentRules) {
    for (const match of rule.body.matchAll(/(?<![\w-])(\d+(?:\.\d+)?)(px|rem|em)\b/g)) {
      const [text, amount, unit] = match
      if (unit === 'px' && Number.parseFloat(amount) <= limits.literalPx) continue
      report(
        at(rule.offset + match.index),
        `"${rule.selector}" sizes something ${text}; use a token, or add one to :root if none fits (only px values up to ${limits.literalPx} may be written here)`,
      )
    }
    // Hex, rgb() and hsl() only: "transparent" and "currentColor" are not
    // palette decisions, and a stylesheet themed from custom properties has no
    // reason to reach for a named color.
    for (const match of rule.body.matchAll(/#[0-9a-fA-F]{3,8}\b|\b(?:rgba?|hsla?)\(/g)) {
      report(at(rule.offset + match.index), `"${rule.selector}" names the color ${match[0]} directly; put it in the palette`)
    }
  }

  return problems
}

// ---- proof that the rules still fire -----------------------------------

const CLEAN = ':root {\n  --space-1: 4px;\n}\n\n.a {\n  gap: var(--space-1);\n}\n'
const withBody = (body) => `:root {\n  --space-1: 4px;\n}\n\n.a {\n  gap: var(--space-1);\n${body}}\n`

const FIXTURES = [
  { name: 'a sheet with nothing wrong', css: CLEAN, expect: null },
  { name: 'a hairline, which may stay where it is used', css: withBody('  border: 1px solid;\n'), expect: null },
  { name: 'a length written on a component', css: withBody('  padding: 9px;\n'), expect: /sizes something 9px/ },
  { name: 'a length in em', css: withBody('  padding: 3em;\n'), expect: /sizes something 3em/ },
  { name: 'a color written on a component', css: withBody('  color: #abc;\n'), expect: /names the color #abc/ },
  { name: 'an rgba() written on a component', css: withBody('  color: rgba(0, 0, 0, 0.5);\n'), expect: /names the color rgba\(/ },
  { name: 'a token declared on a component', css: withBody('  --gap: 4px;\n'), expect: /--gap is declared on "\.a"/ },
  { name: 'a token nothing uses', css: ':root {\n  --space-1: 4px;\n}\n', expect: /--space-1 is declared but never used/ },
  { name: 'a misspelt reference', css: ':root {\n  --space-1: 4px;\n}\n\n.a {\n  gap: var(--spcae-1);\n}\n', expect: /var\(--spcae-1\) is used but never declared/ },
  { name: 'a spacing step off the 4px scale', css: ':root {\n  --space-1: 5px;\n}\n\n.a {\n  gap: var(--space-1);\n}\n', expect: /the spacing family takes a multiple of 4px/ },
  { name: 'a type token nobody can read', css: ':root {\n  --font-size-a: 5px;\n}\n\n.a {\n  font-size: var(--font-size-a);\n}\n', expect: /between 11 and 18/ },
  { name: 'a corner too round', css: ':root {\n  --radius-a: 12px;\n}\n\n.a {\n  border-radius: var(--radius-a);\n}\n', expect: /an even number of pixels, 8 or under/ },
  {
    name: 'one type token too many',
    css: ':root {\n  --font-size-a: 12px;\n  --font-size-b: 13px;\n}\n\n.a {\n  font-size: var(--font-size-a);\n}\n\n.b {\n  font-size: var(--font-size-b);\n}\n',
    limits: { ...LIMITS, families: [{ ...FAMILIES[0], budget: 1 }] },
    expect: /the type family holds 2 tokens, over its budget of 1/,
  },
  {
    name: 'one layout length too many',
    css: ':root {\n  --a-width: 40px;\n  --b-width: 80px;\n}\n\n.a {\n  width: var(--a-width);\n}\n\n.b {\n  width: var(--b-width);\n}\n',
    limits: { ...LIMITS, otherLengths: { ...LIMITS.otherLengths, budget: 1 } },
    expect: /2 control and layout lengths are declared, over the budget of 1/,
  },
  {
    name: 'one palette entry too many',
    css: ':root {\n  --a: #fff;\n  --b: #000;\n}\n\n.a {\n  color: var(--a);\n  background: var(--b);\n}\n',
    limits: { ...LIMITS, palette: 1 },
    expect: /the palette holds 2 tokens, over its budget of 1/,
  },
]

const broken = []
for (const { name, css, limits, expect } of FIXTURES) {
  const problems = findProblems({ css, label: 'fixture', limits })
  if (expect === null) {
    if (problems.length > 0) broken.push(`"${name}" should pass, but: ${problems.join('; ')}`)
  } else if (!problems.some((problem) => expect.test(problem))) {
    broken.push(`"${name}" should be reported by ${expect}, but got: ${problems.join('; ') || '(nothing)'}`)
  }
}
if (broken.length > 0) {
  console.error('lint-style.mjs is not checking what it claims to:')
  for (const line of broken) console.error(`  ${line}`)
  process.exit(1)
}

// ---- the real stylesheet -----------------------------------------------

const STYLE = new URL('./src/style.css', import.meta.url)
// src/dark-theme.ts reads the palette too, so a token used only there is used.
const OTHER_CONSUMERS = [new URL('./src/dark-theme.ts', import.meta.url)]

const problems = findProblems({
  css: readFileSync(STYLE, 'utf8'),
  usedIn: OTHER_CONSUMERS.map((url) => readFileSync(url, 'utf8')),
  label: 'src/style.css',
})

if (problems.length > 0) {
  for (const problem of problems) console.error(problem)
  console.error(`\n${problems.length} problem${problems.length === 1 ? '' : 's'}. See "UI の寸法と色" in DEVELOPMENT.md.`)
  process.exit(1)
}

console.log(`src/style.css: every size and color comes from a token, and ${FIXTURES.length} fixtures confirm this check still reports.`)
