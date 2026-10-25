import { readFileSync } from 'node:fs'
import { expect, test, type BrowserContext, type Page } from '@playwright/test'
import {
  apiAs,
  createListing,
  readFixtures,
  type FixtureRole,
} from './support/stack'

interface RatingState {
  rating_average: number
  rating_count: number
  my_stars: number | null
}

async function applyStorage(page: Page, storagePath: string): Promise<void> {
  const state = JSON.parse(readFileSync(storagePath, 'utf8')) as {
    cookies: Parameters<BrowserContext['addCookies']>[0]
  }
  await page.context().addCookies(state.cookies)
}

function fixtureSlug(): string {
  const segment = readFixtures().detailPath.split('/').at(-1)
  if (!segment) throw new Error('Detail fixture slug is missing')
  return decodeURIComponent(segment)
}

async function mutateRating(
  role: FixtureRole,
  slug: string,
  method: 'PUT' | 'DELETE',
  stars?: number,
): Promise<RatingState> {
  const api = await apiAs(role)
  try {
    const response = await api.fetch(`/api/v1/listings/${encodeURIComponent(slug)}/rating`, {
      method,
      data: method === 'PUT' ? { stars } : undefined,
    })
    expect(response.ok(), `${role} ${method} rating`).toBe(true)
    return await response.json() as RatingState
  } finally {
    await api.dispose()
  }
}

async function clearFixtureRatings(): Promise<void> {
  const slug = fixtureSlug()
  for (const role of ['admin', 'buyer', 'buyerTwo'] as const) {
    await mutateRating(role, slug, 'DELETE')
  }
}

function productJsonLd(html: string): Record<string, unknown> {
  const scripts = [...html.matchAll(/<script[^>]*type="application\/ld\+json"[^>]*>(.*?)<\/script>/gs)]
  for (const match of scripts) {
    const value = JSON.parse(match[1] ?? '{}') as Record<string, unknown>
    if (value['@type'] === 'Product') return value
  }
  throw new Error('Product JSON-LD is missing')
}

test('@rating public SSR is viewer-independent and AggregateRating starts at three real ratings', async ({ page }) => {
  await clearFixtureRatings()
  const fixtures = readFixtures()
  const slug = fixtureSlug()

  let response = await page.request.get(fixtures.detailPath)
  expect(response.ok()).toBe(true)
  let html = await response.text()
  expect(html).not.toMatch(/"my_stars"\s*:/)
  expect(html).not.toContain('role="radiogroup"')
  expect(productJsonLd(html)).not.toHaveProperty('aggregateRating')

  await mutateRating('admin', slug, 'PUT', 5)
  await mutateRating('buyer', slug, 'PUT', 3)
  response = await page.request.get(fixtures.detailPath)
  html = await response.text()
  expect(productJsonLd(html)).not.toHaveProperty('aggregateRating')

  const third = await mutateRating('buyerTwo', slug, 'PUT', 1)
  expect(third.rating_count).toBe(3)
  expect(third.rating_average).toBe(3)
  response = await page.request.get(fixtures.detailPath)
  html = await response.text()
  expect(productJsonLd(html).aggregateRating).toEqual({
    '@type': 'AggregateRating',
    ratingValue: 3,
    ratingCount: 3,
    bestRating: 5,
    worstRating: 1,
  })

  await page.goto(fixtures.detailPath)
  await expect(page.locator('.listing-rating__summary')).toHaveText('۳ از ۵ · ۳ امتیاز')
  await expect(page.getByRole('radiogroup')).toHaveCount(0)
  await expect(page.getByRole('link', { name: 'ورود برای امتیازدهی', exact: true })).toBeVisible()
  await clearFixtureRatings()
})

test('@rating keyboard create update reload and delete preserve exact summary state', async ({ page }) => {
  await clearFixtureRatings()
  await applyStorage(page, readFixtures().adminStorage)
  await page.goto(readFixtures().detailPath)
  const group = page.getByRole('radiogroup', { name: 'امتیاز شما', exact: true })
  await expect(group).toBeVisible()
  await expect(page.locator('.listing-rating__summary')).toHaveText('هنوز امتیازی ثبت نشده است')

  let four = page.getByRole('radio', { name: '۴ از ۵ ستاره', exact: true })
  await four.focus()
  await page.keyboard.press('Enter')
  await expect(four).toHaveAttribute('aria-checked', 'true')
  await expect(page.locator('.listing-rating__summary')).toHaveText('۴ از ۵ · ۱ امتیاز')

  await page.reload()
  four = page.getByRole('radio', { name: '۴ از ۵ ستاره', exact: true })
  await expect(four).toHaveAttribute('aria-checked', 'true')
  const two = page.getByRole('radio', { name: '۲ از ۵ ستاره', exact: true })
  await two.focus()
  await page.keyboard.press('Space')
  await expect(two).toHaveAttribute('aria-checked', 'true')
  await expect(four).toHaveAttribute('aria-checked', 'false')
  await expect(page.locator('.listing-rating__summary')).toHaveText('۲ از ۵ · ۱ امتیاز')

  await page.getByRole('button', { name: 'حذف امتیاز', exact: true }).click()
  await expect(page.locator('.listing-rating__summary')).toHaveText('هنوز امتیازی ثبت نشده است')
  await expect(page.locator('[role="radio"][aria-checked="true"]')).toHaveCount(0)
  await expect(page.getByRole('button', { name: 'حذف امتیاز', exact: true })).toHaveCount(0)
})

