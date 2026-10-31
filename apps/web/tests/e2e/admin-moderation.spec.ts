import { randomBytes } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { expect, test, type BrowserContext, type Page } from '@playwright/test'
import { cleanupE2eModeration, runSql } from './support/database'
import { apiAs, createListing, readFixtures, type FixtureRole } from './support/stack'

type MatchMode = 'compact' | 'phrase'
interface RuleFixture {
  public_id: string
  phrase: string
  match_mode: MatchMode
  is_active: boolean
  revision: number
}

async function applyStorage(page: Page, storagePath: string): Promise<void> {
  const state = JSON.parse(readFileSync(storagePath, 'utf8')) as {
    cookies: Parameters<BrowserContext['addCookies']>[0]
  }
  await page.context().addCookies(state.cookies)
}

async function createRule(phrase?: string, matchMode: MatchMode = 'compact'): Promise<RuleFixture> {
  const api = await apiAs('admin')
  try {
    const response = await api.post('/api/v1/admin/moderation/rules', {
      data: {
        phrase: phrase ?? `e2e${randomBytes(6).toString('hex')}`,
        match_mode: matchMode,
        reason: 'آزمون مدیریت قوانین',
      },
    })
    expect(response.status(), await response.text()).toBe(200)
    return await response.json() as RuleFixture
  } finally {
    await api.dispose()
  }
}

async function patchRule(publicId: string, patch: Record<string, unknown>): Promise<RuleFixture> {
  const api = await apiAs('admin')
  try {
    const response = await api.patch(`/api/v1/admin/moderation/rules/${publicId}`, {
      data: { ...patch, reason: 'تغییر fixture قانون' },
    })
    expect(response.status(), await response.text()).toBe(200)
    return await response.json() as RuleFixture
  } finally {
    await api.dispose()
  }
}

async function createComments(
  slug: string,
  bodies: string[],
  expectedStatus: number,
  role: FixtureRole = 'buyer',
): Promise<Array<{ public_id: string }>> {
  const api = await apiAs(role)
  try {
    const created: Array<{ public_id: string }> = []
    for (const body of bodies) {
      const response = await api.post(`/api/v1/listings/${encodeURIComponent(slug)}/comments`, {
        data: { body },
      })
      expect(response.status(), await response.text()).toBe(expectedStatus)
      created.push(await response.json() as { public_id: string })
    }
    return created
  } finally {
    await api.dispose()
  }
}

async function deleteComment(publicId: string): Promise<void> {
  const api = await apiAs('buyer')
  try {
    const response = await api.delete(`/api/v1/comments/${publicId}`)
    expect(response.status(), await response.text()).toBe(200)
  } finally {
    await api.dispose()
  }
}

async function createCommentReport(commentId: string): Promise<{ public_id: string }> {
  const api = await apiAs('buyerTwo')
  try {
    const response = await api.post('/api/v1/reports', {
      data: {
        target_type: 'comment',
        target_id: commentId,
        reason: 'offensive',
        details: 'گزارش fixture تداخل قانون',
      },
    })
    expect(response.status(), await response.text()).toBe(200)
    return await response.json() as { public_id: string }
  } finally {
    await api.dispose()
  }
}

async function hideReportedComment(reportId: string): Promise<void> {
  const api = await apiAs('admin')
  try {
    const response = await api.post(`/api/v1/admin/reports/${reportId}/actions`, {
      data: { action: 'hide_comment', reason: 'پنهان‌سازی fixture' },
    })
    expect(response.status(), await response.text()).toBe(200)
  } finally {
    await api.dispose()
  }
}

test.afterEach(async () => {
  await cleanupE2eModeration()
})

