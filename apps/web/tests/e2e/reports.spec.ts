import { readFileSync } from 'node:fs'
import { expect, test, type BrowserContext, type Locator, type Page } from '@playwright/test'
import { runSql } from './support/database'
import { apiAs, createListing, readFixtures, type FixtureRole } from './support/stack'

interface CommentFixture {
  public_id: string
  body: string
  author: { public_id: string; display_name: string | null }
}

async function applyStorage(page: Page, storagePath: string): Promise<void> {
  const state = JSON.parse(readFileSync(storagePath, 'utf8')) as {
    cookies: Parameters<BrowserContext['addCookies']>[0]
  }
  await page.context().addCookies(state.cookies)
}

function detailPath(slug: string): string {
  return `/listings/${encodeURIComponent(slug)}`
}

async function me(role: FixtureRole): Promise<{ public_id: string }> {
  const api = await apiAs(role)
  try {
    const response = await api.get('/api/v1/me')
    expect(response.ok()).toBe(true)
    return (await response.json() as { user: { public_id: string } }).user
  } finally {
    await api.dispose()
  }
}

async function createComment(role: FixtureRole, slug: string, body: string): Promise<CommentFixture> {
  const api = await apiAs(role)
  try {
    const response = await api.post(`/api/v1/listings/${encodeURIComponent(slug)}/comments`, {
      data: { body },
    })
    expect(response.ok()).toBe(true)
    return await response.json() as CommentFixture
  } finally {
    await api.dispose()
  }
}

async function fillReport(panel: Locator, reason: string, details = ''): Promise<void> {
  await panel.getByLabel('دلیل گزارش', { exact: true }).selectOption(reason)
  await panel.getByLabel('توضیحات (اختیاری)', { exact: true }).fill(details)
}

test.beforeEach(async () => {
  await runSql('DELETE FROM reports')
})

test('@reports anonymous login and all four targets submit once with safe confirmed state', async ({ page }) => {
  const created = await createListing({ title: 'گزارش همه اهداف', state: 'active' })
  const admin = await me('admin')
  const seller = await me('seller')
  const other = await createComment('admin', created.slug, 'دیدگاه قابل گزارش')
  const own = await createComment('buyer', created.slug, 'دیدگاه خودِ گزارشگر')

  const response = await page.request.get(detailPath(created.slug))
  expect(response.ok()).toBe(true)
  const html = await response.text()
  expect(html).not.toContain('گزارش آگهی')
  expect(html).not.toContain('ورود برای گزارش')
  await page.goto(detailPath(created.slug))
  const anonymousListing = page.locator('[data-report-control="listing"]')
  await anonymousListing.getByRole('link', { name: 'ورود برای گزارش', exact: true }).click()
  await expect(page).toHaveURL(/\/login$/)

  await applyStorage(page, readFixtures().buyerStorage)
  await page.goto(detailPath(created.slug))
  const listing = page.locator('[data-report-control="listing"]')
  const sellerControl = page.locator('[data-report-control="seller"]')
  const otherItem = page.locator(`[data-comment-id="${other.public_id}"]`)
  const ownItem = page.locator(`[data-comment-id="${own.public_id}"]`)
  await expect(ownItem.getByRole('button', { name: 'گزارش دیدگاه', exact: true })).toHaveCount(0)
  await expect(ownItem.getByRole('button', { name: 'گزارش نویسنده', exact: true })).toHaveCount(0)

  await listing.getByRole('button', { name: 'گزارش آگهی', exact: true }).click()
  const listingPanel = listing.locator('.report-control__panel')
  await listingPanel.getByRole('button', { name: 'ثبت گزارش', exact: true }).click()
  await expect(listingPanel.getByText('انتخاب دلیل گزارش الزامی است.', { exact: true })).toBeVisible()
  const unsafe = '<img src=x onerror="window.__reportXss = true"> متن امن'
  await fillReport(listingPanel, 'spam', unsafe)
  let release!: () => void
  const gate = new Promise<void>((resolve) => { release = resolve })
  let requests = 0
  await page.route('**/api/v1/reports', async (route) => {
    if (route.request().method() === 'POST') {
      requests += 1
      await gate
    }
    await route.continue()
  })
  const submit = listingPanel.getByRole('button', { name: 'ثبت گزارش', exact: true })
  await submit.click()
  await expect(submit).toBeDisabled()
  await submit.evaluate((element) => (element as HTMLButtonElement).click())
  expect(requests).toBe(1)
  release()
  await expect(listingPanel.getByText('گزارش شما ثبت شده است.', { exact: true })).toBeVisible()
  await page.unroute('**/api/v1/reports')
  expect(await page.evaluate(() => (window as unknown as { __reportXss?: boolean }).__reportXss)).toBeUndefined()
  await expect(listingPanel.locator('img')).toHaveCount(0)

  const targets: Array<{ control: Locator; button: string; reason: string }> = [
    { control: sellerControl, button: 'گزارش فروشنده', reason: 'fraud' },
    { control: otherItem.locator('[data-report-control="comment"]'), button: 'گزارش دیدگاه', reason: 'offensive' },
    { control: otherItem.locator('[data-report-control="author"]'), button: 'گزارش نویسنده', reason: 'abuse' },
  ]
  for (const target of targets) {
    await target.control.getByRole('button', { name: target.button, exact: true }).click()
    const panel = target.control.locator('.report-control__panel')
    await fillReport(panel, target.reason)
    await panel.getByRole('button', { name: 'ثبت گزارش', exact: true }).click()
    await expect(panel.getByText('گزارش شما ثبت شده است.', { exact: true })).toBeVisible()
  }
  expect(await runSql('SELECT COUNT(*) FROM reports')).toBe('4')
  expect(await runSql("SELECT COUNT(DISTINCT target_type, target_id) FROM reports")).toBe('4')
  const listingTarget = await runSql("SELECT l.slug FROM reports r JOIN listings l ON l.id = r.target_id WHERE r.target_type = 'listing'")
  expect(listingTarget).toBe(created.slug)
  const userTargets = await runSql("SELECT GROUP_CONCAT(u.public_id ORDER BY u.public_id SEPARATOR ',') FROM reports r JOIN users u ON u.id = r.target_id WHERE r.target_type = 'user'")
  expect(userTargets.split(',').sort()).toEqual([admin.public_id, seller.public_id].sort())
})

