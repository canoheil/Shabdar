import { readFileSync } from 'node:fs'
import { expect, test, type BrowserContext, type Page } from '@playwright/test'
import {
  apiAs,
  createListing,
  makeLegacyForFixture,
  readFixtures,
} from './support/stack'

async function applyStorage(page: Page, storagePath: string): Promise<void> {
  const state = JSON.parse(readFileSync(storagePath, 'utf8')) as {
    cookies: Parameters<BrowserContext['addCookies']>[0]
  }
  await page.context().addCookies(state.cookies)
}

interface AdminImage {
  id: number
  url: string
}

async function adminImages(publicId: string): Promise<AdminImage[]> {
  const admin = await apiAs('admin')
  try {
    const response = await admin.get(`/api/v1/admin/listings/${publicId}`)
    expect(response.ok()).toBe(true)
    const body = await response.json() as { images: AdminImage[] }
    return body.images
  } finally {
    await admin.dispose()
  }
}

test('@admin pending review confirms every real image and publishes only the exact set', async ({ page }) => {
  const title = 'آگهی با بررسی کامل دو تصویر'
  const listing = await createListing({ title, state: 'pending', imageCount: 2 })
  const images = await adminImages(listing.publicId)
  expect(images).toHaveLength(2)
  await applyStorage(page, readFixtures().adminStorage)
  await page.goto('/admin/queue')
  const row = page.locator('.card').filter({ hasText: title })
  await row.getByRole('link', { name: /بررسی تصاویر/ }).click()
  await expect(page).toHaveURL(new RegExp(`/admin/listings/${listing.publicId}$`))
  await expect(page.getByRole('heading', { name: 'بررسی تصاویر آگهی', exact: true })).toBeVisible()
  await expect(page.locator('.review-image')).toHaveCount(2)
  await expect(page.locator('.review-image img')).toHaveCount(2)
  await expect(page.getByText('۱۲۰۰ × ۹۰۰').or(page.getByText('1200 × 900'))).toHaveCount(2)

  const approve = page.getByRole('button', { name: 'تأیید تصاویر و انتشار', exact: true })
  const confirmations = page.getByRole('checkbox', { name: /را بررسی کردم/ })
  await expect(confirmations).toHaveCount(2)
  await expect(approve).toBeDisabled()
  await expect(confirmations.nth(0)).toBeEnabled()
  await confirmations.nth(0).check()
  await expect(approve).toBeDisabled()
  await expect(confirmations.nth(1)).toBeEnabled()
  await confirmations.nth(1).check()
  await expect(approve).toBeEnabled()

  const requestPromise = page.waitForRequest((request) => (
    request.method() === 'POST' && request.url().endsWith(`/admin/listings/${listing.publicId}/approve`)
  ))
  await approve.click()
  const request = await requestPromise
  expect(request.postDataJSON()).toEqual({ image_ids: images.map((image) => image.id) })
  await expect(page).toHaveURL(/\/admin\/queue\?review=published$/)
  await expect(page.locator('#queue-action')).toHaveAttribute('data-tone', 'success')
  await expect(page.locator('#queue-action')).toHaveText('آگهی تأیید و منتشر شد.')
  await expect(page.locator('.card').filter({ hasText: title })).toHaveCount(0)
})

test('@admin broken images cannot be confirmed or published', async ({ page }) => {
  const listing = await createListing({ title: 'آگهی با تصویر خراب در مرور مدیر', state: 'pending', imageCount: 2 })
  const images = await adminImages(listing.publicId)
  await applyStorage(page, readFixtures().adminStorage)
  await page.route(`**${images[0]!.url}`, (route) => route.abort())
  await page.goto(`/admin/listings/${listing.publicId}`)
  await expect(page.getByRole('heading', { name: 'بررسی تصاویر آگهی', exact: true })).toBeVisible()
  const confirmations = page.getByRole('checkbox', { name: /را بررسی کردم/ })
  await expect(confirmations.nth(0)).toBeDisabled()
  await expect(confirmations.nth(1)).toBeEnabled()
  await confirmations.nth(1).check()
  await expect(page.getByText('تصویر بارگذاری نشد؛ امکان تأیید آن نیست.', { exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: 'تأیید تصاویر و انتشار', exact: true })).toBeDisabled()
})

