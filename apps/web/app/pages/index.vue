<template>
  <section class="home">
    <div class="home__hero">
      <div class="home__hero-layout">
        <header class="home__intro">
          <p class="home__eyebrow"><span aria-hidden="true" /> بازارچهٔ آگهی‌های ایران</p>
          <h1>چیزهای خوب را همین نزدیکی <em>جستجو</em> کنید</h1>
          <p class="home__intro-copy">از طعم‌های خانگی و صنایع دستی تا مهارت‌های محلی؛ آگهی‌های تازه را از آدم‌های اطراف خودتان پیدا کنید.</p>
          <ul class="home__hero-points" aria-label="ویژگی‌های شبدر">
            <li>{{ formatNumber(categoryCount) }} دستهٔ محلی</li>
            <li>شهرهای سراسر ایران</li>
            <li>دیدن آگهی‌ها بدون ورود</li>
          </ul>
        </header>
        <div class="home__hero-emblem" aria-hidden="true">
          <span class="home__hero-emblem-ring">
            <BrandMascot />
          </span>
          <span>محلی، صمیمی، نزدیک</span>
        </div>
      </div>
    </div>

    <ListingBrowser
      kicker="تازه‌های شبدر"
      heading="آگهی‌های تازه"
      show-advertising
    />

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
          <NuxtLink to="/post" class="btn">ثبت آگهی</NuxtLink>
          <AppButton variant="secondary" :loading="loggingOut" @click="logout">خروج</AppButton>
        </div>
      </template>
      <template v-else-if="authStatus === 'anonymous'">
        <div>
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

.home__hero {
  position: relative;
  isolation: isolate;
  overflow: hidden;
  padding: var(--space-5);
  color: var(--color-hero-text);
  background:
    radial-gradient(circle at 10% 0%, rgb(110 231 183 / 0.17), transparent 34%),
    linear-gradient(135deg, var(--color-hero) 0%, var(--color-hero-deep) 82%);
  border: 1px solid var(--color-hero-border);
  border-radius: var(--radius-panel);
  box-shadow: var(--shadow-lifted);
}

.home__hero::before {
  position: absolute;
  z-index: -1;
  inset: 0;
  content: '';
  opacity: 0.24;
  background-image: radial-gradient(rgb(255 255 255 / 0.32) 0.75px, transparent 0.75px);
  background-size: 18px 18px;
  mask-image: linear-gradient(to left, #000, transparent 70%);
}

.home__hero::after {
  position: absolute;
  z-index: -1;
  inset-block-start: -7rem;
  inset-inline-end: -5rem;
  inline-size: 18rem;
  block-size: 18rem;
  content: '';
  border: 1px solid rgb(243 200 118 / 0.2);
  border-radius: 50%;
  box-shadow: 0 0 0 2.5rem rgb(255 255 255 / 0.025), 0 0 0 5rem rgb(255 255 255 / 0.02);
}

.home__hero-layout {
  position: relative;
  z-index: 1;
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  align-items: center;
  gap: var(--space-6);
}

.home__intro {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  min-inline-size: 0;
}

.home__intro h1,
.home__intro p,
.home__account p {
  margin: 0;
}

.home__eyebrow {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  color: var(--color-hero-accent);
  font-size: 0.8125rem;
  font-weight: 650;
}

.home__eyebrow > span {
  inline-size: 8px;
  block-size: 8px;
  content: '';
  background: currentColor;
  border-radius: 50%;
  box-shadow: 0 0 0 5px rgb(243 200 118 / 0.12);
}

.home__intro h1 {
  max-inline-size: 16ch;
  font-size: clamp(1.85rem, 5.2vw, 3.1rem);
  font-weight: 780;
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
  font-size: 0.9375rem;
  line-height: 2;
}

.home__hero-points {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2) var(--space-5);
  margin: var(--space-1) 0 0;
  padding: 0;
  color: var(--color-hero-muted);
  font-size: 0.75rem;
  list-style: none;
}

.home__hero-points li {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.home__hero-points li::before {
  inline-size: 5px;
  block-size: 5px;
  content: '';
  background: var(--color-hero-accent);
  border-radius: 50%;
}

.home__hero-points li:last-child {
  display: none;
}

.home__hero-emblem {
  display: none;
}

.home__account {
  contain: layout style;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  padding: var(--space-5);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-inline-start: 4px solid var(--color-accent);
  border-radius: var(--radius-card);
  box-shadow: var(--shadow-xs);
}

.home__account strong {
  font-size: 1.0625rem;
}

.home__account .meta {
  margin-block-start: var(--space-1);
}

.home__actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

@media (min-width: 600px) {
  .home__hero-layout {
    grid-template-columns: minmax(0, 1fr) 9.5rem;
  }

  .home__hero-points li:last-child {
    display: flex;
  }

  .home__hero-emblem {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-3);
    color: var(--color-hero-muted);
    font-size: 0.75rem;
    text-align: center;
  }

  .home__hero-emblem-ring {
    display: grid;
    place-items: center;
    inline-size: 8.5rem;
    block-size: 8.5rem;
    color: var(--color-hero-accent);
    background: rgb(255 255 255 / 0.07);
    border: 1px solid rgb(255 255 255 / 0.14);
    border-radius: 42% 58% 54% 46% / 48% 42% 58% 52%;
    box-shadow: inset 0 0 0 12px rgb(255 255 255 / 0.025);
    transform: rotate(-5deg);
  }

  .home__hero-emblem :deep(.brand-mascot) {
    inline-size: 84px;
    block-size: 84px;
    color: var(--color-hero-accent);
    transform: rotate(5deg);
  }
}

@media (min-width: 768px) {
  .home__hero {
    padding: var(--space-6);
  }

  .home__intro-copy {
    font-size: 1rem;
  }
}

@media (min-width: 1200px) {
  .home__hero {
    padding: var(--space-8);
  }

  .home__hero-layout {
    grid-template-columns: minmax(0, 1fr) 12rem;
  }

  .home__hero-emblem-ring {
    inline-size: 10rem;
    block-size: 10rem;
  }
}
</style>
