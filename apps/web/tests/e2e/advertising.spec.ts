import { randomUUID } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { expect, test, type BrowserContext, type Page } from '@playwright/test'
import { runSql } from './support/database'
import { apiAs, createListing, readFixtures } from './support/stack'

const SLOT = {
  title: 'جایگاه تبلیغاتی آزمون شبدر',
  body: 'متن واقعی جایگاه برای بررسی نمایش و مدیریت تبلیغ در فید خانه.',
  cta_label: 'ثبت تبلیغ در شبدر',
  cta_url: '/advertising',
  enabled: true,
}

async function applyStorage(page: Page, storagePath: string): Promise<void> {
  const state = JSON.parse(readFileSync(storagePath, 'utf8')) as {
    cookies: Parameters<BrowserContext['addCookies']>[0]
  }
  await page.context().addCookies(state.cookies)
}

async function expectSingleFourthPositionAd(page: Page): Promise<void> {
  const ad = page.locator('[data-advertising-card]')
  await expect(ad).toHaveCount(1)
  const adIndex = await page.locator('.browser__cards > li').evaluateAll((elements) => (
    elements.findIndex((element) => Boolean(element.querySelector('[data-advertising-card]')))
  ))
  expect(adIndex).toBe(4, 'four real listing nodes precede the only advertising node')
}

