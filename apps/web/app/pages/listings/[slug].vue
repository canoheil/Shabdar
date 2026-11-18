<template>
  <section class="detail">
    <AppSkeleton v-if="pending" variant="detail" label="در حال بارگذاری آگهی…" />
    <AppAlert v-else-if="gone" tone="error">
      <p class="detail__alert-copy">این آگهی دیگر موجود نیست.</p>
      <NuxtLink to="/" class="btn btn--secondary">بازگشت به خانه</NuxtLink>
    </AppAlert>
    <AppAlert v-else-if="missing" tone="error">
      <p class="detail__alert-copy">آگهی یافت نشد.</p>
      <NuxtLink to="/" class="btn btn--secondary">بازگشت به خانه</NuxtLink>
    </AppAlert>
    <article v-else-if="listing" class="detail__article">
      <header class="detail__header">
        <NuxtLink to="/" class="detail__back">
          <AppIcon name="arrow-right-bold" :size="20" />
          بازگشت به جستجو
        </NuxtLink>
      </header>
      <div class="detail__layout">
        <ListingGallery
          class="detail__gallery"
          :images="listing.images"
          :listing-title="listing.title"
        />
        <section class="detail__summary" aria-label="خلاصه و اقدامات آگهی">
          <div>
            <p class="detail__category">{{ listing.category }}</p>
            <h1>{{ listing.title }}</h1>
            <p class="detail__facts">
              <AppIcon name="star-fill" :size="14" />
              <span v-if="listing.rating_count > 0">{{ formatNumber(Math.round(listing.rating_average * 10) / 10) }} از ۵ ({{ formatNumber(listing.rating_count) }} نظر)</span>
              <span v-else>بدون امتیاز</span>
              <span aria-hidden="true">·</span>
              <span>{{ formatNumber(listing.comment_count) }} دیدگاه</span>
              <span aria-hidden="true">·</span>
              <time :datetime="listing.published_at">{{ formatListingDay(listing.published_at, renderNow) }}</time>
            </p>
          </div>
          <div class="detail__price-block">
            <span class="detail__price-label">قیمت</span>
            <p class="detail__price">{{ formatPrice(listing.price_toman, listing.price_type) }}</p>
          </div>
          <p class="detail__location">
            <AppIcon name="map-pin" :size="14" />
            <span>{{ listing.city }}، {{ listing.province }}</span>
          </p>
          <div class="detail__seller">
            <span class="detail__seller-avatar avatar" aria-hidden="true">{{ (listing.seller.display_name ?? 'ش').slice(0, 1) }}</span>
            <div>
              <span class="detail__seller-label">فروشنده</span>
              <strong>{{ listing.seller.display_name ?? 'کاربر شبدر' }}</strong>
            </div>
          </div>

          <div class="detail__actions">
          <div
            v-if="hydrated && !mePending && authStatus !== 'unavailable' && me?.public_id !== listing.seller.public_id"
            class="detail__contact"
          >
            <p class="detail__contact-title">گفت‌وگو با فروشنده</p>
            <NuxtLink
              v-if="authStatus !== 'authenticated' || contactAuthExpired"
              :to="contactLoginHref"
              class="btn"
            >
              ورود برای گفت‌وگو
            </NuxtLink>
            <template v-else-if="contactQuery">
              <AppButton :loading="contactPending" :disabled="contactPending" @click="contactSeller">
                {{ contactPending ? 'در حال باز کردن گفت‌وگو…' : 'ادامهٔ گفت‌وگو' }}
              </AppButton>
            </template>
            <template v-else>
              <AppButton :disabled="contactPending" @click="contactSeller">
                {{ contactPending ? 'در حال باز کردن گفت‌وگو…' : 'گفت‌وگو با فروشنده' }}
              </AppButton>
            </template>
            <AppAlert v-if="contactError" tone="error">
              <div class="detail__contact-state">
                <span>{{ contactError }}</span>
                <AppButton variant="secondary" :disabled="contactPending" @click="contactSeller">
                  تلاش دوباره
                </AppButton>
              </div>
            </AppAlert>
            <p v-if="contactQuery && authStatus !== 'authenticated'" class="meta">
              برای ادامهٔ گفت‌وگو ابتدا وارد شوید؛ پس از ورود همین آگهی باز می‌ماند.
            </p>
            <p class="meta" role="status" aria-live="polite" aria-atomic="true">{{ contactNotice }}</p>
          </div>

          <div v-if="!mePending" class="detail__favorite">
            <NuxtLink
              v-if="authStatus === 'anonymous' || favoriteAuthExpired"
              to="/login"
              class="btn detail__favorite-button"
            >
              <AppIcon name="heart" :size="22" />
              <span class="sr-only">ورود برای ذخیره</span>
            </NuxtLink>
            <template v-else-if="authStatus === 'authenticated' && me">
              <AppButton
                v-if="favorited !== undefined"
                class="detail__favorite-button"
                :disabled="favoritePending"
                :aria-pressed="favorited"
                @click="toggleFavorite"
              >
                <AppIcon :name="favorited ? 'heart-fill' : 'heart'" :size="22" />
                <span class="sr-only">{{ favoritePending ? 'در حال به‌روزرسانی…' : favorited ? removeLabel : saveLabel }}</span>
              </AppButton>
              <p v-else-if="!favoriteError" class="meta">در حال بررسی علاقه‌مندی…</p>
              <AppAlert v-if="favoriteError" tone="error">{{ favoriteError }}</AppAlert>
              <AppButton
                v-if="favoriteError"
                variant="secondary"
                :disabled="favoritePending"
                @click="retryFavorite"
              >
                تلاش دوباره
              </AppButton>
            </template>
            <p class="meta" role="status" aria-live="polite" aria-atomic="true">{{ favoriteNotice }}</p>
          </div>
          </div>

          <details
            v-if="hydrated && !mePending && authStatus !== 'unavailable' && me?.public_id !== listing.seller.public_id"
            class="detail__reports"
          >
            <summary class="detail__reports-title">
              <AppIcon name="shield-check" :size="18" />
              گزارش و ایمنی
              <AppIcon name="caret-down-bold" :size="14" />
            </summary>
            <div class="detail__report-row">
              <span>محتوای آگهی</span>
              <ReportControl
                target-type="listing"
                :target-id="listing.slug"
                control-key="listing"
                button-label="گزارش آگهی"
                :anonymous="authStatus !== 'authenticated'"
              />
            </div>
            <div class="detail__report-row">
              <span>رفتار فروشنده</span>
              <ReportControl
                target-type="user"
                :target-id="listing.seller.public_id"
                control-key="seller"
                button-label="گزارش فروشنده"
                :anonymous="authStatus !== 'authenticated'"
              />
            </div>
          </details>
        </section>
      </div>
      <section class="detail__body" aria-labelledby="detail-description-title">
        <h2 id="detail-description-title">توضیحات</h2>
        <p class="detail__description">{{ listing.description }}</p>
      </section>
      <div class="detail__feedback">
      <ListingRating
        :slug="listing.slug"
        :average="listing.rating_average"
        :count="listing.rating_count"
        :my-stars="myStars"
        :auth-status="authStatus"
        :owner="me?.public_id === listing.seller.public_id"
        :viewer-pending="viewerPending"
        :viewer-error="viewerReadFailed"
        :auth-expired="favoriteAuthExpired"
        @updated="applyRatingState"
        @retry-viewer="loadFavoriteState"
        @expired="favoriteAuthExpired = true"
      />
      <ListingComments
        :slug="listing.slug"
        :count="listing.comment_count"
        :initial-page="initialComments ?? null"
        :initial-error="commentsReadFailed"
        :auth-status="authStatus"
        :auth-expired="favoriteAuthExpired"
        :viewer="me"
        @expired="favoriteAuthExpired = true"
      />
      </div>
    </article>
    <AppAlert v-else tone="error">خطایی رخ داد؛ دوباره تلاش کنید.</AppAlert>
  </section>
