import { expect, test, type Page } from '@playwright/test'
import {
  apiAs,
  clientMessageId,
  createListing,
  readFixtures,
  sendMessage,
  startConversation,
} from './support/stack'
import {
  applyStorage,
  bubbles,
  bubbleBodies,
  loginViaUi,
  openThread,
  resetChatRows,
  waitForDelay,
} from './support/chat'

test.setTimeout(180_000)

test.beforeEach(async () => {
  await resetChatRows()
})

function composerInput(page: Page) {
  return page.locator('#composer-body')
}

function sendButton(page: Page) {
  return page.locator('.composer button[type="submit"]')
}

async function threadRequests(page: Page): Promise<{ start: string[] }> {
  const start: string[] = []
  page.on('request', (request) => {
    if (request.method() === 'POST' && /\/api\/v1\/listings\/[^/]+\/conversations$/.test(new URL(request.url()).pathname)) {
      start.push(request.url())
    }
  })
  return { start }
}

test('@chat-entry two listings of one seller open the same conversation without system messages', async ({ page }) => {
  const first = await createListing({ title: 'آگهی اول گفت‌وگو', state: 'active' })
  const second = await createListing({ title: 'آگهی دوم گفت‌وگو', state: 'active' })

  await applyStorage(page, readFixtures().buyerStorage)
  await page.goto(`/listings/${first.slug}`)
  await page.getByRole('button', { name: 'گفت‌وگو با فروشنده', exact: true }).click()
  await expect(page).toHaveURL(/\/me\/conversations\/[^/?]+\?listing=/)
  const firstUrl = new URL(page.url())
  const conversationId = firstUrl.pathname.split('/').pop() ?? ''
  expect(conversationId).toBeTruthy()

  await expect(page.locator('.composer__chip')).toContainText('آگهی اول گفت‌وگو')
  await composerInput(page).fill('سلام دربارهٔ آگهی اول')
  await composerInput(page).press('Enter')
  await expect(bubbles(page)).toHaveCount(1)
  await expect(bubbles(page).first().locator('.listing-reference')).toContainText('آگهی اول گفت‌وگو')
  // The pending reference is cleared only after the confirmed send.
  await expect(page.locator('.composer__chip')).toHaveCount(0)
  await expect(page).toHaveURL(new RegExp(`/me/conversations/${conversationId}$`))

  await page.goto(`/listings/${second.slug}`)
  await page.getByRole('button', { name: 'گفت‌وگو با فروشنده', exact: true }).click()
  await expect(page).toHaveURL(new RegExp(`/me/conversations/${conversationId}\\?listing=`))
  await expect(page.locator('.composer__chip')).toContainText('آگهی دوم گفت‌وگو')
  await composerInput(page).fill('و این هم دربارهٔ آگهی دوم')
  await sendButton(page).click()
  await expect(bubbles(page)).toHaveCount(2)
  await expect(bubbles(page).nth(1).locator('.listing-reference')).toContainText('آگهی دوم گفت‌وگو')

  const bodies = await bubbleBodies(page)
  // Exactly the two intended messages: no "conversation started" system row.
  expect(bodies).toEqual(['سلام دربارهٔ آگهی اول', 'و این هم دربارهٔ آگهی دوم'])
})

test('@chat-entry anonymous contact continues through real login and resumes exactly once', async ({ page }) => {
  const listing = await createListing({ title: 'آگهی تماس ناشناس', state: 'active' })
  await page.goto(`/listings/${listing.slug}`)
  const { start } = await threadRequests(page)

  await page.getByRole('link', { name: 'ورود برای گفت‌وگو', exact: true }).click()
  await expect(page).toHaveURL(/\/login\?next=/)
  await loginViaUi(page, 'buyer')

  await expect(page).toHaveURL(new RegExp(`/me/conversations/[^/?]+\\?listing=${listing.slug}`))
  await expect(composerInput(page)).toBeVisible()
  // Exactly one start call: the continuation guard prevents a second attempt.
  expect(start).toHaveLength(1)

  const api = await apiAs('buyer')
  try {
    const inbox = await api.get('/api/v1/me/conversations')
    expect(inbox.ok()).toBe(true)
    const body = await inbox.json() as { data: Array<{ public_id: string }> }
    expect(body.data).toHaveLength(1)
  } finally {
    await api.dispose()
  }
})

