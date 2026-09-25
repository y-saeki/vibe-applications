// The preferences panel's shortcut tab: every shortcut draftpad has, the ones
// that can be changed with their keys as controls, and the rest listed under
// headings marked 変更不可.
//
// A key is changed by clicking it and pressing the new one. Nothing is kept
// in here between renders but what is on its way: the key being waited for, a
// clash waiting on an answer, the last refusal. The keys themselves are the
// store's (`shortcuts` in state.json, as src/shortcuts.ts reads it), and every
// change goes straight into it, the way the panel's other controls work.

import { formatCombo, isAssignable, recordFromEvent } from './keys'
import {
  fixedOn,
  fixedOwner,
  GROUPS,
  keysOn,
  overridesFor,
  ownerOf,
  resolveBindings,
  type Bindings,
  type Shortcut,
  shortcutsOn,
} from './shortcuts'
import type { Store } from './state'

/** Where a new key goes: over one of a shortcut's keys, or after the last of them. */
interface Slot {
  id: string
  index: number
}

/** A key that is already another shortcut's, waiting on 置き換える or キャンセル. */
interface Clash {
  id: string
  /** Set when ↺ asked for keys another shortcut has since been given; `slot` and `combo` are unused then. */
  reset: boolean
  slot: Slot
  combo: string
  /** The keys at stake and who has each of them now. */
  owners: { combo: string; id: string }[]
}

/** How long 「すべて初期設定に戻す」 waits for its second press. */
const RESET_ALL_ARMED_MS = 3000

export interface ShortcutListOptions {
  platform: string
  /** Told when the list starts and stops waiting for a key, so that the menu bar can let go of its own. */
  onRecordingChange: (recording: boolean) => void
}

function element<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props: Partial<HTMLElementTagNameMap[K]> & { dataset?: Record<string, string> } = {},
  ...children: (Node | string | null)[]
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag)
  const { dataset, ...rest } = props
  Object.assign(node, rest)
  if (dataset) Object.assign(node.dataset, dataset)
  for (const child of children) if (child !== null) node.append(child)
  return node
}

export class ShortcutList {
  private readonly shortcuts: Shortcut[]
  private readonly byId: Map<string, Shortcut>
  private recording: Slot | null = null
  private clash: Clash | null = null
  private refusal: { id: string; text: string } | null = null
  private resetArmed: number | undefined
  /** Set while the list is redrawn, which takes the focused control out from under the keyboard. */
  private rendering = false

  constructor(
    private readonly list: HTMLElement,
    private readonly filter: HTMLInputElement,
    private readonly count: HTMLElement,
    private readonly resetAll: HTMLButtonElement,
    private readonly store: Store,
    private readonly options: ShortcutListOptions,
  ) {
    this.shortcuts = shortcutsOn(options.platform)
    this.byId = new Map(this.shortcuts.map((shortcut) => [shortcut.id, shortcut]))
    filter.addEventListener('input', () => this.render())
    resetAll.addEventListener('click', () => this.pressResetAll())
    // Clicking anywhere else, or tabbing away, gives up on the key.
    list.addEventListener('focusout', (event) => {
      if (this.rendering || !this.recording) return
      if (!list.contains(event.relatedTarget as Node | null)) this.stopRecording()
    })
  }

  get isRecording(): boolean {
    return this.recording !== null
  }

  private get platform(): string {
    return this.options.platform
  }

  private get bindings(): Map<string, string[]> {
    return resolveBindings(this.store.state.shortcuts, this.platform)
  }

  private write(bindings: Bindings): void {
    this.store.set({ shortcuts: overridesFor(bindings, this.platform) })
  }

  private title(id: string): string {
    return this.byId.get(id)?.title ?? id
  }

  private defaults(id: string): string[] {
    const shortcut = this.byId.get(id)
    return shortcut ? [...keysOn(shortcut.keys, this.platform)!] : []
  }

  /** Drops whatever was on its way, for when the panel closes. */
  reset(): void {
    this.stopRecording(false)
    this.clash = null
    this.refusal = null
    this.disarm()
  }

