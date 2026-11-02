<template>
  <section class="post">
    <header class="post__header">
      <div>
        <h1>{{ editingId ? 'ویرایش آگهی' : 'ثبت آگهی' }}</h1>
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
        <fieldset class="post__section">
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
            <AppField id="post-city" label="شهر" :error="fieldErrors.city_id">
              <template #default="{ id, describedBy, ariaInvalid }">
                <select
                  :id="id"
                  v-model="form.city_id"
                  class="field__input"
                  :aria-describedby="describedBy"
                  :aria-invalid="ariaInvalid"
                  required
                >
                  <option value="">انتخاب کنید</option>
                  <option v-for="item in cities?.data ?? []" :key="item.slug" :value="item.id">
                    {{ item.name_fa }}
                  </option>
                </select>
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

        <fieldset class="post__section">
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
                :aria-describedby="describedBy"
                :aria-invalid="ariaInvalid"
              >
            </template>
          </AppField>
        </fieldset>

        <fieldset class="post__section">
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
              <input
                :id="id"
                ref="fileInput"
                class="field__input"
                type="file"
                accept="image/jpeg,image/png,image/webp"
                multiple
                :aria-describedby="describedBy"
                :aria-invalid="ariaInvalid"
                @change="replacePreviews"
              >
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
              <div class="post__preview-meta">
                <span>{{ preview.file.name }}</span>
                <AppButton variant="secondary" @click="removePreview(index)">
                  حذف پیش‌نمایش {{ index + 1 }}
                </AppButton>
              </div>
            </li>
          </ul>
        </fieldset>

        <div class="post__actions">
          <AppButton type="submit" :loading="saving">
            {{ editingId ? 'ذخیرهٔ تغییرات' : 'ایجاد پیش‌نویس' }}
          </AppButton>
          <AppButton
            v-if="savedId && !submitted"
            variant="secondary"
            :loading="saving"
            @click="submitForReview"
          >
            ارسال برای بررسی
          </AppButton>
        </div>
      </form>
    </template>
  </section>
</template>

<script setup lang="ts">
import type { ApiCategory, ApiCity, ApiErrorBody, ApiListingOut } from '~/types'

interface ImagePreview {
  id: string
  file: File
  url: string
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

const { data: categories } = await useFetch<{ data: ApiCategory[] }>('/api/v1/categories')
const { data: cities } = await useFetch<{ data: ApiCity[] }>(
  '/api/v1/locations/provinces/gilan/cities',
)

if (editingId) {
  try {
    const mine = await $fetch<{ data: ApiListingOut[] }>(
      '/api/v1/me/listings?status=all',
      { headers: useRequestHeaders(['cookie']) },
    )
    const found = mine.data.find((item) => item.public_id === editingId)
    if (found) {
      form.category_id = found.category_id
      form.city_id = found.city_id
      form.title = found.title
      form.description = found.description
      form.price_type = found.price_type
      form.price_toman = found.price_toman
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
    const data = new FormData()
    data.append('file', preview.file)
    await $fetch(`/api/v1/listings/${publicId}/images`, { method: 'POST', body: data })
  }
}

async function save(): Promise<void> {
  saving.value = true
  errorMessage.value = null
  doneMessage.value = null
  fieldErrors.value = {}
  try {
    if (editingId) {
      await $fetch(`/api/v1/listings/${editingId}`, { method: 'PATCH', body: buildBody() })
      await uploadPhotos(editingId)
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
  if (!savedId.value) return
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
  padding: var(--space-6);
  color: var(--color-hero-text);
  background:
    radial-gradient(circle at 10% 0%, rgb(110 231 183 / 0.14), transparent 36%),
    linear-gradient(135deg, var(--color-hero), var(--color-hero-deep));
  border: 1px solid var(--color-hero-border);
  border-radius: var(--radius-panel);
  box-shadow: var(--shadow-card);
}

.post__header h1,
.post__header p {
  margin: 0;
}

.post__header h1 {
  font-size: clamp(1.75rem, 4vw, 2.4rem);
  line-height: 1.5;
}

.post__header > div > p {
  margin-block-start: var(--space-2);
  color: var(--color-hero-muted);
}

.post__state {
  padding: var(--space-2) var(--space-3);
  font-size: 0.8125rem;
  font-weight: 650;
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
  border: 1px solid var(--color-border);
  border-radius: var(--radius-card);
  box-shadow: var(--shadow-xs);
}

.post__section > legend {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding-inline: var(--space-2);
  font-size: 1.0625rem;
  font-weight: 720;
}

.post__step {
  display: inline-grid;
  place-items: center;
  inline-size: 30px;
  block-size: 30px;
  color: var(--color-on-primary);
  font-size: 0.8125rem;
  background: var(--color-primary);
  border-radius: 10px;
}

.post__price-options {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-2);
}

.post__price-option {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-1);
  min-block-size: 52px;
  padding-inline: var(--space-2);
  background: var(--color-surface-muted);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-control);
  cursor: pointer;
}

.post__price-option:has(input:checked) {
  color: var(--color-primary);
  background: color-mix(in srgb, var(--color-primary) 9%, var(--color-surface));
  border-color: var(--color-primary);
}

.post__price-option input {
  flex: 0 0 44px;
  inline-size: 44px;
  block-size: 44px;
  margin: 0;
  accent-color: var(--color-primary);
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
  font-size: 0.875rem;
  line-height: 1.9;
}

.post__upload-guide ul {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  font-size: 0.75rem;
  list-style: none;
}

.post__upload-guide li {
  padding: var(--space-1) var(--space-3);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-pill);
}

.post__section input[type='file'] {
  min-block-size: 60px;
  padding: var(--space-2);
  background: var(--color-surface-muted);
  border-style: dashed;
  cursor: pointer;
}

.post__section input[type='file']::file-selector-button {
  min-block-size: 44px;
  margin-inline-end: var(--space-3);
  padding-inline: var(--space-4);
  font: inherit;
  font-weight: 600;
  color: var(--color-on-primary);
  background: var(--color-primary);
  border: 0;
  border-radius: 10px;
  cursor: pointer;
}

.post__previews {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(14rem, 100%), 1fr));
  gap: var(--space-4);
  margin: 0;
  padding: 0;
  list-style: none;
}

.post__preview {
  min-inline-size: 0;
  overflow: hidden;
  background: var(--color-surface-muted);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-card);
}

.post__preview img {
  display: block;
  inline-size: 100%;
  aspect-ratio: 4 / 3;
  object-fit: cover;
}

.post__preview-meta {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3);
  font-size: 0.8125rem;
  overflow-wrap: anywhere;
}

.post__actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: var(--space-3);
  padding: var(--space-4);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-card);
}

@media (min-width: 768px) {
  .post__header {
    padding: var(--space-8);
  }

  .post__section {
    padding: var(--space-6);
  }

  .post__fields {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .post__fields > :nth-child(n + 3) {
    grid-column: 1 / -1;
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
