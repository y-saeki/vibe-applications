// The app's icons, drawn on a 20-unit grid in the current text color.

const svg = (body: string) =>
  `<svg viewBox="0 0 20 20" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false">${body}</svg>`

export const ICONS = {
  done: svg('<path d="M4 10.5 8 14.5 16 5.5" />'),
  tag: svg('<path d="M3.5 3.5H10L16.5 10 10 16.5 3.5 10Z" /><circle cx="7" cy="7" r="1.2" />'),
  later: svg('<circle cx="10" cy="10" r="6.5" /><path d="M10 6.5V10L12.5 12" />'),
  star: svg('<path d="M10 3.2 12.1 7.6 16.8 8.1 13.3 11.3 14.2 16 10 13.6 5.8 16 6.7 11.3 3.2 8.1 7.9 7.6Z" />'),
  inbox: svg('<path d="M3 11.5 5 4.5H15L17 11.5V15.5H3Z" /><path d="M3 11.5H7L8 13.5H12L13 11.5H17" />'),
  view: svg('<path d="M3.5 5.5H16.5M5.5 10H14.5M8 14.5H12" />'),
  settings: svg('<circle cx="10" cy="10" r="2.5" /><path d="M10 2.5V4.5M10 15.5V17.5M2.5 10H4.5M15.5 10H17.5M4.7 4.7 6.1 6.1M13.9 13.9 15.3 15.3M4.7 15.3 6.1 13.9M13.9 6.1 15.3 4.7" />'),
  reload: svg('<path d="M15.5 9.5A5.5 5.5 0 1 1 13.8 5.6" /><path d="M14.5 2.8V6.2H11.1" />'),
  sidebar: svg('<rect x="2.5" y="4" width="15" height="12" rx="2.5" /><path d="M7.5 4V16" />'),
  attachment: svg('<path d="M14 9.5 9.2 14.3A3 3 0 0 1 5 10.1L10.4 4.7A2 2 0 0 1 13.3 7.5L8.3 12.5A1 1 0 0 1 6.9 11.1L11.2 6.8" />'),
}
