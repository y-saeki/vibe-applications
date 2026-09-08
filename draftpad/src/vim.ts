// Thin wrapper around @replit/codemirror-vim so the dependency touches one
// file. Loaded on demand: users who never enable Vim mode never download it.

import type { Extension } from '@codemirror/state'

export async function vimExtension(): Promise<Extension> {
  const { vim } = await import('@replit/codemirror-vim')
  return vim({ status: true })
}
