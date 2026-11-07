import { expect, test } from '@playwright/test'
import {
  chatPartnerStoragePath,
  createListing,
  ensureChatPartner,
  inboxAs,
  partnerSendsMessage,
  partnerStartsConversation,
  readFixtures,
  sendMessage,
  startConversation,
  clientMessageId,
  apiAs,
} from './support/stack'
import {
  conversationRow,
  inboxRequests,
  openInbox,
  resetChatRows,
  waitForDelay,
} from './support/chat'

// A real 30-second Inbox interval plus a real 10-second thread interval cannot
// fit the default timeout; the interval itself is never shortened or faked.
test.setTimeout(180_000)

test.beforeEach(async () => {
  await resetChatRows()
})

async function displayName(role: 'seller' | 'buyer' | 'buyerTwo' | 'admin'): Promise<string> {
  const api = await apiAs(role)
  try {
    const response = await api.get('/api/v1/me')
    expect(response.ok()).toBe(true)
    const body = await response.json() as { user: { display_name: string | null } }
    return body.user.display_name?.trim() || 'کاربر شبدر'
  } finally {
    await api.dispose()
  }
}

test('@chat-inbox anonymous SSR carries no private payload and offers a safe login', async ({ page }) => {
  const response = await page.request.get('/me/conversations')
  expect(response.ok()).toBe(true)
  const html = await response.text()
  expect(html).toContain('name="robots"')
  expect(html).toContain('noindex')
  // No conversation or message payload may reach anonymous HTML.
  expect(html).not.toContain('other_party')
  expect(html).not.toContain('body_preview')
  expect(html).not.toContain('unread_count')

  await page.goto('/me/conversations')
  const login = page.getByRole('link', { name: 'ورود', exact: true }).first()
  await expect(login).toBeVisible()
  await expect(page.locator('.inbox h1')).toHaveText('پیام‌ها')
  await expect(page.getByText('برای دیدن پیام‌ها وارد شوید.', { exact: true })).toBeVisible()
})

test('@chat-inbox confirmed rows show the other party, preview, unread count and a real link', async ({ page }) => {
  const fixtures = readFixtures()
  const buyerName = await displayName('buyer')
  const conversation = await startConversation('buyer', fixtures.detailPath.replace('/listings/', ''))
  await sendMessage('buyer', conversation.public_id, {
    client_message_id: clientMessageId(),
    body: 'سلام، این آگهی هنوز موجود است؟',
  })

  await openInbox(page, fixtures.sellerStorage)
  const row = conversationRow(page, conversation.public_id)
  await expect(row).toBeVisible()
  await expect(row).toContainText(buyerName)
  await expect(row).toContainText('سلام، این آگهی هنوز موجود است؟')
  await expect(row.locator('.conversation-row__unread')).toHaveText('۱')
  await expect(row.locator('.conversation-row__time')).toHaveText(/[۰-۹]/)

  await row.getByRole('link').click()
  await expect(page).toHaveURL(new RegExp(`/me/conversations/${conversation.public_id}$`))
  await expect(page.locator('.thread__name')).toHaveText(buyerName)
})

test('@chat-inbox failed initial read fabricates nothing and a manual retry renders the real inbox', async ({ page }) => {
  const fixtures = readFixtures()
  const conversation = await startConversation('buyer', fixtures.detailPath.replace('/listings/', ''))
  await sendMessage('buyer', conversation.public_id, {
    client_message_id: clientMessageId(),
    body: 'پیام قبل از خطای اولیه',
  })

  let failures = 0
  await page.route('**/api/v1/me/conversations**', async (route) => {
    if (failures === 0) {
      failures += 1
      await route.fulfill({
        status: 500,
        contentType: 'application/json',
        body: JSON.stringify({ error: { code: 'internal', message: 'boom' }, request_id: 'e2e' }),
      })
      return
    }
    await route.continue()
  })
  await openInbox(page, fixtures.sellerStorage)
  await expect(page.getByText('خواندن پیام‌ها ممکن نشد؛ دوباره تلاش کنید.', { exact: true })).toBeVisible()
  await expect(page.locator('.conversation-row')).toHaveCount(0)

  await page.unroute('**/api/v1/me/conversations**')
  await page.getByRole('button', { name: 'تلاش دوباره', exact: true }).click()
  await expect(conversationRow(page, conversation.public_id)).toBeVisible()
  expect(failures).toBe(1)
})

