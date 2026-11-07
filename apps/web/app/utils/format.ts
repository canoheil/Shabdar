export function formatNumber(value: number): string {
  return value.toLocaleString('fa-IR')
}

const commentDateFormatter = new Intl.DateTimeFormat('fa-IR', {
  dateStyle: 'medium',
  timeZone: 'UTC',
})

// A stable UTC calendar date avoids server/client hydration timezone drift.
export function formatCommentDate(value: string): string {
  return commentDateFormatter.format(new Date(value))
}

const listingDateFormatter = new Intl.DateTimeFormat('fa-IR', {
  dateStyle: 'medium',
  timeZone: 'UTC',
})

export function formatListingDate(value: string): string {
  return listingDateFormatter.format(new Date(value))
}

// Price display policy, frontend only (ARCHITECTURE.md §8 — stored values
// stay numeric). fa-IR digits with تومان, or the price_type label.
export function formatPrice(
  priceToman: number | null,
  priceType: string,
): string {
  if (priceType === 'free') return 'رایگان'
  if (priceType === 'negotiable' && priceToman === null) return 'توافقی'
  if (typeof priceToman === 'number') {
    return `${priceToman.toLocaleString('fa-IR')} تومان`
  }
  return 'توافقی'
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
