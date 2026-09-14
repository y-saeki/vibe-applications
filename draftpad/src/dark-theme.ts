// The dark look of the editor itself. Everything CodeMirror paints on its own —
// background, caret, selection, search matches, tooltips and the syntax colors —
// is set here; the chrome around it is styled in style.css.
//
// Only the syntax colors are literals. The rest reads the same custom properties
// style.css defines for `:root[data-theme="dark"]`, so the palette has one home.

import { HighlightStyle, syntaxHighlighting } from '@codemirror/language'
import type { Extension } from '@codemirror/state'
import { EditorView } from '@codemirror/view'
import { tags as t } from '@lezer/highlight'

const comment = '#808080'
const keyword = '#c792ea'
const string = '#9bd07a'
const constant = '#e0a070'
const type = '#e6c07b'
const name = '#e8737c'
const callable = '#7cc0ff'
const operator = '#5fc9d8'
const invalid = '#ff6b6b'

const darkEditorTheme = EditorView.theme(
  {
    '&': {
      color: 'var(--fg)',
      backgroundColor: 'var(--bg)',
    },
    '.cm-content': { caretColor: 'var(--editor-caret)' },
    '.cm-cursor, .cm-dropCursor': { borderLeftColor: 'var(--editor-caret)' },
    '&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection': {
      backgroundColor: 'var(--editor-selection)',
    },
    '.cm-searchMatch': {
      backgroundColor: 'var(--editor-search-match)',
      outline: '1px solid var(--editor-search-match-outline)',
    },
    '.cm-searchMatch.cm-searchMatch-selected': { backgroundColor: 'var(--editor-search-match-selected)' },
    '.cm-selectionMatch': { backgroundColor: 'var(--editor-selection-match)' },
    '&.cm-focused .cm-matchingBracket, &.cm-focused .cm-nonmatchingBracket': {
      backgroundColor: 'var(--editor-bracket)',
    },
    '.cm-tooltip': {
      border: '1px solid var(--border)',
      borderRadius: '6px',
      backgroundColor: 'var(--control-bg)',
      color: 'var(--fg)',
    },
    '.cm-tooltip .cm-tooltip-arrow:before': {
      borderTopColor: 'transparent',
      borderBottomColor: 'transparent',
    },
    '.cm-tooltip .cm-tooltip-arrow:after': {
      borderTopColor: 'var(--control-bg)',
      borderBottomColor: 'var(--control-bg)',
    },
    '.cm-tooltip-autocomplete': {
      '& > ul > li[aria-selected]': {
        backgroundColor: 'var(--control-active)',
        color: 'var(--fg)',
      },
    },
  },
  { dark: true },
)

const darkHighlightStyle = HighlightStyle.define([
  { tag: t.keyword, color: keyword },
  { tag: [t.name, t.deleted, t.character, t.propertyName, t.macroName], color: name },
  { tag: [t.function(t.variableName), t.labelName], color: callable },
  { tag: [t.color, t.constant(t.name), t.standard(t.name)], color: constant },
  { tag: [t.definition(t.name), t.separator], color: 'var(--fg)' },
  { tag: [t.typeName, t.className, t.number, t.changed, t.annotation, t.modifier, t.self, t.namespace], color: type },
  { tag: [t.operator, t.operatorKeyword, t.url, t.escape, t.regexp, t.special(t.string)], color: operator },
  { tag: [t.meta, t.comment], color: comment },
  { tag: t.strong, fontWeight: 'bold' },
  { tag: t.emphasis, fontStyle: 'italic' },
  { tag: t.strikethrough, textDecoration: 'line-through' },
  { tag: t.link, color: callable, textDecoration: 'underline' },
  { tag: t.heading, fontWeight: 'bold', color: callable },
  { tag: [t.atom, t.bool, t.special(t.variableName)], color: constant },
  { tag: [t.processingInstruction, t.string, t.inserted], color: string },
  { tag: t.invalid, color: invalid },
])

export const darkTheme: Extension = [darkEditorTheme, syntaxHighlighting(darkHighlightStyle)]
