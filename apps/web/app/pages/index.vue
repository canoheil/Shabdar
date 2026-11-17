<template>
  <section class="home">
    <ListingBrowser
      kicker="تازه‌های شبدر"
      heading="آگهی‌های تازه"
      show-advertising
      emerald-hero
    >
      <template #intro>
        <header class="home__intro">
          <p class="home__eyebrow"><AppIcon name="map-pin-fill" :size="14" /> بازارچهٔ آگهی‌های ایران</p>
          <h1>چیزهای خوب را همین نزدیکی <em>جستجو</em> کنید</h1>
          <p class="home__intro-copy">از طعم‌های خانگی و صنایع دستی تا مهارت‌های محلی؛ آگهی‌های تازه را از آدم‌های اطراف خودتان پیدا کنید.</p>
          <BrandMascot pose="full" class="home__mascot" />
        </header>
      </template>
    </ListingBrowser>

    <section class="home__trust" aria-label="اعتماد و ایمنی">
      <div class="home__trust-item">
        <AppIcon name="shield-check-duotone" :size="28" />
        <div>
          <strong>بررسی پیش از انتشار</strong>
          <p>هر آگهی قبل از نمایش بازبینی می‌شود.</p>
        </div>
      </div>
      <div class="home__trust-item">
        <AppIcon name="chat-circle-duotone" :size="28" />
        <div>
          <strong>گفت‌وگو داخل شبدر</strong>
          <p>بدون نیاز به دادن شمارهٔ تماس.</p>
        </div>
      </div>
      <div class="home__trust-item">
        <AppIcon name="flag-duotone" :size="28" />
        <div>
          <strong>گزارش و مسدودسازی</strong>
          <p>مزاحمت را گزارش کنید یا کاربر را مسدود کنید.</p>
        </div>
      </div>
    </section>

    <aside class="home__account" aria-label="حساب کاربری">
      <template v-if="mePending">
        <p>در حال بارگذاری…</p>
      </template>
      <template v-else-if="authStatus === 'authenticated' && me">
        <div>
          <strong>خوش آمدید، {{ me.display_name ?? 'کاربر شبدر' }}</strong>
          <p class="meta">آگهی‌های خود را مدیریت کنید یا مورد تازه‌ای ثبت کنید.</p>
        </div>
        <div class="home__actions">
          <NuxtLink to="/post" class="btn btn--accent">ثبت آگهی</NuxtLink>
          <AppButton variant="secondary" :loading="loggingOut" @click="logout">خروج</AppButton>
        </div>
      </template>
      <template v-else-if="authStatus === 'anonymous'">
        <div>
          <h2 class="home__account-title">شما هم چیزی برای عرضه دارید؟</h2>
          <strong>برای ثبت آگهی وارد شوید.</strong>
          <p class="meta">جستجو و دیدن آگهی‌ها بدون ورود هم ممکن است.</p>
        </div>
        <NuxtLink to="/login" class="btn btn--secondary">ورود</NuxtLink>
      </template>
    </aside>
  </section>
</template>

<script setup lang="ts">
import type { ApiCategory } from '~/types'

// The hero advertises the real category count; the browser owns the feed itself.
// The category list and the viewer read are independent, so they are issued
// together: serializing them added a second server round trip to Home's TTFB,
// which is charged directly against the LCP element (the hero heading).
const categoriesRequest = useFetch<{ data: ApiCategory[] }>('/api/v1/categories')
const authRequest = useAuth()

const [
  { data: categories },
  {
    user: me,
    status: authStatus,
    pending: mePending,
    refresh: refreshAuth,
  },
] = await Promise.all([categoriesRequest, authRequest])
const categoryCount = computed(() => categories.value?.data.length ?? 0)

const loggingOut = ref(false)
async function logout(): Promise<void> {
  loggingOut.value = true
  try {
    await $fetch('/api/v1/auth/logout', { method: 'POST' })
    await refreshAuth()
  } finally {
    loggingOut.value = false
  }
}
</script>

<style scoped>
.home {
  display: flex;
  flex-direction: column;
  gap: var(--space-8);
  min-inline-size: 0;
}

.home__intro {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  min-inline-size: 0;
  color: var(--color-hero-text);
}

.home__intro h1,
.home__intro p,
.home__account p,
.home__trust p {
  margin: 0;
}

.home__eyebrow {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  color: var(--color-hero-accent);
  font-size: var(--font-size-12);
  font-weight: 600;
}

.home__intro h1 {
  max-inline-size: 16ch;
  font-size: var(--font-size-36);
  font-weight: 800;
  line-height: 1.34;
  text-wrap: balance;
}

.home__intro h1 em {
  color: var(--color-hero-accent);
  font-style: normal;
}

.home__intro-copy {
  max-inline-size: 43rem;
  color: var(--color-hero-muted);
  font-size: var(--font-size-16);
  line-height: 2;
}

@media (max-width: 699.98px) {
  .home__intro-copy {
    display: none;
  }
}

@media (min-width: 1024px) and (max-width: 1279.98px) {
  .home__intro-copy {
    display: none;
  }
}

.home__mascot {
  inline-size: 72px;
  block-size: 72px;
  color: var(--color-hero-accent);
  transform-origin: 50% 90%;
  animation: mascot-greet calc(var(--duration-normal) * 4) var(--ease-out);
}

@keyframes mascot-greet {
  0% { transform: translateY(10px) rotate(-10deg); }
  55% { transform: translateY(-3px) rotate(6deg); }
}

.home__trust {
  display: grid;
  gap: var(--space-4);
  padding: var(--space-5);
  background: var(--color-surface);
  border-radius: var(--radius-card);
  box-shadow: var(--shadow-card);
}

.home__trust-item {
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
  color: var(--color-primary);
}

.home__trust-item strong {
  display: block;
  font-size: var(--font-size-16);
  font-weight: 800;
  color: var(--color-text);
}

.home__trust-item p {
  font-size: var(--font-size-14);
  color: var(--color-text-muted);
}

.home__account {
  contain: layout style;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  padding: var(--space-5);
  color: var(--color-hero-text);
  background: linear-gradient(135deg, var(--color-hero) 0%, var(--color-hero-deep) 82%);
  border: 1px solid var(--color-hero-border);
  border-radius: var(--radius-panel);
  box-shadow: var(--shadow-lifted);
}

.home__account-title {
  margin: 0 0 var(--space-2);
  font-size: var(--font-size-22);
  font-weight: 800;
  color: var(--color-hero-text);
}

.home__account strong {
  font-size: var(--font-size-18);
}

.home__account .meta {
  margin-block-start: var(--space-1);
  color: var(--color-hero-muted);
}

.home__actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

@media (min-width: 768px) {
  .home__trust {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }

  .home__intro h1 {
    font-size: var(--font-size-display);
  }
}
</style>
