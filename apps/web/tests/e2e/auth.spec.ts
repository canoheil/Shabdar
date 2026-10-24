import { existsSync, readFileSync, statSync } from 'node:fs'
import { expect, test, type BrowserContext, type Page } from '@playwright/test'
import { readFixtures } from './support/stack'

function isMeRequest(url: string): boolean {
  return new URL(url).pathname === '/api/v1/me'
}

async function applyStorage(page: Page, storagePath: string): Promise<void> {
  const state = JSON.parse(readFileSync(storagePath, 'utf8')) as {
    cookies: Parameters<BrowserContext['addCookies']>[0]
  }
  await page.context().addCookies(state.cookies)
}

async function waitForOtp(logPath: string, offset: number, lastFour: string): Promise<string> {
  const deadline = Date.now() + 15_000
  while (Date.now() < deadline) {
    if (existsSync(logPath)) {
      const appended = readFileSync(logPath, 'utf8').slice(offset)
      const matches = [...appended.matchAll(new RegExp(`DEV OTP.*${lastFour}: (\\d{6})`, 'g'))]
      const match = matches.at(-1)
      if (match?.[1]) return match[1]
    }
    await new Promise((resolve) => setTimeout(resolve, 100))
  }
  throw new Error('Timed out waiting for the OTP test code')
}

test('@auth authenticated SSR has Account without Login and zero hydration /me requests', async ({ page }) => {
  await applyStorage(page, readFixtures().sellerStorage)
  const meRequests: string[] = []
  page.on('request', (request) => {
    if (isMeRequest(request.url())) meRequests.push(request.url())
  })
  await page.goto('/')
  await expect(page.getByRole('link', { name: 'داشبورد', exact: true })).toBeVisible()
  await expect(page.getByRole('link', { name: 'ورود', exact: true })).toHaveCount(0)
  await expect(page.getByText('خوش آمدید،', { exact: false })).toBeVisible()
  expect(meRequests).toEqual([])
})

test('@auth shell and page share one /me request on client navigation', async ({ page }) => {
  await applyStorage(page, readFixtures().sellerStorage)
  await page.goto('/login')
  const meRequests: string[] = []
  page.on('request', (request) => {
    if (isMeRequest(request.url())) meRequests.push(request.url())
  })
  await page.locator('.shell__brand').click()
  await expect(page.locator('.home')).toBeVisible()
  await expect.poll(() => meRequests.length).toBe(1)
})

test('@auth persistent /me 500 is unavailable with retry, never a false Login', async ({ page }) => {
  let failures = 0
  await page.route('**/api/v1/me', async (route) => {
    failures += 1
    await route.fulfill({
      status: 500,
      contentType: 'application/json',
      body: JSON.stringify({ error: { code: 'internal', message: 'fixture failure' } }),
    })
  })
  await page.goto('/login')
  await page.locator('.shell__brand').click()
  await expect(page.getByText('وضعیت ورود مشخص نیست؛ دوباره تلاش کنید.', { exact: true })).toBeVisible()
  await expect(page.getByRole('link', { name: 'ورود', exact: true })).toHaveCount(0)
  const before = failures
  await page.getByRole('button', { name: 'تلاش دوباره', exact: true }).click()
  await expect.poll(() => failures).toBeGreaterThan(before)
  await expect(page.getByText('وضعیت ورود مشخص نیست؛ دوباره تلاش کنید.', { exact: true })).toBeVisible()
})

test('@auth real 401 establishes the anonymous state', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByText('برای ثبت آگهی وارد شوید.', { exact: true })).toBeVisible()
  await expect(page.getByRole('navigation', { name: 'ناوبری اصلی' }).getByRole('link', { name: 'ورود', exact: true })).toBeVisible()
})

test('@auth stale session on Detail offers login without fetching favorite state', async ({ page }) => {
  const favoriteReads: string[] = []
  page.on('request', (request) => {
    if (new URL(request.url()).pathname.startsWith('/api/v1/me/favorites')) favoriteReads.push(request.url())
  })
  await page.context().addCookies([{
    name: 'shabdar_session',
    value: 'expired-fixture-session',
    url: process.env.E2E_BASE_URL ?? 'http://127.0.0.1:3000',
  }])
  await page.goto(readFixtures().detailPath)
  await expect(page.getByRole('link', { name: 'ورود برای ذخیره', exact: true })).toBeVisible()
  expect(favoriteReads).toEqual([])
})

test('@auth OTP verify refreshes shared auth before navigating Home', async ({ page }) => {
  const meRequestPages: string[] = []
  page.on('request', (request) => {
    if (isMeRequest(request.url())) meRequestPages.push(page.url())
  })
  await page.goto('/login')
  const logPath = process.env.E2E_API_LOG
  if (!logPath) throw new Error('E2E_API_LOG is required')
  const offset = existsSync(logPath) ? statSync(logPath).size : 0
  await page.getByLabel('شمارهٔ موبایل').fill('+989000000101')
  await page.getByRole('button', { name: 'ارسال کد', exact: true }).click()
  const code = await waitForOtp(logPath, offset, '0101')
  await page.getByLabel('کد تأیید').fill(code)
  await page.getByRole('button', { name: 'تأیید', exact: true }).click()
  await expect(page.getByText('خوش آمدید،', { exact: false })).toBeVisible()
  expect(meRequestPages.some((url) => new URL(url).pathname === '/login')).toBe(true)
})
