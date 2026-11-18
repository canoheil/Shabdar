<template>
  <section class="dashboard">
    <header class="dashboard__header">
      <div>
        <h1>داشبورد فروشنده</h1>
        <p class="meta">وضعیت آگهی‌ها و ذخیره‌های دریافتی را یک‌جا ببینید.</p>
      </div>
      <NuxtLink to="/post" class="btn btn--accent">ثبت آگهی</NuxtLink>
    </header>

    <AppSkeleton v-if="mePending" variant="text" label="در حال بارگذاری…" />

    <AppEmptyState
      v-else-if="authStatus === 'anonymous'"
      title="برای دیدن داشبورد وارد شوید."
      mascot="full"
    >
      <NuxtLink to="/login" class="btn">ورود</NuxtLink>
    </AppEmptyState>

    <template v-else-if="authStatus === 'authenticated' && me">
      <div class="dashboard__welcome">
        <span class="avatar avatar--b" aria-hidden="true">{{ (me.display_name ?? 'ش').slice(0, 1) }}</span>
        <p class="dashboard__hello">خوش آمدید، {{ me.display_name ?? 'کاربر شبدر' }}</p>
      </div>

      <nav class="dashboard__links" aria-label="پیوندهای حساب">
        <NuxtLink to="/me/favorites" class="dashboard__link">
          <span class="dashboard__link-icon" aria-hidden="true">
            <AppIcon name="heart-duotone" :size="22" />
          </span>
          <span class="dashboard__link-label">علاقه‌مندی‌های من</span>
          <AppIcon name="caret-left-bold" :size="14" class="dashboard__link-caret" />
        </NuxtLink>
        <NuxtLink to="/me/blocks" class="dashboard__link">
          <span class="dashboard__link-icon" aria-hidden="true">
            <AppIcon name="shield-check-duotone" :size="22" />
          </span>
          <span class="dashboard__link-label">مدیریت مسدودشدگان</span>
          <AppIcon name="caret-left-bold" :size="14" class="dashboard__link-caret" />
        </NuxtLink>
        <NuxtLink to="/me/conversations" class="dashboard__link">
          <span class="dashboard__link-icon" aria-hidden="true">
            <AppIcon name="chat-circle-duotone" :size="22" />
          </span>
          <span class="dashboard__link-label">پیام‌ها</span>
          <AppIcon name="caret-left-bold" :size="14" class="dashboard__link-caret" />
        </NuxtLink>
      </nav>

      <nav v-if="me.is_admin" class="dashboard__links dashboard__links--admin" aria-label="ناوبری مدیریت">
          <NuxtLink to="/admin/queue" class="dashboard__link">
            <span class="dashboard__link-icon" aria-hidden="true">
              <AppIcon name="clipboard-text-duotone" :size="22" />
            </span>
            <span class="dashboard__link-label">صف بررسی</span>
            <AppIcon name="caret-left-bold" :size="14" class="dashboard__link-caret" />
          </NuxtLink>
          <NuxtLink to="/admin/reports" class="dashboard__link">
            <span class="dashboard__link-icon" aria-hidden="true">
              <AppIcon name="flag-duotone" :size="22" />
            </span>
            <span class="dashboard__link-label">گزارش‌ها</span>
            <AppIcon name="caret-left-bold" :size="14" class="dashboard__link-caret" />
          </NuxtLink>
          <NuxtLink to="/admin/moderation" class="dashboard__link">
            <span class="dashboard__link-icon" aria-hidden="true">
              <AppIcon name="chat-circle-duotone" :size="22" />
            </span>
            <span class="dashboard__link-label">مدیریت دیدگاه</span>
            <AppIcon name="caret-left-bold" :size="14" class="dashboard__link-caret" />
          </NuxtLink>
          <NuxtLink to="/admin/moderation/rules" class="dashboard__link">
            <span class="dashboard__link-icon" aria-hidden="true">
              <AppIcon name="info-fill" :size="22" />
            </span>
            <span class="dashboard__link-label">قوانین دیدگاه</span>
            <AppIcon name="caret-left-bold" :size="14" class="dashboard__link-caret" />
          </NuxtLink>
          <NuxtLink to="/admin/advertising" class="dashboard__link">
            <span class="dashboard__link-icon" aria-hidden="true">
              <AppIcon name="tag-duotone" :size="22" />
            </span>
            <span class="dashboard__link-label">مدیریت تبلیغات</span>
            <AppIcon name="caret-left-bold" :size="14" class="dashboard__link-caret" />
          </NuxtLink>
          <NuxtLink to="/admin/users" class="dashboard__link">
            <span class="dashboard__link-icon" aria-hidden="true">
              <AppIcon name="user-circle" :size="22" />
            </span>
            <span class="dashboard__link-label">کاربران</span>
            <AppIcon name="caret-left-bold" :size="14" class="dashboard__link-caret" />
          </NuxtLink>
      </nav>

      <ul class="stats" aria-label="خلاصهٔ آگهی‌ها">
        <li v-for="tile in tiles" :key="tile.label" class="stat">
          <span class="stat__icon" :class="`stat__icon--${tile.tint}`" aria-hidden="true">
            <AppIcon :name="tile.icon" :size="24" />
          </span>
          <span class="stat__value">{{ tile.value }}</span>
          <span class="stat__label">{{ tile.label }}</span>
        </li>
      </ul>

      <section class="dashboard__listings" aria-labelledby="my-listings-title">
        <div class="dashboard__listings-header">
          <h2 id="my-listings-title">آگهی‌های من</h2>
          <AppField id="listing-status-filter" label="فیلتر وضعیت">
            <template #default="{ id, describedBy, ariaInvalid }">
              <select
                :id="id"
                v-model="statusFilter"
                class="field__input"
                :aria-describedby="describedBy"
                :aria-invalid="ariaInvalid"
                @change="changeStatusFilter"
              >
                <option v-for="option in statusOptions" :key="option.value" :value="option.value">
                  {{ option.label }}
                </option>
              </select>
            </template>
          </AppField>
        </div>
        <AppAlert v-if="actionError" tone="error">{{ actionError }}</AppAlert>
        <p v-if="actionNotice" ref="actionNoticeElement" class="dashboard__notice" role="status" tabindex="-1">
          {{ actionNotice }}
        </p>
        <AppSkeleton v-if="listingsPending" variant="text" :count="3" label="در حال بارگذاری آگهی‌ها…" />
        <AppAlert v-else-if="listingsError" tone="error">
          خطا در خواندن آگهی‌ها؛ دوباره تلاش کنید.
        </AppAlert>
        <AppEmptyState
          v-else-if="myListings.length === 0"
          mascot="full"
          :title="statusFilter === 'all' ? 'هنوز آگهی‌ای ثبت نکرده‌اید.' : 'آگهی‌ای با این وضعیت ندارید.'"
          :description="statusFilter === 'all' ? 'اولین آگهی را ذخیره کنید و برای بررسی ارسال نمایید.' : 'وضعیت دیگری را انتخاب کنید یا همهٔ آگهی‌ها را ببینید.'"
        >
          <AppButton v-if="statusFilter !== 'all'" variant="secondary" @click="clearStatusFilter">
            نمایش همهٔ آگهی‌ها
          </AppButton>
          <NuxtLink v-else to="/post" class="btn">ثبت آگهی</NuxtLink>
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
            <span class="badge" :data-status="item.status"><AppIcon :name="statusIcon(item.status)" :size="13" />{{ statusLabel(item.status) }}</span>
            <div v-if="confirmingDeleteId === item.public_id" class="mine__actions mine__actions--confirm">
              <span>حذف «{{ item.title }}» قطعی است؟</span>
              <AppButton
                variant="danger"
                :data-delete-confirm="item.public_id"
                :loading="pendingAction === `${item.public_id}:delete`"
                :disabled="pendingAction !== null"
                @click="deleteListing(item)"
              >
                تأیید حذف
              </AppButton>
              <AppButton variant="secondary" :disabled="pendingAction !== null" @click="cancelDelete(item)">
                انصراف
              </AppButton>
            </div>
            <div v-else-if="hasOwnerActions(item)" class="mine__actions">
              <NuxtLink
                v-if="canEdit(item)"
                :to="`/post?edit=${item.public_id}`"
                class="btn btn--soft"
              >
                ویرایش
              </NuxtLink>
              <AppButton
                v-if="canSubmit(item)"
                variant="soft"
                :loading="pendingAction === `${item.public_id}:submit`"
                :disabled="pendingAction !== null"
                @click="transitionListing(item, 'submit')"
              >
                ارسال برای بررسی
              </AppButton>
              <AppButton
                v-if="item.status === 'active'"
                variant="soft"
                :loading="pendingAction === `${item.public_id}:mark-sold`"
                :disabled="pendingAction !== null"
                @click="transitionListing(item, 'mark-sold')"
              >
                علامت‌گذاری به‌عنوان فروخته‌شده
              </AppButton>
              <AppButton
                v-if="item.status === 'active'"
                variant="soft"
                :loading="pendingAction === `${item.public_id}:archive`"
                :disabled="pendingAction !== null"
                @click="transitionListing(item, 'archive')"
              >
                بایگانی
              </AppButton>
              <AppButton
                v-if="canDelete(item)"
                variant="danger-quiet"
                :data-delete-trigger="item.public_id"
                :disabled="pendingAction !== null"
                @click="askDelete(item)"
              >
                حذف آگهی
              </AppButton>
            </div>
          </li>
        </ul>
      </section>
    </template>
  </section>
