<template>
  <section class="review">
    <NuxtLink class="review__back" to="/admin/queue">بازگشت به صف بررسی</NuxtLink>
    <AppSkeleton v-if="pending" variant="text" :count="4" label="در حال بارگذاری جزئیات آگهی…" />
    <template v-else-if="loadError">
      <AppAlert tone="error">{{ loadError }}</AppAlert>
      <AppButton variant="secondary" @click="refresh()">تلاش دوباره</AppButton>
    </template>
    <template v-else-if="listing">
      <header class="review__header">
        <div>
          <h1>بررسی تصاویر آگهی</h1>
          <p class="meta">تأیید نهایی محتوا یک تصمیم انسانی است؛ سالم بودن فایل به‌تنهایی کافی نیست.</p>
        </div>
        <span class="review__status">{{ statusLabel }}</span>
      </header>

      <AppAlert v-if="actionMessage" id="review-action" ref="actionAlert" tone="error" aria-atomic="true">
        {{ actionMessage }}
      </AppAlert>

      <article class="review__facts" aria-labelledby="review-listing-title">
        <h2 id="review-listing-title">{{ listing.title }}</h2>
        <p class="review__description">{{ listing.description }}</p>
        <dl>
          <div><dt>قیمت</dt><dd>{{ formatPrice(listing.price_toman, listing.price_type) }}</dd></div>
          <div><dt>مکان و دسته</dt><dd>{{ listing.city }} · {{ listing.category }}</dd></div>
          <div><dt>فروشنده</dt><dd>{{ listing.seller.display_name ?? 'کاربر شبدر' }}</dd></div>
          <div><dt>شناسهٔ فروشنده</dt><dd dir="ltr">{{ listing.seller.public_id }}</dd></div>
          <div><dt>تلفن فروشنده</dt><dd dir="ltr">{{ listing.seller.phone_e164 }}</dd></div>
        </dl>
      </article>

      <aside class="review__policy" aria-labelledby="review-policy-title">
        <h2 id="review-policy-title">یادآوری سیاست بررسی</h2>
        <p>هر تصویر را از نظر محتوای نامناسب، گمراه‌کننده یا غیرمرتبط بررسی کنید. در صورت تغییر مجموعه، تأیید قبلی باطل می‌شود و مرور کامل باید تکرار شود.</p>
      </aside>

      <section class="review__images" aria-labelledby="review-images-title">
        <div class="review__images-header">
          <h2 id="review-images-title">همهٔ تصاویر جاری ({{ formatNumber(listing.images.length) }})</h2>
          <p class="meta">تصویر بارگذاری‌نشده یا خراب قابل تأیید نیست.</p>
        </div>
        <AppEmptyState v-if="listing.images.length === 0" title="این آگهی تصویری ندارد." />
        <ul v-else class="review__grid">
          <li v-for="(image, index) in listing.images" :key="image.id" class="review-image">
            <img
              :src="image.url"
              :alt="`تصویر ${index + 1} آگهی ${listing.title}`"
              :width="image.width ?? 1200"
              :height="image.height ?? 900"
              @load="markLoaded(image.id)"
              @error="markBroken(image.id)"
            >
            <p class="meta">
              تصویر {{ formatNumber(index + 1) }} ·
              <span v-if="image.width && image.height">{{ formatNumber(image.width) }} × {{ formatNumber(image.height) }}</span>
              <span v-else>ابعاد پس از بررسی فایل ثبت می‌شود</span>
            </p>
            <p v-if="brokenIds.includes(image.id)" class="review-image__error">
              تصویر بارگذاری نشد؛ امکان تأیید آن نیست.
            </p>
            <p v-else-if="!loadedIds.includes(image.id)" class="meta">در حال بارگذاری تصویر…</p>
            <label v-if="reviewable" class="review-image__confirm">
              <input
                v-model="checkedIds"
                type="checkbox"
                :value="image.id"
                :disabled="!isLoaded(image.id) || acting !== null"
              >
              <span>تصویر {{ formatNumber(index + 1) }} را بررسی کردم و محتوای آن را تأیید می‌کنم</span>
            </label>
          </li>
        </ul>
      </section>

      <section v-if="listing.image_review" class="review__attribution" aria-labelledby="review-attribution-title">
        <h2 id="review-attribution-title">آخرین بررسی ثبت‌شده</h2>
        <p>
          {{ listing.image_review.reviewer.display_name ?? 'مدیر شبدر' }} ·
          <time :datetime="listing.image_review.reviewed_at">{{ formatCommentDate(listing.image_review.reviewed_at) }}</time>
        </p>
        <p class="meta">مجموعهٔ {{ formatNumber(listing.image_review.image_ids.length) }} تصویر با شناسه‌های دقیق ثبت شده است.</p>
      </section>

      <section v-if="pendingListing || legacyListing" class="review__actions" aria-labelledby="review-actions-title">
        <h2 id="review-actions-title">ثبت تصمیم</h2>
        <AppField
          v-if="pendingListing"
          id="review-reason"
          label="دلیل رد آگهی"
          help="برای رد آگهی الزامی است؛ هنگام تأیید تصاویر لازم نیست."
        >
          <input
            id="review-reason"
            v-model="reason"
            class="field__input"
            type="text"
            maxlength="500"
            :aria-describedby="actionMessage ? 'review-action' : undefined"
          >
        </AppField>
        <AppField
          v-else
          id="review-reason"
          label="دلیل حذف آگهی"
          help="برای حذف آگهی الزامی است؛ هنگام تأیید تصاویر لازم نیست."
        >
          <input
            id="review-reason"
            v-model="reason"
            class="field__input"
            type="text"
            maxlength="500"
            :aria-describedby="actionMessage ? 'review-action' : undefined"
          >
        </AppField>
        <div class="review__buttons">
          <AppButton
            :disabled="!canApprove || (acting !== null && acting !== 'approve')"
            :loading="acting === 'approve'"
            @click="approveImages"
          >
            {{ pendingListing ? 'تأیید تصاویر و انتشار' : 'تأیید تصاویر و بازیابی انتشار' }}
          </AppButton>
          <AppButton
            v-if="pendingListing"
            variant="secondary"
            :disabled="acting !== null"
            :loading="acting === 'reject'"
            @click="rejectListing"
          >
            رد آگهی
          </AppButton>
          <AppButton
            v-else
            variant="danger"
            :disabled="acting !== null"
            :loading="acting === 'remove'"
            @click="removeListing"
          >
            حذف آگهی
          </AppButton>
        </div>
      </section>
    </template>
  </section>
