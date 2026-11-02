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
  await dialog.getByLabel('کمترین قیمت (تومان)', { exact: true }).fill('1000000')
  await dialog.getByLabel('بیشترین قیمت (تومان)', { exact: true }).fill('2000000')
  await dialog.getByLabel('مرتب‌سازی', { exact: true }).selectOption('price_asc')
  await dialog.getByRole('button', { name: 'انصراف', exact: true }).click()
  await expect(dialog).not.toBeVisible()
  await expect(filterButton).toBeFocused()
  await expect(page).toHaveURL(originalUrl)

  await filterButton.click()
  await expect(dialog.getByLabel('دسته', { exact: true })).toHaveValue('')
  await expect(dialog.getByLabel('کمترین قیمت (تومان)', { exact: true })).toHaveValue('')
  await expect(dialog.getByLabel('بیشترین قیمت (تومان)', { exact: true })).toHaveValue('')
  await expect(dialog.getByLabel('مرتب‌سازی', { exact: true })).toHaveValue('newest')
  await dialog.getByLabel('کمترین قیمت (تومان)', { exact: true }).fill('3000000')
  await dialog.getByLabel('مرتب‌سازی', { exact: true }).selectOption('price_desc')
  await page.keyboard.press('Escape')
  await expect(dialog).not.toBeVisible()
  await expect(filterButton).toBeFocused()
  await expect(page).toHaveURL(originalUrl)
  expect(feedRequests).toEqual([])

  await filterButton.click()
  await dialog.getByLabel('دسته', { exact: true }).selectOption({ index: 1 })
  await dialog.getByLabel('شهر', { exact: true }).selectOption({ index: 1 })
  await dialog.getByLabel('کمترین قیمت (تومان)', { exact: true }).fill('1000000')
  await dialog.getByLabel('بیشترین قیمت (تومان)', { exact: true }).fill('2000000')
  await dialog.getByLabel('مرتب‌سازی', { exact: true }).selectOption('price_asc')
  await dialog.getByRole('button', { name: 'اعمال فیلترها', exact: true }).click()
  await expect(dialog).not.toBeVisible()
  await expect(filterButton).toBeFocused()
  await expect.poll(() => feedRequests.length).toBe(1)
  const query = new URL(page.url()).searchParams
  expect(query.get('category')).toBeTruthy()
  expect(query.get('city')).toBeTruthy()
  expect(query.get('min_price')).toBe('1000000')
  expect(query.get('max_price')).toBe('2000000')
  expect(query.get('sort')).toBe('price_asc')
})

test('@home desktop price filters update results and Back restores values', async ({ page }) => {
  const term = 'آگهی محدوده قیمت خانه'
  await createListing({ title: `${term} ارزان`, priceToman: 1_000_000, imageCount: 0, state: 'active' })
  await createListing({ title: `${term} میانی`, priceToman: 2_000_000, imageCount: 0, state: 'active' })
  await createListing({ title: `${term} گران`, priceToman: 3_000_000, imageCount: 0, state: 'active' })
  await page.setViewportSize({ width: 1024, height: 900 })
  await page.goto(`/?q=${encodeURIComponent(term)}`)
  const cards = page.locator('.listing-card')
  await expect(cards).toHaveCount(3)
  const min = page.getByLabel('کمترین قیمت (تومان)', { exact: true })
  const max = page.getByLabel('بیشترین قیمت (تومان)', { exact: true })

  await min.fill('1500000')
  await min.press('Enter')
  await expect(page).toHaveURL(/[?&]min_price=1500000(?:&|$)/)
  await expect(cards).toHaveCount(2)
  await max.fill('2500000')
  await max.press('Enter')
  await expect(page).toHaveURL(/[?&]max_price=2500000(?:&|$)/)
  await expect(cards).toHaveCount(1)
  await expect(cards.first()).toContainText(`${term} میانی`)

  await page.goBack()
  await expect(cards).toHaveCount(2)
  await expect(min).toHaveValue('1500000')
  await expect(max).toHaveValue('')
  await page.goBack()
  await expect(cards).toHaveCount(3)
  await expect(min).toHaveValue('')
  await expect(max).toHaveValue('')

  await max.fill('0')
  await max.press('Enter')
  await expect(page).toHaveURL(/[?&]max_price=0(?:&|$)/)
  await expect(page.getByText('آگهی‌ای با این مشخصات پیدا نشد.', { exact: true })).toBeVisible()
})

