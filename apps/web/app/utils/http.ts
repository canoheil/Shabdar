/**
 * Small shared HTTP helpers for confirmed-only mutations.
 *
 * Kept in `utils/` (not in a chat composable) so a public route such as Detail
 * can reuse the status reader without pulling any private chat code into its
 * route chunk.
 */
export function statusOf(error: unknown): number | undefined {
  const candidate = error as {
    status?: unknown
    statusCode?: unknown
    response?: { status?: unknown }
  } | null
  for (const value of [candidate?.status, candidate?.statusCode, candidate?.response?.status]) {
    if (typeof value === 'number') return value
  }
  return undefined
}

export function fieldOf(error: unknown, field: string): string | undefined {
  const candidate = error as {
    data?: { error?: { fields?: Record<string, string> } }
  } | null
  return candidate?.data?.error?.fields?.[field]
}

export function retryAfterOf(error: unknown): number | null {
  const candidate = error as {
    response?: { headers?: { get?: (name: string) => string | null } }
  } | null
  const raw = candidate?.response?.headers?.get?.('retry-after')
  if (!raw) return null
  const parsed = Number.parseInt(raw, 10)
  return Number.isFinite(parsed) && parsed > 0 ? parsed : null
}
