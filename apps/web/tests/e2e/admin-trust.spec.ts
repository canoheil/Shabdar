import { randomBytes } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { expect, test, type BrowserContext, type Page } from '@playwright/test'
import { runSql } from './support/database'
import { apiAs, createListing, readFixtures } from './support/stack'

async function applyStorage(page: Page, storagePath: string): Promise<void> {
  const state = JSON.parse(readFileSync(storagePath, 'utf8')) as {
    cookies: Parameters<BrowserContext['addCookies']>[0]
  }
  await page.context().addCookies(state.cookies)
}

async function createReport(input: {
  targetType: 'listing' | 'comment' | 'user'
  targetId: string
  details?: string
}): Promise<{ public_id: string }> {
  const api = await apiAs('buyer')
  try {
    const response = await api.post('/api/v1/reports', {
      data: {
        target_type: input.targetType,
        target_id: input.targetId,
        reason: 'abuse',
        details: input.details ?? 'گزارش آزمون مدیریت',
      },
    })
    expect(response.ok()).toBe(true)
    return await response.json() as { public_id: string }
  } finally {
    await api.dispose()
  }
}

async function createComment(slug: string, body: string): Promise<{ public_id: string }> {
  const api = await apiAs('admin')
  try {
    const response = await api.post(`/api/v1/listings/${encodeURIComponent(slug)}/comments`, {
      data: { body },
    })
    expect(response.ok()).toBe(true)
    return await response.json() as { public_id: string }
  } finally {
    await api.dispose()
  }
}

async function insertUser(name: string): Promise<{ publicId: string; phone: string }> {
  const user = {
    publicId: randomBytes(13).toString('hex').toUpperCase(),
    phone: `+9895${randomBytes(4).toString('hex').slice(0, 8)}`,
  }
  await runSql(`INSERT INTO users (public_id, phone_e164, display_name, created_at, updated_at) VALUES ('${user.publicId}', '${user.phone}', '${name}', NOW(3), NOW(3))`)
  return user
}

test.beforeEach(async () => {
  await runSql('DELETE FROM reports')
})

test('@admin-trust gates navigation safe rendering filters and confirmed dismiss', async ({ page }) => {
  const created = await createListing({ title: 'هدف <img src=x> گزارش', state: 'active' })
  const unsafe = '<img src=x onerror="window.__adminXss = true"> جزئیات'
  const report = await createReport({ targetType: 'listing', targetId: created.slug, details: unsafe })

  await page.goto('/admin/reports')
  await expect(page.getByText('دسترسی ندارید (ویژهٔ مدیر).', { exact: true })).toBeVisible()
  await applyStorage(page, readFixtures().buyerStorage)
  await page.goto('/admin/reports')
  await expect(page.getByText('دسترسی ندارید (ویژهٔ مدیر).', { exact: true })).toBeVisible()
  await applyStorage(page, readFixtures().adminStorage)
  await page.goto('/me')
  await expect(page.getByRole('navigation', { name: 'ناوبری اصلی' }).getByRole('link', { name: 'گزارش‌ها', exact: true })).toBeVisible()
  await expect(page.getByRole('navigation', { name: 'ناوبری اصلی' }).getByRole('link', { name: 'کاربران', exact: true })).toBeVisible()

  await page.goto('/admin/reports')
  const row = page.locator(`[data-report-id="${report.public_id}"]`)
  await expect(row).toBeVisible()
  await expect(row.getByText(unsafe, { exact: true })).toBeVisible()
  await expect(row.locator('img')).toHaveCount(0)
  expect(await page.evaluate(() => (window as unknown as { __adminXss?: boolean }).__adminXss)).toBeUndefined()

  await page.route(`**/api/v1/admin/reports/${report.public_id}/actions`, async (route) => {
    await route.fulfill({ status: 500, contentType: 'application/json', body: '{"error":{"code":"internal","message":"fixture"}}' })
  })
  await row.getByRole('button', { name: 'رد گزارش', exact: true }).click()
  await expect(page.getByText('ثبت اقدام انجام نشد؛ دوباره تلاش کنید.', { exact: true })).toBeVisible()
  await expect(row).toBeVisible()
  await page.unroute(`**/api/v1/admin/reports/${report.public_id}/actions`)
  await row.getByRole('button', { name: 'رد گزارش', exact: true }).click()
  await expect(row).toHaveCount(0)
  await expect(page.getByText('گزارش بازی نیست.', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'ردشده', exact: true }).click()
  await expect(row).toBeVisible()
  await expect(row.getByText('ردشده', { exact: true })).toBeVisible()
})

