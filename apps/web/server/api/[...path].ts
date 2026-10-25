import { joinURL } from 'ufo'

// Same-origin /api proxy to Axum (ARCHITECTURE.md §3, plan Task 6): every
// request to localhost:3000/api/** goes to apiBase + the original path. The
// session cookie rides along unchanged, and the response — including every
// Set-Cookie header — comes back as-is. Browser code never talks to :8080.
// ufo ships with nuxt (transitive — no new dependency).
//
// Header overrides MUST go through proxyRequest's `headers` option. h3's
// setHeader(event, …) writes RESPONSE headers — an earlier revision set the
// cookie/CSRF values that way, which echoed them back to the browser while
// the upstream API never saw X-CSRF-Token (every proxied mutation 403'd).
// Regression cover: curl a mutation through :3000 with cookies but WITHOUT
// X-CSRF-Token → 2xx (injected); the values must not appear as response
// headers.
export default defineEventHandler(async (event) => {
  const config = useRuntimeConfig(event)
  const headers: Record<string, string> = {}
  // Client IP forward (ARCHITECTURE.md §3). X-Forwarded-For is spoofable by
  // a direct client; accepted for this localhost-only phase (Review Focus 5).
  const ip = getRequestIP(event, { xForwardedFor: true })
  if (ip) headers['x-forwarded-for'] = ip
  // CSRF header injection (plan Task 9): on mutations, copy the shabdar_csrf
  // cookie value into X-CSRF-Token when the client did not send one — browser
  // code never handles the token.
  const method = event.method.toUpperCase()
  const isMutation =
    method === 'POST' || method === 'PATCH' || method === 'PUT' || method === 'DELETE'
  if (isMutation && !getRequestHeader(event, 'x-csrf-token')) {
    const cookie = getRequestHeader(event, 'cookie') ?? ''
    const csrf = cookie
      .split(';')
      .map((pair) => pair.trim())
      .find((pair) => pair.startsWith('shabdar_csrf='))
      ?.slice('shabdar_csrf='.length)
    if (csrf) headers['x-csrf-token'] = csrf
  }
  return proxyRequest(event, joinURL(config.apiBase, event.path), { headers })
})
