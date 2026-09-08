// Default editor font per platform: a Japanese-capable monospaced face first,
// then fallbacks that still pair a monospaced Latin font with a Japanese one.

export function defaultFontFamily(platform: string): string {
  switch (platform) {
    case 'macos':
      // Osaka−等幅 ships with macOS (Supplemental fonts). Menlo + Hiragino otherwise.
      return '"Osaka-Mono", "Osaka−等幅", Menlo, "Hiragino Sans", monospace'
    case 'windows':
      // BIZ UDGothic is the monospaced UD font bundled with Windows 10 1809+ (Japanese fonts).
      return '"BIZ UDGothic", "BIZ UDゴシック", "MS Gothic", "ＭＳ ゴシック", Consolas, "Yu Gothic", monospace'
    default:
      return 'monospace'
  }
}