test('@admin review conflicts and server failures preserve checks and reasons until confirmation', async ({ page }) => {
  const title = 'آگهی با خطای موقت در بررسی تصاویر'
  const listing = await createListing({ title, state: 'pending' })
  await applyStorage(page, readFixtures().adminStorage)
  await page.goto(`/admin/listings/${listing.publicId}`)
  const confirmation = page.getByRole('checkbox', { name: /را بررسی کردم/ })
  await expect(confirmation).toBeEnabled()
  await confirmation.check()
  const reason = page.getByLabel('دلیل رد آگهی', { exact: true })
  await reason.fill('توضیحات آگهی باید کامل‌تر شود.')
  const approve = page.getByRole('button', { name: 'تأیید تصاویر و انتشار', exact: true })
  const secret = 'REVIEW_INTERNAL_STACK_MUST_NOT_RENDER'
  let approveAttempt = 0
  await page.route(`**/admin/listings/${listing.publicId}/approve`, async (route) => {
    approveAttempt += 1
    if (approveAttempt === 1) {
      await route.fulfill({
        status: 400,
        contentType: 'application/json',
        body: JSON.stringify({ error: { code: 'validation', message: secret, fields: { image_ids: 'mismatch' } } }),
      })
    } else {
      await route.fulfill({ status: 500, contentType: 'application/json', body: JSON.stringify({ error: { message: secret } }) })
    }
  })
  await approve.click()
  await expect(page.getByRole('alert')).toContainText('مجموعه تصاویر تغییر کرده است')
  await expect(confirmation).toBeChecked()
  await expect(reason).toHaveValue('توضیحات آگهی باید کامل‌تر شود.')
  await expect(page.getByRole('heading', { name: title, exact: true })).toBeVisible()

  await approve.click()
  await expect(page.getByRole('alert')).toHaveText('خطایی رخ داد؛ دوباره تلاش کنید.')
  await expect(confirmation).toBeChecked()
  await expect(reason).toHaveValue('توضیحات آگهی باید کامل‌تر شود.')
  expect(await page.locator('main').innerText()).not.toContain(secret)

  await page.route(`**/admin/listings/${listing.publicId}/reject`, (route) => route.fulfill({
    status: 500,
    contentType: 'application/json',
    body: JSON.stringify({ error: { message: secret } }),
  }))
  await page.getByRole('button', { name: 'رد آگهی', exact: true }).click()
  await expect(page.getByRole('alert')).toHaveText('خطایی رخ داد؛ دوباره تلاش کنید.')
  await expect(reason).toHaveValue('توضیحات آگهی باید کامل‌تر شود.')
  await expect(confirmation).toBeChecked()

  await page.unroute(`**/admin/listings/${listing.publicId}/approve`)
  await page.unroute(`**/admin/listings/${listing.publicId}/reject`)
  await approve.click()
  await expect(page).toHaveURL(/\/admin\/queue\?review=published$/)
  await expect(page.locator('.card').filter({ hasText: title })).toHaveCount(0)
})

test('@admin legacy backlog preserves removal errors and restores only after exact review', async ({ page }) => {
  const title = 'آگهی قدیمی در صف بررسی مجدد تصاویر'
  const listing = await createListing({ title, state: 'pending' })
  await makeLegacyForFixture(listing.publicId)
  await applyStorage(page, readFixtures().adminStorage)
  await page.goto('/admin/queue')
  const legacyQueue = page.locator('.legacy-queue')
  await expect(legacyQueue.getByRole('heading', { name: 'بررسی مجدد تصاویر آگهی‌های منتشرشده', exact: true })).toBeVisible()
  const row = legacyQueue.locator('.card').filter({ hasText: title })
  await expect(row).toContainText('۱ تصویر')
  await row.getByRole('link', { name: /بررسی تصاویر/ }).click()
  const confirmation = page.getByRole('checkbox', { name: /را بررسی کردم/ })
  await expect(confirmation).toBeEnabled()
  await confirmation.check()
  const reason = page.getByLabel('دلیل حذف آگهی', { exact: true })
  await reason.fill('دلیل حفظ‌شده برای حذف آگهی')
  await page.route(`**/admin/listings/${listing.publicId}/remove`, (route) => route.fulfill({
    status: 500,
    contentType: 'application/json',
    body: JSON.stringify({ error: { message: 'private failure' } }),
  }))
  await page.getByRole('button', { name: 'حذف آگهی', exact: true }).click()
  await expect(page.getByRole('alert')).toHaveText('خطایی رخ داد؛ دوباره تلاش کنید.')
  await expect(reason).toHaveValue('دلیل حفظ‌شده برای حذف آگهی')
  await expect(confirmation).toBeChecked()
  await page.unroute(`**/admin/listings/${listing.publicId}/remove`)

  await page.getByRole('button', { name: 'تأیید تصاویر و بازیابی انتشار', exact: true }).click()
  await expect(page).toHaveURL(/\/admin\/queue\?review=restored$/)
  await expect(page.locator('#queue-action')).toHaveAttribute('data-tone', 'success')
  await expect(page.locator('#queue-action')).toHaveText('بررسی تصاویر ثبت شد و آگهی دوباره قابل نمایش است.')
  await expect(page.locator('.legacy-queue .card').filter({ hasText: title })).toHaveCount(0)
})

test('@media owner draft media loads through Nuxt while anonymous access stays hidden', async ({ page }) => {
  const listing = await createListing({ title: 'پیش‌نویس با رسانهٔ خصوصی', state: 'draft' })
  const images = await adminImages(listing.publicId)
  await applyStorage(page, readFixtures().sellerStorage)
  const owner = await page.request.get(images[0]!.url)
  expect(owner.status()).toBe(200)
  expect(owner.headers()['content-type']).toBe('image/png')
  expect(owner.headers()['cache-control']).toContain('private')
  expect(owner.headers()['x-content-type-options']).toBe('nosniff')
  await page.context().clearCookies()
  const anonymous = await page.request.get(images[0]!.url)
  expect(anonymous.status()).toBe(404)
  expect(anonymous.headers()['cache-control']).toContain('private')
})
