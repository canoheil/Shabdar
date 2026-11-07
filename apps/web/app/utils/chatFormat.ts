/**
 * Chat-only formatters. Kept out of `format.ts` so public route chunks never
 * pay for a counter formatter they never render.
 */

// Counters such as «۴۰۰ از ۱۰۰۰» read as one value, so they never group.
const countFormatter = new Intl.NumberFormat('fa-IR', { useGrouping: false })

export function formatCount(value: number): string {
  return countFormatter.format(value)
}

// Chat timestamps render only after authentication on the client; the chat
// routes never put message data in SSR HTML. Using the viewer's own timezone is
// therefore the honest label (a Tehran reader must see their wall clock), and
// no SSR/hydration pair exists to disagree.
const chatTimeFormatter = new Intl.DateTimeFormat('fa-IR', {
  hour: '2-digit',
  minute: '2-digit',
})

const chatDayFormatter = new Intl.DateTimeFormat('fa-IR', {
  weekday: 'long',
  day: 'numeric',
  month: 'long',
})

export function formatChatTime(value: string): string {
  return chatTimeFormatter.format(new Date(value))
}

export function formatChatDay(value: string): string {
  return chatDayFormatter.format(new Date(value))
}

/// Stable day bucket for separators, in the same local calendar as the labels.
export function chatDayKey(value: string): string {
  const date = new Date(value)
  return `${date.getFullYear()}-${date.getMonth() + 1}-${date.getDate()}`
}
