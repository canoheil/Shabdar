<template>
  <section class="post">
    <header class="post__header">
      <div>
        <h1>{{ savedId ? 'ویرایش آگهی' : 'ثبت آگهی' }}</h1>
        <p>اطلاعات را ذخیره کنید، سپس آگهی را برای بررسی ارسال کنید.</p>
      </div>
      <p
        class="post__state"
        :data-state="submitted ? 'review' : savedId ? 'draft' : 'editing'"
      >
        {{ submitted ? 'در انتظار بررسی' : savedId ? 'پیش‌نویس ذخیره شده' : 'در حال تکمیل اطلاعات' }}
      </p>
    </header>

    <AppSkeleton v-if="checking" variant="text" label="در حال بررسی ورود…" />
    <template v-else-if="authStatus === 'authenticated'">
      <AppAlert v-if="doneMessage" tone="success">{{ doneMessage }}</AppAlert>
      <AppAlert v-if="errorMessage" ref="errorBanner" tone="error">{{ errorMessage }}</AppAlert>

      <form class="post__form" @submit.prevent="save">
        <fieldset class="post__section" :disabled="submitted">
          <legend><span class="post__step" aria-hidden="true">۱</span>مشخصات آگهی</legend>
          <div class="post__fields">
            <AppField id="post-category" label="دسته" :error="fieldErrors.category_id">
              <template #default="{ id, describedBy, ariaInvalid }">
                <select
                  :id="id"
                  v-model="form.category_id"
                  class="field__input"
                  :aria-describedby="describedBy"
                  :aria-invalid="ariaInvalid"
                  required
                >
                  <option value="">انتخاب کنید</option>
                  <option v-for="item in categories?.data ?? []" :key="item.id" :value="item.id">
                    {{ item.name_fa }}
                  </option>
                </select>
              </template>
            </AppField>
            <AppField id="post-province" label="استان">
              <template #default="{ id, describedBy, ariaInvalid }">
                <select
                  :id="id"
                  v-model="selectedProvince"
                  class="field__input"
                  :aria-describedby="describedBy"
                  :aria-invalid="ariaInvalid"
                  required
                  @change="onProvinceChange"
                >
                  <option value="">انتخاب کنید</option>
                  <option v-for="item in provinces?.data ?? []" :key="item.slug" :value="item.slug">
                    {{ item.name_fa }}
                  </option>
                </select>
              </template>
            </AppField>
            <AppField
              id="post-city"
              label="شهر"
              :help="citiesPending ? 'در حال بارگذاری شهرها…' : undefined"
              :error="fieldErrors.city_id ?? (citiesError ? 'بارگذاری شهرها ممکن نشد.' : undefined)"
            >
              <template #default="{ id, describedBy, ariaInvalid }">
                <select
                  :id="id"
                  v-model="form.city_id"
                  class="field__input"
                  :disabled="!selectedProvince || citiesPending || citiesError"
                  :aria-busy="citiesPending"
                  :aria-describedby="describedBy"
                  :aria-invalid="ariaInvalid"
                  required
                >
                  <option value="">{{ selectedProvince ? 'انتخاب کنید' : 'ابتدا استان را انتخاب کنید' }}</option>
                  <option v-for="item in cities" :key="item.slug" :value="item.id">
                    {{ item.name_fa }}
                  </option>
                </select>
                <button
                  v-if="citiesError"
                  type="button"
                  class="btn btn--secondary"
                  @click="retryCities()"
                >
                  تلاش دوباره برای دریافت شهرها
                </button>
              </template>
            </AppField>
            <AppField
              id="post-title"
              label="عنوان"
              help="عنوان روشن و مختصر بنویسید."
              :error="fieldErrors.title"
            >
              <template #default="{ id, describedBy, ariaInvalid }">
                <input
                  :id="id"
                  v-model="form.title"
                  class="field__input"
                  type="text"
                  maxlength="160"
                  :aria-describedby="describedBy"
                  :aria-invalid="ariaInvalid"
                  required
                >
              </template>
            </AppField>
            <AppField
              id="post-description"
              label="توضیحات"
              help="وضعیت و جزئیات مهم برای خریدار را بنویسید."
              :error="fieldErrors.description"
            >
              <template #default="{ id, describedBy, ariaInvalid }">
                <textarea
                  :id="id"
                  v-model="form.description"
                  class="field__input"
                  rows="5"
                  :aria-describedby="describedBy"
                  :aria-invalid="ariaInvalid"
                  required
                />
              </template>
            </AppField>
          </div>
        </fieldset>

        <fieldset class="post__section" :disabled="submitted">
          <legend><span class="post__step" aria-hidden="true">۲</span>قیمت</legend>
          <div class="post__price-options">
            <label class="post__price-option">
              <input v-model="form.price_type" type="radio" name="price_type" value="fixed">
              <span>مقطوع</span>
            </label>
            <label class="post__price-option">
              <input v-model="form.price_type" type="radio" name="price_type" value="negotiable">
              <span>توافقی</span>
            </label>
            <label class="post__price-option">
              <input v-model="form.price_type" type="radio" name="price_type" value="free">
              <span>رایگان</span>
            </label>
          </div>
          <AppField
            v-if="form.price_type !== 'free'"
            id="post-price"
            label="قیمت (تومان)"
            :error="fieldErrors.price_toman"
          >
            <template #default="{ id, describedBy, ariaInvalid }">
              <input
                :id="id"
                v-model.number="form.price_toman"
                class="field__input"
                type="number"
                min="0"
                inputmode="numeric"
                dir="ltr"
                :aria-describedby="[describedBy, typeof form.price_toman === 'number' && form.price_toman > 0 ? 'post-price-readback' : ''].filter(Boolean).join(' ') || undefined"
                :aria-invalid="ariaInvalid"
              >
              <p
                v-if="typeof form.price_toman === 'number' && form.price_toman > 0"
                id="post-price-readback"
                class="post__price-readback"
              >{{ formatPrice(form.price_toman, 'fixed') }}</p>
            </template>
          </AppField>
        </fieldset>

        <fieldset class="post__section" :disabled="submitted">
          <legend><span class="post__step" aria-hidden="true">۳</span>تصاویر</legend>
          <div class="post__upload-guide">
            <div>
              <strong>عکس‌های واضح، انتخاب آسان‌تر</strong>
              <p>در نور روز و از زاویه‌های مختلف عکس بگیرید؛ جزئیات مهم و وضعیت واقعی کالا را نشان دهید.</p>
            </div>
            <ul aria-label="راهنمای تصاویر">
              <li>تا ۸ تصویر</li>
              <li>JPEG، PNG یا WebP ثابت</li>
              <li>تصویر نخست، کاور آگهی می‌شود</li>
            </ul>
          </div>
          <AppField
            id="post-images"
            label="تصاویر آگهی"
            help="JPEG، PNG یا WebP ثابت؛ حداکثر ۸ تصویر. پیش‌نمایش فقط در همین دستگاه ساخته می‌شود."
            :error="fieldErrors.images"
          >
            <template #default="{ id, describedBy, ariaInvalid }">
              <span class="post__photo-drop">
                <span class="post__photo-drop-hint" aria-hidden="true">
                  <AppIcon name="camera-plus-duotone" :size="32" />
                  <span>افزودن تصویر</span>
                  <span class="post__photo-drop-count">{{ formatNumber(previews.length) }} از ۸ تصویر انتخاب شده</span>
                </span>
                <input
                  :id="id"
                  ref="fileInput"
                  class="field__input post__photo-input"
                  type="file"
                  accept="image/jpeg,image/png,image/webp"
                  multiple
                  :aria-describedby="describedBy"
                  :aria-invalid="ariaInvalid"
                  @change="replacePreviews"
                >
              </span>
            </template>
          </AppField>
          <ul v-if="previews.length" class="post__previews" aria-label="پیش‌نمایش تصاویر">
            <li v-for="(preview, index) in previews" :key="preview.id" class="post__preview">
              <img
                :src="preview.url"
                :alt="`پیش‌نمایش ${preview.file.name}`"
                width="320"
                height="240"
              >
              <span v-if="index === 0" class="post__preview-cover">کاور</span>
              <div class="post__preview-meta">
                <span>{{ preview.file.name }}</span>
                <AppButton variant="secondary" class="post__preview-remove" @click="removePreview(index)">
                  <AppIcon name="x-bold" :size="16" />
                  <span class="sr-only">حذف پیش‌نمایش {{ index + 1 }}</span>
                </AppButton>
              </div>
            </li>
          </ul>
        </fieldset>

        <div v-if="!submitted" class="post__actions">
          <AppButton type="submit" :loading="saving">
            {{ savedId ? 'ذخیرهٔ تغییرات' : 'ایجاد پیش‌نویس' }}
          </AppButton>
          <AppButton
            v-if="savedId"
            variant="accent"
            :loading="saving"
            @click="submitForReview"
          >
            ارسال برای بررسی
          </AppButton>
        </div>
        <div v-else class="post__actions">
          <NuxtLink to="/me" class="btn">مشاهده در آگهی‌های من</NuxtLink>
          <NuxtLink to="/" class="btn btn--secondary">بازگشت به آگهی‌ها</NuxtLink>
        </div>
      </form>
    </template>
  </section>
