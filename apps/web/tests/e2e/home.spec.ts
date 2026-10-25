import { expect, test } from '@playwright/test'
import { expectNoOverflow, expectNoSeriousA11y, expectTouchTargets } from './support/quality'
import { createListing, removeImagesForFixture } from './support/stack'

function isFeedRequest(url: string): boolean {
  return new URL(url).pathname === '/api/v1/listings'
}

test('@home search-led hierarchy and visible categories work at mobile and desktop', async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 812 })
  await page.goto('/')
  await expect(page.getByRole('heading', { level: 1 })).toContainText('جستجو')
  await expect(page.getByRole('searchbox', { name: 'جستجو', exact: true })).toBeVisible()
  const categories = page.locator('.home__categories')
  await expect(categories).toBeVisible()
  // Seven seeded categories plus the native "همه" action.
  await expect(categories.getByRole('button')).toHaveCount(8)
  await expect(categories.getByRole('button', { name: 'همه', exact: true })).toHaveAttribute('aria-pressed', 'true')
  const firstCategory = categories.getByRole('button').nth(1)
  await firstCategory.click()
  await expect(firstCategory).toHaveAttribute('aria-pressed', 'true')
  await expect(page).toHaveURL(/[?&]category=[^&]+/)
  await expect(page.getByRole('button', { name: 'فیلترها', exact: true })).toBeVisible()
  await expectNoOverflow(page)
  await expectTouchTargets(page)
  await expectNoSeriousA11y(page)

  await page.setViewportSize({ width: 1024, height: 900 })
  const desktopFilters = page.locator('.home__desktop-filters')
  await expect(desktopFilters).toBeVisible()
  await expect(desktopFilters.getByLabel('دسته', { exact: true })).toBeVisible()
  await expect(desktopFilters.getByLabel('شهر', { exact: true })).toBeVisible()
  await expect(desktopFilters.getByLabel('مرتب‌سازی', { exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: 'فیلترها', exact: true })).toHaveCount(0)
  await expectNoOverflow(page)
})

test('@home search debounce, URL state and Back restore controls', async ({ page }) => {
  await page.goto('/')
  const originalUrl = page.url()
  const search = page.getByRole('searchbox', { name: 'جستجو', exact: true })
  const feedRequests: string[] = []
  page.on('request', (request) => {
    if (isFeedRequest(request.url())) feedRequests.push(request.url())
  })
  await search.fill('فرش')
  await expect.poll(() => new URL(page.url()).searchParams.get('q')).toBe('فرش')
  await expect.poll(() => feedRequests.length).toBe(1)
  const searchUrl = page.url()
  const categories = page.locator('.home__categories')
  await categories.getByRole('button').nth(1).click()
  await expect(page).toHaveURL(/[?&]category=[^&]+/)
  await page.goBack()
  await expect(page).toHaveURL(searchUrl)
  await expect(search).toHaveValue('فرش')
  await expect(categories.getByRole('button', { name: 'همه', exact: true })).toHaveAttribute('aria-pressed', 'true')
  await page.goBack()
  await expect(page).toHaveURL(originalUrl)
  await expect(search).toHaveValue('')
})

test('@home mobile filter dialog applies once, discards Cancel/Escape and returns focus', async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 812 })
  await page.goto('/')
  const originalUrl = page.url()
  const filterButton = page.getByRole('button', { name: 'فیلترها', exact: true })
  const dialog = page.getByRole('dialog', { name: 'فیلتر آگهی‌ها', exact: true })
  const feedRequests: string[] = []
  page.on('request', (request) => {
    if (isFeedRequest(request.url())) feedRequests.push(request.url())
  })

  await filterButton.click()
  await expect(dialog).toBeVisible()
  await dialog.getByLabel('دسته', { exact: true }).selectOption({ index: 1 })
  await dialog.getByLabel('شهر', { exact: true }).selectOption({ index: 1 })
  await dialog.getByLabel('مرتب‌سازی', { exact: true }).selectOption('price_asc')
  await dialog.getByRole('button', { name: 'انصراف', exact: true }).click()
  await expect(dialog).not.toBeVisible()
  await expect(filterButton).toBeFocused()
  await expect(page).toHaveURL(originalUrl)

  await filterButton.click()
  await expect(dialog.getByLabel('دسته', { exact: true })).toHaveValue('')
  await expect(dialog.getByLabel('مرتب‌سازی', { exact: true })).toHaveValue('newest')
  await dialog.getByLabel('مرتب‌سازی', { exact: true }).selectOption('price_desc')
  await page.keyboard.press('Escape')
  await expect(dialog).not.toBeVisible()
  await expect(filterButton).toBeFocused()
  await expect(page).toHaveURL(originalUrl)
  expect(feedRequests).toEqual([])

  await filterButton.click()
  await dialog.getByLabel('دسته', { exact: true }).selectOption({ index: 1 })
  await dialog.getByLabel('شهر', { exact: true }).selectOption({ index: 1 })
  await dialog.getByLabel('مرتب‌سازی', { exact: true }).selectOption('price_asc')
  await dialog.getByRole('button', { name: 'اعمال فیلترها', exact: true }).click()
  await expect(dialog).not.toBeVisible()
  await expect(filterButton).toBeFocused()
  await expect.poll(() => feedRequests.length).toBe(1)
  const query = new URL(page.url()).searchParams
  expect(query.get('category')).toBeTruthy()
  expect(query.get('city')).toBeTruthy()
  expect(query.get('sort')).toBe('price_asc')
})

