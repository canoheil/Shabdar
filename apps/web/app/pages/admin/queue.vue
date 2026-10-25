<template>
  <section class="queue">
    <header class="queue__header">
      <h1>صف بررسی آگهی‌ها</h1>
      <p class="meta">آگهی‌های در انتظار را بررسی و نتیجه را ثبت کنید.</p>
    </header>

    <AppSkeleton v-if="pending" variant="text" :count="3" label="در حال بارگذاری…" />
    <AppAlert v-else-if="loadError" tone="error">{{ loadError }}</AppAlert>
    <template v-else>
      <AppAlert
        v-if="actionMessage"
        id="queue-action"
        :tone="actionTone"
        aria-atomic="true"
      >
        {{ actionMessage }}
      </AppAlert>
      <AppEmptyState v-if="queue?.data.length === 0" title="آگهی در انتظاری نیست." />
      <ul v-else class="cards">
        <li v-for="item in queue?.data ?? []" :key="item.public_id" class="card">
          <p class="card__title">{{ item.title }}</p>
          <p class="meta">{{ item.city }} · {{ item.category }}</p>
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
            <AppButton
              :disabled="acting === item.public_id"
              @click="approve(item.public_id)"
            >
              تأیید
            </AppButton>
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
</template>

<script setup lang="ts">
import type { ApiErrorBody, ApiPage, ApiQueueItem } from '~/types'

const acting = ref<string | null>(null)
const actionMessage = ref('')
const actionTone = ref<'success' | 'error'>('success')
const loadError = ref<string | null>(null)
const reasons = reactive<Record<string, string>>({})

const {
  data: queue,
  pending,
  error,
  refresh,
} = await useFetch<ApiPage<ApiQueueItem>>('/api/v1/admin/listings/queue', {
  headers: useRequestHeaders(['cookie']),
})

if (error.value) {
  // The API remains the authority for logged-out and non-admin access.
  loadError.value = [401, 403].includes(error.value.statusCode ?? 0)
    ? 'دسترسی ندارید (ویژهٔ مدیر).'
    : 'خطایی رخ داد؛ دوباره تلاش کنید.'
}

function showError(err: unknown, fallback: string) {
  const body = (err as { data?: ApiErrorBody }).data
  const reason = body?.error?.fields?.reason
  actionTone.value = 'error'
  actionMessage.value = reason ? `دلیل رد: ${reason}` : fallback
}

async function approve(publicId: string) {
  acting.value = publicId
  actionMessage.value = ''
  try {
    await $fetch(`/api/v1/admin/listings/${publicId}/approve`, { method: 'POST' })
    actionTone.value = 'success'
    actionMessage.value = 'آگهی تأیید و منتشر شد.'
    await refresh()
  } catch {
    actionTone.value = 'error'
    actionMessage.value = 'خطایی رخ داد؛ دوباره تلاش کنید.'
  } finally {
    acting.value = null
  }
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
.queue {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
}

.queue__header h1,
.queue__header p,
.card p {
  margin: 0;
}

.queue__header h1 {
  font-size: 1.75rem;
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
