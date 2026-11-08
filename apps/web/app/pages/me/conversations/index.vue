<template>
  <section class="inbox">
    <header class="inbox__header">
      <div>
        <h1>پیام‌ها</h1>
        <p class="meta">گفت‌وگوهای شما با خریداران و فروشندگان.</p>
      </div>
      <NuxtLink to="/me" class="inbox__back">بازگشت به داشبورد</NuxtLink>
    </header>

    <AppSkeleton
      v-if="!clientReady || authStatus === 'pending'"
      variant="card"
      :count="4"
      label="در حال بارگذاری پیام‌ها…"
    />

    <AppEmptyState
      v-else-if="authStatus === 'anonymous'"
      title="برای دیدن پیام‌ها وارد شوید."
      description="گفت‌وگوهای شما پس از ورود به حساب نمایش داده می‌شوند."
    >
      <NuxtLink :to="loginHref" class="btn">ورود</NuxtLink>
    </AppEmptyState>

    <AppAlert v-else-if="authStatus === 'unavailable'" tone="error">
      <div class="inbox__state">
        <p>وضعیت ورود مشخص نیست؛ دوباره تلاش کنید.</p>
        <AppButton variant="secondary" @click="retryAll">تلاش دوباره</AppButton>
      </div>
    </AppAlert>

    <template v-else>
      <AppSkeleton
        v-if="pending && !loaded"
        variant="card"
        :count="4"
        label="در حال بارگذاری گفت‌وگوها…"
      />

      <AppAlert v-else-if="error && !loaded" tone="error">
        <div class="inbox__state">
          <p>{{ error }}</p>
          <AppButton variant="secondary" :disabled="pending" @click="retryInitial">
            تلاش دوباره
          </AppButton>
        </div>
      </AppAlert>

      <template v-else>
        <AppEmptyState
          v-if="items.length === 0 && !hasMoreOlder"
          title="هنوز گفت‌وگویی ندارید."
          description="از صفحهٔ هر آگهی می‌توانید با آگهی‌دهنده گفت‌وگو را آغاز کنید."
        >
          <NuxtLink to="/" class="btn">مشاهده آگهی‌ها</NuxtLink>
        </AppEmptyState>

        <ol v-else-if="items.length > 0" class="inbox__list">
          <ConversationListItem
            v-for="item in items"
            :key="item.public_id"
            :item="item"
            :viewer-public-id="viewerPublicId"
          />
        </ol>

        <p v-if="staleError" class="meta" role="status" aria-live="polite">
          {{ staleError }}
          <button type="button" class="inbox__retry-link" @click="manualSync">به‌روزرسانی</button>
        </p>

        <AppAlert v-if="loadOlderError" tone="error">
          <div class="inbox__state">
            <p>{{ loadOlderError }}</p>
            <AppButton variant="secondary" :disabled="pending" @click="loadMore">
              تلاش دوباره
            </AppButton>
          </div>
        </AppAlert>

        <AppButton
          v-if="hasMoreOlder"
          variant="secondary"
          class="inbox__more"
          :loading="pending"
          @click="loadMore"
        >
          {{ pending ? 'در حال بارگذاری…' : 'نمایش گفت‌وگوهای بیشتر' }}
        </AppButton>
      </template>
    </template>
  </section>
</template>

<script setup lang="ts">
const { status: authStatus, user, refresh: refreshAuth } = await useAuth({ server: false })

const {
  items,
  pending,
  loaded,
  error,
  loadOlderError,
  staleError,
  hasMoreOlder,
  loadInitial,
  loadOlder,
  syncNow,
  startPolling,
  stopPolling,
} = useInboxSync()

const clientReady = ref(false)
const viewerPublicId = computed(() => user.value?.public_id ?? '')
const loginHref = computed(() => '/login?next=%2Fme%2Fconversations')
let started = false

async function loadMore(): Promise<void> {
  await loadOlder()
}

async function manualSync(): Promise<void> {
  await syncNow()
}

async function retryInitial(): Promise<void> {
  await loadInitial()
}

async function retryAll(): Promise<void> {
  await refreshAuth()
  if (authStatus.value === 'authenticated') {
    await loadInitial()
    startPolling()
  }
}

// Everything private waits for the client: SSR only ever renders the skeleton,
// so no conversation or message payload can enter the document.
watch(authStatus, (status) => {
  if (status !== 'authenticated') return
  if (!started) {
    started = true
    void loadInitial().then(() => {
      if (clientReady.value) startPolling()
    })
    return
  }
  void syncNow()
}, { immediate: true })

watch(clientReady, (ready) => {
  if (ready && started && authStatus.value === 'authenticated') startPolling()
})

onMounted(() => {
  clientReady.value = true
})

onBeforeUnmount(() => {
  stopPolling()
})

useHead({
  title: 'پیام‌ها | شبدر',
  meta: [{ name: 'robots', content: 'noindex, nofollow, noarchive' }],
})
</script>

<style scoped>
.inbox {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: var(--space-4);
}

.inbox__header {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.inbox__header h1,
.inbox__header p {
  margin: 0;
}

.inbox__header h1 {
  font-size: 1.75rem;
}

.inbox__back {
  display: inline-flex;
  align-items: center;
  min-block-size: 44px;
}

.inbox__list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.inbox__state {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
}

.inbox__state p {
  margin: 0;
}

.inbox__retry-link,
.inbox p .inbox__retry-link {
  display: inline-flex;
  align-items: center;
  min-block-size: 44px;
  padding-inline: var(--space-2);
  font: inherit;
  font-weight: 600;
  color: var(--color-primary);
  background: transparent;
  border: 0;
  cursor: pointer;
}

.inbox__more {
  align-self: flex-start;
}
</style>
