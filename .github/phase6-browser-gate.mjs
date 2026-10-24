import assert from 'node:assert/strict'
import { chromium } from 'playwright'

const base = 'http://127.0.0.1:8080'
const detailPath = '/listings/favorite-browser'
const session = 'a'.repeat(64)
const csrf = 'phase6-browser-csrf'
const cookieHeader = `shabdar_session=${session}; shabdar_csrf=${csrf}`
const saveLabel = 'ذخیره در علاقه‌مندی‌ها'
const removeLabel = 'حذف از علاقه‌مندی‌ها'
const loginLabel = 'ورود برای ذخیره'

async function addSession(context, value = session) {
  await context.addCookies([
    { name: 'shabdar_session', value, domain: '127.0.0.1', path: '/', httpOnly: true },
    { name: 'shabdar_csrf', value: csrf, domain: '127.0.0.1', path: '/' },
  ])
}

async function waitToggle(page, label) {
  const locator = page.getByRole('button', { name: label, exact: true })
  await locator.waitFor({ state: 'visible' })
  return locator
}

async function assertLayout(page) {
  const dimensions = await page.evaluate(() => ({
    dir: document.documentElement.dir,
    scrollWidth: document.documentElement.scrollWidth,
    innerWidth: window.innerWidth,
  }))
  assert.equal(dimensions.dir, 'rtl')
  assert.ok(
    dimensions.scrollWidth <= dimensions.innerWidth,
    `horizontal overflow: ${JSON.stringify(dimensions)}`,
  )
  const toggle = page.locator('button[aria-pressed]')
  const box = await toggle.boundingBox()
  assert.ok(box && box.height >= 44, `favorite target is not 44px: ${JSON.stringify(box)}`)
}

