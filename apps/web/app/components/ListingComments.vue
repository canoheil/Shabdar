<template>
  <section
    ref="root"
    class="listing-comments"
    aria-labelledby="listing-comments-title"
    tabindex="-1"
  >
    <div class="listing-comments__heading">
      <h2 id="listing-comments-title">دیدگاه‌ها</h2>
      <p class="listing-comments__count">{{ formatNumber(visibleCount) }} دیدگاه</p>
    </div>

    <template v-if="readError">
      <AppAlert tone="error">بارگذاری دیدگاه‌ها ممکن نشد؛ دوباره تلاش کنید.</AppAlert>
      <AppButton
        variant="secondary"
        :loading="readPending"
        @click="retryInitialRead"
      >
        تلاش دوباره
      </AppButton>
    </template>

    <div v-if="hydrated" class="listing-comments__auth">
      <NuxtLink
        v-if="authStatus === 'anonymous' || authExpired"
        to="/login"
        class="btn btn--secondary"
      >
        ورود برای ثبت دیدگاه
      </NuxtLink>
      <p v-else-if="authStatus === 'unavailable'" class="meta">
        وضعیت ورود مشخص نیست؛ ثبت دیدگاه در دسترس نیست.
      </p>
      <form
        v-if="authStatus === 'authenticated'"
        class="listing-comments__form"
        :aria-busy="createPending ? 'true' : 'false'"
        @submit.prevent="create"
      >
        <label for="listing-comment-body">دیدگاه شما</label>
        <textarea
          id="listing-comment-body"
          v-model="draft"
          name="body"
          rows="4"
          required
          :disabled="createPending || authExpired"
          :aria-invalid="createError ? 'true' : 'false'"
          :aria-describedby="createError ? 'comment-character-count comment-form-error' : 'comment-character-count'"
        />
        <div class="listing-comments__form-meta">
          <span
            id="comment-character-count"
            class="listing-comments__character-count"
          >{{ formatNumber(characterCount) }} / ۱۰۰۰ نویسه</span>
          <AppButton
            v-if="!authExpired"
            type="submit"
            :loading="createPending"
          >
            {{ createPending ? 'در حال ثبت…' : createError ? 'تلاش دوباره' : 'ثبت دیدگاه' }}
          </AppButton>
        </div>
        <AppAlert
          v-if="createError"
          id="comment-form-error"
          ref="formErrorAlert"
          tone="error"
        >
          {{ createError }}
        </AppAlert>
      </form>
    </div>

    <p
      v-if="!readError && comments.length === 0"
      class="listing-comments__empty meta"
    >
      هنوز دیدگاهی ثبت نشده است
    </p>

    <AppAlert
      v-if="deleteError"
      ref="deleteErrorAlert"
      tone="error"
    >
      {{ deleteError }}
    </AppAlert>
    <AppButton
      v-if="deleteError && deleteAttempt"
      variant="secondary"
      :disabled="deletePendingId !== null"
      @click="removeComment(deleteAttempt)"
    >
      تلاش دوباره حذف
    </AppButton>

    <ol v-if="comments.length" class="listing-comments__list">
      <li
        v-for="comment in comments"
        :key="comment.public_id"
        class="listing-comments__item"
        :data-comment-id="comment.public_id"
      >
        <header class="listing-comments__item-header">
          <strong>{{ comment.author.display_name ?? 'کاربر شبدر' }}</strong>
          <time :datetime="comment.created_at">{{ formatCommentDate(comment.created_at) }}</time>
        </header>
        <p class="listing-comments__body">{{ comment.body }}</p>
        <AppButton
          v-if="hydrated && authStatus === 'authenticated' && !authExpired && viewer?.public_id === comment.author.public_id"
          variant="secondary"
          :disabled="deletePendingId !== null"
          :loading="deletePendingId === comment.public_id"
          @click="removeComment(comment)"
        >
          حذف دیدگاه
        </AppButton>
      </li>
    </ol>

    <AppAlert v-if="paginationError" tone="error">{{ paginationError }}</AppAlert>
    <AppButton
      v-if="hasMore"
      variant="secondary"
      :loading="loadMorePending"
      @click="loadMore"
    >
      {{ loadMorePending ? 'در حال بارگذاری…' : paginationError ? 'تلاش دوباره بارگذاری' : 'نمایش دیدگاه‌های بیشتر' }}
    </AppButton>

    <p class="meta" role="status" aria-live="polite" aria-atomic="true">{{ notice }}</p>
  </section>
