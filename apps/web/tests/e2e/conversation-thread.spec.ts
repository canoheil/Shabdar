import { expect, test, type Page } from '@playwright/test'
import {
  apiAs,
  clientMessageId,
  createListing,
  readFixtures,
  sendMessage,
  setBlock,
  startConversation,
} from './support/stack'
import {
  bubbles,
  bubbleBodies,
  openThread,
  readRequests,
  resetChatRows,
  waitForDelay,
} from './support/chat'

// Real 10-second thread polling; never shortened, never faked.
test.describe.configure({ timeout: 180_000 })

test.beforeEach(async () => {
  await resetChatRows()
})

function slugOf(detailPath: string): string {
  return detailPath.replace('/listings/', '')
}

async function threadUrl(publicId: string): Promise<string> {
  return `/me/conversations/${publicId}`
}

async function buy(slug: string): Promise<string> {
  const conversation = await startConversation('buyer', slug)
  return conversation.public_id
}

async function openBuyerThread(page: Page, publicId: string): Promise<void> {
  await openThread(page, readFixtures().buyerStorage, publicId)
}

test('@chat-thread private SSR carries no payload and outsiders get a neutral state', async ({ page }) => {
  const publicId = await buy(slugOf(readFixtures().detailPath))
  const html = await (await page.request.get(await threadUrl(publicId))).text()
  expect(html).toContain('noindex')
  expect(html).not.toContain('other_party')
  expect(html).not.toContain('read_state')
  expect(html).not.toContain('viewer_read_through')

  // A real participant sees the thread.
  await openBuyerThread(page, publicId)
  await expect(page.locator('.thread__scroller')).toBeVisible()

  // A registered outsider gets exactly the same neutral 404 state as an
  // unknown id: no participant data, no history.
  await openThread(page, readFixtures().buyerTwoStorage, publicId)
  await expect(page.getByText('این گفت‌وگو در دسترس نیست.', { exact: true })).toBeVisible()
  await expect(page.locator('.thread__scroller')).toHaveCount(0)
})

test('@chat-thread a failed poll keeps confirmed history, cursors and recovers on the next tick', async ({ page }) => {
  const conversation = await buy(slugOf(readFixtures().detailPath))
  await sendMessage('buyer', conversation, { client_message_id: clientMessageId(), body: 'پیام پایدار' })

  await openBuyerThread(page, conversation)
  await expect(bubbles(page)).toHaveCount(1)
  await expect(bubbles(page).first()).toContainText('پیام پایدار')
  await expect(page.locator('.thread__day')).toHaveCount(1)

  await page.route(/\/api\/v1\/conversations\/.+\/messages\?after=/, async (route) => {
    await route.fulfill({
      status: 500,
      contentType: 'application/json',
      body: JSON.stringify({ error: { code: 'internal', message: 'boom' }, request_id: 'e2e' }),
    })
  })
  await expect(page.getByText('به‌روزرسانی گفتگو ممکن نشد؛ دوباره تلاش می‌کنیم.', { exact: true }))
    .toBeVisible({ timeout: 25_000 })
  // Confirmed history is never dropped by a failed synchronization.
  await expect(bubbles(page)).toHaveCount(1)

  await page.unroute(/\/api\/v1\/conversations\/.+\/messages\?after=/)
  await expect(page.getByText('به‌روزرسانی گفتگو ممکن نشد؛ دوباره تلاش می‌کنیم.', { exact: true }))
    .toHaveCount(0, { timeout: 25_000 })
})

test('@chat-thread real 10-second polling appends one ascending bubble with a Persian time', async ({ page }) => {
  const conversation = await buy(slugOf(readFixtures().detailPath))
  await sendMessage('buyer', conversation, { client_message_id: clientMessageId(), body: 'پیام اول' })

  await openBuyerThread(page, conversation)
  await expect(bubbles(page)).toHaveCount(1)

  await sendMessage('buyer', conversation, { client_message_id: clientMessageId(), body: 'پیام دوم' })
  await expect(bubbles(page)).toHaveCount(2, { timeout: 30_000 })
  expect(await bubbleBodies(page)).toEqual(['پیام اول', 'پیام دوم'])
  await expect(bubbles(page).nth(1).locator('.bubble__time')).toHaveText(/[۰-۹]/)
  await expect(bubbles(page).nth(1).locator('.bubble__time')).toHaveAttribute('datetime', /T/)
})

