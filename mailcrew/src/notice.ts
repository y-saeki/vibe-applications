// A short message at the foot of the window, for what failed.

const NOTICE_MS = 6000

let timer: number | undefined

export function showNotice(message: string): void {
  const notice = document.querySelector<HTMLElement>('#notice')!
  notice.textContent = message
  notice.hidden = false
  window.clearTimeout(timer)
  timer = window.setTimeout(() => {
    notice.hidden = true
  }, NOTICE_MS)
}