test('@home delayed feed exposes skeleton then filtered search empty state', async ({ page }) => {
  await page.goto('/')
  await page.route('**/api/v1/listings**', async (route) => {
    if (new URL(route.request().url()).pathname !== '/api/v1/listings') {
      await route.continue()
      return
    }
    await new Promise((resolve) => setTimeout(resolve, 700))
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ data: [], page: { next_cursor: null, has_more: false } }),
    })
  })
  await page.getByRole('searchbox', { name: 'جستجو', exact: true }).fill('نتیجه‌ای وجود ندارد')
  await expect(page.locator('.app-skeleton')).toBeVisible()
  await expect(page.locator('.app-skeleton')).toContainText('در حال بارگذاری آگهی‌ها…')
  await expect(page.getByText('آگهی‌ای با این مشخصات پیدا نشد.', { exact: true })).toBeVisible()
  await expect(page.locator('.app-empty-state svg[data-pose="search"]')).toBeVisible()
  await expect(page.getByRole('button', { name: 'پاک کردن فیلترها', exact: true })).toBeVisible()
})

test('@home long titles, neutral no-cover media and one real high-priority cover', async ({ page }) => {
  await createListing({ title: 'آگهی دارای تصویر اولویت‌دار', state: 'active' })
  const longTitle = 'آگهی بدون تصویر با عنوان بسیار طولانی برای بررسی شکستن متن و حفظ چیدمان کارت در شبدر'
  const created = await createListing({ title: longTitle, state: 'active' })
  await removeImagesForFixture(created.publicId)
  await page.goto('/')
  const noCover = page.locator('.listing-card').filter({ hasText: longTitle })
  await expect(noCover).toBeVisible()
  await expect(noCover.getByText('بدون تصویر', { exact: true })).toBeVisible()
  await expect(noCover.locator('img')).toHaveCount(0)
  const highPriority = page.locator('.listing-card img[fetchpriority="high"]')
  await expect(highPriority).toHaveCount(1)
  await expect(highPriority).toHaveAttribute('loading', 'eager')
  await expect(highPriority).toHaveAttribute('width', '480')
  await expect(highPriority).toHaveAttribute('height', '360')
  expect(await highPriority.evaluate((image) => {
    const cards = [...document.querySelectorAll('.listing-card')]
    const firstWithImage = cards.find((card) => card.querySelector('img'))
    return image.closest('article') === firstWithImage
  })).toBe(true)
  await expectNoOverflow(page)
})

test('@home broken covers use a neutral failure placeholder without retry loops', async ({ page }) => {
  const title = 'آگهی با تصویر شکسته در Home'
  await createListing({ title, state: 'active' })
  const mediaRequests: string[] = []
  await page.route('**/media/**', async (route) => {
    mediaRequests.push(route.request().url())
    await route.fulfill({ status: 404, body: '' })
  })
  await page.goto(`/?q=${encodeURIComponent(title)}`)
  const fixtureCard = page.locator('.listing-card').filter({ hasText: title })
  await expect(fixtureCard.getByText('تصویر در دسترس نیست', { exact: true })).toBeVisible()
  await expect(fixtureCard.locator('img')).toHaveCount(0)
  expect(new Set(mediaRequests).size).toBe(mediaRequests.length)
})
