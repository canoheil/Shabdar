import { readFileSync } from 'node:fs'
import { expect, test, type BrowserContext, type Page } from '@playwright/test'
import { apiAs, createListing, readFixtures } from './support/stack'

async function applyStorage(page: Page, storagePath: string): Promise<void> {
  const state = JSON.parse(readFileSync(storagePath, 'utf8')) as {
    cookies: Parameters<BrowserContext['addCookies']>[0]
  }
  await page.context().addCookies(state.cookies)
}

test('@admin desktop queue distinguishes validation errors from successful actions', async ({ page }) => {
  const rejectedTitle = 'آگهی برای بررسی دلیل رد'
  const approvedTitle = 'آگهی برای بررسی تأیید سریع'
  await createListing({ title: rejectedTitle, state: 'pending' })
  await createListing({ title: approvedTitle, state: 'pending' })
  await page.setViewportSize({ width: 1024, height: 800 })
  await applyStorage(page, readFixtures().adminStorage)
  await page.goto('/admin/queue')
  const queue = page.locator('.queue')
  await expect(queue.getByRole('heading', { name: 'صف بررسی آگهی‌ها', exact: true })).toBeVisible()
  await expect(queue.locator('.brand-mascot')).toHaveCount(0)
  const rejectedRow = queue.locator('.card').filter({ hasText: rejectedTitle })
  await rejectedRow.getByRole('button', { name: 'رد', exact: true }).click()
  const action = page.locator('#queue-action')
  await expect(action).toHaveAttribute('data-tone', 'error')
  await expect(action).toHaveAttribute('role', 'alert')
  await expect(action).toContainText('دلیل رد:')
  await rejectedRow.getByLabel('دلیل رد (برای رد کردن الزامی است)', { exact: true }).fill('لطفاً اطلاعات تماس و توضیحات را کامل کنید.')
  await rejectedRow.getByRole('button', { name: 'رد', exact: true }).click()
  await expect(action).toHaveAttribute('data-tone', 'success')
  await expect(action).toHaveAttribute('role', 'status')
  await expect(action).toHaveText('آگهی رد شد.')
  await expect(rejectedRow).toHaveCount(0)

  const approvedRow = queue.locator('.card').filter({ hasText: approvedTitle })
  await approvedRow.getByRole('button', { name: 'تأیید', exact: true }).click()
  await expect(action).toHaveAttribute('data-tone', 'success')
  await expect(action).toHaveText('آگهی تأیید و منتشر شد.')
  await expect(approvedRow).toHaveCount(0)
})

test('@errors persistent client detail 500 hides server details and retry recovers', async ({ page }) => {
  const fixtures = readFixtures()
  const secret = 'fixture-sensitive-server-stack'
  await page.goto(`/?q=${encodeURIComponent('آگهی آزمون شبدر')}`)
  await page.route('**/api/v1/listings/*', async (route) => {
    if (route.request().method() === 'GET') {
      await route.fulfill({
        status: 500,
        contentType: 'application/json',
        body: JSON.stringify({ error: { message: secret, stack: secret } }),
      })
      return
    }
    await route.continue()
  })
  await page.getByRole('link', { name: 'آگهی آزمون شبدر', exact: true }).first().click()
  await expect(page.locator('.error-page__status')).toHaveText('500')
  await expect(page.getByRole('heading', { name: 'مشکلی پیش آمد؛ دوباره تلاش کنید.', exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: 'بازگشت به خانه', exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: 'تلاش دوباره', exact: true })).toBeVisible()
  await expect(page.locator('.error-page .brand-mascot')).toHaveCount(0)
  expect(await page.locator('body').innerText()).not.toContain(secret)

  await page.unroute('**/api/v1/listings/*')
  await page.getByRole('button', { name: 'تلاش دوباره', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'آگهی آزمون شبدر', exact: true })).toBeVisible()
  await expect(page.locator('.listing-gallery__main-image').first()).toBeVisible()
})

test('@errors only 404 uses the mascot and Home and Search actions recover', async ({ page }) => {
  const response = await page.goto('/this-page-does-not-exist')
  expect(response?.status()).toBe(404)
  await expect(page.locator('.error-page__status')).toHaveText('404')
  await expect(page.locator('.error-page .brand-mascot')).toBeVisible()
  await page.getByRole('button', { name: 'بازگشت به خانه', exact: true }).click()
  await expect(page).toHaveURL(/\/$/)
  await expect(page.locator('#search')).toBeVisible()

  await page.goto('/this-page-does-not-exist')
  await page.getByRole('button', { name: 'جست‌وجو در آگهی‌ها', exact: true }).click()
  await expect(page).toHaveURL(/\/#search$/)
  await expect(page.locator('#search')).toBeVisible()
  await expect(page.getByRole('textbox', { name: 'جست‌وجو', exact: true })).toBeVisible()
})

test('@errors removed detail returns sanitized 410 with working Home recovery', async ({ page }) => {
  const secret = 'INTERNAL_REMOVAL_REASON_NOT_PUBLIC'
  const created = await createListing({ title: 'آگهی حذف‌شده برای ۴۱۰', state: 'active' })
  const admin = await apiAs('admin')
  const removed = await admin.post(`/api/v1/admin/listings/${created.publicId}/remove`, {
    data: { reason: secret },
  })
  expect(removed.ok()).toBe(true)
  await admin.dispose()
  const response = await page.goto(`/listings/${created.slug}`)
  expect(response?.status()).toBe(410)
  await expect(page.locator('.error-page__status')).toHaveText('410')
  await expect(page.locator('.error-page .brand-mascot')).toHaveCount(0)
  await expect(page.getByRole('button', { name: 'تلاش دوباره', exact: true })).toBeVisible()
  expect(await page.locator('body').innerText()).not.toContain(secret)
  await page.getByRole('button', { name: 'بازگشت به خانه', exact: true }).click()
  await expect(page).toHaveURL(/\/$/)
  await expect(page.locator('#search')).toBeVisible()
})