test('@admin-moderation gates navigation and active filter case suppress report restore', async ({ page }) => {
  await page.goto('/admin/moderation')
  await expect(page.getByText('دسترسی ندارید (ویژهٔ مدیر).', { exact: true })).toBeVisible()
  await applyStorage(page, readFixtures().buyerStorage)
  await page.goto('/admin/moderation/rules')
  await expect(page.getByText('دسترسی ندارید (ویژهٔ مدیر).', { exact: true })).toBeVisible()
  await applyStorage(page, readFixtures().adminStorage)
  await page.goto('/me')
  const navigation = page.getByRole('navigation', { name: 'ناوبری مدیریت' })
  await expect(navigation.getByRole('link', { name: 'مدیریت دیدگاه', exact: true })).toHaveAttribute('href', '/admin/moderation')
  await expect(navigation.getByRole('link', { name: 'قوانین دیدگاه', exact: true })).toHaveAttribute('href', '/admin/moderation/rules')
  await expect(page.locator('.shell__nav').getByRole('link', { name: 'دیدگاه‌ها', exact: true })).toHaveAttribute('href', '/admin/moderation')
  await expect(page.locator('.shell__nav').getByRole('link', { name: 'قوانین', exact: true })).toHaveAttribute('href', '/admin/moderation/rules')

  const created = await createListing({ title: 'تداخل گزارش و قانون', state: 'active' })
  const phrase = `e2e${randomBytes(6).toString('hex')}`
  const [comment] = await createComments(created.slug, [phrase], 200)
  const report = await createCommentReport(comment.public_id)
  await hideReportedComment(report.public_id)
  const rule = await createRule(phrase)
  await runSql(`
    INSERT INTO comment_moderation_cases
      (comment_id, status, matched_rules, flagged_at, updated_at)
    SELECT id, 'pending', JSON_ARRAY(JSON_OBJECT(
      'public_id', '${rule.public_id}', 'revision', 1,
      'phrase', '${rule.phrase}', 'match_mode', 'compact'
    )), NOW(3), NOW(3)
    FROM listing_comments WHERE public_id = '${comment.public_id}'
  `)
  await page.goto('/admin/reports')
  await page.getByRole('button', { name: 'اقدام‌شده', exact: true }).click()
  const row = page.locator(`[data-report-id="${report.public_id}"]`)
  await expect(row).toBeVisible()
  await expect(row.locator('[data-action="restore_comment"]')).toHaveCount(0)
})

