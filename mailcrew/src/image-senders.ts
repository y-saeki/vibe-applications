// The senders whose messages show images from the network without asking.
// The inbox adds to them (この差出人は常に表示) and the settings take them away;
// both go through this one list, which the Rust side keeps on disk.

import { backend } from './backend'

export class ImageSenders {
  private senders: string[] = []

  /** Every sender, in the order they were allowed. */
  list(): string[] {
    return [...this.senders]
  }

  has(email: string): boolean {
    return this.senders.includes(email.trim().toLowerCase())
  }

  async load(): Promise<void> {
    this.senders = await backend.imageSenders()
  }

  async allow(email: string): Promise<void> {
    if (this.has(email)) return
    await this.save([...this.senders, email])
  }

  async remove(email: string): Promise<void> {
    await this.save(this.senders.filter((known) => known !== email))
  }

  private async save(senders: string[]): Promise<void> {
    this.senders = await backend.saveImageSenders(senders)
  }
}