</template>

<script setup lang="ts">
import type { ApiDashboardSummary, ApiListingOut } from '~/types'

type ListingStatus = 'all' | 'draft' | 'pending' | 'active' | 'rejected' | 'sold' | 'archived'
type ListingAction = 'submit' | 'mark-sold' | 'archive'

const STATUS_OPTIONS: Array<{ value: ListingStatus; label: string }> = [
  { value: 'all', label: 'همهٔ وضعیت‌ها' },
  { value: 'draft', label: 'پیش‌نویس' },
  { value: 'pending', label: 'در انتظار تأیید' },
  { value: 'active', label: 'فعال' },
  { value: 'rejected', label: 'رد شده' },
  { value: 'sold', label: 'فروخته‌شده' },
  { value: 'archived', label: 'بایگانی' },
]
const statusOptions = STATUS_OPTIONS

function queryStatus(value: unknown): ListingStatus {
  return typeof value === 'string' && STATUS_OPTIONS.some((option) => option.value === value)
    ? value as ListingStatus
    : 'all'
}

const route = useRoute()
const statusFilter = ref<ListingStatus>(queryStatus(route.query.status))

const {
  user: me,
  status: authStatus,
  pending: mePending,
} = await useAuth()
const authHeaders = useRequestHeaders(['cookie'])

