<template>
  <section class="dashboard">
    <header class="dashboard__header">
      <div>
        <h1>داشبورد فروشنده</h1>
        <p class="meta">وضعیت آگهی‌ها و ذخیره‌های دریافتی را یک‌جا ببینید.</p>
      </div>
      <NuxtLink to="/post" class="btn">ثبت آگهی</NuxtLink>
    </header>

    <AppSkeleton v-if="mePending" variant="text" label="در حال بارگذاری…" />

    <AppEmptyState
      v-else-if="authStatus === 'anonymous'"
      title="برای دیدن داشبورد وارد شوید."
    >
      <NuxtLink to="/login" class="btn">ورود</NuxtLink>
    </AppEmptyState>

    <template v-else-if="authStatus === 'authenticated' && me">
      <div class="dashboard__welcome">
        <p class="dashboard__hello">خوش آمدید، {{ me.display_name ?? 'کاربر شبدر' }}</p>
        <NuxtLink to="/me/favorites" class="btn btn--secondary">علاقه‌مندی‌های من</NuxtLink>
        <NuxtLink to="/me/blocks" class="btn btn--secondary">مدیریت مسدودشدگان</NuxtLink>
      </div>

      <nav v-if="me.is_admin" class="dashboard__admin" aria-label="ناوبری مدیریت">
        <NuxtLink to="/admin/queue">صف بررسی</NuxtLink>
        <NuxtLink to="/admin/reports">گزارش‌ها</NuxtLink>
        <NuxtLink to="/admin/moderation">مدیریت دیدگاه</NuxtLink>
        <NuxtLink to="/admin/moderation/rules">قوانین دیدگاه</NuxtLink>
        <NuxtLink to="/admin/users">کاربران</NuxtLink>
      </nav>

      <ul class="stats" aria-label="خلاصهٔ آگهی‌ها">
        <li v-for="tile in tiles" :key="tile.label" class="stat">
          <span class="stat__value">{{ tile.value }}</span>
          <span class="stat__label">{{ tile.label }}</span>
        </li>
      </ul>

      <section class="dashboard__listings" aria-labelledby="my-listings-title">
        <h2 id="my-listings-title">آگهی‌های من</h2>
        <AppSkeleton v-if="listingsPending" variant="text" :count="3" label="در حال بارگذاری آگهی‌ها…" />
        <AppAlert v-else-if="listingsError" tone="error">
          خطا در خواندن آگهی‌ها؛ دوباره تلاش کنید.
        </AppAlert>
        <AppEmptyState
          v-else-if="myListings.length === 0"
          title="هنوز آگهی‌ای ثبت نکرده‌اید."
          description="اولین آگهی را ذخیره کنید و برای بررسی ارسال نمایید."
        >
          <NuxtLink to="/post" class="btn">ثبت آگهی</NuxtLink>
        </AppEmptyState>
        <ul v-else class="mine">
          <li v-for="item in myListings" :key="item.public_id" class="mine__row">
            <div class="mine__body">
              <NuxtLink
                v-if="item.status === 'active'"
                :to="`/listings/${item.slug}`"
                class="mine__title"
              >
                {{ item.title }}
              </NuxtLink>
              <span v-else class="mine__title">{{ item.title }}</span>
              <p class="meta">{{ formatPrice(item.price_toman, item.price_type) }}</p>
              <p v-if="item.status === 'rejected' && item.rejection_reason" class="mine__rejection">
                دلیل رد: {{ item.rejection_reason }}
              </p>
            </div>
            <span class="badge" :data-status="item.status">{{ statusLabel(item.status) }}</span>
          </li>
        </ul>
      </section>
    </template>
  </section>
</template>

<script setup lang="ts">
import type { ApiDashboardSummary, ApiListingOut, ApiPage } from '~/types'

const {
  user: me,
  status: authStatus,
  pending: mePending,
} = await useAuth()
const authHeaders = useRequestHeaders(['cookie'])

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
  { label: 'فعال', value: formatNumber(summary.value?.active_count ?? 0) },
  { label: 'در انتظار تأیید', value: formatNumber(summary.value?.pending_count ?? 0) },
  { label: 'فروخته‌شده', value: formatNumber(summary.value?.sold_count ?? 0) },
  { label: 'ذخیرهٔ آگهی‌های من', value: formatNumber(summary.value?.favorite_count ?? 0) },
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
  gap: var(--space-6);
}

.dashboard__header,
.dashboard__welcome {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.dashboard__admin {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
  padding: var(--space-3);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-card);
}

.dashboard__admin a {
  display: inline-flex;
  align-items: center;
  min-block-size: 44px;
  padding-inline: var(--space-3);
}

.dashboard__header h1,
.dashboard__header p,
.dashboard__hello {
  margin: 0;
}

.dashboard__header h1 {
  font-size: 1.75rem;
}

.dashboard__hello {
  font-weight: 600;
}

.stats {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.stat {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-inline-size: 0;
  padding: var(--space-4);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-card);
}

.stat__value {
  font-size: 1.65rem;
  font-weight: 700;
  color: var(--color-primary);
}

.stat__label {
  color: var(--color-text-muted);
  font-size: 0.875rem;
}

.dashboard__listings {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.dashboard__listings h2 {
  margin: 0;
  font-size: 1.25rem;
}

.mine {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.mine__row {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-4);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-card);
}

.mine__body {
  display: flex;
  flex: 1 1 16rem;
  flex-direction: column;
  gap: var(--space-1);
  min-inline-size: 0;
}

.mine__body p {
  margin: 0;
}

.mine__title {
  display: inline-flex;
  align-items: center;
  min-block-size: 44px;
  font-weight: 700;
  color: var(--color-text);
  overflow-wrap: anywhere;
}

.mine__rejection {
  color: var(--color-error);
  line-height: 1.8;
  overflow-wrap: anywhere;
}

.badge {
  flex: none;
  padding: var(--space-1) var(--space-3);
  font-size: 0.8125rem;
  font-weight: 600;
  color: var(--color-text);
  background: var(--color-surface-muted);
  border: 1px solid var(--color-border);
  border-radius: 999px;
}

.badge[data-status='active'] {
  color: var(--color-primary);
  border-color: var(--color-primary);
}

.badge[data-status='pending'] {
  color: var(--color-warning);
  background: var(--color-warning-bg);
  border-color: var(--color-warning);
}

.badge[data-status='rejected'] {
  color: var(--color-error);
  background: var(--color-error-bg);
  border-color: var(--color-error);
}

@media (min-width: 768px) {
  .stats {
    grid-template-columns: repeat(4, minmax(0, 1fr));
  }
}
</style>