</template>

<script setup lang="ts">
import type { ApiAdminListingDetail, ApiErrorBody } from '~/types'

const route = useRoute()
const publicId = String(route.params.public_id)
const reason = ref('')
const checkedIds = ref<number[]>([])
const loadedIds = ref<number[]>([])
const brokenIds = ref<number[]>([])
const acting = ref<'approve' | 'reject' | 'remove' | null>(null)
const actionMessage = ref('')
const actionAlert = ref<{ focus: () => void } | null>(null)

const {
  data: listing,
  pending,
  error,
  refresh,
} = await useFetch<ApiAdminListingDetail>(`/api/v1/admin/listings/${publicId}`, {
  headers: useRequestHeaders(['cookie']),
})

const loadError = computed(() => {
  if (!error.value) return null
  return [401, 403].includes(error.value.statusCode ?? 0)
    ? 'دسترسی ندارید (ویژهٔ مدیر).'
    : 'خطایی رخ داد؛ دوباره تلاش کنید.'
})
const pendingListing = computed(() => listing.value?.status === 'pending')
const legacyListing = computed(() => (
  listing.value?.status === 'active' && listing.value.image_review === null
))
const reviewable = computed(() => pendingListing.value || legacyListing.value)
const statusLabel = computed(() => {
  const labels: Record<string, string> = {
    draft: 'پیش‌نویس',
    pending: 'در انتظار بررسی',
    active: legacyListing.value ? 'منتشرشدهٔ نیازمند بررسی تصاویر' : 'منتشرشده',
    rejected: 'ردشده',
    sold: 'فروخته‌شده',
    archived: 'بایگانی‌شده',
    removed: 'حذف‌شده',
  }
  return labels[listing.value?.status ?? ''] ?? listing.value?.status ?? ''
})
const canApprove = computed(() => {
  const current = listing.value
  if (!current || !reviewable.value || current.images.length === 0) return false
  return current.images.every((image) => (
    isLoaded(image.id) && checkedIds.value.includes(image.id)
  ))
})

