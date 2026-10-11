<!--
  Admin moderation queue (Phase 3 Task 7): pending listings oldest first,
  approve / reject-with-reason. Admin-only UX hint — the API enforces
  is_admin (403 otherwise). CSRF rides the proxy injection.
-->
<template>
  <section class="queue">
    <h1>صف بررسی آگهی‌ها</h1>

    <p v-if="pending">در حال بارگذاری…</p>
    <p v-else-if="loadError" class="alert-error" role="alert">{{ loadError }}</p>
    <template v-else>
      <p v-if="actionMessage" class="alert-ok" role="status">{{ actionMessage }}</p>
      <p v-if="queue?.data.length === 0">آگهی در انتظاری نیست.</p>
      <ul v-else class="cards">
        <li v-for="item in queue?.data ?? []" :key="item.public_id" class="card">
          <p class="card__title">{{ item.title }}</p>
          <p class="meta">{{ item.city }} · {{ item.category }}</p>
          <p class="meta">
            فروشنده: {{ item.seller.display_name ?? 'کاربر شبدر' }}
          </p>
          <label class="field">
            <span>دلیل رد (برای رد کردن الزامی است)</span>
            <input
              v-model="reasons[item.public_id]"
              class="field__input"
              type="text"
              maxlength="500"
            >
          </label>
          <div class="queue__actions">
            <button
              type="button"
              class="btn"
              :disabled="acting === item.public_id"
              @click="approve(item.public_id)"
            >
              تأیید
            </button>
            <button
              type="button"
              class="btn btn--secondary"
              :disabled="acting === item.public_id"
              @click="reject(item.public_id)"
            >
              رد
            </button>
          </div>
        </li>
      </ul>
    </template>
  </section>
</template>

<script setup lang="ts">
import type { ApiErrorBody, ApiPage, ApiQueueItem } from '~/types'

const acting = ref<string | null>(null)
const actionMessage = ref<string | null>(null)
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
  // Logged out (401, no session for the extractor) or non-admin (403):
  // either way this page has nothing to show. The API enforces both.
  loadError.value = [401, 403].includes(error.value.statusCode ?? 0)
    ? 'دسترسی ندارید (ویژهٔ مدیر).'
    : 'خطایی رخ داد؛ دوباره تلاش کنید.'
}

function showError(err: unknown, fallback: string) {
  const body = (err as { data?: ApiErrorBody }).data
  const reason = body?.error?.fields?.reason
  // Reuses the API's field reason verbatim (required/too_long).
  actionMessage.value = reason ? `دلیل رد: ${reason}` : fallback
}

async function approve(publicId: string) {
  acting.value = publicId
  try {
    await $fetch(`/api/v1/admin/listings/${publicId}/approve`, { method: 'POST' })
    actionMessage.value = 'آگهی تأیید و منتشر شد.'
    await refresh()
  } catch {
    actionMessage.value = 'خطایی رخ داد؛ دوباره تلاش کنید.'
  } finally {
    acting.value = null
  }
}

async function reject(publicId: string) {
  acting.value = publicId
  try {
    await $fetch(`/api/v1/admin/listings/${publicId}/reject`, {
      method: 'POST',
      body: { reason: reasons[publicId] ?? '' },
    })
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
  gap: 1rem;
}

.queue__actions {
  display: flex;
  gap: 0.5rem;
}
</style>
