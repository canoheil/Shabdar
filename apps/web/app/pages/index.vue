<!--
  Home feed (Phase 3 Task 7): active listings, newest first, with
  category/city filters. Public — no login needed. States: loading / list /
  empty / error. Auth block kept from the skeleton (welcome + logout).
-->
<template>
  <section class="home">
    <h1>شبدر</h1>

    <template v-if="mePending">
      <p>در حال بارگذاری…</p>
    </template>
    <template v-else-if="me">
      <p>خوش آمدید، {{ me.user.display_name ?? 'کاربر شبدر' }}</p>
      <div class="home__actions">
        <NuxtLink to="/post" class="btn">ثبت آگهی</NuxtLink>
        <button type="button" class="btn btn--secondary" :disabled="loggingOut" @click="logout">
          خروج
        </button>
      </div>
    </template>
    <template v-else>
      <p>برای ثبت آگهی وارد شوید.</p>
      <NuxtLink to="/login" class="btn">ورود</NuxtLink>
    </template>

    <h2>تازه‌ترین آگهی‌ها</h2>

    <form class="filters" @submit.prevent>
      <label class="field">
        <span>دسته</span>
        <select v-model="category" class="field__input" @change="applyFilters">
          <option value="">همه</option>
          <option v-for="c in categories?.data ?? []" :key="c.slug" :value="c.slug">
            {{ c.name_fa }}
          </option>
        </select>
      </label>
      <label class="field">
        <span>شهر</span>
        <select v-model="city" class="field__input" @change="applyFilters">
          <option value="">همه</option>
          <option v-for="c in cities?.data ?? []" :key="c.slug" :value="c.slug">
            {{ c.name_fa }}
          </option>
        </select>
      </label>
    </form>

    <p v-if="feedPending">در حال بارگذاری آگهی‌ها…</p>
    <p v-else-if="feedError" class="alert-error" role="alert">
      خطایی رخ داد؛ دوباره تلاش کنید.
    </p>
    <p v-else-if="(feed?.data ?? []).length === 0">هنوز آگهی‌ای ثبت نشده است.</p>
    <ul v-else class="cards">
      <li v-for="item in feed?.data ?? []" :key="item.public_id" class="card">
        <NuxtLink :to="`/listings/${item.slug}`">
          <img
            v-if="item.cover_image"
            class="card__cover"
            :src="item.cover_image"
            :alt="item.title"
            loading="lazy"
          >
          <span v-else class="card__cover" aria-hidden="true" />
        </NuxtLink>
        <p class="card__title">
          <NuxtLink :to="`/listings/${item.slug}`">{{ item.title }}</NuxtLink>
        </p>
        <p class="card__price">{{ formatPrice(item.price_toman, item.price_type) }}</p>
        <p class="meta">{{ item.city }} · {{ item.category }}</p>
      </li>
    </ul>
  </section>
</template>

<script setup lang="ts">
import type { ApiCategory, ApiCity, ApiListingCard, ApiPage, ApiUser } from '~/types'

const route = useRoute()
const category = ref(typeof route.query.category === 'string' ? route.query.category : '')
const city = ref(typeof route.query.city === 'string' ? route.query.city : '')

function feedQuery() {
  const q: Record<string, string> = {}
  if (category.value) q.category = category.value
  if (city.value) q.city = city.value
  return q
}

function applyFilters() {
  navigateTo({ path: '/', query: feedQuery() })
}

// Reference data for the filters (SSR, public).
const { data: categories } = await useFetch<{ data: ApiCategory[] }>('/api/v1/categories')
const { data: cities } = await useFetch<{ data: ApiCity[] }>(
  '/api/v1/locations/provinces/gilan/cities',
)

// Feed (SSR). useFetch re-runs when the URL query changes.
const {
  data: feed,
  pending: feedPending,
  error: feedError,
  refresh: refreshFeed,
} = await useFetch<ApiPage<ApiListingCard>>('/api/v1/listings', {
  query: feedQuery(),
  watch: [() => route.query],
})

const { data: me, pending: mePending, refresh: refreshMe } = await useFetch<{ user: ApiUser }>(
  '/api/v1/me',
  // The cookie must ride SSR requests (ARCHITECTURE.md §3).
  { headers: useRequestHeaders(['cookie']) },
)

const loggingOut = ref(false)

async function logout() {
  loggingOut.value = true
  try {
    await $fetch('/api/v1/auth/logout', { method: 'POST' })
    await refreshMe()
  } finally {
    loggingOut.value = false
  }
}

// Keep the feed in sync after filter navigation on the client.
watch(() => route.query, () => refreshFeed())
</script>

<style scoped>
.home {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  align-items: stretch;
}

.home__actions {
  display: flex;
  gap: 0.5rem;
  flex-wrap: wrap;
}
</style>
