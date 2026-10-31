<template>
  <section class="queue">
    <header class="queue__header">
      <h1>صف بررسی آگهی‌ها</h1>
      <p class="meta">آگهی‌های در انتظار و تصاویر آگهی‌های قدیمی را با مرور انسانی بررسی کنید.</p>
    </header>

    <AppAlert
      v-if="actionMessage"
      id="queue-action"
      :tone="actionTone"
      aria-atomic="true"
    >
      {{ actionMessage }}
    </AppAlert>

    <section aria-labelledby="pending-queue-title" class="queue__section">
      <h2 id="pending-queue-title">آگهی‌های در انتظار انتشار</h2>
      <AppSkeleton v-if="pending" variant="text" :count="3" label="در حال بارگذاری…" />
      <AppAlert v-else-if="loadError" tone="error">{{ loadError }}</AppAlert>
      <template v-else>
        <AppEmptyState v-if="queue?.data.length === 0" title="آگهی در انتظاری نیست." />
        <ul v-else class="cards">
          <li v-for="item in queue?.data ?? []" :key="item.public_id" class="card">
            <img
              v-if="item.cover_image"
              class="card__cover"
              :src="item.cover_image"
              alt=""
              width="240"
              height="180"
              loading="lazy"
            >
            <p class="card__title">{{ item.title }}</p>
            <p class="meta">{{ item.city }} · {{ item.category }} · {{ formatNumber(item.image_count) }} تصویر</p>
            <p class="meta">فروشنده: {{ item.seller.display_name ?? 'کاربر شبدر' }}</p>
            <AppField
              :id="`reject-${item.public_id}`"
              label="دلیل رد (برای رد کردن الزامی است)"
            >
              <input
                :id="`reject-${item.public_id}`"
                v-model="reasons[item.public_id]"
                class="field__input"
                type="text"
                maxlength="500"
                :aria-describedby="actionMessage ? 'queue-action' : undefined"
                :aria-invalid="actionTone === 'error' && actionMessage.startsWith('دلیل رد:') ? 'true' : undefined"
              >
            </AppField>
            <div class="queue__actions">
              <NuxtLink class="btn" :to="`/admin/listings/${item.public_id}`">
                بررسی تصاویر و تأیید
              </NuxtLink>
              <AppButton
                variant="secondary"
                class="queue__reject"
                :disabled="acting === item.public_id"
                @click="reject(item.public_id)"
              >
                رد
              </AppButton>
            </div>
          </li>
        </ul>
      </template>
    </section>

    <section class="queue__section legacy-queue" aria-labelledby="legacy-queue-title">
      <div class="queue__section-header">
        <h2 id="legacy-queue-title">بررسی مجدد تصاویر آگهی‌های منتشرشده</h2>
        <p class="meta">این آگهی‌ها تا تأیید انسانی مجموعهٔ فعلی تصاویر، عمومی نمایش داده نمی‌شوند.</p>
      </div>
      <AppSkeleton v-if="legacyPending" variant="text" :count="2" label="در حال بارگذاری صف تصاویر…" />
      <AppAlert v-else-if="legacyLoadError" tone="error">{{ legacyLoadError }}</AppAlert>
      <template v-else>
        <AppEmptyState
          v-if="legacyQueue?.data.length === 0"
          title="آگهی قدیمی نیازمند بررسی تصویر نیست."
        />
        <ul v-else class="cards">
          <li v-for="item in legacyQueue?.data ?? []" :key="item.public_id" class="card">
            <img
              v-if="item.cover_image"
              class="card__cover"
              :src="item.cover_image"
              alt=""
              width="240"
              height="180"
              loading="lazy"
            >
            <p class="card__title">{{ item.title }}</p>
            <p class="meta">{{ item.city }} · {{ item.category }} · {{ formatNumber(item.image_count) }} تصویر</p>
            <p class="meta">فروشنده: {{ item.seller.display_name ?? 'کاربر شبدر' }}</p>
            <NuxtLink class="btn" :to="`/admin/listings/${item.public_id}`">
              بررسی تصاویر
            </NuxtLink>
          </li>
        </ul>
      </template>
    </section>
  </section>
