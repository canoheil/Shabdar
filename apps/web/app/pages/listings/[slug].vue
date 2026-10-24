<!--
  Listing detail (Phase 3 Task 7): SSR title, canonical, meta tags, and
  JSON-LD Product/Offer with only true fields (no AggregateRating — counts
  are 0 until the ratings phase). 404/410 map to plain Persian messages.
-->
<template>
  <section class="detail">
    <p v-if="pending">در حال بارگذاری…</p>
    <div v-else-if="gone" class="alert-error" role="alert">
      <p>این آگهی دیگر موجود نیست.</p>
      <NuxtLink to="/">بازگشت به خانه</NuxtLink>
    </div>
    <div v-else-if="missing" class="alert-error" role="alert">
      <p>آگهی یافت نشد.</p>
      <NuxtLink to="/">بازگشت به خانه</NuxtLink>
    </div>
    <article v-else-if="listing">
      <h1>{{ listing.title }}</h1>
      <p class="card__price">{{ formatPrice(listing.price_toman, listing.price_type) }}</p>
      <p class="meta">{{ listing.city }}، {{ listing.province }} · {{ listing.category }}</p>
      <div v-if="listing.images.length > 0" class="detail__gallery">
        <img
          v-for="(img, i) in listing.images"
          :key="i"
          class="card__cover"
          :src="img.url"
          :alt="img.alt_text ?? listing.title"
          loading="lazy"
        >
      </div>
      <p class="detail__description">{{ listing.description }}</p>
      <p class="meta">فروشنده: {{ listing.seller.display_name ?? 'کاربر شبدر' }}</p>

      <div v-if="!mePending" class="detail__favorite">
        <NuxtLink v-if="!me || favoriteAuthExpired" to="/login" class="btn btn--secondary">
          ورود برای ذخیره
        </NuxtLink>
        <template v-else>
          <button
            v-if="favorited !== undefined"
            type="button"
            class="btn btn--secondary"
            :disabled="favoritePending"
            :aria-pressed="favorited"
            @click="toggleFavorite"
          >
            {{ favoritePending ? 'در حال به‌روزرسانی…' : favorited ? removeLabel : saveLabel }}
          </button>
          <p v-else-if="!favoriteError" class="meta">در حال بررسی علاقه‌مندی…</p>
          <p v-if="favoriteError" class="alert-error" role="alert">{{ favoriteError }}</p>
          <button
            v-if="favoriteError"
            type="button"
            class="btn btn--secondary"
            :disabled="favoritePending"
            @click="retryFavorite"
          >
            تلاش دوباره
          </button>
        </template>
        <p class="meta" role="status" aria-live="polite" aria-atomic="true">{{ favoriteNotice }}</p>
      </div>
    </article>
    <p v-else class="alert-error" role="alert">خطایی رخ داد؛ دوباره تلاش کنید.</p>
  </section>
</template>

<script setup lang="ts">
import type { ApiListingDetail, ApiUser } from '~/types'

const route = useRoute()
const slug = route.params.slug as string
const detailUrl = `/api/v1/listings/${encodeURIComponent(slug)}`

// Keep this original read public: personal state is fetched after hydration.
const { data: listing, pending, error } = await useFetch<ApiListingDetail>(detailUrl)

// The auth check alone may use the SSR cookie; it never adds favorite state
// to the public detail payload or its cached HTML.
const authHeaders = useRequestHeaders(['cookie'])
const { data: me, pending: mePending } = await useFetch<{ user: ApiUser }>('/api/v1/me', {
  headers: authHeaders,
})

const gone = computed(() => error.value?.statusCode === 410)
const missing = computed(() => error.value?.statusCode === 404)

const saveLabel = 'ذخیره در علاقه‌مندی‌ها'
const removeLabel = 'حذف از علاقه‌مندی‌ها'
const favorited = ref<boolean>()
const favoritePending = ref(false)
const favoriteAuthExpired = ref(false)
const favoriteError = ref('')
const favoriteNotice = ref('')

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
  if (favoritePending.value || !me.value?.user || !listing.value) return
  favoritePending.value = true
  favoriteError.value = ''
  favoriteNotice.value = ''
  try {
    const detail = await $fetch<ApiListingDetail>(detailUrl, { cache: 'no-store' })
    if (typeof detail.favorited === 'boolean') {
      favorited.value = detail.favorited
      favoriteAuthExpired.value = false
    } else {
      // Detail deliberately treats a stale session as anonymous. In that
      // case there is no viewer field, so the UI must offer login again.
      favoriteAuthExpired.value = true
    }
  } catch (requestError) {
    if (requestStatus(requestError) === 401) {
      favoriteAuthExpired.value = true
      favoriteNotice.value = 'نشست شما منقضی شده است؛ دوباره وارد شوید.'
    } else {
      favoriteError.value = 'خواندن وضعیت علاقه‌مندی ممکن نشد؛ دوباره تلاش کنید.'
    }
  } finally {
    favoritePending.value = false
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

onMounted(() => {
  if (me.value?.user && listing.value) void loadFavoriteState()
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
  link: listing.value ? [{ rel: 'canonical', href: `/listings/${listing.value.slug}` }] : [],
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
              // No AggregateRating: counts are 0 until the ratings phase
              // (ARCHITECTURE.md §8 — only true fields).
              price: listing.value.price_toman ?? undefined,
              priceCurrency: 'IRT',
              availability: 'https://schema.org/InStock',
            },
          }),
        },
      ]
    : [],
}))
</script>

<style scoped>
.detail {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.detail__gallery {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(10rem, 1fr));
  gap: 0.75rem;
}

.detail__description {
  white-space: pre-line;
}

.detail__favorite {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 0.75rem;
}

.detail__favorite p {
  margin: 0;
}

.detail__favorite .btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
}
</style>