const { data: summary, refresh: refreshSummary } = await useFetch<ApiDashboardSummary>('/api/v1/me/dashboard/summary', {
  headers: authHeaders,
})

const {
  data: listings,
  pending: listingsPending,
  error: listingsError,
} = await useFetch<{ data: ApiListingOut[] }>('/api/v1/me/listings', {
  headers: authHeaders,
  query: computed(() => ({ status: queryStatus(route.query.status) })),
})

const myListings = computed(() => listings.value?.data ?? [])
const pendingAction = ref<string | null>(null)
const actionError = ref('')
const actionNotice = ref('')
const confirmingDeleteId = ref<string | null>(null)
const actionNoticeElement = ref<HTMLElement | null>(null)

function focusActionNotice(): void {
  void nextTick(() => actionNoticeElement.value?.focus())
}
function askDelete(item: ApiListingOut): void {
  confirmingDeleteId.value = item.public_id
  void nextTick(() => {
    document.querySelector<HTMLButtonElement>(`[data-delete-confirm="${item.public_id}"]`)?.focus()
  })
}
function cancelDelete(item: ApiListingOut): void {
  confirmingDeleteId.value = null
  void nextTick(() => {
    document.querySelector<HTMLButtonElement>(`[data-delete-trigger="${item.public_id}"]`)?.focus()
  })
}

watch(() => route.query.status, (status) => {
  statusFilter.value = queryStatus(status)
  actionError.value = ''
  actionNotice.value = ''
  confirmingDeleteId.value = null
})

async function changeStatusFilter(): Promise<void> {
  await navigateTo({
    path: '/me',
    query: statusFilter.value === 'all' ? {} : { status: statusFilter.value },
  })
}

