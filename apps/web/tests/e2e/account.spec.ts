import { readFileSync } from 'node:fs'
import { expect, test, type BrowserContext, type Page } from '@playwright/test'
import { apiAs, createListing, readFixtures } from './support/stack'

async function applyStorage(page: Page, storagePath: string): Promise<void> {
  const state = JSON.parse(readFileSync(storagePath, 'utf8')) as {
    cookies: Parameters<BrowserContext['addCookies']>[0]
  }
  await page.context().addCookies(state.cookies)
}

function parsePersianNumber(value: string): number {
  const normalized = value
    .replace(/[۰-۹]/g, (digit) => String('۰۱۲۳۴۵۶۷۸۹'.indexOf(digit)))
    .replace(/[٬,\s]/g, '')
  return Number(normalized)
}

async function receivedTile(page: Page): Promise<number> {
  const text = await page.locator('.stat').filter({ hasText: 'ذخیرهٔ آگهی‌های من' }).locator('.stat__value').innerText()
  return parsePersianNumber(text)
}

test('@account dashboard has Persian stat tiles, status chips and rejection reason', async ({ page }) => {
  const title = 'آگهی ردشده با دلیل خوانا'
  const created = await createListing({ title, imageCount: 0, state: 'pending' })
  const admin = await apiAs('admin')
  const rejected = await admin.post(`/api/v1/admin/listings/${created.publicId}/reject`, {
    data: { reason: 'اطلاعات آگهی برای بررسی کافی نیست.' },
  })
  expect(rejected.ok()).toBe(true)
  await admin.dispose()
  await applyStorage(page, readFixtures().sellerStorage)
  await page.goto('/me')
  await expect(page.locator('.stat')).toHaveCount(4)
  for (const label of ['فعال', 'در انتظار تأیید', 'فروخته‌شده', 'ذخیرهٔ آگهی‌های من']) {
    const tile = page.locator('.stat').filter({ hasText: label })
    await expect(tile).toBeVisible()
    await expect(tile.locator('.stat__value')).toHaveText(/^[۰-۹٬]+$/)
  }
  const row = page.locator('.mine__row').filter({ hasText: title })
  await expect(row.locator('.badge')).toHaveText('رد شده')
  await expect(row).toContainText('دلیل رد: اطلاعات آگهی برای بررسی کافی نیست.')
})

test('@account dashboard and favorites have useful empty states', async ({ page }) => {
  await applyStorage(page, readFixtures().adminStorage)
  await page.goto('/me')
  await expect(page.getByText('هنوز آگهی‌ای ثبت نکرده‌اید.', { exact: true })).toBeVisible()
  await expect(page.getByRole('link', { name: 'ثبت آگهی', exact: true }).last()).toBeVisible()
  await page.goto('/me/favorites')
  await expect(page.getByText('هنوز آگهی‌ای ذخیره نکرده‌اید.', { exact: true })).toBeVisible()
  await expect(page.getByRole('link', { name: 'مشاهده آگهی‌ها', exact: true })).toBeVisible()
})

test('@account favorites preserve cursor and cards through failures and anchor removal', async ({ page }) => {
  test.setTimeout(120_000)
  const seller = await apiAs('seller')
  const created = []
  for (let index = 1; index <= 21; index += 1) {
    created.push(await createListing({
      title: `آگهی صفحه‌بندی ${String(index).padStart(2, '0')}`,
      imageCount: 0,
      state: 'active',
    }))
  }
  for (const listing of created) {
    const saved = await seller.put(`/api/v1/me/favorites/${listing.publicId}`)
    expect(saved.ok()).toBe(true)
  }
  await seller.dispose()
  await applyStorage(page, readFixtures().sellerStorage)
  await page.goto('/me/favorites')
  const cards = page.locator('.listing-card')
  await expect(cards).toHaveCount(20)
  expect(new Set(await cards.locator('h3').allTextContents()).size).toBe(20)
  const anchorTitle = (await cards.nth(19).locator('h3').innerText()).trim()
  const more = page.getByRole('button', { name: 'نمایش موارد بیشتر', exact: true })
  await expect(more).toBeVisible()

  let failLoads = true
  let failDeletes = false
  const cursors: string[] = []
  await page.route('**/api/v1/me/favorites**', async (route) => {
    const request = route.request()
    const url = new URL(request.url())
    if (request.method() === 'GET' && url.searchParams.has('cursor')) {
      cursors.push(url.searchParams.get('cursor') ?? '')
      if (failLoads) {
        await route.fulfill({ status: 500, contentType: 'application/json', body: '{"error":{"code":"internal","message":"fixture"}}' })
        return
      }
    }
    if (request.method() === 'DELETE' && failDeletes) {
      await route.fulfill({ status: 500, contentType: 'application/json', body: '{"error":{"code":"internal","message":"fixture"}}' })
      return
    }
    await route.continue()
  })

  await more.click()
  await expect(page.getByText('خواندن موارد بیشتر ممکن نشد؛ دوباره تلاش کنید.', { exact: true })).toBeVisible()
  await expect(cards).toHaveCount(20)
  await more.click()
  await expect(cards).toHaveCount(20)
  // ofetch may retry a failed GET; every transport attempt must reuse the
  // same cursor and leave all confirmed cards in place.
  expect(cursors.length).toBeGreaterThanOrEqual(2)
  expect(cursors[0]).toBeTruthy()
  expect(new Set(cursors).size).toBe(1)
  const attemptsAfterFailures = cursors.length

  failDeletes = true
  const anchor = cards.filter({ hasText: anchorTitle })
  await anchor.getByRole('button', { name: 'حذف از علاقه‌مندی‌ها', exact: true }).click()
  await expect(page.getByText('حذف انجام نشد؛ دوباره تلاش کنید.', { exact: true })).toBeVisible()
  await expect(cards).toHaveCount(20)
  await expect(anchor).toBeVisible()
  failDeletes = false
  await anchor.getByRole('button', { name: 'حذف از علاقه‌مندی‌ها', exact: true }).click()
  await expect(cards).toHaveCount(19)
  await expect(page.locator('.favorites [role="status"]')).toHaveText('از علاقه‌مندی‌ها حذف شد.')

  failLoads = false
  await more.click()
  await expect(cards).toHaveCount(20)
  await expect(more).toHaveCount(0)
  expect(cursors.length).toBeGreaterThan(attemptsAfterFailures)
  expect(new Set(cursors).size).toBe(1)
  const finalTitles = await cards.locator('h3').allTextContents()
  expect(new Set(finalTitles).size).toBe(20)
  expect(finalTitles).not.toContain(anchorTitle)
})

test('@account another viewer save changes only the seller received tile', async ({ page, browser }) => {
  const fixtures = readFixtures()
  await applyStorage(page, fixtures.sellerStorage)
  await page.goto('/me')
  const sellerBefore = await receivedTile(page)
  const adminContext = await browser.newContext({
    baseURL: process.env.E2E_BASE_URL,
    storageState: fixtures.adminStorage,
  })
  const adminPage = await adminContext.newPage()
  await adminPage.goto('/me')
  const viewerBefore = await receivedTile(adminPage)

  const admin = await apiAs('admin')
  const saved = await admin.put(`/api/v1/me/favorites/${fixtures.listingPublicId}`)
  expect(saved.ok()).toBe(true)
  await admin.dispose()
  await page.reload()
  await adminPage.reload()
  expect(await receivedTile(page)).toBe(sellerBefore + 1)
  expect(await receivedTile(adminPage)).toBe(viewerBefore)
  await adminContext.close()
})
