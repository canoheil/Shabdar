import { readFileSync } from 'node:fs'
import { expect, test, type BrowserContext, type Page } from '@playwright/test'
import { expectNoOverflow, expectNoSeriousA11y, expectTouchTargets } from './support/quality'
import { readFixtures } from './support/stack'

function navigation(page: Page) {
  return page.getByRole('navigation', { name: 'ناوبری اصلی' })
}

function isFeedRequest(url: string): boolean {
  return new URL(url).pathname === '/api/v1/listings'
}

async function applyStorage(page: Page, storagePath: string): Promise<void> {
  const state = JSON.parse(readFileSync(storagePath, 'utf8')) as {
    cookies: Parameters<BrowserContext['addCookies']>[0]
  }
  await page.context().addCookies(state.cookies)
}

test('@navigation mobile has five safe bottom actions and one current page', async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 812 })
  await page.goto('/')
  const nav = navigation(page)
  await expect(nav.locator('.shell__nav-action:visible')).toHaveCount(5)
  for (const name of ['خانه', 'جستجو', 'ثبت آگهی', 'علاقه‌مندی‌ها', 'ورود']) {
    await expect(nav.getByText(name, { exact: true })).toBeVisible()
  }
  await expect(nav.getByText('پیام‌ها', { exact: true })).toHaveCount(0)
  await expect(nav.locator('[aria-current="page"]')).toHaveCount(1)
  await expect(nav.locator('[aria-current="page"]')).toHaveText('خانه')
  const inset = await page.evaluate(() => {
    const navElement = document.querySelector('.shell__nav')
    const main = document.querySelector('.shell__main')
    const footer = document.querySelector('.shell__footer')
    if (!navElement || !main || !footer) throw new Error('Shell regions are missing')
    return {
      position: getComputedStyle(navElement).position,
      navHeight: navElement.getBoundingClientRect().height,
      mainBottom: Number.parseFloat(getComputedStyle(main).paddingBottom),
      footerBottom: Number.parseFloat(getComputedStyle(footer).paddingBottom),
    }
  })
  expect(inset.position).toBe('fixed')
  expect(inset.mainBottom).toBeGreaterThanOrEqual(inset.navHeight)
  expect(inset.footerBottom).toBeGreaterThanOrEqual(inset.navHeight)
  await expectNoOverflow(page)
  await expectTouchTargets(page)
  await expectNoSeriousA11y(page)
})

test('@navigation tablet header uses two rows without overflow', async ({ page }) => {
  await page.setViewportSize({ width: 768, height: 1024 })
  await page.goto('/')
  const boxes = await page.evaluate(() => {
    const brand = document.querySelector('.shell__brand')?.getBoundingClientRect()
    const nav = document.querySelector('.shell__nav')
    const navRect = nav?.getBoundingClientRect()
    if (!brand || !nav || !navRect) throw new Error('Shell regions are missing')
    return {
      brandBottom: brand.bottom,
      navTop: navRect.top,
      position: getComputedStyle(nav).position,
    }
  })
  expect(boxes.position).not.toBe('fixed')
  expect(boxes.navTop).toBeGreaterThanOrEqual(boxes.brandBottom - 1)
  await expect(navigation(page).locator('.shell__nav-action:visible')).toHaveCount(5)
  await expectNoOverflow(page)
})

test('@navigation desktop uses a right RTL sidebar at 1024 and 1440', async ({ page }) => {
  for (const width of [1024, 1440]) {
    await page.setViewportSize({ width, height: 900 })
    await page.goto('/')
    const geometry = await page.evaluate(() => {
      const header = document.querySelector('.shell__header')
      const main = document.querySelector('.shell__main')
      if (!header || !main) throw new Error('Shell regions are missing')
      const headerRect = header.getBoundingClientRect()
      const mainRect = main.getBoundingClientRect()
      return {
        position: getComputedStyle(header).position,
        headerWidth: headerRect.width,
        headerRight: headerRect.right,
        headerLeft: headerRect.left,
        mainRight: mainRect.right,
        viewport: window.innerWidth,
      }
    })
    expect(geometry.position).toBe('fixed')
    expect(geometry.headerWidth).toBeCloseTo(272, 0)
    expect(geometry.headerRight).toBeCloseTo(geometry.viewport, 0)
    expect(geometry.mainRight).toBeLessThanOrEqual(geometry.headerLeft + 1)
    await expect(navigation(page).getByRole('link', { name: 'صف بررسی', exact: true })).toHaveCount(0)
    await expectNoOverflow(page)
  }
})

test('@navigation Queue is desktop-only and admin-only', async ({ page }) => {
  await applyStorage(page, readFixtures().adminStorage)
  await page.setViewportSize({ width: 1024, height: 900 })
  await page.goto('/')
  const nav = navigation(page)
  await expect(nav.getByRole('link', { name: 'صف بررسی', exact: true })).toBeVisible()
  await expect(nav.getByRole('link', { name: 'داشبورد', exact: true })).toBeVisible()
  await page.setViewportSize({ width: 375, height: 812 })
  await expect(nav.getByRole('link', { name: 'صف بررسی', exact: true })).toHaveCount(0)
  await expect(nav.locator('.shell__nav-action:visible')).toHaveCount(5)
})

test('@navigation same-Home Search only focuses and preserves feed and filters', async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 812 })
  await page.goto('/?q=%D8%A2%DA%AF%D9%87%DB%8C&sort=price_asc')
  const search = page.getByRole('searchbox', { name: 'جستجو', exact: true })
  await expect(search).toHaveValue('آگهی')
  const originalUrl = page.url()
  const feedRequests: string[] = []
  page.on('request', (request) => {
    if (isFeedRequest(request.url())) feedRequests.push(request.url())
  })
  await navigation(page).getByRole('button', { name: 'جستجو', exact: true }).click()
  await expect(search).toBeFocused()
  await expect(page).toHaveURL(originalUrl)
  await expect(search).toHaveValue('آگهی')
  await page.waitForTimeout(500)
  expect(feedRequests).toEqual([])
})

test('@navigation cross-page Search performs one feed fetch and focuses Home search', async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 812 })
  await page.goto(readFixtures().detailPath)
  const feedRequests: string[] = []
  page.on('request', (request) => {
    if (isFeedRequest(request.url())) feedRequests.push(request.url())
  })
  await navigation(page).getByRole('button', { name: 'جستجو', exact: true }).click()
  const search = page.getByRole('searchbox', { name: 'جستجو', exact: true })
  await expect(search).toBeFocused()
  await expect(page).toHaveURL(/\/#search$/)
  await expect.poll(() => feedRequests.length).toBe(1)
  const nav = navigation(page)
  await expect(nav.locator('[aria-current="page"]')).toHaveCount(1)
  await expect(nav.locator('[aria-current="page"]')).toHaveText('خانه')
  await expect(nav.getByRole('button', { name: 'جستجو', exact: true })).not.toHaveAttribute('aria-current')
})