</template>

<script setup lang="ts">
import type { ApiCategory, ApiErrorBody, ApiListingOut, ApiProvince } from '~/types'

interface ImagePreview {
  id: string
  file: File
  url: string
  uploaded: boolean
}

const route = useRoute()
const editingId = typeof route.query.edit === 'string' ? route.query.edit : ''

const saving = ref(false)
const submitted = ref(false)
const savedId = ref(editingId)
const doneMessage = ref<string | null>(null)
const errorMessage = ref<string | null>(null)
const fieldErrors = ref<Record<string, string>>({})
const errorBanner = ref<{ focus: () => void } | null>(null)
const fileInput = ref<HTMLInputElement | null>(null)
const previews = ref<ImagePreview[]>([])
let previewSequence = 0

const selectedProvince = ref('')
const form = reactive({
  category_id: '' as number | '',
  city_id: '' as number | '',
  title: '',
  description: '',
  price_type: 'fixed',
  price_toman: null as number | null,
})

const { status: authStatus, pending: checking } = await useAuth()
if (authStatus.value === 'anonymous') await navigateTo('/login')

const [
  categoriesResult,
  provincesResult,
  citiesResult,
  editResult,
] = await Promise.all([
  useFetch<{ data: ApiCategory[] }>('/api/v1/categories'),
  useFetch<{ data: ApiProvince[] }>('/api/v1/locations/provinces'),
  useProvinceCities(selectedProvince, 'post:cities'),
  editingId
    ? $fetch<{ data: ApiListingOut[] }>('/api/v1/me/listings?status=all', {
        headers: useRequestHeaders(['cookie']),
      })
    : Promise.resolve(null),
])

