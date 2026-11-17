import { expect, test } from '@playwright/test'
import { createListing } from './support/stack'

// TEMPORARY diagnostic for the broken-cover duplicate request. Deleted once
// the mechanism is known; both tests throw on purpose so CI prints the trace.
async function trace(page: import('@playwright/test').Page, title: string): Promise<string[]> {
  const seen: string[] = []
  const cdp = await page.context().newCDPSession(page)
  await cdp.send('Network.enable')
  cdp.on('Network.requestWillBeSent', (e) => {
    if (new URL(e.request.url).pathname.startsWith('/media/')) {
      seen.push(`${new URL(e.request.url).pathname.split('/').pop()} type=${e.initiator?.type ?? '?'}`)
    }
  })
  await page.route('**/media/**', async (route) => {
    await route.fulfill({ status: 404, body: '' })
  })
  await page.goto(`/?q=${encodeURIComponent(title)}`)
  await page.waitForTimeout(2000)
  return seen
}

test('@diag broken cover trace with hydration', async ({ page }) => {
  const title = 'آگهی تشخیصی یکتا با هیدریشن'
  await createListing({ title, state: 'active' })
  const seen = await trace(page, title)
  throw new Error(`DIAG-JS ${JSON.stringify(seen)}`)
})

test('@diag broken cover trace without JavaScript', async ({ browser }) => {
  const title = 'آگهی تشخیصی یکتا بدون جاوااسکریپت'
  await createListing({ title, state: 'active' })
  const context = await browser.newContext({ javaScriptEnabled: false, viewport: { width: 375, height: 812 } })
  const page = await context.newPage()
  const seen = await trace(page, title)
  await context.close()
  throw new Error(`DIAG-NOJS ${JSON.stringify(seen)}`)
})
