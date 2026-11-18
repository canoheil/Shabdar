// Local visual-check fixture API. NOT part of the app: it stands in for the
// Rust/Axum service on :8080 so the real Nuxt build (with its real /api proxy,
// SSR, and client hydration) can be reviewed and screenshotted without MariaDB.
//
//   node scripts/visual-fixture-api.mjs      # terminal 1 — fixture API on :8080
//   PORT=3000 node .output/server/index.mjs  # terminal 2 — the real Nuxt build
//
// The pages then behave exactly as they do in production; only the data is fake.
import { createServer } from 'node:http'

const CATEGORIES = [
  { id: 1, parent_id: null, name_fa: 'شیرینی خانگی', slug: 'home-sweets', sort_order: 1 },
  { id: 2, parent_id: null, name_fa: 'غذای خانگی', slug: 'home-food', sort_order: 2 },
  { id: 3, parent_id: null, name_fa: 'صنایع دستی', slug: 'handicrafts', sort_order: 3 },
  { id: 4, parent_id: null, name_fa: 'پوشاک و خیاطی', slug: 'clothing-sewing', sort_order: 4 },
  { id: 5, parent_id: null, name_fa: 'گیاه و گل', slug: 'plants-flowers', sort_order: 5 },
  { id: 6, parent_id: null, name_fa: 'زیبایی', slug: 'beauty', sort_order: 6 },
  { id: 7, parent_id: null, name_fa: 'آموزش', slug: 'education', sort_order: 7 },
]
const PROVINCES = [
  { name_fa: 'گیلان', slug: 'gilan' }, { name_fa: 'تهران', slug: 'tehran' },
  { name_fa: 'اصفهان', slug: 'isfahan' }, { name_fa: 'فارس', slug: 'fars' },
]
const CITIES = [
  { id: 1, name_fa: 'رشت', slug: 'rasht', is_launch_city: true },
  { id: 2, name_fa: 'لاهیجان', slug: 'lahijan', is_launch_city: true },
  { id: 3, name_fa: 'تهران', slug: 'tehran', is_launch_city: true },
]

const PHOTO = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 480 360">
<rect width="480" height="360" fill="#EFE3D0"/>
<ellipse cx="240" cy="300" rx="300" ry="90" fill="#E2D2B8"/>
<circle cx="240" cy="190" r="118" fill="#F7F1E6"/>
<circle cx="240" cy="190" r="98" fill="#C98B4B"/>
<circle cx="240" cy="190" r="74" fill="#A96B32"/>
<circle cx="212" cy="164" r="26" fill="#D8A264" opacity=".65"/>
<circle cx="266" cy="204" r="18" fill="#8A5220" opacity=".5"/>
<circle cx="248" cy="156" r="12" fill="#E8C08A" opacity=".8"/>
</svg>`

const NAMES = [
  'باقلوا خانگی تازه با پسته', 'کاسه و پارچه سفالی نقاب‌دوزی اصفهان',
  'میرزاقاسمی خانگی دودی', 'شال و روسری نقاب گیلان',
  'گلدان سفالی دست‌ساز', 'کیک شکلاتی خانگی بدون افزودنی',
  'ژله توت‌فرنگی خانگی', 'کیف چرم دست‌دوز',
  'عسل طبیعی کوهستان', 'سبد حصیری دست‌بافت',
]
const CAT = ['شیرینی خانگی', 'صنایع دستی', 'غذای خانگی', 'پوشاک و خیاطی']
const CITY = [
  ['رشت', 'rasht'], ['اصفهان', 'isfahan'], ['تهران', 'tehran'], ['لاهیجان', 'lahijan'],
]

function listing(index) {
  const price = 380000 - index * 21000
  return {
    public_id: `fixture-${index}`,
    slug: `listing-${index}`,
    title: NAMES[index % NAMES.length],
    price_toman: price > 0 ? price : 96000,
    price_type: 'fixed',
    city: CITY[index % CITY.length][0],
    city_slug: CITY[index % CITY.length][1],
    province: 'گیلان',
    category: CAT[index % CAT.length],
    cover_image: `/media/photo-${index}.svg`,
    rating_average: 4.8,
    rating_count: index % 3 === 0 ? 0 : 12 + index,
    published_at: new Date(Date.now() - index * 86400000).toISOString(),
  }
}

const LISTINGS = Array.from({ length: 12 }, (_, i) => listing(i))

function json(res, body, status = 200) {
  res.statusCode = status
  res.setHeader('content-type', 'application/json; charset=utf-8')
  res.setHeader('cache-control', 'no-store')
  res.end(JSON.stringify(body))
}

const server = createServer((req, res) => {
  const path = new URL(req.url, 'http://localhost').pathname
  if (path.startsWith('/media/')) {
    res.setHeader('content-type', 'image/svg+xml')
    res.setHeader('cache-control', 'public, max-age=600')
    return res.end(PHOTO)
  }
  if (path === '/api/v1/me') return json(res, { error: { code: 'unauthorized', message: 'fixture' } }, 401)
  if (path === '/api/v1/categories') return json(res, { data: CATEGORIES })
  if (path === '/api/v1/locations/provinces') return json(res, { data: PROVINCES })
  if (path.startsWith('/api/v1/locations/provinces/')) return json(res, { data: CITIES })
  if (path.startsWith('/api/v1/locations/cities/')) {
    return json(res, {
      data: { id: 1, name_fa: 'رشت', slug: 'rasht', province_slug: 'gilan', province_name_fa: 'گیلان' },
    })
  }
  if (path === '/api/v1/advertising/slot') return json(res, { data: null })
  if (path === '/api/v1/listings') {
    return json(res, { data: LISTINGS, page: { next_cursor: null, has_more: false } })
  }
  // The detail read is not enveloped: `ApiListingDetail` is the response body.
  if (/^\/api\/v1\/listings\/[^/]+$/.test(path) && !path.endsWith('/comments')) {
    return json(res, {
      public_id: 'fixture-0',
      slug: 'listing-0',
      title: 'میرزاقاسمی خانگی دودی',
      description: 'میرزاقاسمی اصل گیلان با بادمجان کبابی روی ذغال، سیر تازه و گوجهٔ محلی.\nهر ظرف برای دو نفر کافی است و با نان بربری تازه ارسال می‌شود.',
      price_toman: 165000,
      price_type: 'fixed',
      city: 'رشت',
      province: 'گیلان',
      category: 'غذای خانگی',
      images: [0, 1, 2, 3].map((i) => ({ url: `/media/photo-${i}.svg`, alt_text: null })),
      seller: { public_id: 'seller-1', display_name: 'سارا احمدی' },
      rating_average: 4.9,
      rating_count: 31,
      comment_count: 3,
      published_at: new Date(Date.now() - 3 * 86400000).toISOString(),
    })
  }
  if (/^\/api\/v1\/listings\/[^/]+\/comments$/.test(path)) {
    return json(res, { data: [], page: { next_cursor: null, has_more: false } })
  }
  return json(res, { error: { code: 'not_found', message: path } }, 404)
})

server.listen(8080, '0.0.0.0', () => {
  console.log('visual fixture API on http://0.0.0.0:8080')
})
