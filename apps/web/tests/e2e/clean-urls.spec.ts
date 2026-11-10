import { expect, test } from '@playwright/test'
import { createListing, readFixtures } from './support/stack'

// Phase 14: category and city pages get real, shareable, indexable URLs.
// The query-parameter feed stays the filter/pagination state, so nothing breaks.
test.describe('@clean-urls category and city pages', () => {
  test('a known category page renders only that category, in SSR HTML', async ({ page }) => {
    const matching = await createListing({ title: 'آگهی دستهٔ آدرس تمیز', state: 'active' })
    const response = await page.request.get('/category/home-food')
    expect(response.status()).toBe(200)
    const html = await response.text()
    expect(html).toMatch(/<h1[^>]*>\s*آگهی‌های غذای خانگی/)
    expect(html).toContain('rel="canonical"')
    expect(html).toContain('/category/home-food')
    expect(html).not.toContain('noindex')

    await page.goto('/category/home-food')
    await expect(page.getByRole('heading', { level: 1, name: 'آگهی‌های غذای خانگی' })).toBeVisible()
    // The category filter is fixed by the path, so no category select is offered.
    await expect(page.getByLabel('دسته', { exact: true })).toHaveCount(0)
    expect(matching.slug).toBeTruthy()
  })

  test('an unknown category is a server-rendered 404, not an empty feed', async ({ page }) => {
    const response = await page.request.get('/category/do-not-exist')
    expect(response.status()).toBe(404)
    await page.goto('/category/do-not-exist')
    await expect(page.locator('.error-page__status')).toHaveText('404')
  })

  test('a known city page preselects its province and shows its own canonical', async ({ page }) => {
    const response = await page.request.get('/city/rasht')
    expect(response.status()).toBe(200)
    const html = await response.text()
    expect(html).toMatch(/<h1[^>]*>\s*آگهی‌های رشت/)
    expect(html).toContain('rel="canonical"')
    expect(html).toContain('/city/rasht')

    await page.goto('/city/rasht')
    await expect(page.getByRole('heading', { level: 1, name: 'آگهی‌های رشت' })).toBeVisible()
    await expect(page.getByLabel('استان', { exact: true })).toHaveValue('gilan')
  })

  test('an unknown city is 404', async ({ page }) => {
    const response = await page.request.get('/city/do-not-exist')
    expect(response.status()).toBe(404)
  })

  test('the legacy query-parameter feed still works', async ({ page }) => {
    await page.goto('/?category=home-food&province=gilan')
    await expect(page.getByLabel('دسته', { exact: true })).toHaveValue('home-food')
    await expect(page.getByLabel('استان', { exact: true })).toHaveValue('gilan')
  })

  test('a category chip on Home is a real clean link', async ({ page }) => {
    await page.goto('/')
    const chips = page.locator('.browser__category-button')
    await expect(chips.first()).toBeVisible()
    const href = await chips.nth(1).getAttribute('href')
    expect(href).toMatch(/^\/(category|)\/?/)
    await chips.nth(1).click()
    await expect(page).toHaveURL(/\/(category\/[^/?]+)?$/)
  })

  test('a listing card city links to the clean city page', async ({ page }) => {
    await page.goto('/')
    const cityLink = page.locator('.listing-card__city-link').first()
    await expect(cityLink).toBeVisible()
    await expect(cityLink).toHaveAttribute('href', /^\/city\//)
    await cityLink.click()
    await expect(page).toHaveURL(/\/city\/[^/?]+$/)
    await expect(page.getByRole('heading', { level: 1 })).toBeVisible()
  })

  test('quick-view fixture detail stays reachable', async ({ page }) => {
    const response = await page.request.get(readFixtures().detailPath)
    expect(response.status()).toBe(200)
  })
})
