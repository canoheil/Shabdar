<!--
  Seller dashboard (Phase 4 Task 6): the four summary counts and the owner's
  own listings. Read-only page — active listings are immutable in v1, so the
  only actions it offers are "post a listing" and links to public pages.
  Responsive by grid auto-fit: one column on a phone, four tiles on desktop.
-->
<template>
  <section class="dashboard">
    <h1>داشبورد فروشنده</h1>

    <p v-if="mePending">در حال بارگذاری…</p>

    <template v-else-if="!me">
      <p>برای دیدن داشبورد وارد شوید.</p>
      <NuxtLink to="/login" class="btn">ورود</NuxtLink>
    </template>

    <template v-else>
      <p class="dashboard__hello">
        خوش آمدید، {{ me.user.display_name ?? 'کاربر شبدر' }}
      </p>
      <NuxtLink to="/me/favorites" class="btn btn--secondary dashboard__favorites">
        علاقه‌مندی‌های من
      </NuxtLink>

      <ul class="stats">
        <li v-for="tile in tiles" :key="tile.label" class="stat">
          <span class="stat__value">{{ tile.value }}</span>
          <span class="stat__label">{{ tile.label }}</span>
        </li>
      </ul>

      <h2>آگهی‌های من</h2>
      <p v-if="listingsPending">در حال بارگذاری آگهی‌ها…</p>
      <p v-else-if="listingsError" class="alert-error" role="alert">
        خطا در خواندن آگهی‌ها؛ دوباره تلاش کنید.
      </p>
      <div v-else-if="myListings.length === 0" class="empty">
        <p>هنوز آگهی‌ای ثبت نکرده‌اید.</p>
        <NuxtLink to="/post" class="btn">ثبت آگهی</NuxtLink>
      </div>
      <ul v-else class="mine">
        <li v-for="item in myListings" :key="item.public_id" class="mine__row">
          <div class="mine__body">
            <NuxtLink v-if="item.status === 'active'" :to="`/listings/${item.slug}`">
              {{ item.title }}
            </NuxtLink>
            <span v-else class="mine__title">{{ item.title }}</span>
            <p class="meta">{{ formatPrice(item.price_toman, item.price_type) }}</p>
            <p v-if="item.status === 'rejected' && item.rejection_reason" class="meta">
              دلیل رد: {{ item.rejection_reason }}
            </p>
          </div>
          <span class="badge" :data-status="item.status">{{ statusLabel(item.status) }}</span>
        </li>
      </ul>
    </template>
  </section>
</template>

<script setup lang="ts">
import type { ApiDashboardSummary, ApiListingOut, ApiPage, ApiUser } from '~/types'

// Sessions ride SSR requests (ARCHITECTURE.md §3).
const authHeaders = useRequestHeaders(['cookie'])

const { data: me, pending: mePending } = await useFetch<{ user: ApiUser }>('/api/v1/me', {
  headers: authHeaders,
})

const { data: summary } = await useFetch<ApiDashboardSummary>('/api/v1/me/dashboard/summary', {
  headers: authHeaders,
})

const {
  data: listings,
  pending: listingsPending,
  error: listingsError,
} = await useFetch<ApiPage<ApiListingOut>>('/api/v1/me/listings', {
  headers: authHeaders,
})

const myListings = computed(() => listings.value?.data ?? [])

const tiles = computed(() => [
  { label: 'فعال', value: summary.value?.active_count ?? 0 },
  { label: 'در انتظار تأیید', value: summary.value?.pending_count ?? 0 },
  { label: 'فروخته‌شده', value: summary.value?.sold_count ?? 0 },
  { label: 'ذخیرهٔ آگهی‌های من', value: summary.value?.favorite_count ?? 0 },
])

const STATUS_LABELS: Record<string, string> = {
  draft: 'پیش‌نویس',
  pending: 'در انتظار تأیید',
  active: 'فعال',
  rejected: 'رد شده',
  sold: 'فروخته‌شده',
  archived: 'بایگانی',
  removed: 'حذف‌شده',
}

function statusLabel(status: string): string {
  return STATUS_LABELS[status] ?? status
}
</script>

<style scoped>
.dashboard {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  align-items: stretch;
}

.dashboard__hello {
  margin: 0;
}

.dashboard__favorites {
  display: inline-flex;
  align-items: center;
  align-self: flex-start;
}

.stats {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(7.5rem, 1fr));
  gap: 0.75rem;
  margin: 0;
  padding: 0;
  list-style: none;
}

.stat {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  padding: 0.875rem;
  background: #fff;
  border: 1px solid #e2e8f0;
  border-radius: 12px;
}

.stat__value {
  font-size: 1.5rem;
  font-weight: 700;
  color: #1a5c2e;
}

.stat__label {
  font-size: 0.875rem;
  color: #555;
}

.empty {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 0.75rem;
  padding: 1.25rem;
  background: #fff;
  border: 1px dashed #cbd5e0;
  border-radius: 12px;
}

.empty p {
  margin: 0;
}

.mine {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  margin: 0;
  padding: 0;
  list-style: none;
}

.mine__row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 0.75rem;
  padding: 0.75rem;
  background: #fff;
  border: 1px solid #e2e8f0;
  border-radius: 12px;
}

.mine__body {
  display: flex;
  flex-direction: column;
  gap: 0.125rem;
  min-inline-size: 0;
}

.mine__title {
  font-weight: 700;
  overflow-wrap: anywhere;
}

/* Persian status chip; colour alone never carries the meaning (the label
   spells it out), which keeps it readable in greyscale. */
.badge {
  flex: none;
  padding-inline: 0.5rem;
  padding-block: 0.125rem;
  font-size: 0.8125rem;
  border-radius: 999px;
  border: 1px solid #cbd5e0;
  background: #f7fafc;
  color: #333;
}

.badge[data-status='active'] {
  border-color: #9ae6b4;
  background: #f0fff4;
  color: #276749;
}

.badge[data-status='pending'] {
  border-color: #fbd38d;
  background: #fffaf0;
  color: #7b341e;
}

.badge[data-status='rejected'] {
  border-color: #feb2b2;
  background: #fff5f5;
  color: #9b2c2c;
}
</style>
