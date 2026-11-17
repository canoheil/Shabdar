import { test, expect } from '@playwright/test'
import { createListing } from './support/stack'

// TEMPORARY diagnostics for the broken-cover duplicate request. Deleted once
// the mechanism is known. Each test asserts the single-fetch ideal so the
// pass/fail summary alone answers the question.
const WEBP_1PX = Buffer.from('UklGRiQAAABXRUJQVlA4IBgAAAAwAQCdASoBAAEAAwA0JaQAA3AA/vuUAAA=', 'base64')

async function trace(page: import('@playwright/test').Page, title: string, ok: boolean): Promise<string[]> {
  const seen: string[] = []
  await page.route('**/media/**', async (route) => {
    seen.push(route.request().url())
    if (ok) await route.fulfill({ status: 200, contentType: 'image/webp', headers: { 'cache-control': 'no-store' }, body: WEBP_1PX })
    else await route.fulfill({ status: 404, body: '' })
  })
  await page.goto(`/?q=${encodeURIComponent(title)}`)
  await page.waitForTimeout(2000)
  return seen
}

test('@diag nojs broken cover fetches once', async ({ browser }) => {
  const title = 'آگهی تشخیصی بدون جاوااسکریپت یکتا'
  await createListing({ title, state: 'active' })
  const context = await browser.newContext({ javaScriptEnabled: false, viewport: { width: 375, height: 812 } })
  const page = await context.newPage()
  const seen = await trace(page, title, false)
  await context.close()
  expect(seen).toHaveLength(1)
})

test('@diag js ok cover fetches each url once', async ({ page }) => {
  const title = 'آگهی تشخیصی موفق یکتا'
  await createListing({ title, state: 'active' })
  const seen = await trace(page, title, true)
  expect(new Set(seen).size).toBe(seen.length)
})