async function clearStatusFilter(): Promise<void> {
  statusFilter.value = 'all'
  await changeStatusFilter()
}

function canEdit(item: ApiListingOut): boolean {
  return item.status === 'draft' || item.status === 'rejected'
}
function canSubmit(item: ApiListingOut): boolean {
  return canEdit(item)
}
function canDelete(item: ApiListingOut): boolean {
  return item.status === 'draft' || item.status === 'rejected' || item.status === 'archived'
}
function hasOwnerActions(item: ApiListingOut): boolean {
  return canEdit(item) || canDelete(item) || item.status === 'active'
}

function replaceConfirmedListing(confirmed: ApiListingOut): void {
  if (!listings.value) return
  const status = queryStatus(route.query.status)
  const data = listings.value.data
    .map((item) => item.public_id === confirmed.public_id ? confirmed : item)
    .filter((item) => status === 'all' || item.status === status)
  listings.value = { ...listings.value, data }
}

async function transitionListing(item: ApiListingOut, action: ListingAction): Promise<void> {
  if (pendingAction.value) return
  pendingAction.value = `${item.public_id}:${action}`
  actionError.value = ''
  actionNotice.value = ''
  try {
    const confirmed = await $fetch<ApiListingOut>(`/api/v1/listings/${item.public_id}/${action}`, {
      method: 'POST',
    })
    replaceConfirmedListing(confirmed)
    actionNotice.value = {
      submit: 'آگهی برای بررسی ارسال شد.',
      'mark-sold': 'آگهی به‌عنوان فروخته‌شده علامت‌گذاری شد.',
      archive: 'آگهی بایگانی شد.',
    }[action]
    // The transition is already confirmed; a summary refresh must never roll
    // back that state or turn a successful mutation into a false failure.
    void refreshSummary().catch(() => undefined)
    focusActionNotice()
  } catch {
    actionError.value = 'تغییر وضعیت انجام نشد؛ آگهی در وضعیت قبلی باقی ماند. دوباره تلاش کنید.'
  } finally {
    pendingAction.value = null
  }
}

async function deleteListing(item: ApiListingOut): Promise<void> {
  if (pendingAction.value || !canDelete(item)) return
  pendingAction.value = `${item.public_id}:delete`
  actionError.value = ''
  actionNotice.value = ''
  try {
    await $fetch<{ ok: true }>(`/api/v1/listings/${item.public_id}`, { method: 'DELETE' })
    if (listings.value) {
      listings.value = {
        ...listings.value,
        data: listings.value.data.filter((listing) => listing.public_id !== item.public_id),
      }
    }
    confirmingDeleteId.value = null
    actionNotice.value = 'آگهی حذف شد.'
    void refreshSummary().catch(() => undefined)
    focusActionNotice()
  } catch {
    actionError.value = 'حذف انجام نشد؛ آگهی حفظ شد. دوباره تلاش کنید.'
  } finally {
    pendingAction.value = null
  }
}

