import { randomBytes } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { expect, test, type BrowserContext, type Page } from '@playwright/test'
import { cleanupE2eModeration, runSql } from './support/database'
import { apiAs, createListing, readFixtures } from './support/stack'

const PENDING_NOTICE = 'دیدگاه شما ثبت شد و پس از بررسی نمایش داده می‌شود.'

async function applyStorage(page: Page, storagePath: string): Promise<void> {
  const state = JSON.parse(readFileSync(storagePath, 'utf8')) as {
    cookies: Parameters<BrowserContext['addCookies']>[0]
  }
  await page.context().addCookies(state.cookies)
}

function detailPath(slug: string): string {
  return `/listings/${encodeURIComponent(slug)}`
}

async function createRule(): Promise<{ public_id: string; phrase: string }> {
  const api = await apiAs('admin')
  try {
    const phrase = `e2epending${randomBytes(5).toString('hex')}`
    const response = await api.post('/api/v1/admin/moderation/rules', {
      data: { phrase, match_mode: 'compact', reason: 'آزمون مرورگر دیدگاه' },
    })
    expect(response.status(), await response.text()).toBe(200)
    return await response.json() as { public_id: string; phrase: string }
  } finally {
    await api.dispose()
  }
}

async function createCleanComment(slug: string, body: string): Promise<void> {
  const api = await apiAs('buyer')
  try {
    const response = await api.post(`/api/v1/listings/${encodeURIComponent(slug)}/comments`, {
      data: { body },
    })
    expect(response.status(), await response.text()).toBe(200)
  } finally {
    await api.dispose()
  }
}

test.afterEach(async () => {
  await cleanupE2eModeration()
})

test('@comment-moderation clean 200 and pending 202 have exact confirmed effects and privacy', async ({ page }) => {
  const created = await createListing({ title: 'ثبت دیدگاه سالم و در انتظار', state: 'active' })
  const rule = await createRule()
  await applyStorage(page, readFixtures().buyerStorage)
  await page.goto(detailPath(created.slug))
  const comments = page.locator('.listing-comments')
  const textarea = comments.getByRole('textbox', { name: 'دیدگاه شما', exact: true })
  const submit = comments.getByRole('button', { name: 'ثبت دیدگاه', exact: true })
  const endpoint = `**/api/v1/listings/${encodeURIComponent(created.slug)}/comments`

  const cleanBody = 'دیدگاه سالم و عمومی'
  await textarea.fill(cleanBody)
  const cleanResponsePromise = page.waitForResponse((response) => (
    response.url().includes(`/listings/${encodeURIComponent(created.slug)}/comments`)
      && response.request().method() === 'POST'
  ))
  await submit.click()
  const cleanResponse = await cleanResponsePromise
  expect(cleanResponse.status()).toBe(200)
  const clean = await cleanResponse.json() as Record<string, unknown>
  expect(Object.keys(clean).sort()).toEqual(['author', 'body', 'created_at', 'public_id'])
  await expect(comments.locator('.listing-comments__item')).toHaveCount(1)
  await expect(comments.locator('.listing-comments__count')).toHaveText('۱ دیدگاه')
  await expect(comments.getByRole('status')).toHaveText('دیدگاه شما ثبت شد.')

  const pendingBody = `${rule.phrase} <img data-pending-xss src=x onerror=alert(1)>`
  await textarea.fill(pendingBody)
  const pendingResponsePromise = page.waitForResponse((response) => (
    response.url().includes(`/listings/${encodeURIComponent(created.slug)}/comments`)
      && response.request().method() === 'POST'
  ))
  await submit.click()
  const pendingResponse = await pendingResponsePromise
  expect(pendingResponse.status()).toBe(202)
  const pending = await pendingResponse.json() as Record<string, unknown>
  expect(Object.keys(pending).sort()).toEqual(['created_at', 'public_id', 'status'])
  expect(pending.status).toBe('pending_review')
  expect(pending.body).toBeUndefined()
  await expect(textarea).toHaveValue('')
  await expect(comments.getByRole('status')).toHaveText(PENDING_NOTICE)
  await expect(comments.locator('.listing-comments__item')).toHaveCount(1)
  await expect(comments.locator('.listing-comments__body').first()).toHaveText(cleanBody)
  await expect(comments.locator('.listing-comments__count')).toHaveText('۱ دیدگاه')
  await expect(comments.getByText(rule.phrase, { exact: false })).toHaveCount(0)
  await expect(comments.locator('img[data-pending-xss]')).toHaveCount(0)
  expect(await runSql(`SELECT comment_count FROM listings WHERE public_id = '${created.publicId}'`)).toBe('1')

  await page.context().clearCookies()
  const htmlResponse = await page.request.get(detailPath(created.slug))
  const html = await htmlResponse.text()
  expect(htmlResponse.ok()).toBe(true)
  expect(html).toContain(cleanBody)
  expect(html).not.toContain(rule.phrase)
  expect(html).not.toContain(String(pending.public_id))
  expect(html).not.toContain('pending_review')
  const publicResponse = await page.request.get(endpoint.replace('**', ''))
  expect(publicResponse.ok()).toBe(true)
  const publicPage = await publicResponse.json() as { data: Record<string, unknown>[] }
  expect(publicPage.data).toHaveLength(1)
  expect(publicPage.data[0]?.body).toBe(cleanBody)
  expect(Object.keys(publicPage.data[0] ?? {}).sort()).toEqual(['author', 'body', 'created_at', 'public_id'])
  expect(JSON.stringify(publicPage)).not.toContain(rule.phrase)
  expect(JSON.stringify(publicPage)).not.toContain('pending_review')

  await page.reload()
  await expect(comments.locator('.listing-comments__item')).toHaveCount(1)
  await expect(comments.getByText(rule.phrase, { exact: false })).toHaveCount(0)
  await expect(comments.locator('.listing-comments__count')).toHaveText('۱ دیدگاه')
})

