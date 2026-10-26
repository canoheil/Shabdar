<template>
  <section class="listing-rating" aria-labelledby="listing-rating-title">
    <div class="listing-rating__heading">
      <h2 id="listing-rating-title">امتیازها</h2>
      <p class="listing-rating__summary" aria-live="polite" aria-atomic="true">
        {{ summary }}
      </p>
    </div>

    <p v-if="authStatus === 'pending'" class="meta">در حال بررسی ورود…</p>
    <NuxtLink
      v-else-if="authStatus === 'anonymous' || authExpired"
      to="/login"
      class="btn btn--secondary"
    >
      ورود برای امتیازدهی
    </NuxtLink>
    <p v-else-if="authStatus === 'unavailable'" class="meta">
      وضعیت ورود مشخص نیست؛ امتیازدهی در دسترس نیست.
    </p>
    <template v-else>
      <p v-if="owner" class="meta">امتیازدهی به آگهی خودتان ممکن نیست.</p>
      <template v-else>
        <p v-if="viewerPending" class="meta">در حال بررسی امتیاز شما…</p>
        <template v-else-if="viewerError">
          <AppAlert tone="error">خواندن امتیاز شما ممکن نشد؛ دوباره تلاش کنید.</AppAlert>
          <AppButton variant="secondary" @click="emit('retry-viewer')">تلاش دوباره</AppButton>
        </template>
        <template v-else-if="myStars !== undefined">
          <div
            class="listing-rating__stars"
            role="radiogroup"
            aria-label="امتیاز شما"
            :aria-busy="pending ? 'true' : 'false'"
          >
            <button
              v-for="stars in 5"
              :key="stars"
              class="listing-rating__star"
              type="button"
              role="radio"
              :aria-label="`${formatNumber(stars)} از ۵ ستاره`"
              :aria-checked="myStars === stars"
              :tabindex="myStars === stars || (myStars === null && stars === 1) ? 0 : -1"
              :disabled="pending"
              @click="choose(stars)"
              @keydown="onStarKeydown($event, stars)"
            >
              <span aria-hidden="true">{{ (myStars ?? 0) >= stars ? '★' : '☆' }}</span>
            </button>
          </div>
          <AppButton
            v-if="myStars !== null"
            variant="secondary"
            :disabled="pending"
            @click="remove"
          >
            حذف امتیاز
          </AppButton>
        </template>
      </template>
    </template>

    <AppAlert v-if="error" tone="error">{{ error }}</AppAlert>
    <AppButton
      v-if="error && lastAttempt"
      variant="secondary"
      :disabled="pending"
      @click="retry"
    >
      تلاش دوباره
    </AppButton>
    <p class="meta" role="status" aria-live="polite" aria-atomic="true">{{ notice }}</p>
  </section>
</template>

<script setup lang="ts">
import type { AuthStatus } from '~/composables/useAuth'
import type { ApiRatingState } from '~/types'

type RatingAttempt = { method: 'PUT'; stars: number } | { method: 'DELETE' }

const props = defineProps<{
  slug: string
  average: number
  count: number
  myStars?: number | null
  authStatus: AuthStatus
  owner: boolean
  viewerPending: boolean
  viewerError: boolean
  authExpired: boolean
}>()

const emit = defineEmits<{
  updated: [state: ApiRatingState]
  'retry-viewer': []
  expired: []
}>()

const pending = ref(false)
const error = ref('')
const notice = ref('')
const lastAttempt = ref<RatingAttempt | null>(null)
const summary = computed(() => props.count === 0
  ? 'هنوز امتیازی ثبت نشده است'
  : `${formatNumber(Math.round(props.average * 10) / 10)} از ۵ · ${formatNumber(props.count)} امتیاز`)

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

async function execute(attempt: RatingAttempt): Promise<void> {
  if (pending.value) return
  pending.value = true
  lastAttempt.value = attempt
  error.value = ''
  notice.value = attempt.method === 'PUT' ? 'در حال ثبت امتیاز…' : 'در حال حذف امتیاز…'
  try {
    const state = await $fetch<ApiRatingState>(
      `/api/v1/listings/${encodeURIComponent(props.slug)}/rating`,
      {
        method: attempt.method,
        body: attempt.method === 'PUT' ? { stars: attempt.stars } : undefined,
      },
    )
    // The public summary and private selection change only after confirmation.
    emit('updated', state)
    lastAttempt.value = null
    notice.value = attempt.method === 'PUT' ? 'امتیاز شما ثبت شد.' : 'امتیاز شما حذف شد.'
  } catch (requestError) {
    if (requestStatus(requestError) === 401) {
      error.value = ''
      notice.value = 'نشست شما منقضی شده است؛ دوباره وارد شوید.'
      emit('expired')
    } else {
      error.value = 'تغییر امتیاز انجام نشد؛ دوباره تلاش کنید.'
      notice.value = ''
    }
  } finally {
    pending.value = false
  }
}

function choose(stars: number): void {
  void execute({ method: 'PUT', stars })
}

function remove(): void {
  void execute({ method: 'DELETE' })
}

function retry(): void {
  if (lastAttempt.value) void execute(lastAttempt.value)
}

function onStarKeydown(event: KeyboardEvent, stars: number): void {
  let next: number | undefined
  if (event.key === 'ArrowLeft' || event.key === 'ArrowDown') next = stars === 1 ? 5 : stars - 1
  if (event.key === 'ArrowRight' || event.key === 'ArrowUp') next = stars === 5 ? 1 : stars + 1
  if (event.key === 'Home') next = 1
  if (event.key === 'End') next = 5
  if (next === undefined) return
  event.preventDefault()
  const group = (event.currentTarget as HTMLElement).parentElement
  const target = group?.querySelectorAll<HTMLButtonElement>('[role="radio"]')[next - 1]
  target?.focus()
  choose(next)
}
</script>

<style scoped>
.listing-rating {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: var(--space-3);
  max-inline-size: 70ch;
  padding: var(--space-4);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-card);
}

.listing-rating__heading {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--space-2) var(--space-4);
}

.listing-rating h2,
.listing-rating p {
  margin: 0;
}

.listing-rating h2 {
  font-size: 1.25rem;
}

.listing-rating__summary {
  font-weight: 700;
  color: var(--color-primary);
}

.listing-rating__stars {
  display: flex;
  flex-direction: row;
  align-items: center;
  gap: var(--space-1);
  inline-size: fit-content;
}

.listing-rating__star {
  display: inline-grid;
  place-items: center;
  inline-size: 2.75rem;
  block-size: 2.75rem;
  padding: 0;
  color: var(--color-text-muted);
  background: transparent;
  border: 1px solid transparent;
  border-radius: var(--radius-control);
  cursor: pointer;
}

.listing-rating__star span {
  font-size: 1.8rem;
  line-height: 1;
}

.listing-rating__star[aria-checked="true"],
.listing-rating__star:has(~ [aria-checked="true"]),
.listing-rating__star:hover {
  color: var(--color-primary);
}

.listing-rating__star:focus-visible {
  outline: 3px solid var(--color-focus);
  outline-offset: 2px;
}

.listing-rating__star:disabled {
  cursor: wait;
  opacity: 0.65;
}
</style>