</template>

<script setup lang="ts">
import type { ApiComment, ApiListingDetail, ApiPage, ApiRatingState } from '~/types'

const route = useRoute()
const slug = route.params.slug as string
const detailUrl = `/api/v1/listings/${encodeURIComponent(slug)}`

// Keep these original reads public on SSR and client navigation: personal
// state is fetched separately after hydration, never by forwarding a session.
const publicReadOptions = {
  headers: { cookie: '' },
  credentials: 'omit' as const,
}
// These three SSR reads are independent of one another: the comment URL is
// built from the route slug, not from the listing body, and the viewer read
// needs only the incoming cookie. Awaiting them one after another therefore
// made the document pay three serial API round trips before the first byte,
// which is the dominant term in Detail's Largest Contentful Paint (measured
// TTFB 454ms of a 2556ms LCP). Fire them together and await once, so the
// server response costs one round trip instead of three.
const commentsUrl = `/api/v1/listings/${encodeURIComponent(slug)}/comments`
const listingRequest = useFetch<ApiListingDetail>(detailUrl, publicReadOptions)
const commentsRequest = useFetch<ApiPage<ApiComment>>(commentsUrl, {
  key: `listing-comments:${slug}`,
  ...publicReadOptions,
})
const authRequest = useAuth()

const [
  { data: listing, pending, error },
  { data: initialComments, error: initialCommentsError },
  {
    user: me,
    status: authStatus,
    pending: mePending,
  },
] = await Promise.all([listingRequest, commentsRequest, authRequest])

