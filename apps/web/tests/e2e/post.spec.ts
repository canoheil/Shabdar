import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { expect, test, type BrowserContext, type Page } from '@playwright/test'
import { apiAs, readFixtures } from './support/stack'

async function applyStorage(page: Page, storagePath: string): Promise<void> {
  const state = JSON.parse(readFileSync(storagePath, 'utf8')) as {
    cookies: Parameters<BrowserContext['addCookies']>[0]
  }
  await page.context().addCookies(state.cookies)
}

interface UrlTracker {
  created: string[]
  revoked: string[]
}

async function installUrlTracker(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const tracker = { created: [] as string[], revoked: [] as string[] }
    Object.assign(window, { __shabdarPreviewUrls: tracker })
    const create = URL.createObjectURL.bind(URL)
    const revoke = URL.revokeObjectURL.bind(URL)
    URL.createObjectURL = (blob: Blob) => {
      const url = create(blob)
      tracker.created.push(url)
      return url
    }
    URL.revokeObjectURL = (url: string) => {
      tracker.revoked.push(url)
      revoke(url)
    }
  })
}

async function readUrlTracker(page: Page): Promise<UrlTracker> {
  return page.evaluate(() => (
    window as unknown as { __shabdarPreviewUrls: UrlTracker }
  ).__shabdarPreviewUrls)
}

function activeUrls(tracker: UrlTracker): string[] {
  return tracker.created.filter((url) => !tracker.revoked.includes(url))
}

const cover = readFileSync(resolve(process.cwd(), 'tests/e2e/fixtures/listing-cover.png'))
const file = (name: string) => ({ name, mimeType: 'image/png', buffer: cover })

test('@post preview object URLs are revoked on replacement, removal and client exit', async ({ page }) => {
  await installUrlTracker(page)
  await applyStorage(page, readFixtures().sellerStorage)
  await page.goto('/post')
  const input = page.getByLabel('تصاویر آگهی', { exact: true })
  await input.setInputFiles([file('one.png'), file('two.png')])
  await expect(page.locator('.post__preview')).toHaveCount(2)
  expect(activeUrls(await readUrlTracker(page))).toHaveLength(2)

  await input.setInputFiles(file('replacement.png'))
  await expect(page.locator('.post__preview')).toHaveCount(1)
  let tracker = await readUrlTracker(page)
  expect(tracker.created).toHaveLength(3)
  expect(tracker.revoked).toHaveLength(2)
  expect(activeUrls(tracker)).toHaveLength(1)

  await page.getByRole('button', { name: 'حذف پیش‌نمایش 1', exact: true }).click()
  await expect(page.locator('.post__preview')).toHaveCount(0)
  tracker = await readUrlTracker(page)
  expect(activeUrls(tracker)).toEqual([])

  await input.setInputFiles([file('three.png'), file('four.png')])
  await expect(page.locator('.post__preview')).toHaveCount(2)
  await page.locator('.shell__brand').click()
  await expect(page.locator('.home')).toBeVisible()
  tracker = await readUrlTracker(page)
  expect(tracker.created).toHaveLength(5)
  expect(tracker.revoked).toHaveLength(5)
  expect(activeUrls(tracker)).toEqual([])
})

test('@post server validation and failure preserve fields and previews without false success', async ({ page }) => {
  await applyStorage(page, readFixtures().sellerStorage)
  await page.goto('/post')
  await page.getByLabel('دسته', { exact: true }).selectOption({ index: 1 })
  await page.getByLabel('شهر', { exact: true }).selectOption({ index: 1 })
  await page.getByLabel('عنوان', { exact: true }).fill('عنوان حفظ‌شده')
  await page.getByLabel('توضیحات', { exact: true }).fill('توضیحات واردشده پس از خطا باید باقی بماند.')
  await page.getByLabel('تصاویر آگهی', { exact: true }).setInputFiles(file('preserve.png'))
  await page.route('**/api/v1/listings', async (route) => {
    if (route.request().method() !== 'POST') {
      await route.continue()
      return
    }
    await route.fulfill({
      status: 400,
      contentType: 'application/json',
      body: JSON.stringify({ error: { code: 'validation', message: 'fixture', fields: { title: 'عنوان معتبر نیست.' } } }),
    })
  })
  await page.getByRole('button', { name: 'ایجاد پیش‌نویس', exact: true }).click()
  await expect(page.getByRole('alert').filter({ hasText: 'خطا در title: عنوان معتبر نیست.' })).toBeFocused()
  await expect(page.locator('#post-title-error')).toHaveText('عنوان معتبر نیست.')
  await expect(page.getByLabel('عنوان', { exact: true })).toHaveValue('عنوان حفظ‌شده')
  await expect(page.getByLabel('عنوان', { exact: true })).toHaveAttribute('aria-invalid', 'true')
  await expect(page.getByLabel('توضیحات', { exact: true })).toHaveValue('توضیحات واردشده پس از خطا باید باقی بماند.')
  await expect(page.locator('.post__preview')).toHaveCount(1)
  await expect(page.getByText('پیش‌نویس ساخته شد؛ برای انتشار، ارسال برای بررسی را بزنید.', { exact: true })).toHaveCount(0)
})

