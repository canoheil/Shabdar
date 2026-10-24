<!--
  Personal favorites (Phase 6 Task 7): active saved listings only. The first
  page may be SSR'd with the viewer's cookie; later pages and removals are
  local, confirmed API operations. Pagination is forward-only by design.
-->
<template>
  <section class="favorites">
    <h1>علاقه‌مندی‌های من</h1>

    <p v-if="mePending">در حال بارگذاری…</p>

    <template v-else-if="!me">
      <p>برای دیدن علاقه‌مندی‌های خود وارد شوید.</p>
      <NuxtLink to="/login" class="btn">ورود</NuxtLink>
    </template>

    <template v-else>
      <NuxtLink to="/me" class="favorites__back">بازگشت به داشبورد</NuxtLink>

      <p v-if="favoritesPending">در حال بارگذاری علاقه‌مندی‌ها…</p>

      <template v-else-if="favoritesError">
        <p class="alert-error" role="alert">
          خواندن علاقه‌مندی‌ها ممکن نشد؛ دوباره تلاش کنید.
        </p>
        <button type="button" class="btn btn--secondary favorites__retry" @click="retryInitial">
          تلاش دوباره
        </button>
      </template>

      <template v-else>
        <p v-if="items.length === 0 && !hasMore" class="favorites__empty">
          هنوز آگهی‌ای ذخیره نکرده‌اید.
        </p>

        <ul v-else-if="items.length > 0" class="cards">
          <li v-for="item in items" :key="item.public_id" class="card">
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
            <button
              type="button"
              class="btn btn--secondary"
              :disabled="removingId !== null"
              @click="removeFavorite(item)"
            >
              {{ removingId === item.public_id ? 'در حال حذف…' : 'حذف از علاقه‌مندی‌ها' }}
            </button>
          </li>
        </ul>

        <p v-if="actionError" class="alert-error" role="alert">{{ actionError }}</p>
        <p v-if="loadMoreError" class="alert-error" role="alert">{{ loadMoreError }}</p>

        <button
          v-if="hasMore"
          type="button"
          class="btn btn--secondary favorites__more"
          :disabled="loadingMore || removingId !== null"
          @click="loadMore"
        >
          {{ loadingMore ? 'در حال بارگذاری…' : 'نمایش موارد بیشتر' }}
        </button>

        <p class="meta" role="status" aria-live="polite" aria-atomic="true">{{ resultMessage }}</p>
      </template>
    </template>
  </section>
</template>

<script setup lang="ts">
import type { ApiListingCard, ApiPage, ApiUser } from '~/types'

const authHeaders = useRequestHeaders(['cookie'])
const { data: me, pending: mePending } = await useFetch<{ user: ApiUser }>('/api/v1/me', {
  headers: authHeaders,
})

const {
  data: favorites,
  pending: favoritesPending,
  error: favoritesError,
  refresh: refreshFavorites,
} = await useFetch<ApiPage<ApiListingCard>>('/api/v1/me/favorites', {
  headers: authHeaders,
})

const items = ref<ApiListingCard[]>(favorites.value?.data ?? [])
const nextCursor = ref<string | null>(favorites.value?.page.next_cursor ?? null)
const hasMore = ref(favorites.value?.page.has_more ?? false)
const removingId = ref<string | null>(null)
const loadingMore = ref(false)
const actionError = ref('')
const loadMoreError = ref('')
const resultMessage = ref('')

function replaceFirstPage(page: ApiPage<ApiListingCard>) {
  items.value = page.data
  nextCursor.value = page.page.next_cursor
  hasMore.value = page.page.has_more
}

async function retryInitial() {
  try {
    await refreshFavorites()
    if (!favoritesError.value && favorites.value) replaceFirstPage(favorites.value)
  } catch {
    // useFetch retains the error state and the visible retry control.
  }
}

async function loadMore() {
  if (loadingMore.value || removingId.value !== null || !hasMore.value) return
  loadingMore.value = true
  loadMoreError.value = ''
  resultMessage.value = ''
  try {
    const page = await $fetch<ApiPage<ApiListingCard>>('/api/v1/me/favorites', {
      query: nextCursor.value ? { cursor: nextCursor.value } : {},
    })
    const known = new Set(items.value.map((item) => item.public_id))
    const additions = page.data.filter((item) => !known.has(item.public_id))
    items.value = [...items.value, ...additions]
    nextCursor.value = page.page.next_cursor
    hasMore.value = page.page.has_more
  } catch {
    // Keep every loaded card and the old cursor so the same request can retry.
    loadMoreError.value = 'خواندن موارد بیشتر ممکن نشد؛ دوباره تلاش کنید.'
  } finally {
    loadingMore.value = false
  }
}

async function removeFavorite(item: ApiListingCard) {
  if (removingId.value !== null) return
  removingId.value = item.public_id
  actionError.value = ''
  loadMoreError.value = ''
  resultMessage.value = ''
  try {
    await $fetch(`/api/v1/me/favorites/${encodeURIComponent(item.public_id)}`, {
      method: 'DELETE',
    })
    // The card leaves only after the API confirms the deletion. In
    // particular, nextCursor remains valid even if this card was its anchor.
    items.value = items.value.filter((candidate) => candidate.public_id !== item.public_id)
    resultMessage.value = 'از علاقه‌مندی‌ها حذف شد.'
  } catch {
    actionError.value = 'حذف انجام نشد؛ دوباره تلاش کنید.'
  } finally {
    removingId.value = null
  }
}

useHead({ title: 'علاقه‌مندی‌های من | شبدر' })
</script>

<style scoped>
.favorites {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 1rem;
}

.favorites .btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  align-self: flex-start;
}

.favorites__back {
  display: inline-flex;
  align-items: center;
  align-self: flex-start;
  min-block-size: 44px;
}

.favorites__empty {
  margin: 0;
  padding: 1.25rem;
  background: #fff;
  border: 1px dashed #cbd5e0;
  border-radius: 12px;
}

.card .btn {
  margin-block-start: auto;
}
</style>
