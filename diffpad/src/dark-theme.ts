// The dark look of the editors themselves: the background, the caret and the
// selection, which CodeMirror paints on its own. Everything else it draws —
// the gutters, the changed lines and characters — is styled in style.css, where
// the same selectors serve both themes through the tokens.
//
// Nothing here is a literal: the values are the custom properties style.css
// defines for `:root[data-theme="dark"]`, so the palette has one home.

import type { Extension } from '@codemirror/state'
import { EditorView } from '@codemirror/view'

export const darkTheme: Extension = EditorView.theme(
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
  },
  { dark: true },
)
