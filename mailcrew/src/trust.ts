// The row of checks under a card's subject: what Gmail found for SPF, DKIM and
// DMARC, and what Mail Crew found for BIMI and the mark certificate. Each check
// is a word with a dot; the dot's colour says how it went and the tooltip says
// the rest.

import type { Authentication, Brand, Check } from './backend'

type Tone = 'pass' | 'fail' | 'neutral' | 'pending'

interface Item {
  key: string
  label: string
  tone: Tone
  detail: string
}

const RESULT_TEXT: Record<string, string> = {
  pass: '合格',
  fail: '不合格',
  softfail: '不合格（ソフトフェイル）',
  neutral: '判定なし',
  none: '設定なし',
  temperror: '一時的なエラー',
  permerror: '設定の誤り',
  policy: 'ポリシーにより不合格',
}

function toneOf(result: string): Tone {
  if (result === 'pass') return 'pass'
  if (result === 'fail' || result === 'softfail' || result === 'permerror') return 'fail'
  return 'neutral'
}

function gmailItem(key: string, label: string, check: Check | null): Item {
  if (!check) return { key, label, tone: 'neutral', detail: `${label}: Gmail の検証結果がありません` }
  const text = RESULT_TEXT[check.result] ?? check.result
  const domain = check.domain ? `（${check.domain}）` : ''
  return { key, label, tone: toneOf(check.result), detail: `${label}: ${text}${domain}` }
}

const BIMI_TEXT: Record<Brand['bimi'], [Tone, string]> = {
  pass: ['pass', '登録あり'],
  none: ['neutral', '登録なし'],
  declined: ['neutral', '送信元が使わないと宣言しています'],
  fail: ['fail', 'レコードに誤りがあります'],
  skipped: ['neutral', 'DMARC の条件を満たさないため確認していません'],
  temperror: ['neutral', 'DNS に問い合わせられませんでした'],
}

const MARK_TEXT: Record<Brand['mark'], [Tone, string]> = {
  vmc: ['pass', '認証局が登録商標を確認したロゴです'],
  cmc: ['pass', '認証局が使用実績を確認したロゴです'],
  none: ['neutral', '証明書はありません'],
  fail: ['fail', '証明書を確認できませんでした'],
  temperror: ['neutral', '証明書を取得できませんでした'],
}

/**
 * The BIMI and certificate items: `undefined` while they are being looked up,
 * `null` when the lookup failed.
 */
function brandItems(brand: Brand | null | undefined): Item[] {
  if (brand === undefined) {
    return [
      { key: 'bimi', label: 'BIMI', tone: 'pending', detail: 'BIMI: 確認しています' },
      { key: 'mark', label: 'VMC', tone: 'pending', detail: 'VMC: 確認しています' },
    ]
  }
  if (brand === null) {
    return [
      { key: 'bimi', label: 'BIMI', tone: 'neutral', detail: 'BIMI: 確認できませんでした' },
      { key: 'mark', label: 'VMC', tone: 'neutral', detail: 'VMC: 確認できませんでした' },
    ]
  }
  const [bimiTone, bimiText] = BIMI_TEXT[brand.bimi]
  const domain = brand.domain ? `（${brand.domain}）` : ''
  const markLabel = brand.mark === 'cmc' ? 'CMC' : 'VMC'
  const [markTone, markText] = MARK_TEXT[brand.mark]
  return [
    { key: 'bimi', label: 'BIMI', tone: bimiTone, detail: `BIMI: ${bimiText}${domain}` },
    { key: 'mark', label: markLabel, tone: markTone, detail: `${markLabel}: ${markText}` },
  ]
}

/**
 * Lays out the checks in `row`. Without `lookUpBrand` (the user turned the
 * BIMI lookups off) only Gmail's checks show; see `brandItems` for `brand`.
 */
export function renderTrust(
  row: HTMLElement,
  authentication: Authentication,
  brand: Brand | null | undefined,
  lookUpBrand: boolean,
): void {
  const items = [
    gmailItem('spf', 'SPF', authentication.spf),
    gmailItem('dkim', 'DKIM', authentication.dkim),
    gmailItem('dmarc', 'DMARC', authentication.dmarc),
    ...(lookUpBrand ? brandItems(brand) : []),
  ]
  row.replaceChildren(
    ...items.map((item) => {
      const element = document.createElement('span')
      element.className = `trust trust-${item.tone}`
      element.dataset.check = item.key
      element.dataset.tone = item.tone
      element.title = item.detail
      element.setAttribute('aria-label', item.detail)
      element.textContent = item.label
      return element
    }),
  )
  row.hidden = false
}
