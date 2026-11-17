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

const tehranDayFormatter = new Intl.DateTimeFormat('en-US', {
  timeZone: 'Asia/Tehran',
  year: 'numeric',
  month: 'numeric',
  day: 'numeric',
})

function tehranDayNumber(timestamp: number): number {
  const parts = tehranDayFormatter.formatToParts(new Date(timestamp))
  const part = (type: string) => Number(parts.find((item) => item.type === type)?.value)
  return Date.UTC(part('year'), part('month') - 1, part('day')) / 86_400_000
}

// Relative day label pinned to the Asia/Tehran calendar, never the device
// timezone, so the server and the browser render the same word.
export function formatListingDay(value: string, now: number): string {
  const timestamp = Date.parse(value)
  if (Number.isNaN(timestamp)) return ''
  const diff = tehranDayNumber(now) - tehranDayNumber(timestamp)
  if (diff <= 0) return 'امروز'
  if (diff === 1) return 'دیروز'
  if (diff <= 6) return `${diff.toLocaleString('fa-IR')} روز پیش`
  return formatListingDate(value)
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

