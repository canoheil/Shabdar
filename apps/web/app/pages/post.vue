<!--
  Post listing (Phase 3 Task 7): create → upload photos → submit for review.
  Login required (UX redirect only — the API enforces auth). ?edit=<public_id>
  loads the seller's own listing for editing (draft/rejected only).
  CSRF rides the proxy injection — no client token code (Phase 2 WP3).
-->
<template>
  <section class="post">
    <h1>{{ editingId ? 'ویرایش آگهی' : 'ثبت آگهی' }}</h1>

    <p v-if="checking">در حال بررسی ورود…</p>
    <template v-else>
      <p v-if="doneMessage" class="alert-ok" role="status">{{ doneMessage }}</p>
      <p v-if="errorMessage" class="alert-error" role="alert" tabindex="-1">{{ errorMessage }}</p>

      <form class="form" @submit.prevent="save">
        <label class="field">
          <span>دسته</span>
          <select v-model="form.category_id" class="field__input" required>
            <option value="">انتخاب کنید</option>
            <option v-for="c in categories?.data ?? []" :key="c.id" :value="c.id">
              {{ c.name_fa }}
            </option>
          </select>
        </label>
        <label class="field">
          <span>شهر</span>
          <select v-model="form.city_id" class="field__input" required>
            <option value="">انتخاب کنید</option>
            <option v-for="c in cities?.data ?? []" :key="c.slug" :value="c.id">
              {{ c.name_fa }}
            </option>
          </select>
        </label>
        <label class="field">
          <span>عنوان</span>
          <input v-model="form.title" class="field__input" type="text" maxlength="160" required>
        </label>
        <label class="field">
          <span>توضیحات</span>
          <textarea v-model="form.description" class="field__input" rows="4" required />
        </label>
        <fieldset class="post__price-type">
          <legend>قیمت</legend>
          <label><input v-model="form.price_type" type="radio" value="fixed"> مقطوع</label>
          <label><input v-model="form.price_type" type="radio" value="negotiable"> توافقی</label>
          <label><input v-model="form.price_type" type="radio" value="free"> رایگان</label>
        </fieldset>
        <label v-if="form.price_type !== 'free'" class="field">
          <span>قیمت (تومان)</span>
          <input
            v-model.number="form.price_toman"
            class="field__input"
            type="number"
            min="0"
            inputmode="numeric"
            dir="ltr"
          >
        </label>
        <label class="field">
          <span>عکس‌ها (حداکثر ۸)</span>
          <input ref="fileInput" class="field__input" type="file" accept="image/*" multiple>
        </label>
        <button type="submit" class="btn" :disabled="saving">
          {{ editingId ? 'ذخیرهٔ تغییرات' : 'ایجاد پیش‌نویس' }}
        </button>
        <button
          v-if="savedId && !submitted"
          type="button"
          class="btn btn--secondary"
          :disabled="saving"
          @click="submitForReview"
        >
          ارسال برای بررسی
        </button>
      </form>
    </template>
  </section>
</template>

<script setup lang="ts">
import type { ApiCategory, ApiCity, ApiErrorBody, ApiListingOut } from '~/types'

interface CityWithId extends ApiCity {
  id: number
}

const route = useRoute()
const editingId = typeof route.query.edit === 'string' ? route.query.edit : ''

const checking = ref(true)
const saving = ref(false)
const submitted = ref(false)
const savedId = ref(editingId)
const doneMessage = ref<string | null>(null)
const errorMessage = ref<string | null>(null)
const fileInput = ref<HTMLInputElement | null>(null)

const form = reactive({
  category_id: '' as number | '',
  city_id: '' as number | '',
  title: '',
  description: '',
  price_type: 'fixed',
  price_toman: null as number | null,
})

// Login gate (UX redirect only — the API owns auth). A direct useFetch call
// keeps the Nuxt context (an async IIFE wrapper breaks navigateTo on SSR).
const { error: meError } = await useFetch('/api/v1/me', {
  headers: useRequestHeaders(['cookie']),
})
if (meError.value) {
  await navigateTo('/login')
} else {
  checking.value = false
}

const { data: categories } = await useFetch<{ data: ApiCategory[] }>('/api/v1/categories')
// City ids are not in the public payload — resolve Rasht (id 1) statically
// for v1 (ponytail: the city picker stays single-launch-city until the
// reference endpoints grow ids; Gilan has exactly one city today).
const { data: citiesRaw } = await useFetch<{ data: ApiCity[] }>(
  '/api/v1/locations/provinces/gilan/cities',
)
const cities = computed<{ data: (ApiCity & { id: number })[] }>(() => ({
  data: (citiesRaw.value?.data ?? []).map((c, i) => ({ ...c, id: i + 1 })),
}))

// Edit mode: prefill from the seller's own listings.
if (editingId) {
  try {
    const mine = await $fetch<{ data: ApiListingOut[] }>(
      '/api/v1/me/listings?status=all',
      { headers: useRequestHeaders(['cookie']) },
    )
    const found = mine.data.find((l) => l.public_id === editingId)
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

function showError(err: unknown) {
  const body = (err as { data?: ApiErrorBody }).data
  const fields = body?.error?.fields
  if (fields && Object.keys(fields).length > 0) {
    const first = Object.entries(fields)[0]
    errorMessage.value = `خطا در ${first[0]}: ${first[1]}`
  } else {
    errorMessage.value = 'خطایی رخ داد؛ دوباره تلاش کنید.'
  }
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

async function uploadPhotos(publicId: string) {
  const files = fileInput.value?.files
  if (!files || files.length === 0) return
  for (const file of files) {
    const data = new FormData()
    data.append('file', file)
    await $fetch(`/api/v1/listings/${publicId}/images`, { method: 'POST', body: data })
  }
}

async function save() {
  saving.value = true
  errorMessage.value = null
  doneMessage.value = null
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

async function submitForReview() {
  if (!savedId.value) return
  saving.value = true
  errorMessage.value = null
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
  gap: 1rem;
}

.post__price-type {
  display: flex;
  gap: 1rem;
  padding: 0.75rem;
  border: 1px solid #cbd5e0;
  border-radius: 8px;
}

.post__price-type label {
  display: flex;
  align-items: center;
  gap: 0.25rem;
  min-block-size: 44px;
}
</style>
