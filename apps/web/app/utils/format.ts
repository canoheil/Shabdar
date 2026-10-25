export function formatNumber(value: number): string {
  return value.toLocaleString('fa-IR')
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
