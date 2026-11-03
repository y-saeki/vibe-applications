// How a card writes its date: the time for today, the day for this year, the
// full date for anything older.

export function formatDate(epochMs: number, now: Date = new Date()): string {
  const date = new Date(epochMs)
  const pad = (value: number) => String(value).padStart(2, '0')
  if (date.toDateString() === now.toDateString()) return `${date.getHours()}:${pad(date.getMinutes())}`
  if (date.getFullYear() === now.getFullYear()) return `${date.getMonth() + 1}月${date.getDate()}日`
  return `${date.getFullYear()}/${date.getMonth() + 1}/${date.getDate()}`
}