test('@chat-thread a real Read produces Seen only after the other side polls', async ({ page, browser }) => {
  const conversation = await buy(slugOf(readFixtures().detailPath))
  const sellerContext = await browser.newContext({ storageState: readFixtures().sellerStorage })
  const sellerPage = await sellerContext.newPage()
  try {
    // Seller sends; the buyer's thread is open, so the buyer renders and reads.
    const sent = await sendMessage('seller', conversation, {
      client_message_id: clientMessageId(),
      body: 'پیام فروشنده',
    })
    // Unseen is an explicit `false`, not a missing field.
    expect(sent.message.seen).toBe(false)

    await openBuyerThread(page, conversation)
    await expect(bubbles(page)).toHaveCount(1)
    const reads = readRequests(page)
    await expect.poll(() => reads.length, { timeout: 20_000 }).toBeGreaterThan(0)

    // The seller sees Seen only after a later real poll of the same thread.
    await openThread(sellerPage, readFixtures().sellerStorage, conversation)
    await expect(sellerPage.locator('.bubble--own')).toHaveCount(1)
    await expect(sellerPage.locator('.bubble__seen')).toHaveText('دیده شد', { timeout: 25_000 })
  } finally {
    await sellerContext.close()
  }
})

test('@chat-thread the latest message is rendered before Read is claimed', async ({ page }) => {
  const conversation = await buy(slugOf(readFixtures().detailPath))
  await sendMessage('buyer', conversation, { client_message_id: clientMessageId(), body: 'پیام قدیمی' })

  let renderedWhenRead = false
  let readCount = 0
  await page.route(/\/api\/v1\/conversations\/.+\/read$/, async (route) => {
    readCount += 1
    const bodies = await page.locator('.bubble__body').allTextContents()
    if (bodies.at(-1) === 'پیام تازه') renderedWhenRead = true
    await route.continue()
  })

  await openBuyerThread(page, conversation)
  await expect(bubbles(page)).toHaveCount(1)
  await sendMessage('buyer', conversation, { client_message_id: clientMessageId(), body: 'پیام تازه' })
  await expect(bubbles(page)).toHaveCount(2, { timeout: 30_000 })
  await expect.poll(() => readCount, { timeout: 20_000 }).toBeGreaterThan(1)
  expect(renderedWhenRead).toBe(true)
})

test('@chat-thread hidden pages neither poll nor claim Read and resume in order', async ({ page }) => {
  const conversation = await buy(slugOf(readFixtures().detailPath))
  await sendMessage('buyer', conversation, { client_message_id: clientMessageId(), body: 'پیام پایه' })

  await openBuyerThread(page, conversation)
  await expect(bubbles(page)).toHaveCount(1)

  const reads = readRequests(page)
  const polls: string[] = []
  page.on('request', (request) => {
    if (new URL(request.url()).pathname.endsWith('/messages')) polls.push(request.url())
  })

  await page.evaluate(() => {
    Object.defineProperty(document, 'hidden', { configurable: true, get: () => true })
    document.dispatchEvent(new Event('visibilitychange'))
  })
  const readsBefore = reads.length
  const pollsBefore = polls.length

  await sendMessage('buyer', conversation, { client_message_id: clientMessageId(), body: 'پیام هنگام پنهان‌بودن' })
  await waitForDelay(13_000)
  expect(polls.length).toBe(pollsBefore)
  expect(reads.length).toBe(readsBefore)

  await page.evaluate(() => {
    Object.defineProperty(document, 'hidden', { configurable: true, get: () => false })
    document.dispatchEvent(new Event('visibilitychange'))
  })
  await expect(bubbles(page)).toHaveCount(2, { timeout: 20_000 })
  await expect.poll(() => reads.length, { timeout: 20_000 }).toBeGreaterThan(readsBefore)
})