test('@reports failures and 401 retain drafts until a confirmed retry', async ({ page }) => {
  const created = await createListing({ title: 'خطا و پیش‌نویس گزارش', state: 'active' })
  await applyStorage(page, readFixtures().buyerStorage)
  await page.goto(detailPath(created.slug))
  const listing = page.locator('[data-report-control="listing"]')
  await listing.getByRole('button', { name: 'گزارش آگهی', exact: true }).click()
  const panel = listing.locator('.report-control__panel')
  const unsafe = '<svg onload="window.__reportXss = true">پیش‌نویس</svg>'
  await fillReport(panel, 'other', unsafe)
  await page.route('**/api/v1/reports', async (route) => {
    await route.fulfill({
      status: 500,
      contentType: 'application/json',
      body: '{"error":{"code":"internal","message":"fixture"}}',
    })
  })
  await panel.getByRole('button', { name: 'ثبت گزارش', exact: true }).click()
  await expect(panel.getByText('ثبت گزارش انجام نشد؛ دوباره تلاش کنید.', { exact: true })).toBeVisible()
  await expect(panel.getByLabel('توضیحات (اختیاری)', { exact: true })).toHaveValue(unsafe)
  await page.unroute('**/api/v1/reports')
  await panel.getByRole('button', { name: 'تلاش دوباره ثبت گزارش', exact: true }).click()
  await expect(panel.getByText('گزارش شما ثبت شده است.', { exact: true })).toBeVisible()
  await expect(panel.getByLabel('توضیحات (اختیاری)', { exact: true })).toHaveValue(unsafe)
  expect(await page.evaluate(() => (window as unknown as { __reportXss?: boolean }).__reportXss)).toBeUndefined()

  const seller = page.locator('[data-report-control="seller"]')
  await seller.getByRole('button', { name: 'گزارش فروشنده', exact: true }).click()
  const sellerPanel = seller.locator('.report-control__panel')
  await fillReport(sellerPanel, 'abuse', 'پیش‌نویس نشست منقضی')
  await page.route('**/api/v1/reports', async (route) => {
    await route.fulfill({
      status: 401,
      contentType: 'application/json',
      body: '{"error":{"code":"unauthorized","message":"fixture"}}',
    })
  })
  await sellerPanel.getByRole('button', { name: 'ثبت گزارش', exact: true }).click()
  await expect(sellerPanel.getByText('نشست شما منقضی شده است؛ دوباره وارد شوید.', { exact: true })).toBeVisible()
  await expect(sellerPanel.getByRole('link', { name: 'ورود', exact: true })).toBeVisible()
  await expect(sellerPanel.getByLabel('دلیل گزارش', { exact: true })).toHaveValue('abuse')
  await expect(sellerPanel.getByLabel('توضیحات (اختیاری)', { exact: true })).toHaveValue('پیش‌نویس نشست منقضی')
})

test('@reports self listing seller comment and author controls are omitted', async ({ page }) => {
  const created = await createListing({ title: 'حذف گزارش خود', state: 'active' })
  const own = await createComment('seller', created.slug, 'دیدگاه فروشنده')
  const other = await createComment('admin', created.slug, 'دیدگاه کاربر دیگر')
  await applyStorage(page, readFixtures().sellerStorage)
  await page.goto(detailPath(created.slug))
  await expect(page.locator('[data-report-control="listing"]')).toHaveCount(0)
  await expect(page.locator('[data-report-control="seller"]')).toHaveCount(0)
  const ownItem = page.locator(`[data-comment-id="${own.public_id}"]`)
  await expect(ownItem.locator('[data-report-control="comment"]')).toHaveCount(0)
  await expect(ownItem.locator('[data-report-control="author"]')).toHaveCount(0)
  const otherItem = page.locator(`[data-comment-id="${other.public_id}"]`)
  await expect(otherItem.getByRole('button', { name: 'گزارش دیدگاه', exact: true })).toBeVisible()
  await expect(otherItem.getByRole('button', { name: 'گزارش نویسنده', exact: true })).toBeVisible()
})
