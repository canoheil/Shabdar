import { expect, test } from '@playwright/test'
import { readFixtures } from './support/stack'

test('@fixture-smoke real active fixture renders on Home and Detail without browser errors', async ({
  page,
  request,
}) => {
  const fixtures = readFixtures()
  const browserErrors: string[] = []
  page.on('pageerror', (error) => browserErrors.push(error.message))
  page.on('console', (message) => {
    if (message.type() === 'error') browserErrors.push(message.text())
  })

  const homePath = `/?q=${encodeURIComponent('آگهی آزمون شبدر')}`
  const homeResponse = await request.get(homePath)
  expect(homeResponse.ok()).toBe(true)
  expect(await homeResponse.text()).toContain('آگهی آزمون شبدر')

  await page.goto(homePath)
  await expect(page.getByRole('link', { name: 'آگهی آزمون شبدر' }).first()).toBeVisible()
  await page.goto(fixtures.detailPath)
  await expect(page.getByRole('heading', { name: 'آگهی آزمون شبدر', exact: true })).toBeVisible()
  const mainImage = page.locator('.listing-gallery__main-image').first()
  await expect(mainImage).toBeVisible()
  await expect(mainImage).toHaveAttribute('alt', /آگهی آزمون شبدر/)
  expect(browserErrors).toEqual([])
})