function isLoaded(id: number): boolean {
  return loadedIds.value.includes(id) && !brokenIds.value.includes(id)
}

function markLoaded(id: number): void {
  brokenIds.value = brokenIds.value.filter((imageId) => imageId !== id)
  if (!loadedIds.value.includes(id)) loadedIds.value.push(id)
}

function markBroken(id: number): void {
  loadedIds.value = loadedIds.value.filter((imageId) => imageId !== id)
  checkedIds.value = checkedIds.value.filter((imageId) => imageId !== id)
  if (!brokenIds.value.includes(id)) brokenIds.value.push(id)
}

function showError(err: unknown): void {
  const failure = err as {
    data?: ApiErrorBody
    status?: number
    statusCode?: number
    response?: { status?: number }
  }
  const fields = failure.data?.error?.fields
  const status = failure.status ?? failure.statusCode ?? failure.response?.status
  if (fields?.image_ids === 'mismatch') {
    actionMessage.value = 'مجموعه تصاویر تغییر کرده است؛ صفحه را تازه‌سازی کنید و دوباره بررسی کنید.'
  } else if (fields?.image_ids) {
    actionMessage.value = 'همهٔ تصاویر جاری باید با شناسه‌های دقیق تأیید شوند.'
  } else if (fields?.images) {
    const messages: Record<string, string> = {
      unreadable: 'فایل یک تصویر روی ذخیره‌ساز قابل خواندن نیست؛ آگهی منتشر نشد.',
      invalid_image: 'یک فایل تصویر سالم نیست یا قالب آن پشتیبانی نمی‌شود؛ آگهی منتشر نشد.',
      animated: 'تصاویر متحرک پذیرفته نمی‌شوند؛ آگهی منتشر نشد.',
      too_large: 'حجم یا ابعاد یک تصویر بیش از حد مجاز است؛ آگهی منتشر نشد.',
    }
    actionMessage.value = messages[fields.images] ?? 'تصاویر قابل تأیید نیستند؛ آگهی منتشر نشد.'
  } else if (fields?.reason) {
    actionMessage.value = 'دلیل رد یا حذف را با متن معتبر وارد کنید.'
  } else if (fields?.status) {
    actionMessage.value = 'وضعیت آگهی تغییر کرده است؛ صفحه را تازه‌سازی کنید.'
  } else if ([401, 403].includes(status ?? 0)) {
    actionMessage.value = 'دسترسی ندارید (ویژهٔ مدیر).'
  } else {
    actionMessage.value = 'خطایی رخ داد؛ دوباره تلاش کنید.'
  }
  void nextTick(() => actionAlert.value?.focus())
}

async function approveImages(): Promise<void> {
  const current = listing.value
  if (!current || !canApprove.value) return
  acting.value = 'approve'
  actionMessage.value = ''
  const legacy = legacyListing.value
  try {
    await $fetch(`/api/v1/admin/listings/${publicId}/${legacy ? 'images/review' : 'approve'}`, {
      method: 'POST',
      body: { image_ids: current.images.map((image) => image.id) },
    })
    await navigateTo({ path: '/admin/queue', query: { review: legacy ? 'restored' : 'published' } })
  } catch (err) {
    showError(err)
  } finally {
    acting.value = null
  }
}