test.describe('advertising inventory and internal requests', () => {
  test.setTimeout(240_000)

  test.beforeAll(async () => {
    // One load-more page exists even when this spec runs alone. Other specs may
    // add more rows; assertions below deliberately use the returned page size.
    for (let index = 1; index <= 21; index += 1) {
      await createListing({
        title: `آگهی تبلیغ خانه ${String(index).padStart(2, '0')}`,
        imageCount: 0,
        state: 'active',
      })
    }
  })

  test.beforeEach(async () => {
    await runSql('DELETE FROM advertising_requests')
    const admin = await apiAs('admin')
    try {
      const response = await admin.put('/api/v1/admin/advertising/slot', { data: SLOT })
      expect(response.ok()).toBe(true)
    } finally {
      await admin.dispose()
    }
  })

  test.afterAll(async () => {
    // Do not leak the house ad into the established Home/account scenarios.
    await runSql('DELETE FROM advertising_requests; DELETE FROM advertising_slots')
  })

  test('Home places one labelled card after four real listings and dismissal survives reload', async ({ page }) => {
    await page.goto('/')
    const cards = page.locator('.listing-card')
    await expect(cards).toHaveCount(20)
    await expectSingleFourthPositionAd(page)
    await expect(page.locator('[data-advertising-card]')).toContainText('فرصت تبلیغاتی')
    await expect(page.locator('.browser__result-count')).toContainText('۲۰')
    await expect(page.locator('.browser__result-count')).not.toContainText('۲۱')

    await page.getByRole('button', { name: 'بستن فرصت تبلیغاتی', exact: true }).click()
    await expect(page.locator('[data-advertising-card]')).toHaveCount(0)
    await expect(cards).toHaveCount(20)
    await page.reload()
    await expect(cards).toHaveCount(20)
    await expect(page.locator('[data-advertising-card]')).toHaveCount(0)
    const stored = await page.evaluate(() => localStorage.getItem('shabdar.home-ad.dismissed'))
    expect(stored).toMatch(/^[0-9A-HJKMNP-TV-Z]{26}$/)
  })

  test('Home load-more preserves listing cursors/cards and never repeats advertising', async ({ page }) => {
    await page.goto('/')
    const cards = page.locator('.listing-card')
    await expect(cards).toHaveCount(20)
    await expectSingleFourthPositionAd(page)
    const initialHrefs = await cards.locator('.listing-card__link').evaluateAll((links) => (
      links.map((link) => link.getAttribute('href'))
    ))
    expect(new Set(initialHrefs).size).toBe(20)

    const more = page.locator('.browser__pagination').getByRole('button')
    await expect(more).toBeVisible()
    await more.click()
    await expect.poll(() => cards.count()).toBeGreaterThan(20)
    const finalHrefs = await cards.locator('.listing-card__link').evaluateAll((links) => (
      links.map((link) => link.getAttribute('href'))
    ))
    expect(finalHrefs.slice(0, 20)).toEqual(initialHrefs)
    expect(new Set(finalHrefs).size).toBe(finalHrefs.length)
    await expectSingleFourthPositionAd(page)
  })

  test('Advertising disappears for filtered feeds and is absent from account screens', async ({ page }) => {
    await page.goto('/')
    await expectSingleFourthPositionAd(page)
    const search = page.getByRole('searchbox', { name: 'جستجو', exact: true })
    await search.fill('آگهی تبلیغ خانه 01')
    await expect(page).toHaveURL(/[?&]q=/)
    await expect(page.locator('.listing-card')).toHaveCount(1)
    await expect(page.locator('[data-advertising-card]')).toHaveCount(0)
    await search.fill('')
    await expect(page).not.toHaveURL(/[?&]q=/)
    await expect(page.locator('.listing-card')).toHaveCount(20)
    await expectSingleFourthPositionAd(page)

    await page.goto('/login')
    await expect(page.locator('[data-advertising-card]')).toHaveCount(0)
    await applyStorage(page, readFixtures().sellerStorage)
    await page.goto('/post')
    await expect(page.getByRole('heading', { name: /ثبت آگهی/ })).toBeVisible()
    await expect(page.locator('[data-advertising-card]')).toHaveCount(0)
    await page.goto('/me')
    await expect(page.locator('[data-advertising-card]')).toHaveCount(0)
  })

  test('Public form preserves fields after a lost response and retry stays idempotent', async ({ page }) => {
    await page.goto('/advertising')
    const name = page.getByLabel('نام شما یا کسب‌وکار *', { exact: true })
    const phone = page.getByLabel('شمارهٔ موبایل برای پیگیری *', { exact: true })
    const details = page.getByLabel('دربارهٔ کسب‌وکار یا جایگاه پیشنهادی', { exact: true })
    const submit = page.locator('.advertising-form button[type="submit"]')
    await expect(submit).toHaveText('ثبت درخواست تبلیغ')
    await name.fill('فروشگاه آزمون فرم تبلیغ')
    await phone.fill('02112345678')
    await details.fill('جزئیات واقعی درخواست برای بررسی حفظ ورودی و ثبت تکراری.')
    await submit.click()
    await expect(page.getByText('شمارهٔ موبایل معتبر ایران وارد کنید.', { exact: true })).toBeVisible()
    await expect(name).toHaveValue('فروشگاه آزمون فرم تبلیغ')

    await phone.fill('09120000101')
    let requestCount = 0
    let loseResponse = true
    await page.route('**/api/v1/advertising/requests', async (route) => {
      if (route.request().method() !== 'POST') {
        await route.continue()
        return
      }
      requestCount += 1
      if (loseResponse) {
        // The real API commits first; only the browser response is replaced.
        const response = await route.fetch()
        expect(response.ok()).toBe(true)
        await new Promise((resolve) => setTimeout(resolve, 250))
        await route.fulfill({ status: 500, contentType: 'application/json', body: '{"error":{"code":"internal_error"}}' })
        return
      }
      await route.continue()
    })
    await submit.click()
    await expect(submit).toBeDisabled()
    await submit.evaluate((button: HTMLButtonElement) => {
      button.click()
      button.click()
    })
    await expect(page.getByText(/ثبت درخواست انجام نشد؛ اطلاعات فرم حفظ شده است/)).toBeVisible()
    expect(requestCount).toBe(1, 'pending/disabled submission cannot create a parallel request')
    await expect(name).toHaveValue('فروشگاه آزمون فرم تبلیغ')
    await expect(phone).toHaveValue('09120000101')
    await expect(details).toHaveValue('جزئیات واقعی درخواست برای بررسی حفظ ورودی و ثبت تکراری.')

    loseResponse = false
    await submit.click()
    const success = page.getByRole('status').filter({ hasText: 'درخواست شما ثبت شد' })
    await expect(success).toBeVisible()
    await expect(success).toBeFocused()
    expect(requestCount).toBe(2)

    const admin = await apiAs('admin')
    try {
      const response = await admin.get('/api/v1/admin/advertising/requests?status=all&limit=50')
      expect(response.ok()).toBe(true)
      const body = await response.json() as { data: Array<{ contact_name: string; phone_e164: string }> }
      const matching = body.data.filter((item) => item.contact_name === 'فروشگاه آزمون فرم تبلیغ')
      expect(matching).toHaveLength(1)
      expect(matching[0]?.phone_e164).toBe('+989120000101')
    } finally {
      await admin.dispose()
    }
  })

  test('Admin edits, disables, previews, deletes and recreates the real slot', async ({ page }) => {
    await applyStorage(page, readFixtures().adminStorage)
    await page.goto('/admin/advertising')
    const title = page.getByLabel('عنوان', { exact: true })
    const enabled = page.getByLabel('جایگاه پس از ذخیره فعال باشد', { exact: true })
    await expect(title).toHaveValue(SLOT.title)
    await title.fill('جایگاه ویرایش‌شدهٔ مدیریت')
    await enabled.uncheck()
    await page.getByRole('button', { name: 'ذخیره تغییرات', exact: true }).click()
    await expect(page.getByText('جایگاه با موفقیت ذخیره شد.', { exact: true })).toBeVisible()
    await expect(page.locator('.advertising-slot__state')).toHaveText('غیرفعال')
    await page.goto('/')
    await expect(page.locator('.listing-card')).toHaveCount(20)
    await expect(page.locator('[data-advertising-card]')).toHaveCount(0)

    await page.goto('/admin/advertising')
    await enabled.check()
    await page.getByRole('button', { name: 'ذخیره تغییرات', exact: true }).click()
    await expect(page.getByText('جایگاه با موفقیت ذخیره شد.', { exact: true })).toBeVisible()
    await page.goto('/')
    await expect(page.locator('[data-advertising-card]')).toContainText('جایگاه ویرایش‌شدهٔ مدیریت')

    await page.goto('/admin/advertising')
    const destination = page.getByLabel('مقصد دکمه', { exact: true })
    await destination.fill('javascript:alert(1)')
    await expect(page.locator('.advertising-slot__preview .home-ad__cta')).toHaveAttribute('href', '/advertising')
    await page.getByRole('button', { name: 'ذخیره تغییرات', exact: true }).click()
    await expect(page.getByText(/ذخیره انجام نشد؛ ورودی‌ها حفظ شدند/)).toBeVisible()
    await expect(destination).toHaveValue('javascript:alert(1)')
    const admin = await apiAs('admin')
    try {
      const confirmed = await admin.get('/api/v1/admin/advertising/slot')
      const body = await confirmed.json() as { slot: { cta_url: string } }
      expect(body.slot.cta_url).toBe('/advertising')
    } finally {
      await admin.dispose()
    }

    await destination.fill('/advertising')
    await page.getByRole('button', { name: 'ذخیره تغییرات', exact: true }).click()
    await expect(page.getByText('جایگاه با موفقیت ذخیره شد.', { exact: true })).toBeVisible()
    await page.getByRole('button', { name: 'حذف جایگاه', exact: true }).click()
    await page.getByRole('button', { name: 'تأیید حذف', exact: true }).click()
    await expect(page.locator('.advertising-slot__state')).toHaveText('بدون جایگاه')
    await page.goto('/')
    await expect(page.locator('[data-advertising-card]')).toHaveCount(0)
    await page.goto('/admin/advertising')
    await page.getByRole('button', { name: 'ایجاد جایگاه', exact: true }).click()
    await expect(page.locator('.advertising-slot__state')).toHaveText('فعال')
    await page.goto('/')
    await expectSingleFourthPositionAd(page)
  })

  test('Admin request status changes wait for confirmation and retain private contact details', async ({ page }) => {
    const created = await page.request.post('/api/v1/advertising/requests', {
      data: {
        submission_id: randomUUID(),
        contact_name: 'درخواست پیگیری ادمین',
        phone: '09120000102',
        details: 'جزئیات خصوصی درخواست برای بررسی گردش کار مدیریت.',
      },
    })
    expect(created.ok()).toBe(true)
    const receipt = await created.json() as { request: { public_id: string } }
    await applyStorage(page, readFixtures().adminStorage)
    await page.goto('/admin/advertising')
    const row = page.locator(`[data-request-id="${receipt.request.public_id}"]`)
    await expect(row).toBeVisible()
    await expect(row.locator('.advertising-request__phone')).toHaveAttribute('href', 'tel:+989120000102')

    let releasePatch: (() => void) | undefined
    const gate = new Promise<void>((resolve) => { releasePatch = resolve })
    await page.route(`**/api/v1/admin/advertising/requests/${receipt.request.public_id}`, async (route) => {
      if (route.request().method() === 'PATCH') await gate
      await route.continue()
    })
    await row.getByRole('button', { name: 'شروع پیگیری', exact: true }).click()
    await expect(row).toBeVisible()
    await expect(row.locator('.advertising-request__status')).toHaveText('جدید')
    releasePatch?.()
    await expect(row).toHaveCount(0)

    await page.getByRole('button', { name: 'در حال پیگیری', exact: true }).click()
    await expect(row).toBeVisible()
    await expect(row.locator('.advertising-request__status')).toHaveText('در حال پیگیری')
    await row.getByRole('button', { name: 'بستن درخواست', exact: true }).click()
    await expect(row).toHaveCount(0)
    await page.getByRole('button', { name: 'بسته‌شده', exact: true }).click()
    await expect(row).toBeVisible()
    await expect(row.locator('.advertising-request__status')).toHaveText('بسته‌شده')
    await expect(row.locator('.advertising-request__phone')).toHaveText('+989120000102')
  })

  test('Non-admin cannot read the advertising workflow or its contact details', async ({ page }) => {
    const created = await page.request.post('/api/v1/advertising/requests', {
      data: {
        submission_id: randomUUID(),
        contact_name: 'درخواست محرمانهٔ دسترسی',
        phone: '09120000103',
      },
    })
    expect(created.ok()).toBe(true)
    await applyStorage(page, readFixtures().buyerStorage)
    await page.goto('/admin/advertising')
    await expect(page.getByText('دسترسی ندارید؛ این بخش ویژهٔ مدیر است.', { exact: true })).toBeVisible()
    await expect(page.getByText('درخواست محرمانهٔ دسترسی', { exact: true })).toHaveCount(0)
    await expect(page.getByText('+989120000103', { exact: true })).toHaveCount(0)
  })
})
