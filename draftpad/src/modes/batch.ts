// Minimal Windows batch (.bat / .cmd) highlighter. CodeMirror has no built-in
// batch grammar; this covers comments, strings, variables, labels and the
// common commands, which is enough for pasting a script into a chat draft.

import type { StreamParser, StringStream } from '@codemirror/language'

const COMMANDS = new Set([
  'assoc', 'attrib', 'break', 'call', 'cd', 'chdir', 'choice', 'cls', 'cmd', 'color', 'copy',
  'date', 'defined', 'del', 'dir', 'do', 'echo', 'else', 'endlocal', 'equ', 'erase', 'errorlevel',
  'exist', 'exit', 'find', 'findstr', 'for', 'geq', 'goto', 'gtr', 'if', 'in', 'leq', 'lss', 'md',
  'mkdir', 'more', 'move', 'neq', 'net', 'not', 'path', 'pause', 'ping', 'popd', 'powershell',
  'prompt', 'pushd', 'rd', 'reg', 'ren', 'rename', 'rmdir', 'robocopy', 'sc', 'set', 'setlocal',
  'shift', 'sort', 'start', 'taskkill', 'tasklist', 'time', 'timeout', 'title', 'type', 'ver',
  'vol', 'where', 'wmic', 'xcopy',
])

function token(stream: StringStream): string | null {
  if (stream.sol() && stream.match(/^\s*(?:@?rem\b|::).*$/i)) return 'comment'
  if (stream.eatSpace()) return null
  if (stream.match(/^"(?:[^"]|"")*"?/)) return 'string'
  if (stream.match(/^%%~?\w+/) || stream.match(/^%~?[\w*]+(?::[^%]*)?%/) || stream.match(/^%\d/)) {
    return 'variableName'
  }
  if (stream.match(/^![^!\s]+!/)) return 'variableName'
  if (stream.match(/^:[\w.-]+/)) return 'labelName'
  if (stream.match(/^\d+\b/)) return 'number'
  if (stream.match(/^\/[a-z?][\w-]*/i)) return 'atom'
  if (stream.match(/^(?:>>|[<>|&()@^=])/)) return 'operator'
  if (stream.match(/^[A-Za-z_][\w-]*/)) {
    return COMMANDS.has(stream.current().toLowerCase()) ? 'keyword' : null
  }
  stream.next()
  return null
}

export const batch: StreamParser<unknown> = {
  name: 'batch',
  token,
  languageData: {
    commentTokens: { line: 'REM ' },
  },
}