test('@admin-moderation rule CRUD is XSS safe validates reasons and preserves failed drafts', async ({ page }) => {
  await applyStorage(page, readFixtures().adminStorage)
  await page.goto('/admin/moderation/rules')
  await expect(page.getByRole('heading', { name: 'قوانین دیدگاه', exact: true })).toBeVisible()
  await expect(page.getByText('پس از هر تغییر، بررسی کامل دیدگاه‌های موجود را دوباره اجرا کنید.', { exact: true })).toBeVisible()
  const form = page.locator('.admin-rule-create')
  const phraseInput = form.getByLabel('عبارت جدید', { exact: true })
  const reasonInput = form.getByLabel('دلیل ایجاد', { exact: true })
  const unsafe = 'e2e<img src=x onerror="window.__adminRuleXss = true">'
  await phraseInput.fill(unsafe)
  await form.getByRole('button', { name: 'افزودن قانون', exact: true }).click()
  await expect(form.getByText('دلیل ایجاد الزامی است.', { exact: true })).toBeVisible()
  await reasonInput.fill('ایجاد قانون امن')

  let markSeen!: () => void
  let releaseRequest!: () => void
  const seen = new Promise<void>((resolve) => { markSeen = resolve })
  const gate = new Promise<void>((resolve) => { releaseRequest = resolve })
  let createPosts = 0
  await page.route('**/api/v1/admin/moderation/rules', async (route) => {
    if (route.request().method() !== 'POST') {
      await route.continue()
      return
    }
    createPosts += 1
    markSeen()
    await gate
    await route.fulfill({ status: 500, contentType: 'application/json', body: '{"error":{"code":"fixture","message":"fixture"}}' })
  })
  const createButton = form.getByRole('button', { name: 'افزودن قانون', exact: true })
  await createButton.click()
  await seen
  await form.evaluate((element: HTMLFormElement) => {
    element.requestSubmit()
    element.requestSubmit()
  })
  releaseRequest()
  await expect(form.getByText('ثبت قانون انجام نشد؛ دوباره تلاش کنید.', { exact: true })).toBeVisible()
  expect(createPosts).toBe(1)
  await expect(phraseInput).toHaveValue(unsafe)
  await expect(reasonInput).toHaveValue('ایجاد قانون امن')
  await page.unroute('**/api/v1/admin/moderation/rules')

  const createdResponsePromise = page.waitForResponse((response) => (
    response.url().endsWith('/api/v1/admin/moderation/rules') && response.request().method() === 'POST'
  ))
  await createButton.click()
  const created = await (await createdResponsePromise).json() as RuleFixture
  const row = page.locator(`[data-rule-id="${created.public_id}"]`)
  await expect(row).toBeVisible()
  await expect(row.locator('.admin-rule__phrase')).toHaveText(unsafe)
  await expect(row.locator('img')).toHaveCount(0)
  expect(await page.evaluate(() => (window as unknown as { __adminRuleXss?: boolean }).__adminRuleXss)).toBeUndefined()
  await expect(phraseInput).toHaveValue('')
  await expect(reasonInput).toHaveValue('')

  await row.getByRole('button', { name: 'ویرایش', exact: true }).click()
  const edited = 'e2eedited<img src=x onerror="window.__adminRuleXss = true">'
  const editPhrase = row.getByLabel('عبارت قانون', { exact: true })
  const editReason = row.getByLabel('دلیل ویرایش', { exact: true })
  await editPhrase.fill(edited)
  await row.getByLabel('حالت ویرایش', { exact: true }).selectOption('phrase')
  await editReason.fill('ویرایش امن قانون')
  await page.route(`**/api/v1/admin/moderation/rules/${created.public_id}`, async (route) => {
    await route.fulfill({ status: 500, contentType: 'application/json', body: '{"error":{"code":"fixture","message":"fixture"}}' })
  })
  await row.getByRole('button', { name: 'ذخیره تغییرات', exact: true }).click()
  await expect(row.getByText('ویرایش قانون انجام نشد؛ دوباره تلاش کنید.', { exact: true })).toBeVisible()
  await expect(editPhrase).toHaveValue(edited)
  await expect(editReason).toHaveValue('ویرایش امن قانون')
  await expect(row.locator('.admin-rule__phrase')).toHaveText(unsafe)
  await page.unroute(`**/api/v1/admin/moderation/rules/${created.public_id}`)
  await row.getByRole('button', { name: 'ذخیره تغییرات', exact: true }).click()
  await expect(row.locator('.admin-rule__phrase')).toHaveText(edited)
  await expect(row.getByLabel('عبارت قانون', { exact: true })).toHaveCount(0)

  const toggleReason = row.getByLabel('دلیل تغییر وضعیت', { exact: true })
  await row.getByRole('button', { name: 'غیرفعال کردن', exact: true }).click()
  await expect(row.getByText('دلیل تغییر وضعیت الزامی است.', { exact: true })).toBeVisible()
  await toggleReason.fill('غیرفعال‌سازی آزمون')
  await row.getByRole('button', { name: 'غیرفعال کردن', exact: true }).click()
  await expect(row.getByRole('button', { name: 'فعال کردن', exact: true })).toBeVisible()
  expect(await runSql(`SELECT is_active FROM moderation_rules WHERE public_id = '${created.public_id}'`)).toBe('0')
})