test('@admin-trust comment moderation preserves failures and supports confirmed restore', async ({ page }) => {
  const created = await createListing({ title: 'مدیریت دیدگاه گزارش‌شده', state: 'active' })
  const comment = await createComment(created.slug, 'دیدگاه هدف مدیر')
  const report = await createReport({ targetType: 'comment', targetId: comment.public_id })
  await applyStorage(page, readFixtures().adminStorage)
  await page.goto('/admin/reports')
  const row = page.locator(`[data-report-id="${report.public_id}"]`)
  await expect(row).toBeVisible()
  const hide = row.locator('[data-action="hide_comment"]')
  await hide.click()
  await expect(row.getByText('دلیل اقدام الزامی است.', { exact: true })).toBeVisible()
  await row.getByLabel('دلیل اقدام', { exact: true }).fill('نقض قوانین')
  await page.route(`**/api/v1/admin/reports/${report.public_id}/actions`, async (route) => {
    await route.fulfill({ status: 500, contentType: 'application/json', body: '{"error":{"code":"internal","message":"fixture"}}' })
  })
  await hide.click()
  await expect(row.getByText('ثبت اقدام انجام نشد؛ دوباره تلاش کنید.', { exact: true })).toBeVisible()
  await expect(row.getByLabel('دلیل اقدام', { exact: true })).toHaveValue('نقض قوانین')
  await page.unroute(`**/api/v1/admin/reports/${report.public_id}/actions`)

  let release!: () => void
  const gate = new Promise<void>((resolve) => { release = resolve })
  let requests = 0
  await page.route(`**/api/v1/admin/reports/${report.public_id}/actions`, async (route) => {
    requests += 1
    await gate
    await route.continue()
  })
  await hide.click()
  await expect(hide).toBeDisabled()
  await hide.evaluate((element) => (element as HTMLButtonElement).click())
  expect(requests).toBe(1)
  release()
  await expect(row).toHaveCount(0)
  await page.unroute(`**/api/v1/admin/reports/${report.public_id}/actions`)
  expect(await runSql(`SELECT status FROM listing_comments WHERE public_id = '${comment.public_id}'`)).toBe('hidden')
  expect(await runSql(`SELECT comment_count FROM listings WHERE public_id = '${created.public_id}'`)).toBe('0')

  await page.getByRole('button', { name: 'اقدام‌شده', exact: true }).click()
  await expect(row).toBeVisible()
  await row.locator('[data-action="restore_comment"]').click()
  await expect(row.getByText('بازیابی دیدگاه', { exact: true })).toBeVisible()
  expect(await runSql(`SELECT status FROM listing_comments WHERE public_id = '${comment.public_id}'`)).toBe('visible')
  expect(await runSql(`SELECT comment_count FROM listings WHERE public_id = '${created.public_id}'`)).toBe('1')
})

test('@admin-trust user search keeps failure drafts and confirms suspend unsuspend', async ({ page }) => {
  const user = await insertUser('کاربر تعلیم آزمون')
  await applyStorage(page, readFixtures().adminStorage)
  await page.goto('/admin/users')
  await page.getByLabel('جستجوی نام یا شماره', { exact: true }).fill(user.phone)
  const row = page.locator(`[data-user-id="${user.publicId}"]`)
  await expect(row).toBeVisible()
  await expect(row.getByText(user.phone, { exact: true })).toBeVisible()
  const suspend = row.locator('[data-action="suspend"]')
  await suspend.click()
  await expect(row.getByText('دلیل تعلیق الزامی است.', { exact: true })).toBeVisible()
  await row.getByLabel('دلیل تعلیق', { exact: true }).fill('سوءاستفاده آزمون')
  await page.route(`**/api/v1/admin/users/${user.publicId}/suspend`, async (route) => {
    await route.fulfill({ status: 500, contentType: 'application/json', body: '{"error":{"code":"internal","message":"fixture"}}' })
  })
  await suspend.click()
  await expect(row.getByText('تغییر وضعیت کاربر انجام نشد؛ دوباره تلاش کنید.', { exact: true })).toBeVisible()
  await expect(row.getByLabel('دلیل تعلیق', { exact: true })).toHaveValue('سوءاستفاده آزمون')
  await page.unroute(`**/api/v1/admin/users/${user.publicId}/suspend`)
  await suspend.click()
  await expect(row.getByText('تعلیق‌شده', { exact: true })).toBeVisible()
  const unsuspend = row.locator('[data-action="unsuspend"]')
  await expect(unsuspend).toBeVisible()
  await unsuspend.click()
  await expect(row.getByText('فعال', { exact: true })).toBeVisible()
  await expect(row.locator('[data-action="suspend"]')).toBeVisible()
})

test('@admin-trust user forward pages retain confirmed rows on failure and retry', async ({ page }) => {
  const users = Array.from({ length: 21 }, (_, index) => ({
    publicId: randomBytes(13).toString('hex').toUpperCase(),
    phone: `+9896${String(index).padStart(8, '0')}`,
    name: `صف کاربر ${String(index + 1).padStart(2, '0')}`,
  }))
  const values = users.map((user) => `('${user.publicId}', '${user.phone}', '${user.name}', '2026-10-25 00:00:00.000', NOW(3))`).join(',')
  await runSql(`INSERT INTO users (public_id, phone_e164, display_name, created_at, updated_at) VALUES ${values}`)
  await applyStorage(page, readFixtures().adminStorage)
  await page.goto('/admin/users')
  await page.getByLabel('جستجوی نام یا شماره', { exact: true }).fill('صف کاربر')
  await expect(page.locator('[data-user-id]')).toHaveCount(20)
  const endpoint = '**/api/v1/admin/users?*'
  await page.route(endpoint, async (route) => {
    if (!new URL(route.request().url()).searchParams.has('cursor')) {
      await route.continue()
      return
    }
    await route.fulfill({ status: 500, contentType: 'application/json', body: '{"error":{"code":"internal","message":"fixture"}}' })
  })
  await page.getByRole('button', { name: 'نمایش موارد بیشتر', exact: true }).click()
  await expect(page.getByText('خواندن موارد بیشتر ممکن نشد؛ دوباره تلاش کنید.', { exact: true })).toBeVisible()
  await expect(page.locator('[data-user-id]')).toHaveCount(20)
  await page.unroute(endpoint)
  await page.getByRole('button', { name: 'تلاش دوباره بارگذاری', exact: true }).click()
  await expect(page.locator('[data-user-id]')).toHaveCount(21)
  const ids = await page.locator('[data-user-id]').evaluateAll((rows) => rows.map((row) => row.getAttribute('data-user-id')))
  expect(new Set(ids).size).toBe(21)
})
