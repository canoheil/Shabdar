import { readFileSync } from 'node:fs'
import { expect, type BrowserContext, type Page } from '@playwright/test'
import { runSql } from './database'
import { apiLogSize, devOtp, FIXTURE_PHONES } from './stack'

/**
 * Chat-only row reset. Users, listings and sessions stay intact so a spec can
 * reuse the exported fixtures; conversations, messages, read marks and blocks
 * never leak between tests.
 */
export async function resetChatRows(): Promise<void> {
  await runSql(`
    UPDATE conversations SET last_message_id = NULL, last_message_at = NULL;
    DELETE FROM conversation_reads;
    DELETE FROM messages;
    DELETE FROM conversations;
    DELETE FROM blocks;
  `)
}

export async function applyStorage(page: Page, storagePath: string): Promise<void> {
  const state = JSON.parse(readFileSync(storagePath, 'utf8')) as {
    cookies: Parameters<BrowserContext['addCookies']>[0]
  }
  await page.context().addCookies(state.cookies)
}

export async function openInbox(page: Page, storagePath: string): Promise<void> {
  await applyStorage(page, storagePath)
  await page.goto('/me/conversations')
}

export async function openThread(
  page: Page,
  storagePath: string,
  conversationPublicId: string,
  query = '',
): Promise<void> {
  await applyStorage(page, storagePath)
  await page.goto(`/me/conversations/${encodeURIComponent(conversationPublicId)}${query}`)
}

export function conversationRow(page: Page, conversationPublicId: string) {
  return page.locator('.conversation-row').filter({
    has: page.locator(`a[href="/me/conversations/${conversationPublicId}"]`),
  })
}

export function bubbles(page: Page) {
  return page.locator('.thread__scroller .bubble')
}

export async function bubbleBodies(page: Page): Promise<string[]> {
  return await page.locator('.thread__scroller .bubble__body').allTextContents()
}

export async function expectBubbleBodies(page: Page, expected: string[]): Promise<void> {
  await expect
    .poll(async () => await bubbleBodies(page), { timeout: 30_000 })
    .toEqual(expected)
}

export function composer(page: Page) {
  return page.locator('.composer')
}

export function readRequests(page: Page): string[] {
  const seen: string[] = []
  page.on('request', (request) => {
    if (request.method() === 'POST' && /\/api\/v1\/conversations\/[^/]+\/read$/.test(new URL(request.url()).pathname)) {
      seen.push(request.url())
    }
  })
  return seen
}

export function messagePageRequests(page: Page): string[] {
  const seen: string[] = []
  page.on('request', (request) => {
    if (request.method() === 'GET' && /\/api\/v1\/conversations\/[^/]+\/messages$/.test(new URL(request.url()).pathname)) {
      seen.push(request.url())
    }
  })
  return seen
}

export function inboxRequests(page: Page): string[] {
  const seen: string[] = []
  page.on('request', (request) => {
    if (new URL(request.url()).pathname === '/api/v1/me/conversations') seen.push(request.url())
  })
  return seen
}

export function sentMessages(page: Page): Array<{ body: string }> {
  const payloads: Array<{ body: string }> = []
  page.on('request', (request) => {
    const url = new URL(request.url())
    if (request.method() !== 'POST' || !/\/api\/v1\/conversations\/[^/]+\/messages$/.test(url.pathname)) return
    const data = request.postData()
    if (!data) return
    try {
      payloads.push(JSON.parse(data) as { body: string })
    } catch {
      // A non-JSON body is not a chat send; nothing to record.
    }
  })
  return payloads
}

export function waitForDelay(ms: number): Promise<void> {
  return new Promise((resolveWait) => setTimeout(resolveWait, ms))
}

/**
 * Completes the real OTP login in the browser (no session injection) and waits
 * until the app has navigated away from the login page.
 */
export async function loginViaUi(
  page: Page,
  role: keyof typeof FIXTURE_PHONES = 'buyer',
): Promise<void> {
  const phone = FIXTURE_PHONES[role]
  await expect(page).toHaveURL(/\/login(\?|$)/)
  const phoneField = page.getByLabel('شمارهٔ موبایل', { exact: true })
  const codeField = page.getByLabel('کد تأیید', { exact: true })
  await phoneField.fill(phone)
  const requestCode = page.getByRole('button', { name: 'ارسال کد', exact: true })
  // Back-to-back logins can land inside the configured resend cooldown, so a
  // rejected request is retried instead of failing an unrelated assertion.
  let offset = apiLogSize()
  let codeRequested = false
  for (let attempt = 0; attempt < 3 && !codeRequested; attempt += 1) {
    if (await codeField.isVisible().catch(() => false)) {
      codeRequested = true
      break
    }
    offset = apiLogSize()
    await requestCode.click()
    codeRequested = await codeField
      .waitFor({ state: 'visible', timeout: 5_000 })
      .then(() => true)
      .catch(() => false)
    if (!codeRequested) await page.waitForTimeout(2_500)
  }
  expect(codeRequested).toBe(true)
  const code = await devOtp(offset, phone)
  await codeField.fill(code)
  await page.getByRole('button', { name: 'تأیید', exact: true }).click()
  await expect(page).not.toHaveURL(/\/login(\?|$)/)
}
