import { expect, test } from '@playwright/test'
import { readFixtures } from './support/stack'

// Phase 14: the API sitemap endpoint existed since Phase 3 but nothing consumed
// it. These routes make it real, and robots.txt states the private surfaces.
test.describe('@seo sitemap and robots', () => {
  test('robots.txt allows public pages, blocks private ones and points at the sitemap', async ({ page }) => {
    const response = await page.request.get('/robots.txt')
    expect(response.status()).toBe(200)
    expect(response.headers()['content-type']).toContain('text/plain')
    const body = await response.text()
    expect(body).toContain('User-agent: *')
    expect(body).toContain('Allow: /')
    expect(body).toContain('Disallow: /me')
    expect(body).toContain('Disallow: /admin')
    expect(body).toContain('Disallow: /login')
    expect(body).toMatch(/Sitemap: https?:\/\/\S+\/sitemap\.xml/)
  })

  test('sitemap.xml is well-formed, absolute and same-origin', async ({ page }) => {
    const response = await page.request.get('/sitemap.xml')
    expect(response.status()).toBe(200)
    expect(response.headers()['content-type']).toContain('xml')
    const xml = await response.text()
    expect(xml.startsWith('<?xml')).toBe(true)
    expect(xml).toContain('<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">')
    expect(xml.trimEnd().endsWith('</urlset>')).toBe(true)

    const locs = [...xml.matchAll(/<loc>([^<]+)<\/loc>/g)].map((match) => match[1] ?? '')
    expect(locs.length).toBeGreaterThan(0)
    for (const loc of locs) expect(loc).toMatch(/^https?:\/\//)

    const origin = new URL(locs[0] ?? '').origin
    for (const loc of locs) expect(new URL(loc).origin).toBe(origin)

    // The fixed public entries are always present.
    expect(locs.some((loc) => new URL(loc).pathname === '/')).toBe(true)
    expect(locs.some((loc) => new URL(loc).pathname === '/privacy')).toBe(true)
    expect(locs.some((loc) => new URL(loc).pathname === '/terms')).toBe(true)

    // The real active fixture listing is listed, and a private route never is.
    const detailPath = new URL(readFixtures().detailPath, origin).pathname
    expect(locs.map((loc) => new URL(loc).pathname)).toContain(detailPath)
    for (const loc of locs) {
      const path = new URL(loc).pathname
      expect(path.startsWith('/me')).toBe(false)
      expect(path.startsWith('/admin')).toBe(false)
      expect(path.startsWith('/login')).toBe(false)
    }
  })

  test('sitemap entries actually resolve', async ({ page }) => {
    const xml = await (await page.request.get('/sitemap.xml')).text()
    const locs = [...xml.matchAll(/<loc>([^<]+)<\/loc>/g)].map((match) => match[1] ?? '')
    const origin = new URL(locs[0] ?? '').origin
    for (const loc of locs) {
      const response = await page.request.get(new URL(loc).pathname)
      expect(response.status(), `sitemap entry ${loc}`).toBe(200)
    }
    expect(origin).toBeTruthy()
  })
})