if (error.value) {
  const statusCode = [404, 410].includes(error.value.statusCode ?? 0)
    ? error.value.statusCode!
    : 500
  // The public error view receives only a status, never the API body or stack.
  throw createError({ statusCode, fatal: true })
}

// The first public comment page is crawlable, but its failure never replaces
// the successfully loaded listing with a fatal error page.
const commentsReadFailed = computed(() => Boolean(initialCommentsError.value))

const gone = computed(() => error.value?.statusCode === 410)
const missing = computed(() => error.value?.statusCode === 404)

const renderNow = useState('listing-day-now', () => Date.now())
const saveLabel = 'ذخیره در علاقه‌مندی‌ها'
const removeLabel = 'حذف از علاقه‌مندی‌ها'
const favorited = ref<boolean>()
const myStars = ref<number | null>()
const viewerPending = ref(false)
const viewerReadFailed = ref(false)
const favoritePending = ref(false)
const favoriteAuthExpired = ref(false)
const favoriteError = ref('')
const favoriteNotice = ref('')
const hydrated = ref(false)

// Contact entry (chat MVP). The chat module itself is imported dynamically so
// ordinary Detail visits never download conversation code.
const contactPending = ref(false)
const contactError = ref('')
const contactNotice = ref('')
const contactAuthExpired = ref(false)
const contactQuery = computed(() => route.query.contact === '1')
const contactLoginHref = computed(() => {
  const params = new URLSearchParams()
  params.set('contact', '1')
  return `/login?next=${encodeURIComponent(`${route.path}?${params.toString()}`)}`
})
let contactResumeAttempted = false

async function contactSeller(): Promise<void> {
  if (contactPending.value || !listing.value) return
  contactPending.value = true
  contactError.value = ''
  contactNotice.value = ''
  try {
    const { startListingConversation } = await import('~/utils/contactSeller')
    const conversation = await startListingConversation(listing.value.slug)
    await navigateTo(`/me/conversations/${conversation.public_id}?listing=${encodeURIComponent(listing.value.slug)}`)
  } catch (requestError) {
    const status = statusOf(requestError)
    if (status === 401) {
      contactAuthExpired.value = true
      contactError.value = 'نشست شما منقضی شده است؛ برای گفت‌وگو دوباره وارد شوید.'
    } else if (status === 403) {
      contactError.value = 'گفت‌وگو با این فروشنده ممکن نیست.'
    } else if (status === 410) {
      contactError.value = 'این آگهی حذف شده است و گفت‌وگو آغاز نمی‌شود.'
    } else if (status === 404) {
      contactError.value = 'این آگهی در دسترس نیست.'
    } else {
      contactError.value = 'آغاز گفت‌وگو انجام نشد؛ دوباره تلاش کنید.'
    }
  } finally {
    contactPending.value = false
  }
}

function requestStatus(error: unknown): number | undefined {
  const candidate = error as {
    status?: unknown
    statusCode?: unknown
    response?: { status?: unknown }
  }
  for (const value of [candidate?.status, candidate?.statusCode, candidate?.response?.status]) {
    if (typeof value === 'number') return value
  }
  return undefined
}

async function loadFavoriteState() {
  if (viewerPending.value || authStatus.value !== 'authenticated' || !me.value || !listing.value) return
  viewerPending.value = true
  viewerReadFailed.value = false
  favoriteError.value = ''
  favoriteNotice.value = ''
  try {
    const detail = await $fetch<ApiListingDetail>(detailUrl, { cache: 'no-store' })
    const validStars = detail.my_stars === null
      || (Number.isInteger(detail.my_stars) && (detail.my_stars ?? 0) >= 1 && (detail.my_stars ?? 0) <= 5)
    if (typeof detail.favorited === 'boolean' && validStars) {
      favorited.value = detail.favorited
      myStars.value = detail.my_stars ?? null
      favoriteAuthExpired.value = false
    } else {
      favoriteAuthExpired.value = true
    }
  } catch (requestError) {
    if (requestStatus(requestError) === 401) {
      favoriteAuthExpired.value = true
      favoriteNotice.value = 'نشست شما منقضی شده است؛ دوباره وارد شوید.'
    } else {
      viewerReadFailed.value = true
      favoriteError.value = 'خواندن وضعیت علاقه‌مندی ممکن نشد؛ دوباره تلاش کنید.'
    }
  } finally {
    viewerPending.value = false
  }
}

