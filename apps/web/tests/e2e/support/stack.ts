import {
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs'
import { basename, resolve } from 'node:path'
import { request, type APIRequestContext } from '@playwright/test'
import type { ApiCategory, ApiCity, ApiListingOut } from '../../../app/types'
import { promoteAdmin, resetDynamicRows, runSql } from './database'

const ARTIFACTS_DIR = resolve(process.cwd(), 'tests/e2e/.artifacts')
const FIXTURES_FILE = resolve(ARTIFACTS_DIR, 'fixtures.json')
const SELLER_STORAGE = resolve(ARTIFACTS_DIR, 'seller.json')
const ADMIN_STORAGE = resolve(ARTIFACTS_DIR, 'admin.json')
const COVER_FILE = resolve(process.cwd(), 'tests/e2e/fixtures/listing-cover.png')
const SELLER_PHONE = '+989000000101'
const ADMIN_PHONE = '+989000000102'

export interface E2eFixtures {
  detailPath: string
  listingPublicId: string
  sellerStorage: string
  adminStorage: string
}

export interface CreateListingInput {
  title: string
  description?: string
  priceToman?: number | null
  priceType?: 'fixed' | 'negotiable' | 'free'
  imageCount?: number
  state: 'active' | 'pending'
}

function requiredEnv(name: string): string {
  const value = process.env[name]
  if (!value) throw new Error(`${name} is required for real-stack tests`)
  return value
}

function baseURL(): string {
  return requiredEnv('E2E_BASE_URL')
}

async function responseError(response: { status(): number; text(): Promise<string> }, action: string) {
  const body = await response.text()
  return new Error(`${action} failed (${response.status()}): ${body.slice(0, 500)}`)
}

async function waitForOtp(offset: number, lastFour: string): Promise<string> {
  const logPath = requiredEnv('E2E_API_LOG')
  const deadline = Date.now() + 15_000
  while (Date.now() < deadline) {
    if (existsSync(logPath)) {
      const appended = readFileSync(logPath, 'utf8').slice(offset)
      const matches = [
        ...appended.matchAll(new RegExp(`DEV OTP for \\*\\*\\*${lastFour}: (\\d{6})`, 'g')),
      ]
      const match = matches.at(-1)
      if (match?.[1]) return match[1]
    }
    await new Promise((resolveWait) => setTimeout(resolveWait, 100))
  }
  throw new Error(`Timed out waiting for the masked debug OTP ending in ${lastFour}`)
}

async function registerAndStore(phone: string, storagePath: string): Promise<void> {
  const context = await request.newContext({ baseURL: baseURL() })
  try {
    const logPath = requiredEnv('E2E_API_LOG')
    const offset = existsSync(logPath) ? statSync(logPath).size : 0
    const challenge = await context.post('/api/v1/auth/otp/request', {
      data: { phone, purpose: 'register' },
    })
    if (!challenge.ok()) throw await responseError(challenge, 'OTP request')
    const challengeBody = await challenge.json() as { challenge_id: string }
    const code = await waitForOtp(offset, phone.slice(-4))
    const verify = await context.post('/api/v1/auth/otp/verify', {
      data: { challenge_id: challengeBody.challenge_id, code },
    })
    if (!verify.ok()) throw await responseError(verify, 'OTP verify')
    await context.storageState({ path: storagePath })
  } finally {
    await context.dispose()
  }
}

export function readFixtures(): E2eFixtures {
  return JSON.parse(readFileSync(FIXTURES_FILE, 'utf8')) as E2eFixtures
}

export async function apiAs(role: 'seller' | 'admin'): Promise<APIRequestContext> {
  return request.newContext({
    baseURL: baseURL(),
    storageState: role === 'seller' ? SELLER_STORAGE : ADMIN_STORAGE,
  })
}

function assertCover(): Buffer {
  const cover = readFileSync(COVER_FILE)
  const pngSignature = '89504e470d0a1a0a'
  if (cover.subarray(0, 8).toString('hex') !== pngSignature) {
    throw new Error('E2E cover must be a PNG')
  }
  const width = cover.readUInt32BE(16)
  const height = cover.readUInt32BE(20)
  if (width !== 1200 || height !== 900) {
    throw new Error(`E2E cover must be 1200x900, got ${width}x${height}`)
  }
  if (cover.byteLength < 100 * 1024 || cover.byteLength > 250 * 1024) {
    throw new Error(`E2E cover must be 100-250 KiB, got ${cover.byteLength} bytes`)
  }
  return cover
}

async function referenceIds(context: APIRequestContext): Promise<{ categoryId: number; cityId: number }> {
  const [categories, cities] = await Promise.all([
    context.get('/api/v1/categories'),
    context.get('/api/v1/locations/provinces/gilan/cities'),
  ])
  if (!categories.ok()) throw await responseError(categories, 'Category fixture read')
  if (!cities.ok()) throw await responseError(cities, 'City fixture read')
  const categoryBody = await categories.json() as { data: ApiCategory[] }
  const cityBody = await cities.json() as { data: ApiCity[] }
  const category = categoryBody.data[0]
  const city = cityBody.data.find((item) => item.is_launch_city) ?? cityBody.data[0]
  if (!category || !city) throw new Error('Launch reference fixtures are missing')
  return { categoryId: category.id, cityId: city.id }
}

export async function createListing(input: CreateListingInput): Promise<{ publicId: string; slug: string }> {
  const imageCount = input.imageCount ?? 1
  if (!Number.isInteger(imageCount) || imageCount < 0 || imageCount > 8) {
    throw new Error('createListing imageCount must be an integer from 0 to 8')
  }
  const seller = await apiAs('seller')
  const admin = input.state === 'active' ? await apiAs('admin') : null
  try {
    const { categoryId, cityId } = await referenceIds(seller)
    const priceType = input.priceType ?? 'fixed'
    const priceToman = input.priceToman === undefined
      ? priceType === 'fixed' ? 12_500_000 : null
      : input.priceToman
    const created = await seller.post('/api/v1/listings', {
      data: {
        category_id: categoryId,
        city_id: cityId,
        title: input.title,
        description: input.description ?? `توضیحات واقعی fixture برای «${input.title}» در رشت.`,
        price_type: priceType,
        price_toman: priceToman,
      },
    })
    if (!created.ok()) throw await responseError(created, 'Create listing fixture')
    const listing = await created.json() as ApiListingOut
    const cover = assertCover()
    // Submission requires media. A zero-image fixture uploads one temporary
    // cover, completes moderation, then removes it to exercise real no-cover UI.
    const uploadCount = imageCount === 0 ? 1 : imageCount
    for (let index = 0; index < uploadCount; index += 1) {
      const uploaded = await seller.post(`/api/v1/listings/${listing.public_id}/images`, {
        multipart: {
          file: { name: 'listing-cover.png', mimeType: 'image/png', buffer: cover },
        },
      })
      if (!uploaded.ok()) throw await responseError(uploaded, `Upload fixture image ${index + 1}`)
    }
    const submitted = await seller.post(`/api/v1/listings/${listing.public_id}/submit`)
    if (!submitted.ok()) throw await responseError(submitted, 'Submit listing fixture')
    if (admin) {
      const approved = await admin.post(`/api/v1/admin/listings/${listing.public_id}/approve`)
      if (!approved.ok()) throw await responseError(approved, 'Approve listing fixture')
    }
    if (imageCount === 0) await removeImagesForFixture(listing.public_id)
    return { publicId: listing.public_id, slug: listing.slug }
  } finally {
    await seller.dispose()
    if (admin) await admin.dispose()
  }
}

function resetMedia(): void {
  const media = resolve(requiredEnv('E2E_MEDIA_DIR'))
  if (basename(media) !== 'shabdar-e2e-media' || media === '/') {
    throw new Error('E2E_MEDIA_DIR must end in the dedicated shabdar-e2e-media directory')
  }
  rmSync(media, { recursive: true, force: true })
  mkdirSync(media, { recursive: true })
}

export async function initializeStack(): Promise<void> {
  requiredEnv('E2E_BASE_URL')
  requiredEnv('E2E_API_LOG')
  requiredEnv('E2E_DATABASE_URL')
  requiredEnv('E2E_MEDIA_DIR')
  assertCover()
  mkdirSync(ARTIFACTS_DIR, { recursive: true })
  await resetDynamicRows()
  resetMedia()
  await registerAndStore(SELLER_PHONE, SELLER_STORAGE)
  await registerAndStore(ADMIN_PHONE, ADMIN_STORAGE)
  await promoteAdmin(ADMIN_PHONE)
  const listing = await createListing({
    title: 'آگهی آزمون شبدر',
    description: 'آگهیٔ فعال و پایدار برای آزمون‌های واقعی شبدر در رشت.',
    state: 'active',
  })
  const fixtures: E2eFixtures = {
    detailPath: `/listings/${listing.slug}`,
    listingPublicId: listing.publicId,
    sellerStorage: SELLER_STORAGE,
    adminStorage: ADMIN_STORAGE,
  }
  writeFileSync(FIXTURES_FILE, `${JSON.stringify(fixtures, null, 2)}\n`)
}

// Used by later empty-media fixtures without changing any application contract.
export async function removeImagesForFixture(publicId: string): Promise<void> {
  const escaped = publicId.replaceAll("'", "''")
  await runSql(
    `DELETE FROM listing_images WHERE listing_id = (SELECT id FROM listings WHERE public_id = '${escaped}')`,
  )
}