test('@chat-thread loading older history keeps the anchor, and arrivals need an explicit jump', async ({ page }) => {
  const conversation = await buy(slugOf(readFixtures().detailPath))
  for (let index = 1; index <= 35; index += 1) {
    await sendMessage('buyer', conversation, {
      client_message_id: clientMessageId(),
      body: `پیام شمارهٔ ${index}`,
    })
  }

  await openBuyerThread(page, conversation)
  await expect(bubbles(page)).toHaveCount(30)

  // Reading older history starts from an upward scroll position, which is the
  // case where the pixel anchor must be preserved.
  await page.evaluate(() => {
    const scroller = document.querySelector('.thread__scroller')
    if (!scroller) throw new Error('thread scroller is missing')
    scroller.scrollTop = 0
    scroller.dispatchEvent(new Event('scroll'))
  })
  await expect(page.locator('.thread__jump')).toHaveCount(0)

  const anchorBefore = await page.evaluate(() => {
    const first = document.querySelector('.thread__scroller .bubble')
    const scroller = document.querySelector('.thread__scroller')
    if (!first || !scroller) throw new Error('thread is not rendered')
    return {
      text: first.textContent ?? '',
      top: first.getBoundingClientRect().top,
    }
  })

  await page.getByRole('button', { name: 'پیام‌های پیشین', exact: true }).click()
  await expect(bubbles(page)).toHaveCount(35)
  const anchorAfter = await page.evaluate((text) => {
    const candidates = [...document.querySelectorAll('.thread__scroller .bubble')]
    const match = candidates.find((bubble) => (bubble.textContent ?? '') === text)
    return match ? match.getBoundingClientRect().top : Number.NaN
  }, anchorBefore.text)
  // The bubble the reader was looking at stays put while older pages prepend.
  expect(Math.abs(anchorAfter - anchorBefore.top)).toBeLessThan(12)

  const reads = readRequests(page)
  const readsBefore = reads.length
  await sendMessage('buyer', conversation, { client_message_id: clientMessageId(), body: 'پیام تازه هنگام خواندن گذشته' })
  const jump = page.locator('.thread__jump')
  await expect(jump).toContainText('۱ پیام تازه', { timeout: 30_000 })
  // Reading older history must not claim the unseen arrival as read.
  expect(reads.length).toBe(readsBefore)
  await expect(page.getByText('پیام تازه هنگام خواندن گذشته', { exact: true })).toHaveCount(0)

  await jump.click()
  await expect(page.locator('.thread__scroller .bubble__body').last())
    .toHaveText('پیام تازه هنگام خواندن گذشته')
  await expect.poll(() => reads.length, { timeout: 20_000 }).toBeGreaterThan(readsBefore)
})

