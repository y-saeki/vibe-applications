// The toolbar's icons, drawn on a 20-unit grid in the current text color.

const svg = (body: string) =>
  `<svg viewBox="0 0 20 20" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false">${body}</svg>`

export const ICONS = {
  done: svg('<path d="M4 10.5 8 14.5 16 5.5" />'),
  tag: svg('<path d="M3.5 3.5H10L16.5 10 10 16.5 3.5 10Z" /><circle cx="7" cy="7" r="1.2" />'),
  later: svg('<circle cx="10" cy="10" r="6.5" /><path d="M10 6.5V10L12.5 12" />'),
  star: svg('<path d="M10 3.2 12.1 7.6 16.8 8.1 13.3 11.3 14.2 16 10 13.6 5.8 16 6.7 11.3 3.2 8.1 7.9 7.6Z" />'),
  attachment: svg('<path d="M14 9.5 9.2 14.3A3 3 0 0 1 5 10.1L10.4 4.7A2 2 0 0 1 13.3 7.5L8.3 12.5A1 1 0 0 1 6.9 11.1L11.2 6.8" />'),
}
