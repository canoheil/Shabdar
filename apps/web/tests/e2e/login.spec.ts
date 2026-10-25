import { existsSync, readFileSync, statSync } from 'node:fs'
import { expect, test } from '@playwright/test'
import { expectNoOverflow, expectNoSeriousA11y, expectTouchTargets } from './support/quality'

async function waitForOtp(offset: number, lastFour = '0101'): Promise<string> {
  const logPath = process.env.E2E_API_LOG
  if (!logPath) throw new Error('E2E_API_LOG is required')
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
  throw new Error('Timed out waiting for OTP')
}

function logOffset(): number {
  const logPath = process.env.E2E_API_LOG
  if (!logPath) throw new Error('E2E_API_LOG is required')
  return existsSync(logPath) ? statSync(logPath).size : 0
}

test('@login focused panel preserves geometry, purpose, editable phone and resend countdown', async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 812 })
  await page.goto('/login')
  const panel = page.locator('.login__panel')
  await expect(panel).toBeVisible()
  await expect(panel.locator('svg[data-pose="full"]')).toBeVisible()
  const loginPurpose = page.getByRole('button', { name: 'ورود', exact: true })
  const registerPurpose = page.getByRole('button', { name: 'ثبت‌نام', exact: true })
  await expect(loginPurpose).toHaveAttribute('aria-pressed', 'true')
  await registerPurpose.click()
  await expect(registerPurpose).toHaveAttribute('aria-pressed', 'true')
  await loginPurpose.click()
  const before = await panel.boundingBox()
  const offset = logOffset()
  await page.getByLabel('شمارهٔ موبایل', { exact: true }).fill('+989000000101')
  await page.getByRole('button', { name: 'ارسال کد', exact: true }).click()
  const code = page.getByLabel('کد تأیید', { exact: true })
  await expect(code).toBeVisible()
  await expect(code).toHaveAttribute('autocomplete', 'one-time-code')
  await expect(code).toHaveAttribute('inputmode', 'numeric')
  expect(await waitForOtp(offset)).toMatch(/^\d{6}$/)
  const after = await panel.boundingBox()
  expect(after?.x).toBeCloseTo(before?.x ?? -1, 0)
  expect(after?.width).toBeCloseTo(before?.width ?? -1, 0)
  expect(after?.height).toBeCloseTo(before?.height ?? -1, 0)
  const resend = page.getByRole('button', { name: /ارسال مجدد کد/ })
  await expect(resend).toBeDisabled()
  await expect(resend).toContainText('ثانیه')
  await expect(resend).toBeEnabled({ timeout: 5000 })
  await page.getByRole('button', { name: 'ویرایش شماره', exact: true }).click()
  await expect(page.getByLabel('شمارهٔ موبایل', { exact: true })).toHaveValue('+989000000101')
  await expect(code).toHaveCount(0)
  await expectNoOverflow(page)
  await expectTouchTargets(page)
  await expectNoSeriousA11y(page)
})

test('@login invalid-phone and rate-limit errors receive focus and remain associated', async ({ page }) => {
  await page.goto('/login')
  const phone = page.getByLabel('شمارهٔ موبایل', { exact: true })
  const submit = page.getByRole('button', { name: 'ارسال کد', exact: true })
  await phone.fill('123')
  await page.route('**/api/v1/auth/otp/request', async (route) => {
    await route.fulfill({ status: 400, contentType: 'application/json', body: '{"error":{"code":"invalid_phone","message":"fixture"}}' })
  })
  await submit.click()
  const invalid = page.getByRole('alert').filter({ hasText: 'شمارهٔ موبایل معتبر نیست.' })
  await expect(invalid).toBeFocused()
  await expect(phone).toHaveAttribute('aria-invalid', 'true')
  expect(await phone.getAttribute('aria-describedby')).toContain('login-error')

  await phone.fill('+989000000101')
  await page.unroute('**/api/v1/auth/otp/request')
  await page.route('**/api/v1/auth/otp/request', async (route) => {
    await route.fulfill({ status: 429, contentType: 'application/json', body: '{"error":{"code":"rate_limited","message":"fixture"}}' })
  })
  await submit.click()
  const limited = page.getByRole('alert').filter({ hasText: 'درخواست‌های زیاد؛ کمی صبر کنید.' })
  await expect(limited).toBeFocused()
  await expect(phone).toHaveValue('+989000000101')
})

test('@login invalid-code error is focused without clearing the entered code', async ({ page }) => {
  await page.goto('/login')
  await page.getByLabel('شمارهٔ موبایل', { exact: true }).fill('+989000000101')
  await page.getByRole('button', { name: 'ارسال کد', exact: true }).click()
  const code = page.getByLabel('کد تأیید', { exact: true })
  await expect(code).toBeVisible()
  await page.route('**/api/v1/auth/otp/verify', async (route) => {
    await route.fulfill({ status: 400, contentType: 'application/json', body: '{"error":{"code":"invalid_code","message":"fixture"}}' })
  })
  await code.fill('000000')
  await page.getByRole('button', { name: 'تأیید', exact: true }).click()
  await expect(page.getByRole('alert').filter({ hasText: 'کد واردشده نادرست است.' })).toBeFocused()
  await expect(code).toHaveValue('000000')
  await expect(code).toHaveAttribute('aria-invalid', 'true')
})

test('@login real OTP clipboard paste verifies and reaches authenticated Home', async ({ page }) => {
  await page.context().grantPermissions(['clipboard-read', 'clipboard-write'])
  await page.goto('/login')
  const offset = logOffset()
  await page.getByRole('button', { name: 'ثبت‌نام', exact: true }).click()
  await page.getByLabel('شمارهٔ موبایل', { exact: true }).fill('+989000000199')
  await page.getByRole('button', { name: 'ارسال کد', exact: true }).click()
  const value = await waitForOtp(offset, '0199')
  const code = page.getByLabel('کد تأیید', { exact: true })
  await page.evaluate((text) => navigator.clipboard.writeText(text), value)
  await code.focus()
  await page.keyboard.press('Control+V')
  await expect(code).toHaveValue(value)
  await page.getByRole('button', { name: 'تأیید', exact: true }).click()
  await expect(page.getByText('خوش آمدید،', { exact: false })).toBeVisible()
})
