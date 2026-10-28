import { randomBytes } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { expect, test, type BrowserContext, type Page } from '@playwright/test'
import { runSql } from './support/database'
import { apiAs, createListing, readFixtures, type FixtureRole } from './support/stack'

interface CommentFixture {
  public_id: string
  body: string
  author: { public_id: string; display_name: string | null }
}

interface BlockFixture {
  public_id: string
  display_name: string | null
  blocked_at: string
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

async function createComment(role: FixtureRole, slug: string, body: string): Promise<CommentFixture> {
  const api = await apiAs(role)
  try {
    const response = await api.post(`/api/v1/listings/${encodeURIComponent(slug)}/comments`, {
      data: { body },
    })
    expect(response.ok(), `${role} creates comment`).toBe(true)
    return await response.json() as CommentFixture
  } finally {
    await api.dispose()
  }
}

async function me(role: FixtureRole): Promise<{ public_id: string; display_name: string | null }> {
  const api = await apiAs(role)
  try {
    const response = await api.get('/api/v1/me')
    expect(response.ok()).toBe(true)
    const body = await response.json() as { user: { public_id: string; display_name: string | null } }
    return body.user
  } finally {
    await api.dispose()
  }
}

async function mutateBlock(
  role: FixtureRole,
  targetPublicId: string,
  method: 'PUT' | 'DELETE',
): Promise<void> {
  const api = await apiAs(role)
  try {
    const response = await api.fetch(`/api/v1/me/blocks/${targetPublicId}`, { method })
    expect(response.ok(), `${role} ${method} block`).toBe(true)
  } finally {
    await api.dispose()
  }
}

test.beforeEach(async () => {
  await runSql('DELETE FROM blocks')
})

test('@blocks anonymous SSR stays unfiltered and a failed private read preserves confirmed comments', async ({ page }) => {
  const created = await createListing({ title: 'حریم خصوصی دیدگاه‌های مسدود', state: 'active' })
  const admin = await me('admin')
  const blocked = await createComment('admin', created.slug, 'دیدگاه نویسنده مسدود')
  await createComment('buyerTwo', created.slug, 'دیدگاه کاربر دیگر')
  const own = await createComment('buyer', created.slug, 'دیدگاه خودم')
  await mutateBlock('buyer', admin.public_id, 'PUT')

  const response = await page.request.get(detailPath(created.slug))
  expect(response.ok()).toBe(true)
  const html = await response.text()
  expect(html).toContain('دیدگاه نویسنده مسدود')
  expect(html).toContain('دیدگاه کاربر دیگر')
  expect(html).toContain('دیدگاه خودم')
  expect(html).not.toContain('مسدود کردن نویسنده')
  expect(html).not.toContain('/me/blocks')

  await applyStorage(page, readFixtures().buyerStorage)
  const endpoint = '**/api/v1/listings/*/comments**'
  await page.route(endpoint, async (route) => {
    if (route.request().method() !== 'GET') {
      await route.continue()
      return
    }
    await route.fulfill({
      status: 500,
      contentType: 'application/json',
      body: '{"error":{"code":"internal","message":"fixture"}}',
    })
  })
  await page.goto(detailPath(created.slug))
  const comments = page.locator('.listing-comments')
  await expect(comments.getByText('بارگذاری فهرست خصوصی دیدگاه‌ها ممکن نشد؛ دوباره تلاش کنید.', { exact: true })).toBeVisible()
  await expect(comments.locator('.listing-comments__item')).toHaveCount(3)
  await expect(comments.locator('.listing-comments__count')).toHaveText('۳ دیدگاه')
  await expect(comments.locator(`[data-comment-id="${own.public_id}"]`).getByRole('button', { name: 'مسدود کردن نویسنده', exact: true })).toHaveCount(0)
  await expect(comments.locator(`[data-comment-id="${blocked.public_id}"]`).getByRole('button', { name: 'مسدود کردن نویسنده', exact: true })).toBeVisible()

  await page.unroute(endpoint)
  await comments.getByRole('button', { name: 'تلاش دوباره فهرست خصوصی', exact: true }).click()
  await expect(comments.locator('.listing-comments__item')).toHaveCount(2)
  await expect(comments.getByText('دیدگاه نویسنده مسدود', { exact: true })).toHaveCount(0)
  await expect(comments.locator('.listing-comments__count')).toHaveText('۳ دیدگاه')
})

test('@blocks only confirmed mutations filter one viewer and failures or 401 preserve items', async ({ page, browser }) => {
  const created = await createListing({ title: 'مسدودسازی تأییدشده و ایزوله', state: 'active' })
  const admin = await me('admin')
  const buyerTwo = await me('buyerTwo')
  await createComment('admin', created.slug, 'ادمین یک')
  await createComment('admin', created.slug, 'ادمین دو')
  await createComment('buyerTwo', created.slug, 'دیدگاه نفر دوم')
  await createComment('buyer', created.slug, 'دیدگاه مالک نشست')
  await applyStorage(page, readFixtures().buyerStorage)
  await page.goto(detailPath(created.slug))
  const comments = page.locator('.listing-comments')
  await expect(comments.locator('.listing-comments__item')).toHaveCount(4)
  const targetItem = comments.locator('.listing-comments__item').filter({ hasText: 'ادمین یک' })
  const blockButton = targetItem.getByRole('button', { name: 'مسدود کردن نویسنده', exact: true })
  await expect(blockButton).toBeVisible()

  const endpoint = `**/api/v1/me/blocks/${admin.public_id}`
  await page.route(endpoint, async (route) => {
    await route.fulfill({
      status: 500,
      contentType: 'application/json',
      body: '{"error":{"code":"internal","message":"fixture"}}',
    })
  })
  await blockButton.click()
  await expect(targetItem.getByText('مسدودسازی انجام نشد؛ دوباره تلاش کنید.', { exact: true })).toBeVisible()
  await expect(comments.locator('.listing-comments__item')).toHaveCount(4)
  await expect(comments.locator('.listing-comments__count')).toHaveText('۴ دیدگاه')
  await page.unroute(endpoint)
  await targetItem.getByRole('button', { name: 'تلاش دوباره', exact: true }).click()
  await expect(comments.locator('.listing-comments__item')).toHaveCount(2)
  await expect(comments.getByText('ادمین یک', { exact: true })).toHaveCount(0)
  await expect(comments.getByText('ادمین دو', { exact: true })).toHaveCount(0)
  await expect(comments.locator('.listing-comments__count')).toHaveText('۴ دیدگاه')

  const other = await browser.newPage()
  await applyStorage(other, readFixtures().buyerTwoStorage)
  await other.goto(detailPath(created.slug))
  await expect(other.locator('.listing-comments__item')).toHaveCount(4)
  await expect(other.locator('.listing-comments__count')).toHaveText('۴ دیدگاه')
  await other.close()

  const secondItem = comments.locator('.listing-comments__item').filter({ hasText: 'دیدگاه نفر دوم' })
  await page.route(`**/api/v1/me/blocks/${buyerTwo.public_id}`, async (route) => {
    await route.fulfill({
      status: 401,
      contentType: 'application/json',
      body: '{"error":{"code":"unauthorized","message":"fixture"}}',
    })
  })
  await secondItem.getByRole('button', { name: 'مسدود کردن نویسنده', exact: true }).click()
  await expect(secondItem).toBeVisible()
  await expect(comments.locator('.listing-comments__item')).toHaveCount(2)
  await expect(comments.getByRole('link', { name: 'ورود برای ثبت دیدگاه', exact: true })).toBeVisible()
})

test('@blocks private page covers login empty error and confirmed-only unblock', async ({ page }) => {
  await page.goto('/me/blocks')
  await expect(page.getByRole('heading', { name: 'کاربران مسدودشده', exact: true })).toBeVisible()
  await expect(page.getByRole('main').getByRole('link', { name: 'ورود', exact: true })).toBeVisible()

  await applyStorage(page, readFixtures().buyerStorage)
  await page.goto('/me/blocks')
  await expect(page.getByText('هنوز کاربری را مسدود نکرده‌اید.', { exact: true })).toBeVisible()
  const admin = await me('admin')
  await mutateBlock('buyer', admin.public_id, 'PUT')

  await page.goto('/me')
  const endpoint = '**/api/v1/me/blocks**'
  await page.route(endpoint, async (route) => {
    if (route.request().method() !== 'GET') {
      await route.continue()
      return
    }
    await route.fulfill({
      status: 500,
      contentType: 'application/json',
      body: '{"error":{"code":"internal","message":"fixture"}}',
    })
  })
  await page.getByRole('link', { name: 'مدیریت مسدودشدگان', exact: true }).click()
  await expect(page.getByText('خواندن فهرست مسدودشدگان ممکن نشد؛ دوباره تلاش کنید.', { exact: true })).toBeVisible()
  await page.unroute(endpoint)
  await page.getByRole('button', { name: 'تلاش دوباره', exact: true }).click()
  const item = page.locator(`[data-block-id="${admin.public_id}"]`)
  await expect(item).toBeVisible()

  await page.route(`**/api/v1/me/blocks/${admin.public_id}`, async (route) => {
    await route.fulfill({
      status: 500,
      contentType: 'application/json',
      body: '{"error":{"code":"internal","message":"fixture"}}',
    })
  })
  await item.getByRole('button', { name: 'رفع مسدودسازی', exact: true }).click()
  await expect(page.getByText('رفع مسدودسازی انجام نشد؛ دوباره تلاش کنید.', { exact: true })).toBeVisible()
  await expect(item).toBeVisible()
  await page.unroute(`**/api/v1/me/blocks/${admin.public_id}`)
  await item.getByRole('button', { name: 'رفع مسدودسازی', exact: true }).click()
  await expect(item).toHaveCount(0)
  await expect(page.getByText('هنوز کاربری را مسدود نکرده‌اید.', { exact: true })).toBeVisible()
})

test('@blocks forward retry keeps confirmed items and survives deletion of the cursor anchor', async ({ page }) => {
  const buyer = await me('buyer')
  const users = Array.from({ length: 21 }, (_, index) => ({
    publicId: randomBytes(13).toString('hex').toUpperCase(),
    phone: `+9894${String(index).padStart(8, '0')}`,
    name: `مسدود ${String(index + 1).padStart(2, '0')}`,
  }))
  const values = users.map((user) => `('${user.publicId}', '${user.phone}', '${user.name}', NOW(3), NOW(3))`).join(',')
  const ids = users.map((user) => `'${user.publicId}'`).join(',')
  await runSql(`
    INSERT INTO users (public_id, phone_e164, display_name, created_at, updated_at) VALUES ${values};
    INSERT INTO blocks (blocker_id, blocked_id, created_at)
    SELECT buyer.id, blocked.id, '2026-10-25 00:00:00.000'
    FROM users buyer JOIN users blocked ON blocked.public_id IN (${ids})
    WHERE buyer.public_id = '${buyer.public_id}';
  `)
  await applyStorage(page, readFixtures().buyerStorage)
  await page.goto('/me/blocks')
  await expect(page.locator('[data-block-id]')).toHaveCount(20)

  const api = await apiAs('buyer')
  const firstResponse = await api.get('/api/v1/me/blocks')
  const first = await firstResponse.json() as { data: BlockFixture[] }
  const anchor = first.data[19]
  expect(anchor).toBeTruthy()
  await api.dispose()
  await mutateBlock('buyer', anchor!.public_id, 'DELETE')

  const endpoint = '**/api/v1/me/blocks?*'
  await page.route(endpoint, async (route) => {
    if (!new URL(route.request().url()).searchParams.has('cursor')) {
      await route.continue()
      return
    }
    await route.fulfill({
      status: 500,
      contentType: 'application/json',
      body: '{"error":{"code":"internal","message":"fixture"}}',
    })
  })
  await page.getByRole('button', { name: 'نمایش موارد بیشتر', exact: true }).click()
  await expect(page.getByText('خواندن موارد بیشتر ممکن نشد؛ دوباره تلاش کنید.', { exact: true })).toBeVisible()
  await expect(page.locator('[data-block-id]')).toHaveCount(20)
  await page.unroute(endpoint)
  await page.getByRole('button', { name: 'تلاش دوباره بارگذاری', exact: true }).click()
  await expect(page.locator('[data-block-id]')).toHaveCount(21)
  const idsOnPage = await page.locator('[data-block-id]').evaluateAll((items) => items.map((item) => item.getAttribute('data-block-id')))
  expect(new Set(idsOnPage).size).toBe(21)
})
