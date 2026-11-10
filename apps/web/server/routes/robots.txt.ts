// Public robots policy (Phase 14). Account, admin and login surfaces are
// private by design and were already `noindex` at the page level; this file
// states the same rule to crawlers before they fetch anything.
export default defineEventHandler((event) => {
  const config = useRuntimeConfig(event)
  const origin = String(config.public.siteUrl ?? '').replace(/\/+$/, '')
  const lines = [
    'User-agent: *',
    'Allow: /',
    'Disallow: /me',
    'Disallow: /me/',
    'Disallow: /admin',
    'Disallow: /admin/',
    'Disallow: /login',
    '',
    `Sitemap: ${origin}/sitemap.xml`,
    '',
  ]
  setResponseHeader(event, 'content-type', 'text/plain; charset=utf-8')
  setResponseHeader(event, 'cache-control', 'public, max-age=3600')
  return lines.join('\n')
})
