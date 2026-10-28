<template>
  <section class="detail">
    <AppSkeleton v-if="pending" variant="detail" label="در حال بارگذاری آگهی…" />
    <AppAlert v-else-if="gone" tone="error">
      <p>این آگهی دیگر موجود نیست.</p>
      <NuxtLink to="/" class="btn btn--secondary">بازگشت به خانه</NuxtLink>
    </AppAlert>
    <AppAlert v-else-if="missing" tone="error">
      <p>آگهی یافت نشد.</p>
      <NuxtLink to="/" class="btn btn--secondary">بازگشت به خانه</NuxtLink>
    </AppAlert>
    <article v-else-if="listing" class="detail__article">
      <header class="detail__header">
        <p class="meta">{{ listing.category }}</p>
        <h1>{{ listing.title }}</h1>
      </header>
      <div class="detail__layout">
        <ListingGallery
          class="detail__gallery"
          :images="listing.images"
          :listing-title="listing.title"
        />
        <aside class="detail__summary" aria-label="خلاصه و اقدامات آگهی">
          <p class="detail__price">{{ formatPrice(listing.price_toman, listing.price_type) }}</p>
          <p class="meta">{{ listing.city }}، {{ listing.province }} · {{ listing.category }}</p>
          <p class="meta">فروشنده: {{ listing.seller.display_name ?? 'کاربر شبدر' }}</p>

          <div v-if="!mePending" class="detail__favorite">
            <NuxtLink
              v-if="authStatus === 'anonymous' || favoriteAuthExpired"
              to="/login"
              class="btn btn--secondary"
            >
              ورود برای ذخیره
            </NuxtLink>
            <template v-else-if="authStatus === 'authenticated' && me">
              <AppButton
                v-if="favorited !== undefined"
                variant="secondary"
                :disabled="favoritePending"
                :aria-pressed="favorited"
                @click="toggleFavorite"
              >
                {{ favoritePending ? 'در حال به‌روزرسانی…' : favorited ? removeLabel : saveLabel }}
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

          <ReportControl
            v-if="hydrated && !mePending && authStatus !== 'unavailable' && me?.public_id !== listing.seller.public_id"
            target-type="listing"
            :target-id="listing.slug"
            control-key="listing"
            button-label="گزارش آگهی"
            :anonymous="authStatus !== 'authenticated'"
          />
          <ReportControl
            v-if="hydrated && !mePending && authStatus !== 'unavailable' && me?.public_id !== listing.seller.public_id"
            target-type="user"
            :target-id="listing.seller.public_id"
            control-key="seller"
            button-label="گزارش فروشنده"
            :anonymous="authStatus !== 'authenticated'"
          />
        </aside>
      </div>
      <section class="detail__body" aria-labelledby="detail-description-title">
        <h2 id="detail-description-title">توضیحات</h2>
        <p class="detail__description">{{ listing.description }}</p>
      </section>
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
const { data: listing, pending, error } = await useFetch<ApiListingDetail>(
  detailUrl,
  publicReadOptions,
)

if (error.value) {
  const statusCode = [404, 410].includes(error.value.statusCode ?? 0)
    ? error.value.statusCode!
    : 500
  // The public error view receives only a status, never the API body or stack.
  throw createError({ statusCode, fatal: true })
}

// The first public comment page is crawlable, but its failure never replaces
// the successfully loaded listing with a fatal error page.
const commentsUrl = `/api/v1/listings/${encodeURIComponent(slug)}/comments`
const { data: initialComments, error: initialCommentsError } = await useFetch<ApiPage<ApiComment>>(
  commentsUrl,
  { key: `listing-comments:${slug}`, ...publicReadOptions },
)
const commentsReadFailed = computed(() => Boolean(initialCommentsError.value))

const {
  user: me,
  status: authStatus,
  pending: mePending,
} = await useAuth()

const gone = computed(() => error.value?.statusCode === 410)
const missing = computed(() => error.value?.statusCode === 404)

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
})

useHead(() => ({
  title: listing.value ? `${listing.value.title} | شبدر` : 'شبدر',
  meta: listing.value
    ? [
        { name: 'description', content: listing.value.description.slice(0, 160) },
        { property: 'og:title', content: listing.value.title },
        { property: 'og:description', content: listing.value.description.slice(0, 160) },
        { property: 'og:url', content: `/listings/${listing.value.slug}` },
      ]
    : [],
  link: listing.value
    ? [
        { rel: 'canonical', href: `/listings/${listing.value.slug}` },
        ...(listing.value.images[0]
          ? [{
              key: 'detail-lcp-image',
              rel: 'preload' as const,
              as: 'image' as const,
              href: listing.value.images[0].url,
              fetchpriority: 'high' as const,
            }]
          : []),
      ]
    : [],
  script: listing.value
    ? [
        {
          type: 'application/ld+json',
          innerHTML: JSON.stringify({
            '@context': 'https://schema.org',
            '@type': 'Product',
            name: listing.value.title,
            description: listing.value.description,
            category: listing.value.category,
            offers: {
              '@type': 'Offer',
              price: listing.value.price_toman ?? undefined,
              priceCurrency: 'IRT',
              availability: 'https://schema.org/InStock',
            },
            ...(listing.value.rating_count >= 3
              ? {
                  aggregateRating: {
                    '@type': 'AggregateRating',
                    ratingValue: listing.value.rating_average,
                    ratingCount: listing.value.rating_count,
                    bestRating: 5,
                    worstRating: 1,
                  },
                }
              : {}),
          }).replace(/</g, '\\u003c'),
        },
      ]
    : [],
}))
</script>

<style scoped>
.detail,
.detail__article {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
  min-inline-size: 0;
}

.detail__header {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.detail__header h1,
.detail__header p,
.detail__summary p,
.detail__favorite p,
.detail__body h2,
.detail__body p {
  margin: 0;
}

.detail__header h1 {
  font-size: clamp(1.55rem, 4vw, 2.2rem);
  line-height: 1.45;
  overflow-wrap: anywhere;
}

.detail__layout {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
  min-inline-size: 0;
}

.detail__summary {
  contain: layout style;
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  min-inline-size: 0;
  padding: var(--space-4);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-card);
}

.detail__price {
  font-size: 1.4rem;
  font-weight: 700;
  color: var(--color-primary);
}

.detail__favorite {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: var(--space-3);
  padding-block-start: var(--space-3);
  border-block-start: 1px solid var(--color-border);
}

.detail__body {
  contain: layout style;
  max-inline-size: 70ch;
}

.detail__body h2 {
  margin-block-end: var(--space-3);
  font-size: 1.25rem;
}

.detail__description {
  line-height: 1.9;
  white-space: pre-line;
  overflow-wrap: anywhere;
}

@media (min-width: 900px) {
  .detail__layout {
    display: grid;
    grid-template-columns: minmax(0, 1.6fr) minmax(18rem, 1fr);
    align-items: start;
    gap: var(--space-6);
  }
}
</style>