const browser = await chromium.launch({ headless: true })
let activePage
try {
  const anonymous = await browser.newContext({ viewport: { width: 375, height: 812 } })
  const anonymousPage = await anonymous.newPage()
  activePage = anonymousPage
  anonymousPage.setDefaultTimeout(10_000)
  await anonymousPage.goto(base + detailPath)
  await anonymousPage.getByRole('heading', { name: 'آگهی آزمون علاقه‌مندی' }).waitFor()
  await anonymousPage.getByRole('link', { name: loginLabel, exact: true }).waitFor()
  assert.equal(await anonymousPage.locator('button[aria-pressed]').count(), 0)
  await anonymous.close()
  console.log('PASS anonymous login gate and no save button')

  const stale = await browser.newContext({ viewport: { width: 375, height: 812 } })
  await addSession(stale, 'stale-session')
  const stalePage = await stale.newPage()
  await stalePage.goto(base + detailPath)
  await stalePage.getByRole('link', { name: loginLabel, exact: true }).waitFor()
  assert.equal(await stalePage.locator('button[aria-pressed]').count(), 0)
  await stale.close()
  console.log('PASS stale session remains on the anonymous UI')

  const ssrResponse = await fetch(base + detailPath, { headers: { cookie: cookieHeader } })
  assert.equal(ssrResponse.status, 200)
  const ssrHtml = await ssrResponse.text()
  assert.ok(ssrHtml.includes('آگهی آزمون علاقه‌مندی'))
  assert.ok(!ssrHtml.includes(saveLabel), 'SSR leaked the unsaved personal label')
  assert.ok(!ssrHtml.includes(removeLabel), 'SSR leaked the saved personal label')
  assert.ok(!/"favorited"\s*:\s*(?:true|false)/.test(ssrHtml), 'SSR leaked favorited state')
  console.log('PASS authenticated SSR contains no personal favorite state')

  const context = await browser.newContext({ viewport: { width: 375, height: 812 } })
  await addSession(context)
  const page = await context.newPage()
  activePage = page
  page.setDefaultTimeout(10_000)
  const consoleErrors = []
  const pageErrors = []
  page.on('console', (message) => {
    if (message.type() === 'error') consoleErrors.push(message.text())
  })
  page.on('pageerror', (error) => pageErrors.push(error.message))

  await page.goto(base + detailPath)
  const save = await waitToggle(page, saveLabel)
  assert.equal(await save.getAttribute('aria-pressed'), 'false')
  await assertLayout(page)
  assert.deepEqual(consoleErrors, [])
  assert.deepEqual(pageErrors, [])
  console.log('PASS hydration false state, RTL, 375px layout and 44px target')

  await page.route('**/api/v1/me/favorites/*', async (route) => {
    if (route.request().method() === 'PUT') await new Promise((resolve) => setTimeout(resolve, 400))
    await route.continue()
  })
  await save.focus()
  assert.ok(await save.evaluate((button) => document.activeElement === button))
  await page.keyboard.press('Enter')
  await page.waitForFunction(() => document.querySelector('button[aria-pressed]')?.disabled)
  const remove = await waitToggle(page, removeLabel)
  assert.equal(await remove.getAttribute('aria-pressed'), 'true')
  await page.getByText('در علاقه‌مندی‌ها ذخیره شد.', { exact: true }).waitFor()
  await page.unroute('**/api/v1/me/favorites/*')
  console.log('PASS keyboard save, pending disable and saved announcement')

  await page.reload()
  await waitToggle(page, removeLabel)
  console.log('PASS saved state persists after refresh')

  await page.route('**/api/v1/me/favorites/*', async (route) => {
    if (route.request().method() === 'DELETE') await new Promise((resolve) => setTimeout(resolve, 400))
    await route.continue()
  })
  await (await waitToggle(page, removeLabel)).click()
  await page.waitForFunction(() => document.querySelector('button[aria-pressed]')?.disabled)
  await waitToggle(page, saveLabel)
  await page.getByText('از علاقه‌مندی‌ها حذف شد.', { exact: true }).waitFor()
  await page.unroute('**/api/v1/me/favorites/*')
  console.log('PASS unsave pending state and removed announcement')

  let failNextSave = true
  await page.route('**/api/v1/me/favorites/*', async (route) => {
    if (failNextSave && route.request().method() === 'PUT') {
      failNextSave = false
      await route.fulfill({
        status: 500,
        contentType: 'application/json',
        body: JSON.stringify({ error: { code: 'internal_error', message: 'induced' }, request_id: 'gate' }),
      })
      return
    }
    await route.continue()
  })
  await (await waitToggle(page, saveLabel)).click()
  await page.getByText('تغییر علاقه‌مندی انجام نشد؛ دوباره تلاش کنید.', { exact: true }).waitFor()
  const preservedSave = await waitToggle(page, saveLabel)
  assert.equal(await preservedSave.getAttribute('aria-pressed'), 'false')
  assert.equal(await preservedSave.isDisabled(), false)
  await page.getByRole('button', { name: 'تلاش دوباره', exact: true }).click()
  await waitToggle(page, removeLabel)
  await page.unroute('**/api/v1/me/favorites/*')
  consoleErrors.length = 0
  console.log('PASS failed save preserves state and retry succeeds')

  await page.route('**/api/v1/me/favorites/*', async (route) => {
    if (route.request().method() === 'DELETE') {
      await route.fulfill({
        status: 401,
        contentType: 'application/json',
        body: JSON.stringify({ error: { code: 'unauthorized', message: 'induced' }, request_id: 'gate' }),
      })
      return
    }
    await route.continue()
  })
  await (await waitToggle(page, removeLabel)).click()
  await page.getByRole('link', { name: loginLabel, exact: true }).waitFor()
  assert.equal(await page.locator('button[aria-pressed]').count(), 0)
  await page.unroute('**/api/v1/me/favorites/*')
  consoleErrors.length = 0
  console.log('PASS mutation 401 offers login')

  await page.reload()
  await waitToggle(page, removeLabel)
  await page.setViewportSize({ width: 1280, height: 800 })
  await assertLayout(page)
  assert.deepEqual(consoleErrors, [])
  assert.deepEqual(pageErrors, [])
  console.log('PASS refresh after induced errors, 1280px layout and no console errors')

  await context.close()
  console.log('TASK 6 REAL-STACK BROWSER GATE PASSED')

  const assertTask7Layout = async (targetPage) => {
    const dimensions = await targetPage.evaluate(() => ({
      dir: document.documentElement.dir,
      scrollWidth: document.documentElement.scrollWidth,
      innerWidth: window.innerWidth,
    }))
    assert.equal(dimensions.dir, 'rtl')
    assert.ok(dimensions.scrollWidth <= dimensions.innerWidth, JSON.stringify(dimensions))
  }
  const assertTarget44 = async (locator) => {
    const box = await locator.boundingBox()
    assert.ok(box && box.height >= 44, `target is not 44px: ${JSON.stringify(box)}`)
  }

  const emptyContext = await browser.newContext({ viewport: { width: 375, height: 812 } })
  await addSession(emptyContext, 'b'.repeat(64))
  const emptyPage = await emptyContext.newPage()
  activePage = emptyPage
  emptyPage.setDefaultTimeout(10_000)
  const emptyConsoleErrors = []
  const emptyPageErrors = []
  emptyPage.on('console', (message) => {
    if (message.type() === 'error') emptyConsoleErrors.push(message.text())
  })
  emptyPage.on('pageerror', (error) => emptyPageErrors.push(error.message))
  await emptyPage.goto(base + '/me/favorites')
  await emptyPage.getByText('هنوز آگهی‌ای ذخیره نکرده‌اید.', { exact: true }).waitFor()
  assert.equal(await emptyPage.locator('.cards .card').count(), 0)
  await assertTask7Layout(emptyPage)
  console.log('PASS personal favorites empty state')

  const emptySave = await emptyContext.request.put(
    `${base}/api/v1/me/favorites/01ARZ3NDEKTSV4RRFFQ69G5FAX`,
  )
  assert.equal(emptySave.status(), 200)
  await emptyPage.reload()
  await emptyPage.locator('.cards .card').first().waitFor()
  assert.equal(await emptyPage.locator('.cards .card').count(), 1)
  assert.equal(await emptyPage.getByRole('button', { name: 'نمایش موارد بیشتر', exact: true }).count(), 0)
  await emptyPage.getByRole('button', { name: removeLabel, exact: true }).click()
  await emptyPage.getByText('هنوز آگهی‌ای ذخیره نکرده‌اید.', { exact: true }).waitFor()
  assert.equal(await emptyPage.locator('.cards .card').count(), 0)
  assert.deepEqual(emptyConsoleErrors, [])
  assert.deepEqual(emptyPageErrors, [])
  await emptyContext.close()
  console.log('PASS one-page save/refresh/remove through the Nuxt CSRF proxy')

  const sellerContext = await browser.newContext({ viewport: { width: 375, height: 812 } })
  await addSession(sellerContext, 'c'.repeat(64))
  const sellerPage = await sellerContext.newPage()
  activePage = sellerPage
  sellerPage.setDefaultTimeout(10_000)
  const sellerConsoleErrors = []
  const sellerPageErrors = []
  sellerPage.on('console', (message) => {
    if (message.type() === 'error') sellerConsoleErrors.push(message.text())
  })
  sellerPage.on('pageerror', (error) => sellerPageErrors.push(error.message))
  await sellerPage.goto(base + '/me')
  const favoritesLink = sellerPage.getByRole('link', { name: 'علاقه‌مندی‌های من', exact: true })
  await favoritesLink.waitFor()
  assert.equal(await favoritesLink.getAttribute('href'), '/me/favorites')
  await assertTarget44(favoritesLink)
  const receivedTile = sellerPage.locator('.stat').filter({ hasText: 'ذخیرهٔ آگهی‌های من' })
  assert.equal(await receivedTile.count(), 1)
  assert.equal((await receivedTile.locator('.stat__value').textContent())?.trim(), '22')
  await assertTask7Layout(sellerPage)
  assert.deepEqual(sellerConsoleErrors, [])
  assert.deepEqual(sellerPageErrors, [])
  await sellerContext.close()
  console.log('PASS dashboard link and received count 22, distinct from viewer personal count 23')

  const favoritesContext = await browser.newContext({ viewport: { width: 375, height: 812 } })
  await addSession(favoritesContext)
  const favoritesPage = await favoritesContext.newPage()
  activePage = favoritesPage
  favoritesPage.setDefaultTimeout(10_000)
  const favoritesConsoleErrors = []
  const favoritesPageErrors = []
  favoritesPage.on('console', (message) => {
    if (message.type() === 'error') favoritesConsoleErrors.push(message.text())
  })
  favoritesPage.on('pageerror', (error) => favoritesPageErrors.push(error.message))
  favoritesPage.on('response', (response) => {
    if (response.url().includes('/api/v1/me/favorites')) {
      console.log(`OBSERVE favorites response ${response.status()} ${response.request().method()} ${response.url()}`)
    }
  })
  await favoritesPage.goto(base + '/me')
  await favoritesPage.route('**/api/v1/me/favorites*', async (route) => {
    if (route.request().method() === 'GET') {
      await route.fulfill({
        status: 500,
        contentType: 'application/json',
        body: JSON.stringify({ error: { code: 'internal_error', message: 'induced' }, request_id: 'gate' }),
      })
      return
    }
    await route.continue()
  })
  await favoritesPage.getByRole('link', { name: 'علاقه‌مندی‌های من', exact: true }).click()
  await favoritesPage
    .getByText('خواندن علاقه‌مندی‌ها ممکن نشد؛ دوباره تلاش کنید.', { exact: true })
    .waitFor()
  await favoritesPage.unroute('**/api/v1/me/favorites*')
  await favoritesPage.getByRole('button', { name: 'تلاش دوباره', exact: true }).click()
  await favoritesPage.locator('.cards .card').first().waitFor()
  assert.equal(await favoritesPage.locator('.cards .card').count(), 20)
  await favoritesPage.getByRole('heading', { name: 'علاقه‌مندی‌های من', exact: true }).waitFor()
  await assertTask7Layout(favoritesPage)
  favoritesConsoleErrors.length = 0
  console.log('PASS initial error retry and first page of 20 at 375px')

  const more = favoritesPage.getByRole('button', { name: 'نمایش موارد بیشتر', exact: true })
  await more.waitFor()
  await assertTarget44(more)
  let failMore = true
  await favoritesPage.route('**/api/v1/me/favorites*', async (route) => {
    if (failMore && route.request().method() === 'GET' && route.request().url().includes('cursor=')) {
      failMore = false
      await route.fulfill({
        status: 500,
        contentType: 'application/json',
        body: JSON.stringify({ error: { code: 'internal_error', message: 'induced' }, request_id: 'gate' }),
      })
      return
    }
    await route.continue()
  })
  await more.click()
  await favoritesPage
    .getByText('خواندن موارد بیشتر ممکن نشد؛ دوباره تلاش کنید.', { exact: true })
    .waitFor()
  assert.equal(await favoritesPage.locator('.cards .card').count(), 20)
  await favoritesPage.getByRole('button', { name: 'نمایش موارد بیشتر', exact: true }).waitFor()
  await favoritesPage.unroute('**/api/v1/me/favorites*')
  favoritesConsoleErrors.length = 0
  console.log('PASS load-more failure preserves all 20 cards and retry control')

  const cards = favoritesPage.locator('.cards .card')
  const anchor = cards.last()
  const anchorTitle = (await anchor.locator('.card__title').textContent())?.trim()
  assert.ok(anchorTitle)
  await anchor.getByRole('button', { name: removeLabel, exact: true }).click()
  await favoritesPage.waitForFunction(() => document.querySelectorAll('.cards .card').length === 19)
  await favoritesPage.getByText('از علاقه‌مندی‌ها حذف شد.', { exact: true }).waitFor()
  await favoritesPage.getByRole('button', { name: 'نمایش موارد بیشتر', exact: true }).click()
  await favoritesPage.waitForFunction(() => document.querySelectorAll('.cards .card').length === 22)
  const hrefs = await favoritesPage.locator('.cards .card__title a').evaluateAll((links) =>
    links.map((link) => link.getAttribute('href')),
  )
  assert.equal(new Set(hrefs).size, 22)
  const titles = await favoritesPage.locator('.cards .card__title').allTextContents()
  assert.ok(!titles.includes(anchorTitle), 'removed cursor anchor reappeared')
  console.log('PASS remove cursor anchor then retry load-more: 22 unique cards, no gap/duplicate')

  await favoritesPage.reload()
  await favoritesPage.locator('.cards .card').first().waitFor()
  assert.equal(await favoritesPage.locator('.cards .card').count(), 20)
  const refreshedTitles = await favoritesPage.locator('.cards .card__title').allTextContents()
  assert.ok(!refreshedTitles.includes(anchorTitle), 'remove did not persist after refresh')
  await favoritesPage.getByRole('button', { name: 'نمایش موارد بیشتر', exact: true }).waitFor()
  await favoritesPage.setViewportSize({ width: 1280, height: 800 })
  await assertTask7Layout(favoritesPage)
  await assertTarget44(favoritesPage.locator('.cards .card .btn').first())
  assert.deepEqual(favoritesConsoleErrors, [])
  assert.deepEqual(favoritesPageErrors, [])
  await favoritesContext.close()
  console.log('PASS remove persistence, 1280px layout, 44px targets and no console errors')
  console.log('TASK 7 REAL-STACK BROWSER GATE PASSED')
} catch (error) {
  if (activePage) {
    console.error(`DIAGNOSTIC page URL: ${activePage.url()}`)
    console.error(`DIAGNOSTIC page body: ${(await activePage.locator('body').innerText().catch(() => '')).slice(0, 3000)}`)
    await activePage.screenshot({ path: '/tmp/shabdar-phase6-failure.png', fullPage: true }).catch(() => {})
  }
  throw error
} finally {
  await browser.close()
}