</template>

<script setup lang="ts">
import type { AuthStatus } from '~/composables/useAuth'
import type { ApiComment, ApiPage, ApiUser } from '~/types'

interface AlertControl {
  focus: () => void
}

const props = defineProps<{
  slug: string
  count: number
  initialPage: ApiPage<ApiComment> | null
  initialError: boolean
  authStatus: AuthStatus
  authExpired: boolean
  viewer: ApiUser | null
}>()

const emit = defineEmits<{
  expired: []
}>()

const hydrated = ref(false)
const root = ref<HTMLElement | null>(null)
const formErrorAlert = ref<AlertControl | null>(null)
const deleteErrorAlert = ref<AlertControl | null>(null)
const visibleCount = ref(props.count)
const comments = ref<ApiComment[]>(props.initialPage?.data ?? [])
const nextCursor = ref(props.initialPage?.page.next_cursor ?? null)
const hasMore = ref(props.initialPage?.page.has_more ?? false)
const readPending = ref(false)
const readError = ref(props.initialError)
const draft = ref('')
const createPending = ref(false)
const createError = ref('')
const deletePendingId = ref<string | null>(null)
const deleteAttempt = ref<ApiComment | null>(null)
const deleteError = ref('')
const loadMorePending = ref(false)
const paginationError = ref('')
const notice = ref('')
const characterCount = computed(() => [...draft.value].length)
const commentsUrl = computed(() => `/api/v1/listings/${encodeURIComponent(props.slug)}/comments`)

onMounted(() => {
  hydrated.value = true
})

function requestStatus(requestError: unknown): number | undefined {
  const candidate = requestError as {
    status?: unknown
    statusCode?: unknown
    response?: { status?: unknown }
  }
  for (const value of [candidate?.status, candidate?.statusCode, candidate?.response?.status]) {
    if (typeof value === 'number') return value
  }
  return undefined
}

function validationReason(requestError: unknown): string | undefined {
  const candidate = requestError as {
    data?: { error?: { fields?: { body?: unknown } } }
  }
  const reason = candidate?.data?.error?.fields?.body
  return typeof reason === 'string' ? reason : undefined
}

async function focusCreateError(): Promise<void> {
  await nextTick()
  formErrorAlert.value?.focus()
}

function clientValidation(): string {
  const trimmed = draft.value.trim()
  if (!trimmed) return 'دیدگاه نمی‌تواند خالی باشد.'
  if ([...trimmed].length > 1000) return 'دیدگاه می‌تواند حداکثر ۱۰۰۰ نویسه باشد.'
  return ''
}

async function create(): Promise<void> {
  if (createPending.value || props.authExpired) return
  notice.value = ''
  createError.value = clientValidation()
  if (createError.value) {
    await focusCreateError()
    return
  }
  createPending.value = true
  notice.value = 'در حال ثبت دیدگاه…'
  try {
    const created = await $fetch<ApiComment>(commentsUrl.value, {
      method: 'POST',
      body: { body: draft.value },
    })
    // Prepend the server's trimmed, public representation only after success.
    comments.value = [
      created,
      ...comments.value.filter((comment) => comment.public_id !== created.public_id),
    ]
    visibleCount.value += 1
    draft.value = ''
    createError.value = ''
    notice.value = 'دیدگاه شما ثبت شد.'
  } catch (requestError) {
    const status = requestStatus(requestError)
    if (status === 401) {
      createError.value = ''
      notice.value = 'نشست شما منقضی شده است؛ دوباره وارد شوید.'
      emit('expired')
    } else {
      const reason = validationReason(requestError)
      createError.value = reason === 'required'
        ? 'دیدگاه نمی‌تواند خالی باشد.'
        : reason === 'too_long'
          ? 'دیدگاه می‌تواند حداکثر ۱۰۰۰ نویسه باشد.'
          : status === 429
            ? 'تعداد دیدگاه‌های ارسالی در این بازه زیاد است؛ بعداً تلاش کنید.'
            : 'ثبت دیدگاه انجام نشد؛ دوباره تلاش کنید.'
      notice.value = ''
      await focusCreateError()
    }
  } finally {
    createPending.value = false
  }
}