const { data: categories } = categoriesResult
const { data: provinces } = provincesResult
const {
  cities,
  pending: citiesPending,
  error: citiesError,
  refresh: retryCities,
} = citiesResult

if (editingId) {
  try {
    const found = editResult?.data.find((item) => item.public_id === editingId)
    if (found) {
      selectedProvince.value = found.province_slug ?? ''
      form.category_id = found.category_id
      form.city_id = found.city_id
      form.title = found.title
      form.description = found.description
      form.price_type = found.price_type
      form.price_toman = found.price_toman
      if (selectedProvince.value) await retryCities()
    } else {
      errorMessage.value = 'آگهی یافت نشد.'
    }
  } catch {
    errorMessage.value = 'خطایی رخ داد؛ دوباره تلاش کنید.'
  }
}

function revokeAllPreviews(): void {
  for (const preview of previews.value) URL.revokeObjectURL(preview.url)
  previews.value = []
}

function replacePreviews(event: Event): void {
  const input = event.target as HTMLInputElement
  const files = Array.from(input.files ?? [])
  if (files.length > 8) {
    fieldErrors.value = { images: 'حداکثر ۸ تصویر انتخاب کنید.' }
    input.value = ''
    return
  }
  fieldErrors.value = {}
  revokeAllPreviews()
  previews.value = files.map((file) => {
    previewSequence += 1
    return {
      id: `${file.name}-${file.lastModified}-${previewSequence}`,
      file,
      url: URL.createObjectURL(file),
      uploaded: false,
    }
  })
}

function removePreview(index: number): void {
  const preview = previews.value[index]
  if (!preview) return
  URL.revokeObjectURL(preview.url)
  previews.value.splice(index, 1)
  if (fileInput.value) fileInput.value.value = ''
}

onBeforeUnmount(revokeAllPreviews)