test('@rating persistent mutation failures keep confirmed state until the same retry succeeds', async ({ page }) => {
  await clearFixtureRatings()
  await applyStorage(page, readFixtures().adminStorage)
  await page.goto(readFixtures().detailPath)
  const rating = page.locator('.listing-rating')
  const five = rating.getByRole('radio', { name: '۵ از ۵ ستاره', exact: true })
  const retry = rating.getByRole('button', { name: 'تلاش دوباره', exact: true })
  const endpoint = '**/api/v1/listings/*/rating'
  const failedMethods: string[] = []
  const failMutations = () => page.route(endpoint, async (route) => {
    failedMethods.push(route.request().method())
    await route.fulfill({
      status: 500,
      contentType: 'application/json',
      body: '{"error":{"code":"internal","message":"fixture"}}',
    })
  })

  await failMutations()
  await five.click()
  await expect(rating.getByText('تغییر امتیاز انجام نشد؛ دوباره تلاش کنید.', { exact: true })).toBeVisible()
  await expect(five).toHaveAttribute('aria-checked', 'false')
  await retry.click()
  await expect(five).toHaveAttribute('aria-checked', 'false')
  expect(failedMethods.filter((method) => method === 'PUT')).toHaveLength(2)
  await page.unroute(endpoint)
  await retry.click()
  await expect(five).toHaveAttribute('aria-checked', 'true')
  await expect(rating.locator('.listing-rating__summary')).toHaveText('۵ از ۵ · ۱ امتیاز')

  await failMutations()
  await rating.getByRole('button', { name: 'حذف امتیاز', exact: true }).click()
  await expect(five).toHaveAttribute('aria-checked', 'true')
  await retry.click()
  await expect(five).toHaveAttribute('aria-checked', 'true')
  expect(failedMethods.filter((method) => method === 'DELETE')).toHaveLength(2)
  await page.unroute(endpoint)
  await retry.click()
  await expect(five).toHaveAttribute('aria-checked', 'false')
  await expect(rating.locator('.listing-rating__summary')).toHaveText('هنوز امتیازی ثبت نشده است')
})

test('@rating seller exclusion and expired-session recovery never fake a successful mutation', async ({ page }) => {
  await clearFixtureRatings()
  const fixtures = readFixtures()
  await applyStorage(page, fixtures.sellerStorage)
  await page.goto(fixtures.detailPath)
  await expect(page.getByText('امتیازدهی به آگهی خودتان ممکن نیست.', { exact: true })).toBeVisible()
  await expect(page.getByRole('radiogroup')).toHaveCount(0)

  await applyStorage(page, fixtures.adminStorage)
  await page.goto(fixtures.detailPath)
  const rating = page.locator('.listing-rating')
  await page.route('**/api/v1/listings/*/rating', async (route) => {
    await route.fulfill({
      status: 401,
      contentType: 'application/json',
      body: '{"error":{"code":"unauthorized","message":"fixture"}}',
    })
  })
  await rating.getByRole('radio', { name: '۳ از ۵ ستاره', exact: true }).click()
  await expect(rating.getByRole('link', { name: 'ورود برای امتیازدهی', exact: true })).toBeVisible()
  await expect(rating.getByRole('status')).toHaveText('نشست شما منقضی شده است؛ دوباره وارد شوید.')
  await expect(rating.locator('[aria-checked="true"]')).toHaveCount(0)
})

test('@rating popular sort uses counters committed by real rating mutations', async ({ page }) => {
  const token = `مرتب‌سازی امتیاز ${Date.now()}`
  const high = await createListing({ title: `${token} بالا`, state: 'active' })
  const low = await createListing({ title: `${token} پایین`, state: 'active' })
  for (const role of ['admin', 'buyer', 'buyerTwo'] as const) {
    await mutateRating(role, high.slug, 'PUT', 5)
    await mutateRating(role, low.slug, 'PUT', 1)
  }

  const query = new URLSearchParams({ q: token, sort: 'popular' })
  const response = await page.request.get(`/api/v1/listings?${query}`)
  expect(response.ok()).toBe(true)
  const body = await response.json() as { data: Array<{ public_id: string; rating_count: number }> }
  expect(body.data.map((item) => item.public_id)).toEqual([high.publicId, low.publicId])
  expect(body.data.map((item) => item.rating_count)).toEqual([3, 3])

  await page.goto(`/?${query}`)
  await expect(page.locator('.listing-card')).toHaveCount(2)
  await expect(page.locator('.listing-card__title').first()).toHaveText(`${token} بالا`)
  await expect(page.locator('.listing-card__title').nth(1)).toHaveText(`${token} پایین`)
})