  /**
   * Takes a key press while a key is being waited for. Returns false for one
   * the list lets through: Tab, which moves on and gives up on the key.
   */
  handleKey(event: KeyboardEvent): boolean {
    if (!this.recording) return false
    const plain = !event.ctrlKey && !event.altKey && !event.metaKey
    if (plain && event.key === 'Tab') {
      this.stopRecording()
      return false
    }
    event.preventDefault()
    if (plain && !event.shiftKey && event.key === 'Escape') {
      this.stopRecording()
      return true
    }
    const combo = recordFromEvent(event, this.platform)
    if (combo) this.take(combo)
    return true
  }

  private startRecording(slot: Slot): void {
    const was = this.recording !== null
    this.recording = slot
    this.clash = null
    this.refusal = null
    // Onto the key that waits, by hand: WebKit does not focus a button that is
    // clicked, and it is losing the focus that gives up on the key.
    this.render(this.focusKeyOf(slot))
    if (!was) this.options.onRecordingChange(true)
  }

  private stopRecording(render = true): void {
    if (!this.recording) return
    const slot = this.recording
    this.recording = null
    this.options.onRecordingChange(false)
    if (render) this.render(this.focusKeyOf(slot))
  }

  /** What was pressed while `recording`: refused, held for an answer, or taken. */
  private take(combo: string): void {
    const slot = this.recording!
    this.recording = null
    this.options.onRecordingChange(false)
    const { id } = slot
    if (!isAssignable(combo)) {
      this.refusal = {
        id,
        text:
          this.platform === 'macos'
            ? '⌘・⌃・⌥ のいずれかを含む組み合わせか、ファンクションキーを押してください。'
            : 'Ctrl か Alt を含む組み合わせか、ファンクションキーを押してください。',
      }
      this.render(this.focusKeyOf(slot))
      return
    }
    const fixed = fixedOwner(combo, this.platform)
    if (fixed) {
      this.refusal = { id, text: `${formatCombo(combo, this.platform)} は「${fixed}」に使われているため、割り当てられません。` }
      this.render(this.focusKeyOf(slot))
      return
    }
    const bindings = this.bindings
    const owner = ownerOf(bindings, combo)
    if (owner && owner !== id) {
      this.clash = { id, reset: false, slot, combo, owners: [{ combo, id: owner }] }
      this.render(`clash-confirm:${id}`)
      return
    }
    this.put(bindings, slot, combo)
    this.write(bindings)
    this.render(this.focusKeyOf(slot))
  }

  /** Puts `combo` where `slot` says, and takes out a copy the shortcut already had elsewhere. */
  private put(bindings: Map<string, string[]>, slot: Slot, combo: string): void {
    const keys = [...(bindings.get(slot.id) ?? [])]
    if (slot.index < keys.length) keys[slot.index] = combo
    else keys.push(combo)
    bindings.set(
      slot.id,
      keys.filter((key, index) => keys.indexOf(key) === index),
    )
  }

  private confirmClash(): void {
    const clash = this.clash
    if (!clash) return
    this.clash = null
    const bindings = this.bindings
    for (const { combo, id } of clash.owners) {
      bindings.set(
        id,
        (bindings.get(id) ?? []).filter((key) => key !== combo),
      )
    }
    if (clash.reset) bindings.set(clash.id, this.defaults(clash.id))
    else this.put(bindings, clash.slot, clash.combo)
    this.write(bindings)
    this.render(clash.reset ? `name:${clash.id}` : this.focusKeyOf(clash.slot))
  }

  private cancelClash(): void {
    const clash = this.clash
    if (!clash) return
    this.clash = null
    this.render(clash.reset ? `reset:${clash.id}` : this.focusKeyOf(clash.slot))
  }

  private remove(id: string, index: number): void {
    this.stopRecording(false)
    const bindings = this.bindings
    bindings.set(
      id,
      (bindings.get(id) ?? []).filter((_, at) => at !== index),
    )
    this.clash = null
    this.refusal = null
    this.write(bindings)
    // The key that took its place, or the empty slot once there is none.
    this.render(`key:${id}:${Math.min(index, Math.max(0, (bindings.get(id)?.length ?? 1) - 1))}`)
  }

