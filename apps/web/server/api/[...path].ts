import { joinURL } from 'ufo'

// Same-origin /api proxy to Axum (ARCHITECTURE.md §3, plan Task 6): every
// request to localhost:3000/api/** goes to apiBase + the original path. The
// session cookie rides along unchanged, the client IP is forwarded, and the
// response — including every Set-Cookie header — comes back as-is. Browser
// code never talks to :8080. ufo ships with nuxt (transitive — no new
// dependency).
export default defineEventHandler(async (event) => {
  const config = useRuntimeConfig(event)
  // The two mandated forwards, done explicitly so they are reviewable.
  // X-Forwarded-For is spoofable by a direct client; accepted for this
  // localhost-only phase (Review Focus 5).
  const cookie = getHeader(event, 'cookie')
  if (cookie) setHeader(event, 'cookie', cookie)
  const ip = getRequestIP(event, { xForwarded: true })
  if (ip) setHeader(event, 'x-forwarded-for', ip)
  return proxyRequest(event, joinURL(config.apiBase, event.path))
})
