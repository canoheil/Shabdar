/**
 * Chat-only formatters. Kept out of `format.ts` so public route chunks never
 * pay for a counter formatter they never render.
 */

// Counters such as «۴۰۰ از ۱۰۰۰» read as one value, so they never group.
const countFormatter = new Intl.NumberFormat('fa-IR', { useGrouping: false })

export function formatCount(value: number): string {
  return countFormatter.format(value)
}