  /**
   * Puts one shortcut back on its default keys. One of them may since have
   * been given to another shortcut; taking it back from there waits on the
   * same question a clash does.
   */
  private resetOne(id: string): void {
    this.stopRecording(false)
    this.refusal = null
    const bindings = this.bindings
    const owners = this.defaults(id)
      .map((combo) => ({ combo, id: ownerOf(bindings, combo) }))
      .filter((owner): owner is { combo: string; id: string } => owner.id !== null && owner.id !== id)
    if (owners.length > 0) {
      this.clash = { id, reset: true, slot: { id, index: 0 }, combo: '', owners }
      this.render(`clash-confirm:${id}`)
      return
    }
    this.clash = null
    bindings.set(id, this.defaults(id))
    this.write(bindings)
    this.render(`name:${id}`)
  }

  private pressResetAll(): void {
    if (this.resetArmed === undefined) {
      this.resetAll.textContent = 'もう一度押すと戻します'
      this.resetAll.dataset.armed = ''
      this.resetArmed = window.setTimeout(() => this.disarm(), RESET_ALL_ARMED_MS)
      return
    }
    this.disarm()
    this.stopRecording(false)
    this.clash = null
    this.refusal = null
    this.store.set({ shortcuts: {} })
    this.render()
  }

  private disarm(): void {
    if (this.resetArmed !== undefined) window.clearTimeout(this.resetArmed)
    this.resetArmed = undefined
    this.resetAll.textContent = 'すべて初期設定に戻す'
    delete this.resetAll.dataset.armed
  }

  private focusKeyOf(slot: Slot): string {
    return `key:${slot.id}:${slot.index}`
  }

  /**
   * Draws the list from the store.
   *
   * @param focus which control the keyboard goes to afterwards, by its
   *   `data-focus`; left out, whichever had it keeps it if it is still there
   */
  render(focus?: string): void {
    const active = document.activeElement as HTMLElement | null
    const keep = focus ?? (active && this.list.contains(active) ? active.dataset.focus : undefined)
    const bindings = this.bindings
    const query = this.filter.value.trim().toLowerCase()
    const matches = (title: string, keys: readonly string[]): boolean =>
      !query || [title, ...keys.map((key) => formatCombo(key, this.platform))].join(' ').toLowerCase().includes(query)

    const groups: HTMLElement[] = []
    for (const group of GROUPS) {
      const rows = this.shortcuts
        .filter((shortcut) => shortcut.group === group && matches(shortcut.title, bindings.get(shortcut.id) ?? []))
        .map((shortcut) => this.row(shortcut, bindings.get(shortcut.id) ?? []))
      if (rows.length) groups.push(element('fieldset', { className: 'pref-group' }, element('legend', { textContent: group }), ...rows))
    }
    for (const group of fixedOn(this.platform)) {
      const rows = group.items
        .filter((item) => matches(item.title, keysOn(item.keys, this.platform)!))
        .map((item) =>
          element(
            'div',
            { className: 'shortcut-row fixed' },
            element('span', { className: 'shortcut-name', textContent: item.title }),
            element(
              'span',
              { className: 'shortcut-keys' },
              ...keysOn(item.keys, this.platform)!.map((key) => element('kbd', { textContent: formatCombo(key, this.platform) })),
            ),
          ),
        )
      if (!rows.length) continue
      groups.push(
        element(
          'fieldset',
          { className: 'pref-group' },
          element('legend', {}, group.title, element('span', { className: 'fixed-tag', textContent: '変更不可' })),
          group.note ? element('p', { className: 'group-note', textContent: group.note }) : null,
          ...rows,
        ),
      )
    }
    this.rendering = true
    this.list.replaceChildren(...(groups.length ? groups : [element('p', { className: 'shortcut-empty', textContent: '一致するショートカットはありません。' })]))

    const changed = Object.keys(overridesFor(bindings, this.platform)).length
    this.count.textContent = changed ? `${changed} 件を変更済み` : ''
    this.resetAll.disabled = changed === 0
    if (changed === 0) this.disarm()

    const target = keep ? this.list.querySelector<HTMLElement>(`[data-focus="${CSS.escape(keep)}"]`) : null
    target?.focus()
    this.rendering = false
  }