test('@chat-entry hostile next parameters always fall back to an internal path', async ({ page }) => {
  const hostile = ['https://evil.example', '//evil.example', '/%2f%2fevil.example']
  for (const value of hostile) {
    await page.goto(`/login?next=${encodeURIComponent(value)}`)
    await loginViaUi(page, 'buyer')
    const current = new URL(page.url())
    expect(current.origin).toBe(new URL(process.env.E2E_BASE_URL ?? 'http://127.0.0.1:3000').origin)
    expect(current.pathname).toBe('/')
    // A fresh session per iteration keeps every login real.
    await page.request.post('/api/v1/auth/logout')
    await page.context().clearCookies()
  }
})

test('@chat-entry a seller cannot contact their own listing', async ({ page }) => {
  const listing = await createListing({ title: 'آگهی فروشندهٔ خود', state: 'active' })
  await applyStorage(page, readFixtures().sellerStorage)
  await page.goto(`/listings/${listing.slug}`)
  await expect(page.getByRole('button', { name: 'گفت‌وگو با فروشنده', exact: true })).toHaveCount(0)

  const seller = await apiAs('seller')
  try {
    const response = await seller.post(`/api/v1/listings/${encodeURIComponent(listing.slug)}/conversations`)
    expect(response.status()).toBe(403)
  } finally {
    await seller.dispose()
  }
})

test('@chat-entry a message appears only after the server confirms it', async ({ page }) => {
  const conversation = await startConversation('buyer', readFixtures().detailPath.replace('/listings/', ''))
  await openThread(page, readFixtures().buyerStorage, conversation.public_id)
  await expect(composerInput(page)).toBeEnabled()

  let release: () => void = () => undefined
  const gate = new Promise<void>((resolveGate) => {
    release = resolveGate
  })
  let held = 0
  await page.route(/\/api\/v1\/conversations\/.+\/messages$/, async (route) => {
    if (route.request().method() !== 'POST') {
      await route.continue()
      return
    }
    held += 1
    await gate
    await route.continue()
  })

  await composerInput(page).fill('پیام تأییدشده')
  await composerInput(page).press('Enter')
  await expect.poll(() => held, { timeout: 10_000 }).toBe(1)
  await waitForDelay(1_200)
  // Nothing is rendered before the response: no optimistic bubble.
  await expect(bubbles(page)).toHaveCount(0)
  await expect(composerInput(page)).toHaveValue('پیام تأییدشده')

  release()
  await expect(bubbles(page)).toHaveCount(1)
  await expect(bubbles(page).first()).toContainText('پیام تأییدشده')
  await expect(composerInput(page)).toHaveValue('')
})

test('@chat-entry Shift+Enter keeps a newline and a lost response retries with the identical client id', async ({ page }) => {
  const conversation = await startConversation('buyer', readFixtures().detailPath.replace('/listings/', ''))
  await openThread(page, readFixtures().buyerStorage, conversation.public_id)
  await expect(composerInput(page)).toBeEnabled()

  await composerInput(page).fill('خط اول')
  await composerInput(page).press('Shift+Enter')
  await composerInput(page).type('خط دوم')
  await expect(composerInput(page)).toHaveValue('خط اول\nخط دوم')

  const ids: string[] = []
  let firstPost = true
  await page.route(/\/api\/v1\/conversations\/.+\/messages$/, async (route) => {
    if (route.request().method() !== 'POST') {
      await route.continue()
      return
    }
    const payload = route.request().postDataJSON() as { client_message_id: string }
    ids.push(payload.client_message_id)
    if (firstPost) {
      firstPost = false
      // The server stores the message; the response is "lost".
      await route.fetch()
      await route.fulfill({
        status: 500,
        contentType: 'application/json',
        body: JSON.stringify({ error: { code: 'internal', message: 'lost' }, request_id: 'e2e' }),
      })
      return
    }
    await route.continue()
  })

  await sendButton(page).click()
  await expect(page.getByText('ارسال پیام انجام نشد؛ متن شما حفظ شد.', { exact: true })).toBeVisible()
  await expect(composerInput(page)).toHaveValue('خط اول\nخط دوم')
  await expect(bubbles(page)).toHaveCount(0)

  await page.getByRole('button', { name: 'تلاش دوباره', exact: true }).click()
  await expect(bubbles(page)).toHaveCount(1)
  await expect(bubbles(page).first().locator('.bubble__body')).toHaveText('خط اول\nخط دوم')
  expect(ids).toHaveLength(2)
  expect(ids[0]).toBe(ids[1])

  const api = await apiAs('buyer')
  try {
    const response = await api.get(`/api/v1/conversations/${conversation.public_id}/messages`)
    const body = await response.json() as { data: Array<{ body: string }> }
    expect(body.data).toHaveLength(1)
  } finally {
    await api.dispose()
  }
})