function showError(err: unknown): void {
  const body = (err as { data?: ApiErrorBody }).data
  const fields = body?.error?.fields
  fieldErrors.value = fields ?? {}
  if (fields?.images) {
    const messages: Record<string, string> = {
      invalid_image: 'فایل تصویر معتبر نیست؛ فقط JPEG، PNG یا WebP ثابت و سالم انتخاب کنید.',
      animated: 'تصویر متحرک پذیرفته نیست؛ یک تصویر ثابت انتخاب کنید.',
      too_large: 'حجم یا ابعاد تصویر بیش از حد مجاز است.',
      too_many: 'حداکثر ۸ تصویر برای هر آگهی انتخاب کنید.',
      unreadable: 'خواندن فایل تصویر ممکن نشد؛ دوباره تلاش کنید.',
    }
    const message = messages[fields.images] ?? 'تصویر قابل پذیرش نیست؛ فایل را بررسی و دوباره تلاش کنید.'
    fieldErrors.value = { ...fields, images: message }
    errorMessage.value = `خطا در تصاویر: ${message}`
  } else if (fields && Object.keys(fields).length > 0) {
    const first = Object.entries(fields)[0]!
    errorMessage.value = `خطا در ${first[0]}: ${first[1]}`
  } else {
    errorMessage.value = 'خطایی رخ داد؛ دوباره تلاش کنید.'
  }
  void nextTick(() => errorBanner.value?.focus())
}

function onProvinceChange(): void {
  form.city_id = ''
  fieldErrors.value = {}
  errorMessage.value = null
}

function buildBody() {
  return {
    category_id: Number(form.category_id),
    city_id: Number(form.city_id),
    title: form.title,
    description: form.description,
    price_type: form.price_type,
    price_toman: form.price_type === 'free' ? null : form.price_toman,
  }
}

async function uploadPhotos(publicId: string): Promise<void> {
  for (const preview of previews.value) {
    if (preview.uploaded) continue
    const data = new FormData()
    data.append('file', preview.file)
    await $fetch(`/api/v1/listings/${publicId}/images`, { method: 'POST', body: data })
    preview.uploaded = true
  }
}

async function save(): Promise<void> {
  if (saving.value || submitted.value) return
  saving.value = true
  errorMessage.value = null
  doneMessage.value = null
  fieldErrors.value = {}
  try {
    if (savedId.value) {
      const publicId = savedId.value
      await $fetch(`/api/v1/listings/${publicId}`, { method: 'PATCH', body: buildBody() })
      await uploadPhotos(publicId)
      doneMessage.value = 'تغییرات ذخیره شد.'
    } else {
      const created = await $fetch<ApiListingOut>('/api/v1/listings', {
        method: 'POST',
        body: buildBody(),
      })
      savedId.value = created.public_id
      await uploadPhotos(created.public_id)
      doneMessage.value = 'پیش‌نویس ساخته شد؛ برای انتشار، ارسال برای بررسی را بزنید.'
    }
  } catch (err) {
    showError(err)
  } finally {
    saving.value = false
  }
}

async function submitForReview(): Promise<void> {
  if (!savedId.value || saving.value || submitted.value) return
  saving.value = true
  errorMessage.value = null
  fieldErrors.value = {}
  try {
    await $fetch(`/api/v1/listings/${savedId.value}/submit`, { method: 'POST' })
    submitted.value = true
    doneMessage.value = 'آگهی برای بررسی ارسال شد؛ پس از تأیید نمایش داده می‌شود.'
  } catch (err) {
    showError(err)
  } finally {
    saving.value = false
  }
}
</script>

<style scoped>
.post {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
  max-inline-size: 62rem;
  margin-inline: auto;
}

.post__header {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-4);
}

.post__header h1,
.post__header p {
  margin: 0;
}

.post__header h1 {
  line-height: 1.5;
}

.post__header > div > p {
  margin-block-start: var(--space-2);
  color: var(--color-text-muted);
}

.post__state {
  padding: var(--space-2) var(--space-3);
  font-size: var(--font-size-12);
  font-weight: 800;
  border: 1px solid currentColor;
  border-radius: var(--radius-pill);
}

.post__state[data-state='editing'] {
  color: var(--color-info);
  background: var(--color-info-bg);
}

.post__state[data-state='draft'] {
  color: var(--color-warning);
  background: var(--color-warning-bg);
}

.post__state[data-state='review'] {
  color: var(--color-primary);
  background: var(--color-surface-muted);
}

