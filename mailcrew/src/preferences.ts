// The user's preferences, which the Rust side keeps on disk. The inbox reads
// them and the settings change them; both go through this one copy.

import { backend, type Preferences } from './backend'

export class PreferenceStore {
  private current: Preferences = { brandLogos: true }

  get brandLogos(): boolean {
    return this.current.brandLogos
  }

  async load(): Promise<void> {
    this.current = await backend.preferences()
  }

  async setBrandLogos(brandLogos: boolean): Promise<void> {
    const next = { ...this.current, brandLogos }
    await backend.savePreferences(next)
    this.current = next
  }
}