test('@chat-entry a changed retry conflicts safely and a fresh send needs an explicit action', async ({ page }) => {
  const conversation = await startConversation('buyer', readFixtures().detailPath.replace('/listings/', ''))
  await openThread(page, readFixtures().buyerStorage, conversation.public_id)
  await expect(composerInput(page)).toBeEnabled()

  let firstPost = true
  let newKey: string | null = null
  const ids: string[] = []
  await page.route(/\/api\/v1\/conversations\/.+\/messages$/, async (route) => {
    if (route.request().method() !== 'POST') {
      await route.continue()
      return
    }
    const payload = route.request().postDataJSON() as { client_message_id: string }
    ids.push(payload.client_message_id)
    if (firstPost) {
      firstPost = false
      await route.fetch()
      await route.fulfill({
        status: 500,
        contentType: 'application/json',
        body: JSON.stringify({ error: { code: 'internal', message: 'lost' }, request_id: 'e2e' }),
      })
      return
    }
    await route.continue()
  })

  await composerInput(page).fill('متن نخست')
  await sendButton(page).click()
  await expect(page.getByText('ارسال پیام انجام نشد؛ متن شما حفظ شد.', { exact: true })).toBeVisible()

  await composerInput(page).fill('متن ویرایش‌شده پس از پاسخ گم‌شده')
  await page.getByRole('button', { name: 'تلاش دوباره', exact: true }).click()
  await expect(page.getByText('این پیام با نسخهٔ ثبت‌شده تفاوت دارد؛ تاریخچه را بررسی کنید.', { exact: true }))
    .toBeVisible()

  await page.getByRole('button', { name: 'ارسال به‌عنوان پیام تازه', exact: true }).click()
  // A new key exists only after the explicit action; the send itself is still
  // the user's decision and reuses the preserved draft.
  await expect(page.getByRole('button', { name: 'ارسال به‌عنوان پیام تازه', exact: true })).toHaveCount(0)
  await sendButton(page).click()
  await expect(bubbles(page)).toHaveCount(2)
  await expect(bubbles(page).first()).toContainText('متن نخست')
  await expect(bubbles(page).nth(1)).toContainText('متن ویرایش‌شده پس از پاسخ گم‌شده')
  expect(ids).toHaveLength(3)
  newKey = ids[2] ?? null
  expect(ids[0]).toBe(ids[1])
  expect(newKey).not.toBe(ids[0])
})

test('@chat-entry drafts survive a reload per viewer and conversation', async ({ page }) => {
  const conversation = await startConversation('buyer', readFixtures().detailPath.replace('/listings/', ''))
  await openThread(page, readFixtures().buyerStorage, conversation.public_id)
  await expect(composerInput(page)).toBeEnabled()

  await composerInput(page).fill('پیش‌نویس ماندگار')
  await waitForDelay(300)
  await page.reload()
  await expect(composerInput(page)).toHaveValue('پیش‌نویس ماندگار')

  const key = await page.evaluate(() => {
    const entry = Object.keys(window.sessionStorage)
      .find((candidate) => candidate.includes(':chat:'))
    return entry ? window.sessionStorage.getItem(entry) : null
  })
  expect(key).toBeTruthy()
  const parsed = JSON.parse(key ?? '{}') as { body?: string; clientMessageId?: string }
  expect(parsed.body).toBe('پیش‌نویس ماندگار')
  expect(parsed.clientMessageId).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/)
})