.post__form,
.post__fields {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.post__form {
  gap: var(--space-5);
}

.post__section {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
  min-inline-size: 0;
  margin: 0;
  padding: var(--space-5);
  background: var(--color-surface);
  border: 0;
  border-radius: var(--radius-card);
  box-shadow: var(--shadow-card);
}

.post__section > legend {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding-inline: var(--space-2);
  font-size: var(--font-size-18);
  font-weight: 800;
}

.post__step {
  display: inline-grid;
  place-items: center;
  inline-size: 30px;
  block-size: 30px;
  color: var(--color-on-primary);
  font-size: var(--font-size-12);
  background: var(--color-primary);
  border-radius: var(--radius-small);
}

.post__price-options {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-1);
  padding: var(--space-1);
  background: var(--color-surface-muted);
  border-radius: var(--radius-pill);
}

.post__price-option {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-1);
  min-block-size: 52px;
  padding-inline: var(--space-2);
  font-weight: 600;
  border-radius: var(--radius-pill);
  cursor: pointer;
}

.post__price-option:has(input:checked) {
  color: var(--color-primary);
  background: var(--color-surface);
  box-shadow: var(--shadow-card);
}

.post__price-option:has(input:focus-visible) {
  outline: 3px solid var(--color-focus);
  outline-offset: 2px;
}

.post__price-option input {
  position: absolute;
  inline-size: 1px;
  block-size: 1px;
  margin: 0;
  opacity: 0;
}

.post__price-readback {
  margin: 0;
  font-size: var(--font-size-14);
  font-weight: 600;
  color: var(--color-primary);
}

.post__upload-guide {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  background: var(--color-accent-soft);
  border: 1px solid color-mix(in srgb, var(--color-accent) 25%, var(--color-border));
  border-radius: var(--radius-card);
}

.post__upload-guide strong {
  color: var(--color-accent);
}

.post__upload-guide p {
  margin: var(--space-1) 0 0;
  color: var(--color-text-muted);
  font-size: var(--font-size-14);
  line-height: 1.9;
}

.post__upload-guide ul {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  font-size: var(--font-size-12);
  list-style: none;
}

.post__upload-guide li {
  padding: var(--space-1) var(--space-3);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-pill);
}

.post__photo-drop {
  position: relative;
  display: block;
}

.post__photo-drop-hint {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-6) var(--space-4);
  color: var(--color-primary);
  font-weight: 600;
  text-align: center;
  background: var(--color-surface-muted);
  border: 1.5px dashed var(--color-control-border);
  border-radius: var(--radius-card);
}

.post__photo-drop-count {
  font-size: var(--font-size-12);
  font-weight: 400;
  color: var(--color-text-muted);
}

.post__photo-input {
  position: absolute;
  inset: 0;
  inline-size: 100%;
  block-size: 100%;
  opacity: 0;
  cursor: pointer;
}

.post__previews {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.post__preview {
  min-inline-size: 0;
  overflow: hidden;
  position: relative;
  background: var(--color-surface-muted);
  border-radius: var(--radius-card);
}

.post__preview img {
  display: block;
  inline-size: 100%;
  aspect-ratio: 1 / 1;
  object-fit: cover;
}

.post__preview-cover {
  position: absolute;
  inset-block-start: var(--space-2);
  inset-inline-start: var(--space-2);
  padding: var(--space-1) var(--space-2);
  font-size: var(--font-size-12);
  font-weight: 800;
  color: var(--color-on-accent-fill);
  background: var(--color-accent-fill);
  border-radius: var(--radius-pill);
}

.post__preview-remove {
  align-self: flex-start;
  min-inline-size: 44px;
  min-block-size: 44px;
  padding: 0;
}

.post__preview-meta {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3);
  font-size: var(--font-size-12);
  overflow-wrap: anywhere;
}

.post__actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: var(--space-3);
  padding: var(--space-4);
  background: var(--color-surface);
  border-radius: var(--radius-card);
  box-shadow: var(--shadow-card);
}

@media (min-width: 700px) {
  .post__fields {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }

  .post__fields > :nth-child(n + 4) {
    grid-column: 1 / -1;
  }

  .post__previews {
    grid-template-columns: repeat(5, minmax(0, 1fr));
  }
}

@media (min-width: 768px) {
  .post__header {
    padding: var(--space-8);
  }

  .post__section {
    padding: var(--space-6);
  }


  .post__upload-guide {
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
  }

  .post__upload-guide > div {
    max-inline-size: 38rem;
  }

  .post__upload-guide ul {
    max-inline-size: 15rem;
    justify-content: flex-end;
  }
}
</style>
