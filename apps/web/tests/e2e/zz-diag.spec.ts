import { test } from '@playwright/test'
import { createListing } from './support/stack'

// TEMPORARY diagnostic. Deleted once the mechanism is known.
test('@diag img mutation trace', async ({ page }) => {
  const title = 'آگهی تشخیصی جهش تصویر یکتا'
  await createListing({ title, state: 'active' })
  await page.addInitScript(() => {
    const w = window as unknown as { __ev: string[] }
    w.__ev = []
    new MutationObserver((records) => {
      for (const r of records) {
        if (r.type === 'attributes' && (r.target as Element).tagName === 'IMG') {
          w.__ev.push(`attr:${r.attributeName}=${(r.target as Element).getAttribute(r.attributeName ?? '')?.slice(0, 40)}`)
        }
        r.addedNodes.forEach((n) => {
          if (n.nodeType === 1 && ((n as Element).tagName === 'IMG' || (n as Element).querySelector?.('img'))) w.__ev.push('add:img')
          if (n.nodeType === 1 && (n as Element).classList?.contains('listing-card__placeholder')) w.__ev.push('add:placeholder')
        })
        r.removedNodes.forEach((n) => {
          if (n.nodeType === 1 && ((n as Element).tagName === 'IMG' || (n as Element).querySelector?.('img'))) w.__ev.push('remove:img')
        })
      }
    }).observe(document, { subtree: true, childList: true, attributes: true, attributeFilter: ['src', 'srcset', 'sizes'] })
  })
  let feedFetches = 0
  page.on('request', (r) => { if (new URL(r.url()).pathname === '/api/v1/listings') feedFetches++ })
  await page.route('**/media/**', async (route) => { await route.fulfill({ status: 404, body: '' }) })
  await page.goto(`/?q=${encodeURIComponent(title)}`)
  await page.waitForTimeout(2500)
  const ev = await page.evaluate(() => (window as unknown as { __ev: string[] }).__ev)
  throw new Error(`DIAG-MUT feed=${feedFetches} ev=${JSON.stringify(ev.slice(0, 30))}`)
})
