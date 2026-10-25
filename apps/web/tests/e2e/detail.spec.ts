import { readFileSync } from 'node:fs'
import { expect, test, type BrowserContext, type Page } from '@playwright/test'
import { expectNoOverflow, expectTouchTargets } from './support/quality'
import { apiAs, createListing, readFixtures, removeImagesForFixture } from './support/stack'

async function applyStorage(page: Page, storagePath: string): Promise<void> {
  const state = JSON.parse(readFileSync(storagePath, 'utf8')) as {
    cookies: Parameters<BrowserContext['addCookies']>[0]
  }
  await page.context().addCookies(state.cookies)
}

function isDetailRead(url: string): boolean {
  return new URL(url).pathname.startsWith('/api/v1/listings/')
}

test('@detail eight-image gallery is keyboard operable and responsively composed', async ({ page }) => {
  const title = 'گالری هشت‌تصویرهٔ شبدر'
  const description = 'خط اول توضیحات طولانی آگهی.\nخط دوم با جزئیات واقعی برای بررسی خوانایی و حفظ فاصله‌های متن در صفحهٔ جزئیات.'
  const created = await createListing({ title, description, imageCount: 8, state: 'active' })
  await page.setViewportSize({ width: 375, height: 812 })
  await page.goto(`/listings/${created.slug}`)
  const gallery = page.locator('.listing-gallery')
  const main = gallery.locator('.listing-gallery__main-image')
  await expect(main).toBeVisible()
  await expect(main).toHaveAttribute('width', '1200')
  await expect(main).toHaveAttribute('height', '900')
  await expect(main).toHaveAttribute('loading', 'eager')
  await expect(main).toHaveAttribute('fetchpriority', 'high')
  const thumbnails = gallery.getByRole('button')
  await expect(thumbnails).toHaveCount(8)
  await expect(thumbnails.first()).toHaveAttribute('aria-pressed', 'true')
  const firstSource = await main.getAttribute('src')
  const second = gallery.getByRole('button', { name: 'تصویر 2 از 8', exact: true })
  await second.focus()
  await page.keyboard.press('Enter')
  await expect(second).toHaveAttribute('aria-pressed', 'true')
  await expect(thumbnails.first()).toHaveAttribute('aria-pressed', 'false')
  await expect(main).not.toHaveAttribute('src', firstSource ?? '')
  await expect(main).toHaveAttribute('alt', `${title} — تصویر 2`)
  const third = gallery.getByRole('button', { name: 'تصویر 3 از 8', exact: true })
  await third.focus()
  await page.keyboard.press('Space')
  await expect(third).toHaveAttribute('aria-pressed', 'true')

  const mobileGeometry = await page.evaluate(() => ({
    galleryBottom: document.querySelector('.detail__gallery')?.getBoundingClientRect().bottom,
    summaryTop: document.querySelector('.detail__summary')?.getBoundingClientRect().top,
  }))
  expect(mobileGeometry.galleryBottom ?? 0).toBeLessThanOrEqual((mobileGeometry.summaryTop ?? 0) + 1)
  await expectNoOverflow(page)
  await expectTouchTargets(page)
  await expect(page.locator('.detail__description')).toHaveCSS('white-space', 'pre-line')
  await expect(page.locator('.detail__description')).toContainText('خط دوم')

  await page.setViewportSize({ width: 1024, height: 900 })
  const desktopGeometry = await page.evaluate(() => {
    const galleryRect = document.querySelector('.detail__gallery')?.getBoundingClientRect()
    const summaryRect = document.querySelector('.detail__summary')?.getBoundingClientRect()
    return {
      galleryTop: galleryRect?.top,
      summaryTop: summaryRect?.top,
      galleryWidth: galleryRect?.width,
      summaryWidth: summaryRect?.width,
    }
  })
  expect(desktopGeometry.galleryTop).toBeCloseTo(desktopGeometry.summaryTop ?? -1, 0)
  expect(desktopGeometry.galleryWidth ?? 0).toBeGreaterThan(300)
  expect(desktopGeometry.summaryWidth ?? 0).toBeGreaterThan(250)
  await expectNoOverflow(page)
})

test('@detail empty and broken media have stable neutral fallbacks', async ({ page }) => {
  const created = await createListing({ title: 'آگهی بدون رسانه در جزئیات', state: 'active' })
  await removeImagesForFixture(created.publicId)
  await page.goto(`/listings/${created.slug}`)
  await expect(page.getByText('تصویری برای این آگهی ثبت نشده است.', { exact: true })).toBeVisible()
  await expect(page.locator('.listing-gallery img')).toHaveCount(0)

  await page.route('**/media/**', async (route) => {
    await route.fulfill({ status: 404, body: '' })
  })
  await page.goto(readFixtures().detailPath)
  await expect(page.locator('.listing-gallery__main').getByText('تصویر در دسترس نیست', { exact: true })).toBeVisible()
  await expect(page.locator('.listing-gallery__main img')).toHaveCount(0)
  await expectNoOverflow(page)
})

