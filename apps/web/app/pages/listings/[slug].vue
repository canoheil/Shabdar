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
    </article>
    <p v-else class="alert-error" role="alert">خطایی رخ داد؛ دوباره تلاش کنید.</p>
  </section>
</template>

<script setup lang="ts">
import type { ApiListingDetail } from '~/types'

const route = useRoute()
const slug = route.params.slug as string

const { data: listing, pending, error } = await useFetch<ApiListingDetail>(
  `/api/v1/listings/${encodeURIComponent(slug)}`,
)

const gone = computed(() => error.value?.statusCode === 410)
const missing = computed(() => error.value?.statusCode === 404)

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
</style>
