// Public sitemap (Phase 14). The API has shipped `/api/v1/sitemap/listings`
// since Phase 3; until now nothing consumed it. This route turns it into the
// XML crawlers expect and adds the fixed public entry points.
//
// Ponytail: the page walk is bounded, so a runaway API can never turn one
// crawler request into an unbounded fetch loop. City pages are deliberately
// absent until `GET /api/v1/sitemap/locations` exists (recorded in STATUS.md);
// listing and category coverage is the high-value index today.
const MAX_PAGES = 20
const PAGE_LIMIT = 200

interface SitemapPage {
  data: Array<{ url: string; updated_at: string }>
  page: { next_cursor: string | null; has_more: boolean }
}

function escapeXml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&apos;')
}

export default defineEventHandler(async (event) => {
  const config = useRuntimeConfig(event)
  const origin = String(config.public.siteUrl ?? '').replace(/\/+$/, '')

  // Fixed public pages first: they exist without any database read.
  const entries: Array<{ loc: string; lastmod?: string; changefreq?: string; priority?: string }> = [
    { loc: `${origin}/`, changefreq: 'daily', priority: '1.0' },
    { loc: `${origin}/privacy`, changefreq: 'yearly', priority: '0.3' },
    { loc: `${origin}/terms`, changefreq: 'yearly', priority: '0.3' },
    { loc: `${origin}/advertising`, changefreq: 'monthly', priority: '0.4' },
  ]

  // Category pages are a small fixed set and the strongest static entry point.
  try {
    const categories = await $fetch<{ data: Array<{ slug: string }> }>(
      `${config.apiBase}/api/v1/categories`,
      { timeout: 5000 },
    )
    for (const category of categories.data) {
      entries.push({
        loc: `${origin}/category/${category.slug}`,
        changefreq: 'daily',
        priority: '0.7',
      })
    }
  } catch {
    // A reference read failure must not fail the whole sitemap; the listing
    // entries below are still correct on their own.
  }

  let cursor: string | null = null
  for (let pageNumber = 0; pageNumber < MAX_PAGES; pageNumber += 1) {
    let page: SitemapPage
    try {
      page = await $fetch<SitemapPage>(`${config.apiBase}/api/v1/sitemap/listings`, {
        query: { limit: PAGE_LIMIT, ...(cursor ? { cursor } : {}) },
        timeout: 8000,
      })
    } catch {
      break
    }
    for (const listing of page.data) {
      if (!listing.url) continue
      entries.push({
        loc: `${origin}${listing.url.startsWith('/') ? '' : '/'}${listing.url}`,
        lastmod: listing.updated_at?.slice(0, 10),
        changefreq: 'weekly',
        priority: '0.8',
      })
    }
    if (!page.page.has_more || !page.page.next_cursor) break
    cursor = page.page.next_cursor
  }

  const body = [
    '<?xml version="1.0" encoding="UTF-8"?>',
    '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">',
    ...entries.map((entry) => {
      const parts = [`<loc>${escapeXml(entry.loc)}</loc>`]
      if (entry.lastmod) parts.push(`<lastmod>${escapeXml(entry.lastmod)}</lastmod>`)
      if (entry.changefreq) parts.push(`<changefreq>${entry.changefreq}</changefreq>`)
      if (entry.priority) parts.push(`<priority>${entry.priority}</priority>`)
      return `  <url>${parts.join('')}</url>`
    }),
    '</urlset>',
    '',
  ].join('\n')

  setResponseHeader(event, 'content-type', 'application/xml; charset=utf-8')
  // Cached at the edge: a crawler hitting a shared sitemap must not cost one
  // database pass per request.
  setResponseHeader(event, 'cache-control', 'public, max-age=600, s-maxage=3600')
  return body
})