async function toggleFavorite() {
  if (favoritePending.value || favorited.value === undefined || !listing.value) return
  const wasFavorited = favorited.value
  favoritePending.value = true
  favoriteError.value = ''
  favoriteNotice.value = ''
  try {
    const response = await $fetch<{ favorited: boolean }>(
      `/api/v1/me/favorites/${encodeURIComponent(listing.value.public_id)}`,
      { method: wasFavorited ? 'DELETE' : 'PUT' },
    )
    favorited.value = response.favorited
    favoriteNotice.value = response.favorited
      ? 'در علاقه‌مندی‌ها ذخیره شد.'
      : 'از علاقه‌مندی‌ها حذف شد.'
  } catch (requestError) {
    if (requestStatus(requestError) === 401) {
      favoriteAuthExpired.value = true
      favoriteNotice.value = 'نشست شما منقضی شده است؛ دوباره وارد شوید.'
    } else {
      // No optimistic update: the old boolean and label remain intact.
      favoriteError.value = 'تغییر علاقه‌مندی انجام نشد؛ دوباره تلاش کنید.'
    }
  } finally {
    favoritePending.value = false
  }
}

async function retryFavorite() {
  if (favorited.value === undefined) await loadFavoriteState()
  else await toggleFavorite()
}

function applyRatingState(state: ApiRatingState): void {
  if (!listing.value) return
  listing.value.rating_average = state.rating_average
  listing.value.rating_count = state.rating_count
  myStars.value = state.my_stars
}

onMounted(() => {
  hydrated.value = true
  if (authStatus.value === 'authenticated' && me.value && listing.value) void loadFavoriteState()
  // Exactly one automatic resume after a login continuation; a failure keeps a
  // manual retry on this page instead of looping.
  if (
    contactQuery.value
    && !contactResumeAttempted
    && authStatus.value === 'authenticated'
    && me.value
    && listing.value
    && me.value.public_id !== listing.value.seller.public_id
  ) {
    contactResumeAttempted = true
    void contactSeller()
  }
})

useHead(() => {
  const current = listing.value
  const cover = current?.images[0]
  const description = current?.description.slice(0, 160)
  return {
    title: current ? `${current.title} | شبدر` : 'شبدر',
    meta: current
      ? [
          { name: 'description', content: description },
          { property: 'og:title', content: current.title },
          { property: 'og:description', content: description },
          { property: 'og:url', content: `/listings/${current.slug}` },
        ]
      : [],
    link: current
      ? [
          { rel: 'canonical', href: `/listings/${current.slug}` },
          ...(cover
            ? [{
                key: 'detail-lcp-image',
                rel: 'preload' as const,
                as: 'image' as const,
                href: cover.url,
                imagesrcset: cover.webp_srcset || cover.srcset || undefined,
                imagesizes: (cover.webp_srcset || cover.srcset)
                  ? LISTING_GALLERY_IMAGE_SIZES
                  : undefined,
                type: cover.webp_srcset ? 'image/webp' : undefined,
                fetchpriority: 'high' as const,
              }]
            : []),
        ]
      : [],
    script: current
      ? [
          {
            type: 'application/ld+json',
            innerHTML: JSON.stringify({
              '@context': 'https://schema.org',
              '@type': 'Product',
              name: current.title,
              description: current.description,
              category: current.category,
              offers: {
                '@type': 'Offer',
                price: current.price_toman ?? undefined,
                priceCurrency: 'IRT',
                availability: 'https://schema.org/InStock',
              },
              ...(current.rating_count >= 3
                ? {
                    aggregateRating: {
                      '@type': 'AggregateRating',
                      ratingValue: current.rating_average,
                      ratingCount: current.rating_count,
                      bestRating: 5,
                      worstRating: 1,
                    },
                  }
                : {}),
            }).replace(/</g, '\\u003c'),
          },
        ]
      : [],
  }
})
</script>

<style scoped>
.detail,
.detail__article {
  display: flex;
  flex-direction: column;
  gap: var(--space-8);
  min-inline-size: 0;
}

.detail__alert-copy {
  margin: 0;
}

.detail__header {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-3);
}

.detail__back {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  min-block-size: 44px;
  color: var(--color-text-muted);
  font-size: var(--font-size-14);
  font-weight: 600;
  text-decoration: none;
}

.detail__back:hover {
  color: var(--color-primary);
}

.detail__summary h1,
.detail__summary p,
.detail__favorite p,
.detail__body h2,
.detail__body p {
  margin: 0;
}