test('@chat-inbox real 30-second polling merges updates and the shell never polls', async ({ page }) => {
  const fixtures = readFixtures()
  const slug = fixtures.detailPath.replace('/listings/', '')
  const conversation = await startConversation('buyer', slug)
  await sendMessage('buyer', conversation.public_id, {
    client_message_id: clientMessageId(),
    body: 'پیام نخست',
  })

  await openInbox(page, fixtures.sellerStorage)
  const row = conversationRow(page, conversation.public_id)
  await expect(row).toContainText('پیام نخست')

  await sendMessage('buyer', conversation.public_id, {
    client_message_id: clientMessageId(),
    body: 'پیام دوم پس از پویش',
  })

  // One real 30-second interval, no fake clock and no manual refresh.
  await expect(row).toContainText('پیام دوم پس از پویش', { timeout: 45_000 })
  await expect(page.locator('.conversation-row')).toHaveCount(1)
  await expect(row.locator('.conversation-row__unread')).toHaveText('۲')

  // Leaving the Inbox must stop all conversation traffic: the shell owns no
  // polling and no badge.
  const requests: string[] = []
  page.on('request', (request) => {
    const path = new URL(request.url()).pathname
    if (path === '/api/v1/me/conversations' || path.endsWith('/read') || path.endsWith('/messages')) {
      requests.push(request.url())
    }
  })
  await page.goto('/')
  await waitForDelay(32_000)
  expect(requests).toEqual([])
})

test('@chat-inbox a poll held past the interval keeps exactly one request in flight', async ({ page }) => {
  const fixtures = readFixtures()
  const conversation = await startConversation('buyer', fixtures.detailPath.replace('/listings/', ''))
  await sendMessage('buyer', conversation.public_id, {
    client_message_id: clientMessageId(),
    body: 'پیام آغازین',
  })

  let forwardRequests = 0
  let release: () => void = () => undefined
  const gate = new Promise<void>((resolveGate) => {
    release = resolveGate
  })
  await page.route(/\/api\/v1\/me\/conversations\?after=/, async (route) => {
    forwardRequests += 1
    await gate
    await route.continue()
  })

  await openInbox(page, fixtures.sellerStorage)
  await expect(conversationRow(page, conversation.public_id)).toBeVisible()

  // Tick one starts at 30s and is still pending past the 60s tick; the in-flight
  // guard must keep the timer from adding a parallel forward scan.
  await expect.poll(() => forwardRequests, { timeout: 45_000 }).toBe(1)
  await waitForDelay(32_000)
  expect(forwardRequests).toBe(1)

  release()
  await page.unroute(/\/api\/v1\/me\/conversations\?after=/)
  await expect(conversationRow(page, conversation.public_id)).toBeVisible()
})

test('@chat-inbox older-page failure keeps confirmed rows and the same cursor for an identical retry', async ({ page }) => {
  const fixtures = readFixtures()
  const slug = fixtures.detailPath.replace('/listings/', '')

  // 21 partners give one full page plus one older page (page size is 20).
  for (let index = 1; index <= 21; index += 1) {
    await ensureChatPartner(index)
    const partnerConversation = await partnerStartsConversation(index, slug)
    await partnerSendsMessage(index, partnerConversation.public_id, `پیام شریک ${index}`)
  }

  await openInbox(page, fixtures.sellerStorage)
  await expect(page.locator('.conversation-row')).toHaveCount(20)

  const olderRequests: string[] = []
  await page.route(/\/api\/v1\/me\/conversations\?before=/, async (route) => {
    olderRequests.push(route.request().url())
    if (olderRequests.length === 1) {
      await route.fulfill({
        status: 500,
        contentType: 'application/json',
        body: JSON.stringify({ error: { code: 'internal', message: 'boom' }, request_id: 'e2e' }),
      })
      return
    }
    await route.continue()
  })

  await page.getByRole('button', { name: 'نمایش گفت‌وگوهای بیشتر', exact: true }).click()
  await expect(page.getByText('خواندن موارد بیشتر ممکن نشد؛ دوباره تلاش کنید.', { exact: true })).toBeVisible()
  await expect(page.locator('.conversation-row')).toHaveCount(20)

  await page.getByRole('button', { name: 'تلاش دوباره', exact: true }).click()
  await expect(page.locator('.conversation-row')).toHaveCount(21)
  expect(olderRequests.length).toBe(2)
  const beforeCursors = olderRequests.map((url) => new URL(url).searchParams.get('before'))
  // The retry must reuse the identical recency cursor, not an advanced one.
  expect(beforeCursors[0]).toBeTruthy()
  expect(beforeCursors[0]).toBe(beforeCursors[1])

  const server = await inboxAs('seller', '?limit=50')
  expect(server.data).toHaveLength(21)
  const renderedIds = await page.locator('.conversation-row a').evaluateAll((links) =>
    links.map((link) => link.getAttribute('href')?.split('/').pop() ?? ''),
  )
  expect(new Set(renderedIds).size).toBe(21)
})

