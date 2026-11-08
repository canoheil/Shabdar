const MAX_NEXT_LENGTH = 512

/**
 * Accepts only an internal, single-slash path.
 *
 * Rejected: any scheme or authority (`https://…`, `//host`), backslashes
 * (`/\host`), whitespace or control characters, credential-bearing URLs, and
 * encoded variants that decode into any of the above (`/%2f%2fhost`,
 * `/%5c%5chost`, `%0a`). Everything else falls back to the caller's default.
 */
export function safeInternalNext(value: unknown, fallback = '/'): string {
  if (typeof value !== 'string') return fallback
  const raw = value.trim()
  if (raw.length === 0 || raw.length > MAX_NEXT_LENGTH) return fallback
  if (!raw.startsWith('/') || raw.startsWith('//')) return fallback
  if (raw.includes('\\')) return fallback
  if (hasUnsafeChars(raw)) return fallback

  let decoded = raw
  try {
    decoded = decodeURIComponent(raw)
  } catch {
    return fallback
  }
  if (decoded !== raw) {
    if (!decoded.startsWith('/') || decoded.startsWith('//')) return fallback
    if (decoded.includes('\\') || hasUnsafeChars(decoded)) return fallback
  }

  if (typeof window !== 'undefined') {
    try {
      const resolved = new URL(raw, window.location.origin)
      if (resolved.origin !== window.location.origin) return fallback
      return `${resolved.pathname}${resolved.search}${resolved.hash}`
    } catch {
      return fallback
    }
  }
  return raw
}

function hasUnsafeChars(value: string): boolean {
  // Whitespace, C0 controls, DEL: browsers and proxies strip or reinterpret
  // these, which is how a "safe" path turns into a protocol-relative one.
  return /[\u0000-\u001f\u007f\s]/.test(value)
}