async function removeComment(comment: ApiComment): Promise<void> {
  if (deletePendingId.value !== null || props.authExpired) return
  deletePendingId.value = comment.public_id
  deleteAttempt.value = comment
  deleteError.value = ''
  notice.value = 'در حال حذف دیدگاه…'
  try {
    await $fetch(`/api/v1/comments/${comment.public_id}`, { method: 'DELETE' })
    comments.value = comments.value.filter((item) => item.public_id !== comment.public_id)
    visibleCount.value -= 1
    deleteAttempt.value = null
    notice.value = 'دیدگاه حذف شد.'
    await nextTick()
    root.value?.focus()
  } catch (requestError) {
    if (requestStatus(requestError) === 401) {
      deleteError.value = ''
      notice.value = 'نشست شما منقضی شده است؛ دوباره وارد شوید.'
      emit('expired')
    } else {
      deleteError.value = 'حذف دیدگاه انجام نشد؛ دوباره تلاش کنید.'
      notice.value = ''
      await nextTick()
      deleteErrorAlert.value?.focus()
    }
  } finally {
    deletePendingId.value = null
  }
}

async function retryInitialRead(): Promise<void> {
  if (readPending.value) return
  readPending.value = true
  try {
    const page = await $fetch<ApiPage<ApiComment>>(commentsUrl.value, { cache: 'no-store' })
    comments.value = page.data
    nextCursor.value = page.page.next_cursor
    hasMore.value = page.page.has_more
    readError.value = false
    paginationError.value = ''
  } catch {
    readError.value = true
  } finally {
    readPending.value = false
  }
}

async function loadMore(): Promise<void> {
  if (loadMorePending.value || !nextCursor.value) return
  loadMorePending.value = true
  paginationError.value = ''
  try {
    const page = await $fetch<ApiPage<ApiComment>>(commentsUrl.value, {
      query: { cursor: nextCursor.value, limit: 20 },
      cache: 'no-store',
    })
    const known = new Set(comments.value.map((comment) => comment.public_id))
    comments.value = [
      ...comments.value,
      ...page.data.filter((comment) => !known.has(comment.public_id)),
    ]
    // Only a confirmed response may advance the cursor.
    nextCursor.value = page.page.next_cursor
    hasMore.value = page.page.has_more
  } catch {
    paginationError.value = 'بارگذاری دیدگاه‌های بیشتر ممکن نشد؛ دوباره تلاش کنید.'
  } finally {
    loadMorePending.value = false
  }
}
</script>

<style scoped>
.listing-comments {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: var(--space-4);
  max-inline-size: 70ch;
  min-inline-size: 0;
  padding-block-start: var(--space-6);
  border-block-start: 1px solid var(--color-border);
}

.listing-comments__heading,
.listing-comments__item-header,
.listing-comments__form-meta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2) var(--space-4);
}

.listing-comments h2,
.listing-comments p {
  margin: 0;
}

.listing-comments h2 {
  font-size: 1.25rem;
}

.listing-comments__count {
  color: var(--color-text-muted);
}

.listing-comments__form {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.listing-comments__form label {
  font-weight: 700;
}

.listing-comments textarea {
  inline-size: 100%;
  min-block-size: 7rem;
  resize: vertical;
  padding: var(--space-3);
  color: var(--color-text);
  background: var(--color-surface);
  border: 1px solid var(--color-control-border);
  border-radius: var(--radius-control);
}

.listing-comments textarea:disabled {
  opacity: 0.75;
}

.listing-comments__character-count {
  color: var(--color-text-muted);
  font-size: 0.9rem;
}

.listing-comments__list {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.listing-comments__item {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-3);
  min-inline-size: 0;
  padding: var(--space-4);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-card);
}

.listing-comments__item-header {
  inline-size: 100%;
  color: var(--color-text-muted);
  font-size: 0.9rem;
}

.listing-comments__item-header strong {
  color: var(--color-text);
}

.listing-comments__body {
  inline-size: 100%;
  line-height: 1.9;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
</style>