test('@comment-moderation pending submission prevents duplicate posts until the confirmed response', async ({ page }) => {
  const created = await createListing({ title: 'قفل ارسال تکراری دیدگاه', state: 'active' })
  const rule = await createRule()
  await applyStorage(page, readFixtures().buyerStorage)
  await page.goto(detailPath(created.slug))
  const comments = page.locator('.listing-comments')
  const form = comments.locator('form')
  const textarea = form.getByRole('textbox', { name: 'دیدگاه شما', exact: true })
  const submit = form.locator('button[type="submit"]')
  const endpoint = '**/api/v1/listings/*/comments'
  let releaseRequest!: () => void
  let markSeen!: () => void
  const seen = new Promise<void>((resolve) => { markSeen = resolve })
  const gate = new Promise<void>((resolve) => { releaseRequest = resolve })
  let postCount = 0
  await page.route(endpoint, async (route) => {
    if (route.request().method() !== 'POST') {
      await route.continue()
      return
    }
    postCount += 1
    markSeen()
    await gate
    await route.continue()
  })

  await textarea.fill(rule.phrase)
  await submit.click()
  await seen
  await expect(textarea).toBeDisabled()
  await expect(submit).toBeDisabled()
  await form.evaluate((element: HTMLFormElement) => {
    element.requestSubmit()
    element.requestSubmit()
  })
  releaseRequest()
  await expect(comments.getByRole('status')).toHaveText(PENDING_NOTICE)
  await expect(textarea).toHaveValue('')
  await expect(comments.locator('.listing-comments__item')).toHaveCount(0)
  await expect(comments.locator('.listing-comments__count')).toHaveText('۰ دیدگاه')
  expect(postCount).toBe(1)
  expect(await runSql(`SELECT COUNT(*) FROM listing_comments WHERE body = '${rule.phrase}'`)).toBe('1')
  expect(await runSql(`SELECT COUNT(*) FROM comment_moderation_cases mc JOIN listing_comments c ON c.id = mc.comment_id WHERE c.body = '${rule.phrase}'`)).toBe('1')
})

test('@comment-moderation 401 429 500 and network failures preserve draft and confirmed comments', async ({ page }) => {
  const created = await createListing({ title: 'حفظ پیش‌نویس در خطاهای ثبت', state: 'active' })
  const cleanBody = 'دیدگاه تأییدشدهٔ قبلی'
  await createCleanComment(created.slug, cleanBody)
  const rule = await createRule()
  await applyStorage(page, readFixtures().buyerStorage)
  await page.goto(detailPath(created.slug))
  const comments = page.locator('.listing-comments')
  const endpoint = '**/api/v1/listings/*/comments'

  for (const scenario of [
    { status: 401, message: 'نشست شما منقضی شده است؛ دوباره وارد شوید.' },
    { status: 429, message: 'تعداد دیدگاه‌های ارسالی در این بازه زیاد است؛ بعداً تلاش کنید.' },
    { status: 500, message: 'ثبت دیدگاه انجام نشد؛ دوباره تلاش کنید.' },
    { status: 0, message: 'ثبت دیدگاه انجام نشد؛ دوباره تلاش کنید.' },
  ]) {
    const draft = `${rule.phrase} پیش‌نویس ${scenario.status}`
    const textarea = comments.getByRole('textbox', { name: 'دیدگاه شما', exact: true })
    await textarea.fill(draft)
    await page.route(endpoint, async (route) => {
      if (route.request().method() !== 'POST') {
        await route.continue()
        return
      }
      if (scenario.status === 0) {
        await route.abort('failed')
      } else {
        await route.fulfill({
          status: scenario.status,
          contentType: 'application/json',
          body: '{"error":{"code":"fixture","message":"fixture"}}',
        })
      }
    })
    await comments.getByRole('button', { name: 'ثبت دیدگاه', exact: true }).click()
    await expect(comments.getByText(scenario.message, { exact: true })).toBeVisible()
    await expect(textarea).toHaveValue(draft)
    await expect(comments.locator('.listing-comments__item')).toHaveCount(1)
    await expect(comments.locator('.listing-comments__body').first()).toHaveText(cleanBody)
    await expect(comments.locator('.listing-comments__count')).toHaveText('۱ دیدگاه')
    await page.unroute(endpoint)
    await page.reload()
  }
})
