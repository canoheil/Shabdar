/**
 * Tiny stand-in for the Rust API, used only for local SSR smoke checks when
 * MariaDB and the real service are not available (for example, in a sandbox).
 *
 * It is NOT a test fixture and never runs in CI: the real-stack Playwright
 * suite talks to Axum. It exists so `curl` against a production build can
 * confirm routing, status codes, canonicals and the sitemap shape.
 *
 *   node scripts/smoke-stub-api.mjs            # listens on 8098
 *   NUXT_API_BASE=http://127.0.0.1:8098 PORT=3105 node .output/server/index.mjs
 */
import { createServer } from 'node:http'

const PORT = Number(process.env.SMOKE_STUB_PORT ?? 8098)

const CATEGORIES = [
  { slug: 'home-sweets', name_fa: 'شیرینی خانگی' },
  { slug: 'home-food', name_fa: 'غذای خانگی' },
  { slug: 'handicrafts', name_fa: 'صنایع دستی' },
]

const PROVINCES = [
  { id: 1, slug: 'gilan', name_fa: 'گیلان' },
  { id: 2, slug: 'tehran', name_fa: 'تهران' },
]

const CITIES = {
  gilan: [{ id: 1, slug: 'rasht', name_fa: 'رشت' }],
  tehran: [{ id: 2, slug: 'tehran', name_fa: 'تهران' }],
}

const CARD = {
  public_id: '01smoke0000000000000000001',
  slug: 'دوچرخه-کوهستان',
  title: 'دوچرخه کوهستان',
  price_toman: 4_500_000,
  price_type: 'fixed',
  city: 'رشت',
  city_slug: 'rasht',
  province: 'گیلان',
  category: 'صنایع دستی',
  cover_image: null,
  cover_srcset: null,
  cover_webp_srcset: null,
  rating_average: 4.5,
  rating_count: 2,
  published_at: '2026-11-01T10:00:00Z',
}

const LISTING = {
  ...CARD,
  description: 'دوچرخه کوهستان در حد نو، مناسب مسیرهای جنگلی.',
  // One real entry, not `[]`: the Detail page reads `images[0]` for its
  // canonical/OG image, so an empty gallery used to make the smoke page 500 and
  // the document useless as evidence (see the Phase 14 plan's follow-up).
  images: [{
    url: '/media/01smoke0000000000000000001/01smoke0000000000000000003.png',
    alt_text: 'دوچرخه کوهستان — تصویر 1',
    width: 1200,
    height: 900,
    srcset: '/media/01smoke0000000000000000001/01smoke0000000000000000003.png--w240.jpg 240w, '
      + '/media/01smoke0000000000000000001/01smoke0000000000000000003.png--w640.jpg 640w',
    webp_srcset: '/media/01smoke0000000000000000001/01smoke0000000000000000003.png--w240.webp 240w, '
      + '/media/01smoke0000000000000000001/01smoke0000000000000000003.png--w640.webp 640w',
  }],
  seller: { public_id: '01smoke0000000000000000002', display_name: 'فروشندهٔ نمونه' },
  comment_count: 0,
}

function send(res, status, body, type = 'application/json') {
  const payload = typeof body === 'string' ? body : JSON.stringify(body)
  res.writeHead(status, { 'content-type': `${type}; charset=utf-8` })
  res.end(payload)
}

const server = createServer((req, res) => {
  const path = decodeURIComponent((req.url ?? '/').split('?')[0])

  if (path === '/api/v1/health') return send(res, 200, { status: 'ok', database: 'ok' })
  if (path === '/api/v1/me') return send(res, 401, { error: { code: 'unauthorized' } })
  if (path === '/api/v1/categories') {
    return send(res, 200, { data: CATEGORIES.map((c, i) => ({ id: i + 1, parent_id: null, ...c })) })
  }
  if (path === '/api/v1/locations/provinces') return send(res, 200, { data: PROVINCES })
  if (path === '/api/v1/advertising/slot') return send(res, 200, { slot: null })

  const provinceCities = path.match(/^\/api\/v1\/locations\/provinces\/([^/]+)\/cities$/)
  if (provinceCities) {
    const cities = CITIES[provinceCities[1]]
    if (!cities) return send(res, 404, { error: { code: 'not_found' } })
    return send(res, 200, { data: cities.map((c) => ({ ...c, is_launch_city: false })) })
  }

  const cityBySlug = path.match(/^\/api\/v1\/locations\/cities\/([^/]+)$/)
  if (cityBySlug) {
    for (const province of PROVINCES) {
      const city = (CITIES[province.slug] ?? []).find((c) => c.slug === cityBySlug[1])
      if (!city) continue
      return send(res, 200, {
        data: {
          ...city,
          province_slug: province.slug,
          province_name_fa: province.name_fa,
        },
      })
    }
    return send(res, 404, { error: { code: 'not_found' } })
  }

  if (path === '/api/v1/listings') {
    return send(res, 200, { data: [CARD], page: { next_cursor: null, has_more: false } })
  }
  if (path === '/api/v1/sitemap/listings') {
    return send(res, 200, {
      data: [{ slug: CARD.slug, updated_at: '2026-11-10T09:00:00Z' }],
      page: { next_cursor: null, has_more: false },
    })
  }
  // The comment page must be matched before the detail route: `/listings/{slug}`
  // is a prefix of `/listings/{slug}/comments`, and returning the listing there
  // fed the comments component the wrong shape.
  if (/^\/api\/v1\/listings\/[^/]+\/comments$/.test(path)) {
    return send(res, 200, { data: [], page: { next_cursor: null, has_more: false } })
  }
  if (path.startsWith('/api/v1/listings/')) {
    // Detail is the one listing route that is NOT enveloped: the Rust handler
    // answers `Json(body)` with the listing fields at the top level, which is
    // exactly what `useFetch<ApiListingDetail>` expects. Wrapping it in `data`
    // here made every local smoke check of Detail 500.
    return send(res, 200, { ...LISTING })
  }
  return send(res, 404, { error: { code: 'not_found' } })
})

server.listen(PORT, '127.0.0.1', () => {
  console.log(`smoke stub API listening on http://127.0.0.1:${PORT}`)
})