async function rejectListing(): Promise<void> {
  acting.value = 'reject'
  actionMessage.value = ''
  try {
    await $fetch(`/api/v1/admin/listings/${publicId}/reject`, {
      method: 'POST',
      body: { reason: reason.value },
    })
    await navigateTo({ path: '/admin/queue', query: { review: 'rejected' } })
  } catch (err) {
    showError(err)
  } finally {
    acting.value = null
  }
}

async function removeListing(): Promise<void> {
  acting.value = 'remove'
  actionMessage.value = ''
  try {
    await $fetch(`/api/v1/admin/listings/${publicId}/remove`, {
      method: 'POST',
      body: { reason: reason.value },
    })
    await navigateTo({ path: '/admin/queue', query: { review: 'removed' } })
  } catch (err) {
    showError(err)
  } finally {
    acting.value = null
  }
}
</script>

<style scoped>
.review,
.review__header,
.review__images,
.review__images-header,
.review__actions {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.review {
  gap: var(--space-6);
}

.review__back {
  align-self: flex-start;
  min-block-size: 44px;
  display: inline-flex;
  align-items: center;
}

.review__header {
  flex-direction: row;
  flex-wrap: wrap;
  align-items: flex-start;
  justify-content: space-between;
}

.review__header h1,
.review__header p,
.review__facts h2,
.review__facts p,
.review__policy h2,
.review__policy p,
.review__images h2,
.review__images p,
.review__attribution h2,
.review__attribution p,
.review__actions h2 {
  margin: 0;
}

.review__header h1 {
  font-size: 1.75rem;
}

.review__status {
  padding: var(--space-2) var(--space-3);
  color: var(--color-info);
  background: var(--color-info-bg);
  border-radius: var(--radius-control);
  font-weight: 700;
}

.review__facts,
.review__policy,
.review__attribution,
.review__actions {
  min-inline-size: 0;
  padding: var(--space-4);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-card);
}

.review__facts h2,
.review__description,
.review__attribution p {
  overflow-wrap: anywhere;
}

.review__description {
  margin-block: var(--space-3);
  white-space: pre-wrap;
}

.review__facts dl {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(15rem, 100%), 1fr));
  gap: var(--space-3);
  margin: 0;
}

.review__facts dl div {
  min-inline-size: 0;
}

.review__facts dt {
  color: var(--color-text-muted);
  font-size: 0.875rem;
}

.review__facts dd {
  margin: var(--space-1) 0 0;
  overflow-wrap: anywhere;
}

.review__policy {
  display: grid;
  gap: var(--space-2);
  color: var(--color-info);
  background: var(--color-info-bg);
  border-color: currentColor;
}

.review__policy h2,
.review__images h2,
.review__attribution h2,
.review__actions h2 {
  font-size: 1.125rem;
}

.review__grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(17rem, 100%), 1fr));
  gap: var(--space-4);
  margin: 0;
  padding: 0;
  list-style: none;
}

.review-image {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  min-inline-size: 0;
  padding: var(--space-3);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-card);
}

.review-image img {
  inline-size: 100%;
  aspect-ratio: 4 / 3;
  block-size: auto;
  object-fit: contain;
  background: var(--color-surface-muted);
  border-radius: var(--radius-control);
}

.review-image__error {
  color: var(--color-error);
  font-weight: 700;
}

.review-image__confirm {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  min-block-size: 44px;
  padding: var(--space-2);
  border: 1px solid var(--color-control-border);
  border-radius: var(--radius-control);
  cursor: pointer;
}

.review-image__confirm:has(input:disabled) {
  cursor: not-allowed;
  opacity: 0.75;
}

.review-image__confirm input {
  flex: 0 0 auto;
  inline-size: 24px;
  block-size: 24px;
  margin: 0;
}

.review__buttons {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-3);
  margin-block-start: var(--space-4);
}

@media (max-width: 480px) {
  .review__header {
    flex-direction: column;
  }

  .review__buttons,
  .review__buttons :deep(.btn) {
    inline-size: 100%;
  }
}
</style>