  private row(shortcut: Shortcut, keys: readonly string[]): HTMLElement {
    const { id, title } = shortcut
    const modified = overridesFor(new Map([[id, [...keys]]]), this.platform)[id] !== undefined
    const chips = keys.map((key, index) => this.chip(id, index, key))
    const recordingHere = this.recording?.id === id
    if (keys.length === 0 || (recordingHere && this.recording!.index === keys.length)) chips.push(this.chip(id, keys.length, null))

    const row = element(
      'div',
      { className: 'shortcut-row', dataset: { id } },
      element(
        'span',
        { className: 'shortcut-name', dataset: { focus: `name:${id}` }, tabIndex: -1 },
        title,
        modified ? element('span', { className: 'modified-mark', textContent: '変更済み' }) : null,
      ),
      element('span', { className: 'shortcut-keys' }, ...chips),
      modified
        ? this.iconButton('reset-button', '初期設定に戻す', `${title} を初期設定に戻す`, `reset:${id}`, () => this.resetOne(id))
        : element('span'),
    )
    if (this.clash?.id === id) row.append(this.clashMessage(this.clash))
    if (this.refusal?.id === id) row.append(element('p', { className: 'shortcut-message refusal', role: 'alert', textContent: this.refusal.text }))
    return row
  }

  /** One key, as a control that waits for a new one when clicked; `combo` null for the empty slot. */
  private chip(id: string, index: number, combo: string | null): HTMLElement {
    const recording = this.recording?.id === id && this.recording.index === index
    const label = recording ? 'キーを入力…' : combo ? formatCombo(combo, this.platform) : '未設定'
    const key = element('button', {
      type: 'button',
      className: 'key-button',
      textContent: label,
      title: recording ? 'Esc で取り消し' : combo ? 'クリックしてキーを変更' : 'クリックしてキーを登録',
      dataset: { focus: `key:${id}:${index}` },
    })
    key.addEventListener('click', () => this.startRecording({ id, index }))
    const chip = element('span', { className: 'key-chip' }, key)
    if (recording) chip.dataset.recording = ''
    else if (!combo) chip.dataset.empty = ''
    if (combo && !recording) {
      chip.append(
        this.iconButton('key-remove', 'このキーを外す', `${label} を外す`, `remove:${id}:${index}`, () => this.remove(id, index)),
      )
    }
    return chip
  }

  private iconButton(className: string, title: string, label: string, focus: string, onClick: () => void): HTMLButtonElement {
    const button = element('button', { type: 'button', className: `icon-button ${className}`, title, dataset: { focus } })
    button.setAttribute('aria-label', label)
    button.addEventListener('click', onClick)
    return button
  }

  private clashMessage(clash: Clash): HTMLElement {
    const verb = clash.reset ? '初期設定に戻す' : '置き換える'
    const bindings = this.bindings
    // What the other shortcut is left with: nothing, or the keys it keeps.
    const text = clash.owners
      .map(({ combo, id }) => {
        const shown = formatCombo(combo, this.platform)
        const left = (bindings.get(id) ?? []).filter((key) => key !== combo)
        const outcome = left.length ? `「${this.title(id)}」からは ${shown} が外れます。` : `「${this.title(id)}」は未設定になります。`
        return `${shown} は「${this.title(id)}」に割り当て済みです。${verb}と、${outcome}`
      })
      .join('')
    const cancel = element('button', { type: 'button', className: 'text-button', textContent: 'キャンセル', dataset: { focus: `clash-cancel:${clash.id}` } })
    cancel.addEventListener('click', () => this.cancelClash())
    const confirm = element('button', {
      type: 'button',
      className: 'text-button primary',
      textContent: verb,
      dataset: { focus: `clash-confirm:${clash.id}` },
    })
    confirm.addEventListener('click', () => this.confirmClash())
    return element(
      'div',
      { className: 'shortcut-message clash', role: 'alert' },
      element('span', { textContent: text }),
      element('span', { className: 'shortcut-message-actions' }, cancel, confirm),
    )
  }
}