.detail__category {
  display: inline-flex;
  align-items: center;
  margin: 0 0 var(--space-2);
  padding: var(--space-1) var(--space-3);
  color: var(--color-ink-a);
  font-size: var(--font-size-12);
  font-weight: 600;
  line-height: 1.6;
  background: var(--color-tint-a);
  border-radius: var(--radius-pill);
}

.detail__summary h1 {
  max-inline-size: 30ch;
  font-size: var(--font-size-28);
  font-weight: 800;
  line-height: 1.45;
  text-wrap: balance;
  overflow-wrap: anywhere;
}

.detail__facts {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-1) var(--space-2);
  margin-block-start: var(--space-3);
  font-size: var(--font-size-14);
  color: var(--color-text-muted);
}

.detail__facts .app-icon {
  color: var(--color-accent);
}

.detail__layout {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
  min-inline-size: 0;
}

.detail__summary {
  contain: layout style;
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  min-inline-size: 0;
  padding: var(--space-5);
  background: var(--color-surface);
  border-radius: var(--radius-panel);
  box-shadow: var(--shadow-card);
}

/* The location repeats the category that already sits above the title, so it is
   shown once, quietly, beside the fact row instead of as its own paragraph. */
.detail__location {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  color: var(--color-text-muted);
  font-size: var(--font-size-12);
}

.detail__location .app-icon {
  color: var(--color-accent);
}

.detail__price-block {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  background: var(--color-primary-soft);
  border-radius: var(--radius-card);
}

.detail__price-label,
.detail__seller-label {
  color: var(--color-text-muted);
  font-size: var(--font-size-12);
}

.detail__price-block .detail__price-label {
  color: var(--color-primary);
  font-weight: 600;
}

.detail__price {
  margin: 0;
  font-size: var(--font-size-28);
  font-weight: 800;
  line-height: 1.4;
  color: var(--color-primary);
  text-align: end;
}

.detail__seller {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-3);
  background: var(--color-surface-muted);
  border-radius: var(--radius-card);
}

.detail__seller .avatar {
  inline-size: 44px;
  block-size: 44px;
  font-size: var(--font-size-16);
}

.detail__seller > div {
  display: flex;
  flex-direction: column;
  min-inline-size: 0;
}

.detail__seller strong {
  overflow-wrap: anywhere;
}

.detail__actions {
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
}

.detail__contact {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--space-2);
  min-inline-size: 0;
}

.detail__favorite {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: var(--space-3);
}

.detail__favorite-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-inline-size: 48px;
  min-block-size: 48px;
  padding: 0;
}

.detail__favorite-button:active .app-icon,
.detail__favorite-button:active :deep(.app-icon) {
  animation: icon-pop var(--duration-normal) var(--ease-out);
}

@keyframes icon-pop {
  50% { transform: scale(1.25); }
}

.detail__contact-title {
  margin: 0;
  font-size: var(--font-size-14);
  font-weight: 800;
}

.detail__contact-state {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
}

.detail__reports {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding-block-start: var(--space-4);
  border-block-start: 1px solid var(--color-border);
}

.detail__reports-title {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  min-block-size: 44px;
  font-size: var(--font-size-14);
  font-weight: 800;
  cursor: pointer;
}

.detail__report-row {
  display: grid;
  gap: var(--space-1);
}

.detail__report-row > span {
  color: var(--color-text-muted);
  font-size: var(--font-size-12);
}

.detail__body {
  contain: layout style;
  padding: var(--space-5);
  background: var(--color-surface);
  border-radius: var(--radius-card);
  box-shadow: var(--shadow-card);
}

.detail__body h2 {
  margin-block-end: var(--space-3);
}

.detail__description {
  max-inline-size: 70ch;
  color: var(--color-text);
  line-height: 2.05;
  white-space: pre-line;
  overflow-wrap: anywhere;
}

.detail__feedback {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
  padding: var(--space-5);
  background: var(--color-surface);
  border-radius: var(--radius-panel);
  box-shadow: var(--shadow-card);
}

.detail__feedback :deep(.listing-rating),
.detail__feedback :deep(.listing-comments) {
  padding: 0;
  background: transparent;
  border: 0;
  box-shadow: none;
}

@media (min-width: 1024px) {
  .detail__layout {
    display: grid;
    grid-template-columns: minmax(0, 1.6fr) minmax(18rem, 1fr);
    align-items: start;
    gap: var(--space-6);
  }

  .detail__summary {
    position: sticky;
    inset-block-start: var(--space-6);
  }
}

@media (min-width: 1280px) {
  .detail__layout {
    gap: var(--space-8);
  }

  .detail__summary {
    padding: var(--space-6);
  }
}
</style>