test('@admin-moderation rule filters and pagination retain the confirmed cursor after failure', async ({ page }) => {
  const rules: RuleFixture[] = []
  for (let index = 0; index < 21; index += 1) rules.push(await createRule())
  await applyStorage(page, readFixtures().adminStorage)
  await page.goto('/admin/moderation/rules')
  await page.getByRole('button', { name: 'فعال', exact: true }).click()
  await expect(page.locator('.admin-rule')).toHaveCount(20)
  const anchor = await page.locator('.admin-rule').first().getAttribute('data-rule-id')
  expect(anchor).toBeTruthy()
  await patchRule(anchor!, { is_active: false })

  const cursorUrls: string[] = []
  let failCursorRead = true
  await page.route('**/api/v1/admin/moderation/rules?*', async (route) => {
    const url = new URL(route.request().url())
    if (!url.searchParams.has('cursor')) {
      await route.continue()
      return
    }
    cursorUrls.push(route.request().url())
    if (failCursorRead) {
      failCursorRead = false
      await route.fulfill({ status: 500, contentType: 'application/json', body: '{"error":{"code":"fixture","message":"fixture"}}' })
    } else {
      await route.continue()
    }
  })
  await page.getByRole('button', { name: 'نمایش قوانین بیشتر', exact: true }).click()
  await expect(page.getByText('خواندن قوانین بیشتر ممکن نشد؛ دوباره تلاش کنید.', { exact: true })).toBeVisible()
  await expect(page.locator('.admin-rule')).toHaveCount(20)
  await page.getByRole('button', { name: 'تلاش دوباره بارگذاری قوانین', exact: true }).click()
  await expect.poll(() => page.locator('.admin-rule').count()).toBeGreaterThan(20)
  const visibleIds = await page.locator('.admin-rule').evaluateAll((rows) => rows.map((row) => row.getAttribute('data-rule-id')))
  for (const rule of rules) expect(visibleIds).toContain(rule.public_id)
  expect(cursorUrls).toHaveLength(2)
  expect(cursorUrls[1]).toBe(cursorUrls[0])

  await page.getByRole('button', { name: 'غیرفعال', exact: true }).click()
  await expect(page.locator(`[data-rule-id="${anchor}"]`)).toBeVisible()
})

test('@admin-moderation queue paginates safely and applies only confirmed actions including deleted targets', async ({ page }) => {
  const created = await createListing({ title: 'صف دیدگاه‌های در انتظار', state: 'active' })
  const rule = await createRule()
  const unsafe = `${rule.phrase} <img src=x onerror="window.__adminQueueXss = true">`
  const bodies = [unsafe, ...Array.from({ length: 20 }, (_, index) => `${rule.phrase} دیدگاه ${index + 1}`)]
  const comments = await createComments(created.slug, bodies, 202)
  await deleteComment(comments[2]!.public_id)
  await applyStorage(page, readFixtures().adminStorage)
  await page.goto('/admin/moderation')
  await expect(page.locator('.admin-moderation-case')).toHaveCount(20)
  await page.getByRole('button', { name: 'نمایش دیدگاه‌های بیشتر', exact: true }).click()
  await expect(page.locator('.admin-moderation-case')).toHaveCount(21)

  const unsafeRow = page.locator(`[data-comment-id="${comments[0]!.public_id}"]`)
  await expect(unsafeRow).toContainText(unsafe)
  await expect(unsafeRow.locator('img')).toHaveCount(0)
  expect(await page.evaluate(() => (window as unknown as { __adminQueueXss?: boolean }).__adminQueueXss)).toBeUndefined()
  const deletedRow = page.locator(`[data-comment-id="${comments[2]!.public_id}"]`)
  await expect(deletedRow.getByText('حذف‌شده', { exact: true })).toBeVisible()
  await expect(deletedRow.getByRole('button', { name: 'بازیابی دیدگاه', exact: true })).toHaveCount(0)
  await expect(deletedRow.getByRole('button', { name: 'تأیید پنهان‌سازی', exact: true })).toBeVisible()

  const confirmedRow = page.locator(`[data-comment-id="${comments[1]!.public_id}"]`)
  await confirmedRow.getByRole('button', { name: 'تأیید پنهان‌سازی', exact: true }).click()
  await expect(confirmedRow.getByText('دلیل تأیید الزامی است.', { exact: true })).toBeVisible()
  const reason = confirmedRow.getByLabel('دلیل اقدام', { exact: true })
  await reason.fill('تأیید بررسی دیدگاه')
  await page.route(`**/api/v1/admin/moderation/comments/${comments[1]!.public_id}/actions`, async (route) => {
    await route.fulfill({ status: 500, contentType: 'application/json', body: '{"error":{"code":"fixture","message":"fixture"}}' })
  })
  await confirmedRow.getByRole('button', { name: 'تأیید پنهان‌سازی', exact: true }).click()
  await expect(confirmedRow.getByText('ثبت اقدام انجام نشد؛ دوباره تلاش کنید.', { exact: true })).toBeVisible()
  await expect(reason).toHaveValue('تأیید بررسی دیدگاه')
  await page.unroute(`**/api/v1/admin/moderation/comments/${comments[1]!.public_id}/actions`)
  await confirmedRow.getByRole('button', { name: 'تأیید پنهان‌سازی', exact: true }).click()
  await expect(confirmedRow).toHaveCount(0)

  const restoredRow = page.locator(`[data-comment-id="${comments[3]!.public_id}"]`)
  await restoredRow.getByLabel('دلیل اقدام', { exact: true }).fill('بازیابی پس از بررسی')
  await restoredRow.getByRole('button', { name: 'بازیابی دیدگاه', exact: true }).click()
  await expect(restoredRow).toHaveCount(0)
  expect(await runSql(`SELECT comment_count FROM listings WHERE public_id = '${created.publicId}'`)).toBe('1')
  const deletedReason = deletedRow.getByLabel('دلیل اقدام', { exact: true })
  await deletedReason.fill('بستن مورد حذف‌شده')
  await deletedRow.getByRole('button', { name: 'تأیید پنهان‌سازی', exact: true }).click()
  await expect(deletedRow).toHaveCount(0)
  expect(await runSql(`SELECT comment_count FROM listings WHERE public_id = '${created.publicId}'`)).toBe('1')

  await page.getByRole('button', { name: 'همه', exact: true }).click()
  await expect(page.locator(`[data-comment-id="${comments[1]!.public_id}"]`).getByText('پنهان تأییدشده', { exact: true })).toBeVisible()
  await expect(page.locator(`[data-comment-id="${comments[3]!.public_id}"]`).getByText('بازیابی‌شده', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'بازیابی‌شده', exact: true }).click()
  await expect(page.locator(`[data-comment-id="${comments[3]!.public_id}"]`)).toBeVisible()
  await expect(page.locator(`[data-comment-id="${comments[1]!.public_id}"]`)).toHaveCount(0)
})

