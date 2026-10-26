import { readFileSync } from 'node:fs'
import { expect, test, type BrowserContext, type Page } from '@playwright/test'
import { runSql } from './support/database'
import {
  apiAs,
  createListing,
  readFixtures,
  type FixtureRole,
} from './support/stack'

interface CommentFixture {
  public_id: string
  body: string
  author: { public_id: string; display_name: string | null }
  created_at: string
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

async function createComment(
  role: FixtureRole,
  slug: string,
  body: string,
): Promise<CommentFixture> {
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

async function deleteComment(role: FixtureRole, publicId: string): Promise<void> {
  const api = await apiAs(role)
  try {
    const response = await api.delete(`/api/v1/comments/${publicId}`)
    expect(response.ok(), `${role} deletes comment`).toBe(true)
  } finally {
    await api.dispose()
  }
}

test('@comments public SSR is newest-first, escaped, and free of viewer controls or hydration refetch', async ({ page }) => {
  const created = await createListing({ title: 'دیدگاه‌های امن عمومی', state: 'active' })
  const unsafe = '<img data-comment-xss src=x onerror=alert(1)>\nخط دوم'
  await createComment('buyer', created.slug, unsafe)
  await createComment('admin', created.slug, 'دیدگاه تازه‌تر')

  const response = await page.request.get(detailPath(created.slug))
  expect(response.ok()).toBe(true)
  const html = await response.text()
  expect(html).toContain('&lt;img data-comment-xss')
  expect(html).not.toContain('<img data-comment-xss')
  expect(html).not.toContain('حذف دیدگاه')
  expect(html).not.toContain('دیدگاه شما')

  const commentReads: string[] = []
  const pageErrors: string[] = []
  page.on('request', (request) => {
    const url = new URL(request.url())
    if (url.pathname.endsWith('/comments')) commentReads.push(request.url())
  })
  page.on('pageerror', (error) => pageErrors.push(error.message))
  await page.goto(detailPath(created.slug))
  const comments = page.locator('.listing-comments')
  await expect(comments.locator('.listing-comments__item')).toHaveCount(2)
  await expect(comments.locator('.listing-comments__item').first()).toContainText('دیدگاه تازه‌تر')
  const unsafeItem = comments.locator('.listing-comments__item').nth(1)
  await expect(unsafeItem.locator('.listing-comments__body')).toContainText(unsafe)
  await expect(unsafeItem.locator('.listing-comments__body')).toHaveCSS('white-space', 'pre-wrap')
  await expect(unsafeItem.locator('img')).toHaveCount(0)
  await expect(comments.locator('.listing-comments__count')).toHaveText('۲ دیدگاه')
  await expect(comments.getByRole('textbox')).toHaveCount(0)
  await expect(comments.getByRole('button', { name: 'حذف دیدگاه', exact: true })).toHaveCount(0)
  await expect(comments.getByRole('link', { name: 'ورود برای ثبت دیدگاه', exact: true })).toBeVisible()
  expect(commentReads).toEqual([])
  expect(pageErrors).toEqual([])
})

test('@comments create keeps a failed draft and prepends only confirmed responses with exact counts', async ({ page }) => {
  const created = await createListing({ title: 'ایجاد دیدگاه با تلاش دوباره', state: 'active' })
  await applyStorage(page, readFixtures().buyerStorage)
  await page.goto(detailPath(created.slug))
  const comments = page.locator('.listing-comments')
  const form = comments.locator('form')
  const textarea = form.getByRole('textbox', { name: 'دیدگاه شما', exact: true })
  await expect(comments.getByText('هنوز دیدگاهی ثبت نشده است', { exact: true })).toBeVisible()
  await expect(comments.locator('.listing-comments__count')).toHaveText('۰ دیدگاه')

  const draft = '  خط اول 😀\nخط دوم  '
  await textarea.fill(draft)
  const characterCount = [...draft].length.toLocaleString('fa-IR')
  await expect(form.locator('.listing-comments__character-count')).toHaveText(`${characterCount} / ۱۰۰۰ نویسه`)
  const endpoint = '**/api/v1/listings/*/comments'
  let failures = 0
  await page.route(endpoint, async (route) => {
    if (route.request().method() !== 'POST') {
      await route.continue()
      return
    }
    failures += 1
    await route.fulfill({
      status: 500,
      contentType: 'application/json',
      body: '{"error":{"code":"internal","message":"fixture"}}',
    })
  })
  await form.getByRole('button', { name: 'ثبت دیدگاه', exact: true }).click()
  const error = form.getByText('ثبت دیدگاه انجام نشد؛ دوباره تلاش کنید.', { exact: true })
  await expect(error).toBeVisible()
  await expect(error).toBeFocused()
  await expect(textarea).toHaveValue(draft)
  await expect(comments.locator('.listing-comments__item')).toHaveCount(0)
  await form.getByRole('button', { name: 'تلاش دوباره', exact: true }).click()
  await expect(textarea).toHaveValue(draft)
  expect(failures).toBe(2)

  await page.unroute(endpoint)
  await form.getByRole('button', { name: 'تلاش دوباره', exact: true }).click()
  await expect(textarea).toHaveValue('')
  await expect(comments.locator('.listing-comments__item')).toHaveCount(1)
  await expect(comments.locator('.listing-comments__body').first()).toHaveText(draft.trim())
  await expect(comments.locator('.listing-comments__count')).toHaveText('۱ دیدگاه')

  await textarea.fill('دیدگاه تازه‌تر در ابتدای فهرست')
  await form.getByRole('button', { name: 'ثبت دیدگاه', exact: true }).click()
  await expect(comments.locator('.listing-comments__item')).toHaveCount(2)
  await expect(comments.locator('.listing-comments__body').first()).toHaveText('دیدگاه تازه‌تر در ابتدای فهرست')
  await expect(comments.locator('.listing-comments__count')).toHaveText('۲ دیدگاه')
})

test('@comments delete retry and both mutation 401s preserve confirmed items and drafts', async ({ page }) => {
  const created = await createListing({ title: 'حذف دیدگاه و نشست منقضی', state: 'active' })
  const own = await createComment('buyer', created.slug, 'دیدگاه قابل حذف نویسنده')
  await createComment('admin', created.slug, 'دیدگاه کاربر دیگر')
  await applyStorage(page, readFixtures().buyerStorage)
  await page.goto(detailPath(created.slug))
  const comments = page.locator('.listing-comments')
  const ownItem = comments.locator(`[data-comment-id="${own.public_id}"]`)
  const otherItem = comments.locator('.listing-comments__item').filter({ hasText: 'دیدگاه کاربر دیگر' })
  await expect(ownItem.getByRole('button', { name: 'حذف دیدگاه', exact: true })).toBeVisible()
  await expect(otherItem.getByRole('button', { name: 'حذف دیدگاه', exact: true })).toHaveCount(0)

  const deleteEndpoint = `**/api/v1/comments/${own.public_id}`
  await page.route(deleteEndpoint, async (route) => {
    await route.fulfill({
      status: 500,
      contentType: 'application/json',
      body: '{"error":{"code":"internal","message":"fixture"}}',
    })
  })
  await ownItem.getByRole('button', { name: 'حذف دیدگاه', exact: true }).click()
  await expect(comments.getByText('حذف دیدگاه انجام نشد؛ دوباره تلاش کنید.', { exact: true })).toBeVisible()
  await expect(ownItem).toBeVisible()
  await expect(comments.locator('.listing-comments__count')).toHaveText('۲ دیدگاه')
  await page.unroute(deleteEndpoint)
  await comments.getByRole('button', { name: 'تلاش دوباره حذف', exact: true }).click()
  await expect(ownItem).toHaveCount(0)
  await expect(comments.locator('.listing-comments__count')).toHaveText('۱ دیدگاه')

  const postEndpoint = '**/api/v1/listings/*/comments'
  const textarea = comments.getByRole('textbox', { name: 'دیدگاه شما', exact: true })
  const draft = 'پیش‌نویس بعد از انقضای نشست'
  await textarea.fill(draft)
  await page.route(postEndpoint, async (route) => {
    if (route.request().method() !== 'POST') {
      await route.continue()
      return
    }
    await route.fulfill({
      status: 401,
      contentType: 'application/json',
      body: '{"error":{"code":"unauthorized","message":"fixture"}}',
    })
  })
  await comments.getByRole('button', { name: 'ثبت دیدگاه', exact: true }).click()
  await expect(comments.getByRole('link', { name: 'ورود برای ثبت دیدگاه', exact: true })).toBeVisible()
  await expect(comments.getByRole('status')).toHaveText('نشست شما منقضی شده است؛ دوباره وارد شوید.')
  expect(await textarea.inputValue()).toBe(draft)
  await expect(comments.locator('.listing-comments__item')).toHaveCount(1)

  await page.unroute(postEndpoint)
  await page.reload()
  const replacement = comments.getByRole('textbox', { name: 'دیدگاه شما', exact: true })
  await replacement.fill('دیدگاه تازه برای ۴۰۱ حذف')
  await comments.getByRole('button', { name: 'ثبت دیدگاه', exact: true }).click()
  await expect(comments.locator('.listing-comments__item')).toHaveCount(2)
  const newest = comments.locator('.listing-comments__item').first()
  const newestId = await newest.getAttribute('data-comment-id')
  expect(newestId).toBeTruthy()
  await page.route(`**/api/v1/comments/${newestId}`, async (route) => {
    await route.fulfill({
      status: 401,
      contentType: 'application/json',
      body: '{"error":{"code":"unauthorized","message":"fixture"}}',
    })
  })
  await newest.getByRole('button', { name: 'حذف دیدگاه', exact: true }).click()
  await expect(newest).toBeVisible()
  await expect(comments.locator('.listing-comments__count')).toHaveText('۲ دیدگاه')
  await expect(comments.getByRole('link', { name: 'ورود برای ثبت دیدگاه', exact: true })).toBeVisible()
})

test('@comments load-more retry keeps confirmed cursor after equal-time anchor deletion', async ({ page }) => {
  const created = await createListing({ title: 'صفحه‌بندی دیدگاه‌های هم‌زمان', state: 'active' })
  for (let index = 1; index <= 21; index += 1) {
    await createComment('buyer', created.slug, `دیدگاه صفحه ${String(index).padStart(2, '0')}`)
  }
  await runSql(`
    UPDATE listing_comments SET created_at = '2026-10-25 00:00:00.000'
    WHERE listing_id = (SELECT id FROM listings WHERE public_id = '${created.publicId}')
  `)
  await page.goto(detailPath(created.slug))
  const comments = page.locator('.listing-comments')
  await expect(comments.locator('.listing-comments__item')).toHaveCount(20)
  await expect(comments.getByText('دیدگاه صفحه 02', { exact: true })).toBeVisible()
  await expect(comments.getByText('دیدگاه صفحه 01', { exact: true })).toHaveCount(0)

  const firstPage = await page.request.get(`/api/v1/listings/${encodeURIComponent(created.slug)}/comments`)
  const firstBody = await firstPage.json() as { data: CommentFixture[] }
  const deletedAnchor = firstBody.data[19]
  expect(deletedAnchor?.body).toBe('دیدگاه صفحه 02')
  await deleteComment('buyer', deletedAnchor!.public_id)

  const endpoint = '**/api/v1/listings/*/comments?*'
  await page.route(endpoint, async (route) => {
    if (route.request().method() !== 'GET' || !new URL(route.request().url()).searchParams.has('cursor')) {
      await route.continue()
      return
    }
    await route.fulfill({
      status: 500,
      contentType: 'application/json',
      body: '{"error":{"code":"internal","message":"fixture"}}',
    })
  })
  await comments.getByRole('button', { name: 'نمایش دیدگاه‌های بیشتر', exact: true }).click()
  await expect(comments.getByText('بارگذاری دیدگاه‌های بیشتر ممکن نشد؛ دوباره تلاش کنید.', { exact: true })).toBeVisible()
  await expect(comments.locator('.listing-comments__item')).toHaveCount(20)
  await page.unroute(endpoint)
  await comments.getByRole('button', { name: 'تلاش دوباره بارگذاری', exact: true }).click()
  await expect(comments.locator('.listing-comments__item')).toHaveCount(21)
  await expect(comments.getByText('دیدگاه صفحه 01', { exact: true })).toBeVisible()
  const ids = await comments.locator('.listing-comments__item').evaluateAll((items) => items.map((item) => item.getAttribute('data-comment-id')))
  expect(new Set(ids).size).toBe(ids.length)

  await page.reload()
  await expect(comments.locator('.listing-comments__item')).toHaveCount(20)
  await expect(comments.getByText('دیدگاه صفحه 02', { exact: true })).toHaveCount(0)
  await expect(comments.getByText('دیدگاه صفحه 01', { exact: true })).toBeVisible()
  await expect(comments.locator('.listing-comments__count')).toHaveText('۲۰ دیدگاه')
})

test('@comments initial read failure is nonfatal and retry preserves the authenticated draft', async ({ page }) => {
  const title = `جزئیات سالم با خطای دیدگاه ${Date.now()}`
  const created = await createListing({ title, state: 'active' })
  await createComment('buyer', created.slug, 'دیدگاه بازیابی‌شده')
  await applyStorage(page, readFixtures().buyerStorage)
  await page.goto(`/?q=${encodeURIComponent(title)}`)
  await expect(page.locator('.listing-card__title')).toHaveText(title)
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
  await page.locator('.listing-card__link').click()
  await expect(page.getByRole('heading', { name: title, exact: true })).toBeVisible()
  const comments = page.locator('.listing-comments')
  await expect(comments.getByText('بارگذاری دیدگاه‌ها ممکن نشد؛ دوباره تلاش کنید.', { exact: true })).toBeVisible()
  const draft = 'پیش‌نویس هنگام بازیابی فهرست'
  const textarea = comments.getByRole('textbox', { name: 'دیدگاه شما', exact: true })
  await textarea.fill(draft)
  await page.unroute(endpoint)
  await comments.getByRole('button', { name: 'تلاش دوباره', exact: true }).click()
  await expect(comments.getByText('دیدگاه بازیابی‌شده', { exact: true })).toBeVisible()
  await expect(textarea).toHaveValue(draft)
  await expect(comments.locator('.listing-comments__count')).toHaveText('۱ دیدگاه')
})