test('@detail anonymous SSR has no viewer favorite and no hydration detail read', async ({ page }) => {
  const fixtures = readFixtures()
  const response = await page.request.get(fixtures.detailPath)
  expect(response.ok()).toBe(true)
  expect(await response.text()).not.toMatch(/"favorited"\s*:/)
  const detailReads: string[] = []
  page.on('request', (request) => {
    if (isDetailRead(request.url())) detailReads.push(request.url())
  })
  await page.goto(fixtures.detailPath)
  await expect(page.getByRole('link', { name: 'ورود برای ذخیره', exact: true })).toBeVisible()
  expect(detailReads).toEqual([])
})

test('@detail authenticated favorite reads once and announces false true false', async ({ page }) => {
  const fixtures = readFixtures()
  const seller = await apiAs('seller')
  await seller.delete(`/api/v1/me/favorites/${fixtures.listingPublicId}`)
  await seller.dispose()
  await applyStorage(page, fixtures.sellerStorage)
  const detailReads: string[] = []
  page.on('request', (request) => {
    if (isDetailRead(request.url())) detailReads.push(request.url())
  })
  await page.goto(fixtures.detailPath)
  const save = page.getByRole('button', { name: 'ذخیره در علاقه‌مندی‌ها', exact: true })
  const remove = page.getByRole('button', { name: 'حذف از علاقه‌مندی‌ها', exact: true })
  const notice = page.locator('.detail__favorite [role="status"]')
  await expect(save).toBeVisible()
  await expect(save).toHaveAttribute('aria-pressed', 'false')
  await expect.poll(() => detailReads.length).toBe(1)

  await save.click()
  await expect(remove).toBeVisible()
  await expect(remove).toHaveAttribute('aria-pressed', 'true')
  await expect(notice).toHaveText('در علاقه‌مندی‌ها ذخیره شد.')
  await remove.click()
  await expect(save).toBeVisible()
  await expect(save).toHaveAttribute('aria-pressed', 'false')
  await expect(notice).toHaveText('از علاقه‌مندی‌ها حذف شد.')
  expect(detailReads).toHaveLength(1)
})

test('@detail persistent PUT and DELETE failures retain confirmed state until retry succeeds', async ({ page }) => {
  const fixtures = readFixtures()
  const seller = await apiAs('seller')
  await seller.delete(`/api/v1/me/favorites/${fixtures.listingPublicId}`)
  await seller.dispose()
  await applyStorage(page, fixtures.sellerStorage)
  await page.goto(fixtures.detailPath)
  const save = page.getByRole('button', { name: 'ذخیره در علاقه‌مندی‌ها', exact: true })
  const remove = page.getByRole('button', { name: 'حذف از علاقه‌مندی‌ها', exact: true })
  const retry = page.getByRole('button', { name: 'تلاش دوباره', exact: true })
  await expect(save).toBeVisible()

  const failures: string[] = []
  const failMutations = async () => page.route('**/api/v1/me/favorites/**', async (route) => {
    failures.push(route.request().method())
    await route.fulfill({ status: 500, contentType: 'application/json', body: '{"error":{"code":"internal","message":"fixture"}}' })
  })
  await failMutations()
  await save.click()
  await expect(page.getByText('تغییر علاقه‌مندی انجام نشد؛ دوباره تلاش کنید.', { exact: true })).toBeVisible()
  await expect(save).toHaveAttribute('aria-pressed', 'false')
  await retry.click()
  await expect(save).toHaveAttribute('aria-pressed', 'false')
  expect(failures.filter((method) => method === 'PUT')).toHaveLength(2)
  await page.unroute('**/api/v1/me/favorites/**')
  await retry.click()
  await expect(remove).toHaveAttribute('aria-pressed', 'true')

  await failMutations()
  await remove.click()
  await expect(remove).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByText('تغییر علاقه‌مندی انجام نشد؛ دوباره تلاش کنید.', { exact: true })).toBeVisible()
  await retry.click()
  await expect(remove).toHaveAttribute('aria-pressed', 'true')
  expect(failures.filter((method) => method === 'DELETE')).toHaveLength(2)
  await page.unroute('**/api/v1/me/favorites/**')
  await retry.click()
  await expect(save).toHaveAttribute('aria-pressed', 'false')
})

test('@detail favorite 401 returns to login with a live explanation', async ({ page }) => {
  const fixtures = readFixtures()
  const seller = await apiAs('seller')
  await seller.delete(`/api/v1/me/favorites/${fixtures.listingPublicId}`)
  await seller.dispose()
  await applyStorage(page, fixtures.sellerStorage)
  await page.goto(fixtures.detailPath)
  const save = page.getByRole('button', { name: 'ذخیره در علاقه‌مندی‌ها', exact: true })
  await expect(save).toBeVisible()
  await page.route('**/api/v1/me/favorites/**', async (route) => {
    await route.fulfill({ status: 401, contentType: 'application/json', body: '{"error":{"code":"unauthorized","message":"fixture"}}' })
  })
  await save.click()
  await expect(page.getByRole('link', { name: 'ورود برای ذخیره', exact: true })).toBeVisible()
  await expect(page.locator('.detail__favorite [role="status"]')).toHaveText('نشست شما منقضی شده است؛ دوباره وارد شوید.')
})