const tiles = computed(() => [
  { label: 'فعال', value: formatNumber(summary.value?.active_count ?? 0), icon: 'check-circle-duotone' as const, tint: 'b' },
  { label: 'در انتظار تأیید', value: formatNumber(summary.value?.pending_count ?? 0), icon: 'clock-duotone' as const, tint: 'a' },
  { label: 'فروخته‌شده', value: formatNumber(summary.value?.sold_count ?? 0), icon: 'tag-duotone' as const, tint: 'c' },
  { label: 'ذخیرهٔ آگهی‌های من', value: formatNumber(summary.value?.favorite_count ?? 0), icon: 'heart-duotone' as const, tint: 'a' },
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

function statusIcon(status: string) {
  if (status === 'active') return 'check-circle-fill' as const
  if (status === 'pending') return 'clock-fill' as const
  if (status === 'rejected') return 'x-circle-fill' as const
  return 'minus-circle-fill' as const
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

.dashboard__header,
.dashboard__welcome {
  padding: var(--space-5);
  background: var(--color-surface);
  border-radius: var(--radius-card);
  box-shadow: var(--shadow-card);
}

.dashboard__welcome {
  justify-content: flex-start;
}

.dashboard__links {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-2);
  background: var(--color-surface);
  border-radius: var(--radius-card);
  box-shadow: var(--shadow-card);
}

.dashboard__links--admin {
  padding: var(--space-2);
}

.dashboard__link {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  min-block-size: 52px;
  padding: var(--space-1) var(--space-2);
  font-weight: 600;
  color: var(--color-text);
  text-decoration: none;
  border-radius: var(--radius-control);
  transition: background-color var(--duration-fast), color var(--duration-fast);
}

.dashboard__link + .dashboard__link {
  border-block-start: 1px solid var(--color-border);
  border-start-start-radius: 0;
  border-start-end-radius: 0;
}

.dashboard__link-icon {
  display: grid;
  flex: none;
  place-items: center;
  inline-size: 40px;
  block-size: 40px;
  color: var(--color-ink-b);
  background: var(--color-tint-b);
  border-radius: var(--radius-small);
}

.dashboard__link-label {
  min-inline-size: 0;
  overflow-wrap: anywhere;
}

.dashboard__link:hover {
  color: var(--color-primary);
  background: var(--color-surface-muted);
}

.dashboard__link-caret {
  margin-inline-start: auto;
  color: var(--color-text-muted);
}

.dashboard__header h1,
.dashboard__header p,
.dashboard__hello {
  margin: 0;
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
  position: relative;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-inline-size: 0;
  overflow: hidden;
  padding: var(--space-5);
  background: var(--color-surface);
  border-radius: var(--radius-card);
  box-shadow: var(--shadow-card);
}

.stat::after {
  position: absolute;
  inset-block: var(--space-4);
  inset-inline-start: 0;
  inline-size: 3px;
  content: '';
  background: var(--color-primary);
  border-radius: var(--radius-pill);
}

.stat__icon {
  display: grid;
  place-items: center;
  inline-size: 44px;
  block-size: 44px;
  border-radius: var(--radius-control);
}

.stat__icon--a {
  color: var(--color-ink-a);
  background: var(--color-tint-a);
}

.stat__icon--b {
  color: var(--color-ink-b);
  background: var(--color-tint-b);
}

.stat__icon--c {
  color: var(--color-ink-c);
  background: var(--color-tint-c);
}

.stat__value {
  font-size: var(--font-size-22);
  font-weight: 800;
  color: var(--color-primary);
}

.stat__label {
  color: var(--color-text-muted);
  font-size: var(--font-size-14);
}

.dashboard__listings {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.dashboard__listings-header {
  display: flex;
  flex-wrap: wrap;
  align-items: end;
  justify-content: space-between;
  gap: var(--space-3);
}

.dashboard__listings-header :deep(.field) {
  min-inline-size: min(100%, 13rem);
}

.dashboard__listings h2 {
  margin: 0;
}

.dashboard__notice {
  margin: 0;
  color: var(--color-primary);
  font-size: var(--font-size-14);
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
  border-radius: var(--radius-card);
  box-shadow: var(--shadow-card);
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
  font-weight: 800;
  color: var(--color-text);
  overflow-wrap: anywhere;
}

.mine__rejection {
  color: var(--color-error);
  line-height: 1.8;
  overflow-wrap: anywhere;
}

.mine__actions {
  display: flex;
  flex: 1 0 100%;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  padding-block-start: var(--space-3);
  border-block-start: 1px solid var(--color-border);
}

.mine__actions :deep(.btn) {
  min-block-size: 44px;
  font-size: var(--font-size-14);
}

.mine__actions--confirm > span {
  flex: 1 1 15rem;
  font-size: var(--font-size-14);
  overflow-wrap: anywhere;
}

.badge {
  flex: none;
  padding: var(--space-1) var(--space-3);
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  font-size: var(--font-size-12);
  font-weight: 600;
  color: var(--color-text);
  background: var(--color-surface-muted);
  border-radius: var(--radius-pill);
}

.badge[data-status='active'] {
  color: var(--color-primary);
}

.badge[data-status='pending'] {
  color: var(--color-warning);
  background: var(--color-warning-bg);
}

.badge[data-status='rejected'] {
  color: var(--color-error);
  background: var(--color-error-bg);
}

@media (min-width: 700px) {
  .stats {
    grid-template-columns: repeat(4, minmax(0, 1fr));
  }
}
</style>