test('@chat-entry unicode, counters and literal markup never overflow or execute', async ({ page }) => {
  const conversation = await startConversation('buyer', readFixtures().detailPath.replace('/listings/', ''))
  await openThread(page, readFixtures().buyerStorage, conversation.public_id)
  await expect(composerInput(page)).toBeEnabled()

  let dialogSeen = false
  page.on('dialog', async (dialog) => {
    dialogSeen = true
    await dialog.dismiss()
  })

  const longUndivided = 'س'.repeat(400)
  await composerInput(page).fill(longUndivided)
  await expect(page.locator('#composer-count')).toHaveText('۴۰۰ از ۱۰۰۰')
  await composerInput(page).fill('ا'.repeat(1001))
  await expect(sendButton(page)).toBeDisabled()
  await expect(page.locator('#composer-count')).toHaveClass(/composer__counter--over/)

  const payload = '<script>window.__xss = 1</script>\n<img src=x onerror="window.__xss = 2">\n🎉 پیام چندخطی'
  await composerInput(page).fill(payload)
  await sendButton(page).click()
  await expect(bubbles(page)).toHaveCount(1)
  await expect(bubbles(page).first().locator('.bubble__body')).toHaveText(payload)
  expect(await page.evaluate(() => (window as unknown as { __xss?: number }).__xss)).toBeUndefined()
  expect(dialogSeen).toBe(false)

  const overflow = await page.evaluate(() => {
    const scroller = document.querySelector('.thread__scroller')
    return scroller ? scroller.scrollWidth - scroller.clientWidth : 0
  })
  expect(overflow).toBeLessThanOrEqual(1)
})

test('@chat-entry a late response from a conversation the viewer left mutates nothing', async ({ page }) => {
  const conversation = await startConversation('buyer', readFixtures().detailPath.replace('/listings/', ''))
  await openThread(page, readFixtures().buyerStorage, conversation.public_id)
  await expect(composerInput(page)).toBeEnabled()

  let release: () => void = () => undefined
  const gate = new Promise<void>((resolveGate) => {
    release = resolveGate
  })
  await page.route(/\/api\/v1\/conversations\/.+\/messages$/, async (route) => {
    if (route.request().method() !== 'POST') {
      await route.continue()
      return
    }
    await gate
    try {
      await route.fetch()
    } catch {
      // The viewer already left; nothing to forward.
    }
    await route.abort('failed').catch(() => undefined)
  })

  await composerInput(page).fill('پیام دیرهنگام')
  await sendButton(page).click()
  // Leave for the Inbox while the send is still in flight.
  await page.goto('/me/conversations')
  await expect(page.locator('.inbox h1')).toHaveText('پیام‌ها')
  release()
  await waitForDelay(2_000)
  await expect(page.locator('.inbox h1')).toHaveText('پیام‌ها')
  await expect(bubbles(page)).toHaveCount(0)
})

test('@chat-entry a 429 response keeps the draft and never tight-loops', async ({ page }) => {
  const conversation = await startConversation('buyer', readFixtures().detailPath.replace('/listings/', ''))
  await openThread(page, readFixtures().buyerStorage, conversation.public_id)
  await expect(composerInput(page)).toBeEnabled()

  const posts: string[] = []
  await page.route(/\/api\/v1\/conversations\/.+\/messages$/, async (route) => {
    if (route.request().method() !== 'POST') {
      await route.continue()
      return
    }
    posts.push(route.request().url())
    await route.fulfill({
      status: 429,
      headers: { 'retry-after': '3', 'content-type': 'application/json' },
      body: JSON.stringify({ error: { code: 'rate_limited', message: 'slow down' }, request_id: 'e2e' }),
    })
  })

  await composerInput(page).fill('پیام محدودشده')
  await sendButton(page).click()
  await expect(page.getByText(/ثانیهٔ دیگر دوباره تلاش کنید|کمی بعد دوباره تلاش کنید/)).toBeVisible()
  await expect(composerInput(page)).toHaveValue('پیام محدودشده')
  await expect(sendButton(page)).toBeDisabled()
  const before = posts.length
  await waitForDelay(3_500)
  expect(posts.length).toBe(before)

  await page.unroute(/\/api\/v1\/conversations\/.+\/messages$/)
  await expect(sendButton(page)).toBeEnabled({ timeout: 10_000 })
  await sendButton(page).click()
  await expect(bubbles(page)).toHaveCount(1)
})