test('@chat-thread a failed older page keeps the anchor and reuses the identical cursor', async ({ page }) => {
  const conversation = await buy(slugOf(readFixtures().detailPath))
  for (let index = 1; index <= 32; index += 1) {
    await sendMessage('buyer', conversation, {
      client_message_id: clientMessageId(),
      body: `پیام آرشیو ${index}`,
    })
  }

  const olderRequests: string[] = []
  await page.route(/\/api\/v1\/conversations\/.+\/messages\?before=/, async (route) => {
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

  await openBuyerThread(page, conversation)
  await expect(bubbles(page)).toHaveCount(30)
  await page.getByRole('button', { name: 'پیام‌های پیشین', exact: true }).click()
  await expect(page.getByText('خواندن پیام‌های پیشین ممکن نشد؛ دوباره تلاش کنید.', { exact: true }))
    .toBeVisible()
  await expect(bubbles(page)).toHaveCount(30)

  await page.getByRole('button', { name: 'تلاش دوباره', exact: true }).click()
  await expect(bubbles(page)).toHaveCount(32)
  const cursors = olderRequests.map((url) => new URL(url).searchParams.get('before'))
  expect(cursors[0]).toBeTruthy()
  expect(cursors[0]).toBe(cursors[1])
  const bodies = await bubbleBodies(page)
  expect(new Set(bodies).size).toBe(32)
})

test('@chat-thread forward catch-up beyond one page renders every message exactly once', async ({ page }) => {
  const conversation = await buy(slugOf(readFixtures().detailPath))
  await sendMessage('buyer', conversation, { client_message_id: clientMessageId(), body: 'پیام پایه' })

  await openBuyerThread(page, conversation)
  await expect(bubbles(page)).toHaveCount(1)

  for (let index = 1; index <= 31; index += 1) {
    await sendMessage('buyer', conversation, {
      client_message_id: clientMessageId(),
      body: `پیام همگام ${index}`,
    })
  }

  await expect(bubbles(page)).toHaveCount(32, { timeout: 40_000 })
  const bodies = await bubbleBodies(page)
  expect(bodies).toHaveLength(32)
  expect(new Set(bodies).size).toBe(32)
  expect(bodies.at(-1)).toBe('پیام همگام 31')
})

test('@chat-thread listing cards link only while the listing is public', async ({ page }) => {
  const listing = await createListing({ title: 'آگهی ارجاع گفت‌وگو', state: 'active' })
  const conversation = await buy(listing.slug)
  await sendMessage('buyer', conversation, {
    client_message_id: clientMessageId(),
    body: 'دربارهٔ این آگهی',
    listing_slug: listing.slug,
  })

  await openBuyerThread(page, conversation)
  const card = page.locator('.bubble .listing-reference')
  await expect(card).toContainText('آگهی ارجاع گفت‌وگو')
  await expect(card).toHaveAttribute('href', `/listings/${listing.slug}`)

  const admin = await apiAs('admin')
  try {
    const removed = await admin.post(`/api/v1/admin/listings/${listing.publicId}/remove`, {
      data: { reason: 'حذف آزمون واقعی آگهی ارجاع' },
    })
    expect(removed.ok()).toBe(true)
  } finally {
    await admin.dispose()
  }

  await page.reload()
  await expect(page.locator('.bubble .listing-reference')).toContainText('این آگهی دیگر در دسترس نیست.')
  await expect(page.locator('.bubble a.listing-reference')).toHaveCount(0)
})

test('@chat-thread blocking keeps read-only history and clears only after confirmation', async ({ page }) => {
  const conversation = await buy(slugOf(readFixtures().detailPath))
  await sendMessage('buyer', conversation, { client_message_id: clientMessageId(), body: 'تاریخچهٔ حفظ‌شده' })

  await openBuyerThread(page, conversation)
  await expect(bubbles(page)).toHaveCount(1)
  await expect(page.locator('#composer-body')).toBeEnabled()

  let failedOnce = false
  await page.route('**/api/v1/me/blocks/**', async (route) => {
    if (!failedOnce) {
      failedOnce = true
      await route.fulfill({
        status: 500,
        contentType: 'application/json',
        body: JSON.stringify({ error: { code: 'internal', message: 'boom' }, request_id: 'e2e' }),
      })
      return
    }
    await route.continue()
  })

  await page.getByRole('button', { name: 'مسدود کردن کاربر', exact: true }).click()
  await expect(page.getByText('مسدودسازی انجام نشد؛ دوباره تلاش کنید.', { exact: true })).toBeVisible()
  await expect(page.locator('#composer-body')).toBeEnabled()

  await page.getByRole('button', { name: 'تلاش دوباره', exact: true }).click()
  await expect(page.getByText('شما این کاربر را مسدود کرده‌اید؛ برای ارسال پیام، مسدودسازی را بردارید.', { exact: true }))
    .toBeVisible({ timeout: 20_000 })
  await expect(page.locator('#composer-body')).toBeDisabled()
  // History and Read state stay available while blocked.
  await expect(bubbles(page)).toHaveCount(1)

  await page.getByRole('button', { name: 'رفع مسدودسازی', exact: true }).click()
  await expect(page.locator('#composer-body')).toBeEnabled({ timeout: 20_000 })
})

test('@chat-thread a reverse block keeps the composer read-only after unblocking the other side', async ({ page }) => {
  const conversation = await buy(slugOf(readFixtures().detailPath))
  await sendMessage('buyer', conversation, { client_message_id: clientMessageId(), body: 'پیام دوطرفه' })
  const seller = await apiAs('seller')
  let sellerPublicId = ''
  try {
    const me = await seller.get('/api/v1/me')
    sellerPublicId = ((await me.json()) as { user: { public_id: string } }).user.public_id
  } finally {
    await seller.dispose()
  }

  await setBlock('seller', (await buyerPublicId()), true)
  await setBlock('buyer', sellerPublicId, true)

  await openBuyerThread(page, conversation)
  await expect(page.locator('#composer-body')).toBeDisabled({ timeout: 20_000 })

  await setBlock('buyer', sellerPublicId, false)
  await page.reload()
  await expect(page.locator('#composer-body')).toBeDisabled({ timeout: 20_000 })
  await expect(page.getByText('ارسال پیام در این گفت‌وگو ممکن نیست.', { exact: true })).toBeVisible()
})

async function buyerPublicId(): Promise<string> {
  const api = await apiAs('buyer')
  try {
    const response = await api.get('/api/v1/me')
    return ((await response.json()) as { user: { public_id: string } }).user.public_id
  } finally {
    await api.dispose()
  }
}