</template>

<script setup lang="ts">
import type { ApiErrorBody, ApiPage, ApiQueueItem } from '~/types'

const route = useRoute()
const acting = ref<string | null>(null)
const actionMessage = ref('')
const actionTone = ref<'success' | 'error'>('success')
const loadError = ref<string | null>(null)
const legacyLoadError = ref<string | null>(null)
const reasons = reactive<Record<string, string>>({})

const confirmedMessages: Record<string, string> = {
  published: 'آگهی تأیید و منتشر شد.',
  restored: 'بررسی تصاویر ثبت شد و آگهی دوباره قابل نمایش است.',
  rejected: 'آگهی رد شد.',
  removed: 'آگهی حذف شد.',
}
const confirmed = typeof route.query.review === 'string' ? route.query.review : ''
if (confirmedMessages[confirmed]) actionMessage.value = confirmedMessages[confirmed]

const {
  data: queue,
  pending,
  error,
  refresh,
} = await useFetch<ApiPage<ApiQueueItem>>('/api/v1/admin/listings/queue', {
  headers: useRequestHeaders(['cookie']),
})
const {
  data: legacyQueue,
  pending: legacyPending,
  error: legacyError,
} = await useFetch<ApiPage<ApiQueueItem>>('/api/v1/admin/listings/image-review-queue', {
  headers: useRequestHeaders(['cookie']),
})

function accessMessage(statusCode: number | undefined): string {
  return [401, 403].includes(statusCode ?? 0)
    ? 'دسترسی ندارید (ویژهٔ مدیر).'
    : 'خطایی رخ داد؛ دوباره تلاش کنید.'
}

if (error.value) loadError.value = accessMessage(error.value.statusCode)
if (legacyError.value) legacyLoadError.value = accessMessage(legacyError.value.statusCode)

function showError(err: unknown, fallback: string) {
  const body = (err as { data?: ApiErrorBody }).data
  const reason = body?.error?.fields?.reason
  actionTone.value = 'error'
  actionMessage.value = reason ? `دلیل رد: ${reason}` : fallback
}

async function reject(publicId: string) {
  acting.value = publicId
  actionMessage.value = ''
  try {
    await $fetch(`/api/v1/admin/listings/${publicId}/reject`, {
      method: 'POST',
      body: { reason: reasons[publicId] ?? '' },
    })
    actionTone.value = 'success'
    actionMessage.value = 'آگهی رد شد.'
    await refresh()
  } catch (err) {
    showError(err, 'خطایی رخ داد؛ دوباره تلاش کنید.')
  } finally {
    acting.value = null
  }
}
</script>

<style scoped>
.queue,
.queue__section,
.queue__section-header {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
}

.queue__section {
  gap: var(--space-4);
}

.queue__section-header {
  gap: var(--space-2);
}

.queue__header h1,
.queue__header p,
.queue__section h2,
.queue__section p,
.card p {
  margin: 0;
}

.queue__header h1 {
  font-size: 1.75rem;
}

.queue__section h2 {
  font-size: 1.25rem;
}

.cards {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(20rem, 100%), 1fr));
  gap: var(--space-4);
  margin: 0;
  padding: 0;
  list-style: none;
}

.card {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  min-inline-size: 0;
  padding: var(--space-4);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-card);
}

.card__cover {
  inline-size: 100%;
  block-size: 10rem;
  object-fit: cover;
  background: var(--color-surface-muted);
  border-radius: var(--radius-control);
}

.card__title {
  font-weight: 700;
  overflow-wrap: anywhere;
}

.queue__actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.queue__reject {
  color: var(--color-error);
  border-color: var(--color-error);
}
</style>