test('@home load more preserves confirmed cards and cursor through failure and deduplicates overlap', async ({ page }) => {
  test.setTimeout(180_000)
  const term = 'آگهی صفحه‌بندی خانه'
  for (let index = 1; index <= 21; index += 1) {
    await createListing({
      title: `${term} ${String(index).padStart(2, '0')}`,
      imageCount: 0,
      state: 'active',
    })
  }
  const firstPageResponse = await page.request.get(`/?q=${encodeURIComponent(term)}`)
  expect(firstPageResponse.ok()).toBe(true)
  const apiFirstPage = await page.request.get(`/api/v1/listings?q=${encodeURIComponent(term)}`)
  expect(apiFirstPage.ok()).toBe(true)
  const firstPageBody = await apiFirstPage.json() as {
    data: Array<Record<string, unknown>>
  }
  const overlappingCard = firstPageBody.data[0]
  expect(overlappingCard).toBeTruthy()

  await page.goto(`/?q=${encodeURIComponent(term)}`)
  const cards = page.locator('.listing-card')
  await expect(cards).toHaveCount(20)
  const confirmedTitles = await cards.locator('h3').allTextContents()
  expect(new Set(confirmedTitles).size).toBe(20)
  const more = page.locator('.home__pagination').getByRole('button')
  await expect(more).toBeVisible()

  let failLoads = true
  const cursors: string[] = []
  await page.route('**/api/v1/listings**', async (route) => {
    const request = route.request()
    const url = new URL(request.url())
    if (request.method() !== 'GET' || url.pathname !== '/api/v1/listings' || !url.searchParams.has('cursor')) {
      await route.continue()
      return
    }
    cursors.push(url.searchParams.get('cursor') ?? '')
    if (failLoads) {
      await route.fulfill({ status: 500, contentType: 'application/json', body: '{"error":{"code":"internal","message":"fixture"}}' })
      return
    }
    const response = await route.fetch()
    const body = await response.json() as { data: Array<Record<string, unknown>> }
    // Simulate a legal overlap and a repeated row without replacing the real
    // second-page cursor metadata or its confirmed API response.
    body.data = [overlappingCard!, ...body.data, ...(body.data[0] ? [body.data[0]] : [])]
    await route.fulfill({ response, json: body })
  })

  await more.click()
  await expect(page.getByText('آگهی‌های بیشتر بارگذاری نشد؛ آگهی‌های فعلی حفظ شدند. دوباره تلاش کنید.', { exact: true })).toBeVisible()
  await expect(cards).toHaveCount(20)
  await more.click()
  // Let the second failed attempt, including the client's GET retry, settle
  // before starting the successful retry. Clicking while it is still disabled
  // races the error renderer instead of exercising a deliberate user retry.
  await expect(more).toBeEnabled()
  await expect(cards).toHaveCount(20)
  expect(cursors.length).toBeGreaterThanOrEqual(2)
  expect(cursors[0]).toBeTruthy()
  expect(new Set(cursors).size).toBe(1)
  const failedAttempts = cursors.length

  failLoads = false
  await more.click()
  await expect(cards).toHaveCount(21)
  await expect(more).toHaveCount(0)
  expect(cursors.length).toBeGreaterThan(failedAttempts)
  expect(new Set(cursors).size).toBe(1)
  const finalTitles = await cards.locator('h3').allTextContents()
  expect(finalTitles.slice(0, 20)).toEqual(confirmedTitles)
  expect(new Set(finalTitles).size).toBe(21)
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
  await expect(noCover.getByText('بررسی‌شده برای انتشار', { exact: true })).toBeVisible()
  await expect(noCover.locator('time')).toHaveAttribute('datetime', /.+/)
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

test('@home mobile cover transfers a real responsive WebP derivative, not the original', async ({ page }) => {
  const title = 'کاور واکنش‌گرای واقعی شبدر'
  await createListing({ title, state: 'active' })
  await page.setViewportSize({ width: 375, height: 812 })
  const mediaRequests: string[] = []
  page.on('request', (request) => {
    if (new URL(request.url()).pathname.startsWith('/media/')) mediaRequests.push(request.url())
  })
  await page.goto(`/?q=${encodeURIComponent(title)}`)
  const image = page.locator('.listing-card').filter({ hasText: title }).locator('img')
  await expect(image).toBeVisible()
  await expect(image).toHaveAttribute('srcset', /--w240\.jpg 240w/)
  await expect(image).toHaveAttribute('srcset', /--w960\.jpg 960w/)
  const source = page.locator('.listing-card').filter({ hasText: title }).locator('source[type="image/webp"]')
  await expect(source).toHaveAttribute('srcset', /--w480\.webp 480w/)
  await expect.poll(() => image.evaluate((element) => element.currentSrc)).toContain('--w480.webp')
  expect(mediaRequests.some((url) => new URL(url).pathname.endsWith('.png'))).toBe(false)
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