test('@admin-moderation rescan walks 51 real rows and stops after the current confirmed batch', async ({ page }) => {
  const created = await createListing({ title: 'بررسی کامل پنجاه و یک دیدگاه', state: 'active' })
  const phrase = `e2e${randomBytes(6).toString('hex')}`
  const bodies = Array.from({ length: 51 }, (_, index) => `${phrase} دیدگاه ${index + 1}`)
  await createComments(created.slug, bodies, 200)
  expect(await runSql(`SELECT comment_count FROM listings WHERE public_id = '${created.publicId}'`)).toBe('51')
  await createRule(phrase)
  await applyStorage(page, readFixtures().adminStorage)
  await page.goto('/admin/moderation')

  const requestBodies: Array<Record<string, unknown>> = []
  let markSecondSeen!: () => void
  let releaseSecond!: () => void
  const secondSeen = new Promise<void>((resolve) => { markSecondSeen = resolve })
  const secondGate = new Promise<void>((resolve) => { releaseSecond = resolve })
  let requestCount = 0
  await page.route('**/api/v1/admin/moderation/rescan', async (route) => {
    requestCount += 1
    requestBodies.push(route.request().postDataJSON() as Record<string, unknown>)
    if (requestCount === 2) {
      markSecondSeen()
      await secondGate
    }
    await route.continue()
  })
  const firstResponsePromise = page.waitForResponse((response) => (
    response.url().endsWith('/api/v1/admin/moderation/rescan') && response.request().method() === 'POST'
  ))
  await page.getByRole('button', { name: 'شروع بررسی کامل', exact: true }).click()
  const first = await (await firstResponsePromise).json() as {
    rules_revision: number
    next_cursor: string
    has_more: boolean
    scanned_count: number
  }
  expect(first.scanned_count).toBe(50)
  expect(first.has_more).toBe(true)
  await secondSeen
  await page.getByRole('button', { name: 'توقف پس از دسته‌ی جاری', exact: true }).click()
  releaseSecond()
  await expect(page.locator('[data-rescan-scanned]')).toHaveText('۵۱')
  await expect(page.locator('[data-rescan-flagged]')).toHaveText('۵۱')
  await expect(page.getByRole('button', { name: 'شروع بررسی کامل', exact: true })).toBeEnabled()
  expect(requestCount).toBe(2)
  expect(requestBodies[0]).toEqual({ cursor: null, rules_revision: null, limit: 50 })
  expect(requestBodies[1]).toEqual({ cursor: first.next_cursor, rules_revision: first.rules_revision, limit: 50 })
  expect(await runSql(`SELECT COUNT(*) FROM comment_moderation_cases mc JOIN listing_comments c ON c.id = mc.comment_id WHERE c.body LIKE '${phrase}%'`)).toBe('51')
  expect(await runSql(`SELECT comment_count FROM listings WHERE public_id = '${created.publicId}'`)).toBe('0')
  expect(await runSql("SELECT COUNT(*) FROM admin_audit_logs WHERE action = 'moderation.rescan'")).toBe('2')
})

