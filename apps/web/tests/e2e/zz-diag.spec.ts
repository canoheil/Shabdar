import { expect, test } from '@playwright/test'
import { createListing } from './support/stack'

// TEMPORARY diagnostic for the broken-cover duplicate request. Deleted once
// the mechanism is known; it throws on purpose so CI prints what it saw.
test('@diag broken cover request trace', async ({ page }) => {
  const title = 'آگهی با تصویر شکسته در Home'
  await createListing({ title, state: 'active' })
  const seen: string[] = []
  const cdp = await page.context().newCDPSession(page)
  await cdp.send('Network.enable')
  cdp.on('Network.requestWillBeSent', (e) => {
    if (new URL(e.request.url).pathname.startsWith('/media/')) {
      const init = e.initiator
      const stack = (init?.stack?.callFrames ?? []).slice(0, 2)
        .map((f) => `${f.functionName || '?'}@${f.url.split('/').pop()}:${f.lineNumber}`)
        .join(',')
      seen.push(`${new URL(e.request.url).pathname.split('/').pop()} type=${init?.type ?? '?'} ${stack}`)
    }
  })
  await page.route('**/media/**', async (route) => {
    await route.fulfill({ status: 404, body: '' })
  })
  await page.goto(`/?q=${encodeURIComponent(title)}`)
  const fixtureCard = page.locator('.listing-card').filter({ hasText: title })
  await expect(fixtureCard.getByText('تصویر در دسترس نیست', { exact: true })).toBeVisible()
  await page.waitForTimeout(1500)
  throw new Error(`DIAG ${JSON.stringify(seen)}`)
})