test('@post invalid real images receive Persian feedback without clearing form or previews', async ({ page }) => {
  await applyStorage(page, readFixtures().sellerStorage)
  await page.goto('/post')
  await page.getByLabel('دسته', { exact: true }).selectOption({ index: 1 })
  await page.getByLabel('شهر', { exact: true }).selectOption({ index: 1 })
  await page.getByLabel('عنوان', { exact: true }).fill('عنوان پس از خطای تصویر حفظ می‌شود')
  await page.getByLabel('توضیحات', { exact: true }).fill('توضیحات و هر دو پیش‌نمایش پس از رد فایل خراب باقی می‌مانند.')
  const corrupt = Buffer.concat([cover.subarray(0, 8), Buffer.from('truncated-after-png-signature')])
  await page.getByLabel('تصاویر آگهی', { exact: true }).setInputFiles([
    { name: 'broken.png', mimeType: 'image/png', buffer: corrupt },
    file('still-previewed.png'),
  ])
  await expect(page.locator('.post__preview')).toHaveCount(2)
  await page.getByRole('button', { name: 'ایجاد پیش‌نویس', exact: true }).click()
  const message = 'فایل تصویر معتبر نیست؛ فقط JPEG، PNG یا WebP ثابت و سالم انتخاب کنید.'
  await expect(page.getByRole('alert').filter({ hasText: `خطا در تصاویر: ${message}` })).toBeFocused()
  await expect(page.locator('#post-images-error')).toHaveText(message)
  await expect(page.getByLabel('عنوان', { exact: true })).toHaveValue('عنوان پس از خطای تصویر حفظ می‌شود')
  await expect(page.getByLabel('توضیحات', { exact: true })).toHaveValue('توضیحات و هر دو پیش‌نمایش پس از رد فایل خراب باقی می‌مانند.')
  await expect(page.locator('.post__preview')).toHaveCount(2)
  await expect(page.getByText('پیش‌نویس ساخته شد؛ برای انتشار، ارسال برای بررسی را بزنید.', { exact: true })).toHaveCount(0)
  await expect(page.getByText(/JPEG، PNG یا WebP ثابت/).first()).toBeVisible()
})

test('@post real create upload submit exposes distinct draft and review states', async ({ page }) => {
  await applyStorage(page, readFixtures().sellerStorage)
  await page.goto('/post')
  const title = 'آگهی واقعی فرم شبدر'
  await page.getByLabel('دسته', { exact: true }).selectOption({ index: 1 })
  await page.getByLabel('شهر', { exact: true }).selectOption({ index: 1 })
  await page.getByLabel('عنوان', { exact: true }).fill(title)
  await page.getByLabel('توضیحات', { exact: true }).fill('توضیحات آگهی‌ای که با آپلود واقعی به بررسی ارسال می‌شود.')
  await page.getByLabel('قیمت (تومان)', { exact: true }).fill('12500000')
  await page.getByLabel('تصاویر آگهی', { exact: true }).setInputFiles(file('upload.png'))
  await page.getByRole('button', { name: 'ایجاد پیش‌نویس', exact: true }).click()
  await expect(page.getByText('پیش‌نویس ساخته شد؛ برای انتشار، ارسال برای بررسی را بزنید.', { exact: true })).toBeVisible()
  await expect(page.locator('.post__state')).toHaveText('پیش‌نویس ذخیره شده')
  await page.getByRole('button', { name: 'ارسال برای بررسی', exact: true }).click()
  await expect(page.getByText('آگهی برای بررسی ارسال شد؛ پس از تأیید نمایش داده می‌شود.', { exact: true })).toBeVisible()
  await expect(page.locator('.post__state')).toHaveText('در انتظار بررسی')

  const seller = await apiAs('seller')
  const response = await seller.get('/api/v1/me/listings?status=all')
  expect(response.ok()).toBe(true)
  const body = await response.json() as { data: Array<{ title: string; status: string }> }
  expect(body.data.find((listing) => listing.title === title)?.status).toBe('pending')
  await seller.dispose()
})