test('@admin-moderation rescan retries the same cursor and requires explicit restart after stale revision', async ({ page }) => {
  await applyStorage(page, readFixtures().adminStorage)
  await page.goto('/admin/moderation')
  const bodies: Array<Record<string, unknown>> = []
  let scenario: 'retry' | 'stale' = 'retry'
  let requestCount = 0
  await page.route('**/api/v1/admin/moderation/rescan', async (route) => {
    requestCount += 1
    bodies.push(route.request().postDataJSON() as Record<string, unknown>)
    if (scenario === 'retry') {
      if (requestCount === 1) {
        await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ rules_revision: 11, next_cursor: 'cursor-one', has_more: true, scanned_count: 50, flagged_count: 40 }) })
      } else if (requestCount === 2) {
        await route.fulfill({ status: 500, contentType: 'application/json', body: '{"error":{"code":"fixture","message":"fixture"}}' })
      } else {
        await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ rules_revision: 11, next_cursor: null, has_more: false, scanned_count: 1, flagged_count: 1 }) })
      }
      return
    }
    if (requestCount === 4) {
      await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ rules_revision: 12, next_cursor: 'stale-cursor', has_more: true, scanned_count: 50, flagged_count: 50 }) })
    } else if (requestCount === 5) {
      await route.fulfill({ status: 400, contentType: 'application/json', body: '{"error":{"code":"validation_error","message":"fixture","fields":{"rules_revision":"changed"}}}' })
    } else {
      await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ rules_revision: 13, next_cursor: null, has_more: false, scanned_count: 0, flagged_count: 0 }) })
    }
  })

  await page.getByRole('button', { name: 'شروع بررسی کامل', exact: true }).click()
  await expect(page.getByText('بررسی دسته انجام نشد؛ با همان نشانگر دوباره تلاش کنید.', { exact: true })).toBeVisible()
  await expect(page.locator('[data-rescan-scanned]')).toHaveText('۵۰')
  await expect(page.locator('[data-rescan-flagged]')).toHaveText('۴۰')
  expect(bodies[1]).toEqual({ cursor: 'cursor-one', rules_revision: 11, limit: 50 })
  await page.getByRole('button', { name: 'تلاش دوباره دسته', exact: true }).click()
  await expect(page.locator('[data-rescan-scanned]')).toHaveText('۵۱')
  await expect(page.locator('[data-rescan-flagged]')).toHaveText('۴۱')
  expect(bodies[2]).toEqual(bodies[1])

  scenario = 'stale'
  await page.getByRole('button', { name: 'شروع بررسی کامل', exact: true }).click()
  await expect(page.getByText('قوانین تغییر کرده‌اند؛ بررسی را از ابتدا دوباره شروع کنید.', { exact: true })).toBeVisible()
  await expect(page.locator('[data-rescan-scanned]')).toHaveText('۵۰')
  await expect(page.locator('[data-rescan-flagged]')).toHaveText('۵۰')
  expect(requestCount).toBe(5)
  await page.getByRole('button', { name: 'شروع دوباره از ابتدا', exact: true }).click()
  await expect(page.locator('[data-rescan-scanned]')).toHaveText('۰')
  await expect(page.locator('[data-rescan-flagged]')).toHaveText('۰')
  expect(bodies[5]).toEqual({ cursor: null, rules_revision: null, limit: 50 })
})
