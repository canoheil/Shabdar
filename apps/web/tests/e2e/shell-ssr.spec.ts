import { readFileSync } from 'node:fs'
import { expect, test } from '@playwright/test'
import { readFixtures } from './support/stack'

const nav = '.shell__nav'
const actions = ['خانه', 'جستجو', 'ثبت', 'پیام‌ها', 'ذخیره‌ها', 'ورود']

test('public pages return SSR navigation icons even without JavaScript', async ({ browser, request, baseURL }) => {
  const context = await browser.newContext({ baseURL, javaScriptEnabled: false })
  const page = await context.newPage()
  try {
    for (const path of ['/', '/privacy', readFixtures().detailPath]) {
      const response = await request.get(path)
      expect(response.status(), path).toBe(200)
      const html = await response.text()
      expect(html).toContain('class="shell__nav"')
      const browserResponse = await page.goto(path)
      expect(browserResponse?.status(), path).toBe(200)
      const navigation = page.locator(nav)
      for (const label of actions) {
        const action = navigation.locator('.shell__nav-action').filter({ hasText: label })
        await expect(action.locator('svg path')).toHaveCount(1)
        await expect(action.locator('svg')).toHaveAttribute('aria-hidden', 'true')
      }
    }
  } finally {
    await context.close()
  }
})

test('shell hydrates without mismatch for anonymous and admin navigation', async ({ browser, baseURL }) => {
  const admin = JSON.parse(readFileSync(readFixtures().adminStorage, 'utf8'))
  for (const storageState of [undefined, admin]) {
    const context = await browser.newContext({ baseURL, storageState })
    const page = await context.newPage()
    const errors: string[] = []
    page.on('console', message => {
      if (/hydration|hydrating/i.test(message.text())) errors.push(message.text())
    })
    page.on('pageerror', error => errors.push(error.message))
    try {
      const response = await page.goto('/')
      expect(response?.status()).toBe(200)
      await expect(page.locator(`${nav} .shell__nav-action svg`).first()).toBeVisible()
      if (storageState) {
        await expect(page.getByRole('link', { name: 'صف بررسی', exact: true })).toHaveCount(1)
        await expect(page.locator('.shell__nav-item--admin svg path')).toHaveCount(6)
      }
      await page.getByRole('button', { name: 'جستجو', exact: true }).click()
      await expect(page.getByRole('searchbox', { name: 'جستجو', exact: true })).toBeFocused()
      expect(errors).toEqual([])
    } finally {
      await context.close()
    }
  }
})
