// The language list shown in the status bar. Markdown uses
// the Lezer-based @codemirror/lang-markdown; everything else is a stream
// parser from @codemirror/legacy-modes or a small mode of our own. Each
// grammar is loaded on demand so startup only pays for the selected language.

import { LanguageDescription, LanguageSupport, StreamLanguage, type StreamParser } from '@codemirror/language'
import type { Extension } from '@codemirror/state'

type ParserLoader = () => Promise<StreamParser<unknown>>

export interface LanguageDef {
  id: string
  label: string
  /** Extra names recognised in Markdown fenced code blocks (```js ...). */
  alias?: readonly string[]
  parser?: ParserLoader
}

const stream = (id: string, label: string, parser: ParserLoader, alias?: readonly string[]): LanguageDef =>
  alias ? { id, label, parser, alias } : { id, label, parser }

export const LANGUAGES: readonly LanguageDef[] = [
  { id: 'markdown', label: 'Markdown', alias: ['md'] },
  { id: 'text', label: 'プレーンテキスト' },
  stream('yaml', 'YAML', async () => (await import('@codemirror/legacy-modes/mode/yaml')).yaml, ['yml']),
  stream('bat', 'Batch', async () => (await import('./modes/batch')).batch, ['cmd', 'batch']),
  stream('html', 'HTML', async () => (await import('@codemirror/legacy-modes/mode/xml')).html),
  stream('xml', 'XML', async () => (await import('@codemirror/legacy-modes/mode/xml')).xml),
  stream('dockerfile', 'Dockerfile', async () => (await import('@codemirror/legacy-modes/mode/dockerfile')).dockerFile, ['docker']),
  stream('javascript', 'JavaScript', async () => (await import('@codemirror/legacy-modes/mode/javascript')).javascript, ['js', 'jsx']),
  stream('typescript', 'TypeScript', async () => (await import('@codemirror/legacy-modes/mode/javascript')).typescript, ['ts', 'tsx']),
  stream('ruby', 'Ruby', async () => (await import('@codemirror/legacy-modes/mode/ruby')).ruby, ['rb']),
  stream('go', 'Go', async () => (await import('@codemirror/legacy-modes/mode/go')).go, ['golang']),
  stream('css', 'CSS', async () => (await import('@codemirror/legacy-modes/mode/css')).css),
  stream('less', 'LESS', async () => (await import('@codemirror/legacy-modes/mode/css')).less),
  stream('scss', 'SCSS', async () => (await import('@codemirror/legacy-modes/mode/css')).sCSS),
  stream('solidity', 'Solidity', async () => (await import('./modes/solidity')).solidity, ['sol']),
  stream('mysql', 'MySQL', async () => (await import('@codemirror/legacy-modes/mode/sql')).mySQL, ['sql']),
  stream('pgsql', 'pgSQL', async () => (await import('@codemirror/legacy-modes/mode/sql')).pgSQL, ['postgres', 'postgresql']),
  stream('php', 'PHP', async () => (await import('./modes/php')).php),
  stream('powershell', 'PowerShell', async () => (await import('@codemirror/legacy-modes/mode/powershell')).powerShell, ['ps1', 'pwsh']),
  stream('rust', 'Rust', async () => (await import('@codemirror/legacy-modes/mode/rust')).rust, ['rs']),
]

export const DEFAULT_LANGUAGE = 'markdown'

export function findLanguage(id: string): LanguageDef {
  return LANGUAGES.find((lang) => lang.id === id) ?? LANGUAGES[0]!
}

async function streamSupport(id: string, parser: ParserLoader): Promise<LanguageSupport> {
  const loaded = await parser()
  // Some legacy modes carry no name; give them ours so `data-language` is set.
  return new LanguageSupport(StreamLanguage.define(loaded.name ? loaded : { ...loaded, name: id }))
}

/** Descriptions for the fenced code blocks inside Markdown. */
const codeLanguages: readonly LanguageDescription[] = LANGUAGES.filter((lang) => lang.parser).map((lang) =>
  LanguageDescription.of({
    name: lang.label,
    alias: [lang.id, ...(lang.alias ?? [])],
    load: () => streamSupport(lang.id, lang.parser!),
  }),
)

export async function languageExtension(id: string): Promise<Extension> {
  const lang = findLanguage(id)
  if (lang.id === 'markdown') {
    const { markdown } = await import('@codemirror/lang-markdown')
    return markdown({ codeLanguages })
  }
  if (!lang.parser) return []
  return streamSupport(lang.id, lang.parser)
}