test('@chat-inbox duplicate rows in one response collapse to one DOM row and the server cursor still advances', async ({ page }) => {
  const fixtures = readFixtures()
  const conversation = await startConversation('buyer', fixtures.detailPath.replace('/listings/', ''))
  await sendMessage('buyer', conversation.public_id, {
    client_message_id: clientMessageId(),
    body: 'پیام تکراری',
  })

  let served = false
  await page.route(/\/api\/v1\/me\/conversations\?after=/, async (route) => {
    if (served) {
      await route.continue()
      return
    }
    served = true
    const response = await route.fetch()
    const body = await response.json() as { data: Array<{ public_id: string }> }
    const first = body.data[0]
    if (first) {
      body.data = [...body.data, { ...first }, { ...first }]
      // The injected duplicates stay within one response; the real cursor is
      // forwarded untouched so the next poll keeps progressing.
      await route.fulfill({ response, json: body })
      return
    }
    await route.fulfill({ response, json: body })
  })

  await openInbox(page, fixtures.sellerStorage)
  await sendMessage('buyer', conversation.public_id, {
    client_message_id: clientMessageId(),
    body: 'پیام پس از پویش تکراری',
  })
  await expect(conversationRow(page, conversation.public_id)).toContainText(
    'پیام پس از پویش تکراری',
    { timeout: 45_000 },
  )
  expect(served).toBe(true)
  await expect(page.locator('.conversation-row')).toHaveCount(1)
})

test('@chat-inbox hidden and offline pages pause polling and catch up on resume', async ({ page, context }) => {
  const fixtures = readFixtures()
  const slug = fixtures.detailPath.replace('/listings/', '')
  const conversation = await startConversation('buyer', slug)
  await sendMessage('buyer', conversation.public_id, {
    client_message_id: clientMessageId(),
    body: 'پیام پیش از پنهان‌شدن',
  })

  await openInbox(page, fixtures.sellerStorage)
  const row = conversationRow(page, conversation.public_id)
  await expect(row).toContainText('پیام پیش از پنهان‌شدن')

  await page.evaluate(() => {
    Object.defineProperty(document, 'hidden', { configurable: true, get: () => true })
    document.dispatchEvent(new Event('visibilitychange'))
  })
  await sendMessage('buyer', conversation.public_id, {
    client_message_id: clientMessageId(),
    body: 'پیام هنگام پنهان‌بودن',
  })
  await waitForDelay(34_000)
  await expect(row).not.toContainText('پیام هنگام پنهان‌بودن')

  await page.evaluate(() => {
    Object.defineProperty(document, 'hidden', { configurable: true, get: () => false })
    document.dispatchEvent(new Event('visibilitychange'))
  })
  await expect(row).toContainText('پیام هنگام پنهان‌بودن', { timeout: 20_000 })

  // Offline: the timer must not even attempt a request.
  await context.setOffline(true)
  const offlineRequests = inboxRequests(page)
  await waitForDelay(32_000)
  expect(offlineRequests).toEqual([])
  await context.setOffline(false)
})
