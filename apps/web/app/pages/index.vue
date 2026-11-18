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
          <div class="home__intro-copy">
            <p class="home__eyebrow">
              <AppIcon name="map-pin-fill" :size="14" />
              بازارچهٔ آگهی‌های ایران
            </p>
            <h1>چیزهای خوب را همین نزدیکی <em>جستجو</em> کنید</h1>
            <p class="home__intro-lead">
              از طعم‌های خانگی و صنایع دستی تا مهارت‌های محلی؛ آگهی‌های تازه را از
              آدم‌های اطراف خودتان پیدا کنید.
            </p>
            <ul class="home__intro-facts">
              <li>
                <AppIcon name="squares-four-duotone" :size="16" />
                <span>{{ categoryFact }}</span>
              </li>
              <li>
                <AppIcon name="map-pin" :size="16" />
                <span>سراسر ایران، از شهر خودتان</span>
              </li>
            </ul>
          </div>
          <BrandMascot pose="full" class="home__mascot" />
        </header>
      </template>
    </ListingBrowser>

    <section class="home__trust" aria-label="اعتماد و ایمنی">
      <div class="home__trust-item">
        <span class="home__trust-icon" aria-hidden="true">
          <AppIcon name="shield-check-duotone" :size="26" />
        </span>
        <div>
          <strong>بررسی پیش از انتشار</strong>
          <p>هر آگهی و هر تصویر پیش از نمایش، انسانی بازبینی می‌شود.</p>
        </div>
      </div>
      <div class="home__trust-item">
        <span class="home__trust-icon" aria-hidden="true">
          <AppIcon name="chat-circle-duotone" :size="26" />
        </span>
        <div>
          <strong>گفت‌وگو داخل شبدر</strong>
          <p>برای هماهنگی، شمارهٔ تماس خود را در آگهی نمی‌گذارید.</p>
        </div>
      </div>
      <div class="home__trust-item">
        <span class="home__trust-icon" aria-hidden="true">
          <AppIcon name="flag-duotone" :size="26" />
        </span>
        <div>
          <strong>گزارش و مسدودسازی</strong>
          <p>مزاحمت را گزارش کنید یا همان کاربر را برای خودتان مسدود کنید.</p>
        </div>
      </div>
    </section>

    <aside class="home__account" aria-label="حساب کاربری">
      <template v-if="mePending">
        <p>در حال بارگذاری…</p>
      </template>
      <template v-else-if="authStatus === 'authenticated' && me">
        <div class="home__account-copy">
          <strong>خوش آمدید، {{ me.display_name ?? 'کاربر شبدر' }}</strong>
          <p>آگهی‌های خود را مدیریت کنید یا مورد تازه‌ای ثبت کنید.</p>
        </div>
        <div class="home__actions">
          <NuxtLink to="/post" class="btn btn--accent">ثبت آگهی</NuxtLink>
          <AppButton variant="secondary" :loading="loggingOut" @click="logout">خروج</AppButton>
        </div>
      </template>
      <template v-else-if="authStatus === 'anonymous'">
        <div class="home__account-copy">
          <h2 class="home__account-title">شما هم چیزی برای عرضه دارید؟</h2>
          <p>برای ثبت آگهی وارد شوید؛ جستجو و دیدن آگهی‌ها بدون ورود هم ممکن است.</p>
        </div>
        <div class="home__actions">
          <NuxtLink to="/login" class="btn btn--accent">ورود و ثبت آگهی</NuxtLink>
        </div>
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
const categoryFact = computed(() =>
  categoryCount.value > 0
    ? `${formatNumber(categoryCount.value)} دستهٔ خرید و فروش`
    : 'دسته‌بندی‌شده برای خرید و فروش',
)

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

/* The hero is a two-column composition: the sentence and the two facts on one
   side, Shabdarak on the other. On a phone it collapses to one column and the
   mascot leaves the reading order instead of pushing the search bar down. */
.home__intro {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  min-inline-size: 0;
  color: var(--color-hero-text);
}

.home__intro-copy {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  min-inline-size: 0;
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
  line-height: 1.6;
}

.home__intro h1 {
  max-inline-size: 18ch;
  font-size: var(--font-size-28);
  font-weight: 800;
  line-height: 1.35;
  text-wrap: balance;
}

.home__intro h1 em {
  color: var(--color-hero-accent);
  font-style: normal;
}

.home__intro-lead {
  max-inline-size: 40rem;
  color: var(--color-hero-muted);
  font-size: var(--font-size-14);
  line-height: 2;
}

.home__intro-facts {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-1) var(--space-4);
  margin: var(--space-1) 0 0;
  padding: 0;
  color: var(--color-hero-muted);
  font-size: var(--font-size-12);
  list-style: none;
}

.home__intro-facts li {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
}

.home__intro-facts .app-icon {
  color: var(--color-hero-accent);
}

.home__mascot {
  display: none;
  flex: none;
  inline-size: 84px;
  block-size: 84px;
  color: var(--color-hero-accent);
  transform-origin: 50% 90%;
  animation: mascot-greet calc(var(--duration-normal) * 4) var(--ease-out);
}

@keyframes mascot-greet {
  0% { transform: translateY(10px) rotate(-10deg); }
  55% { transform: translateY(-3px) rotate(6deg); }
}

/* Three promises, not three boxes: a tinted icon tile, a bold line and one
   sentence each. They sit on paper with a hairline between rows on a phone. */
.home__trust {
  display: grid;
  gap: var(--space-4);
}

.home__trust-item {
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
}

.home__trust-item + .home__trust-item {
  padding-block-start: var(--space-4);
  border-block-start: 1px solid var(--color-border);
}

/* The icon sits in its own tinted tile: three bare icons in a row read as
   decoration, three labelled tiles read as a promise. */
.home__trust-icon {
  display: grid;
  flex: none;
  place-items: center;
  inline-size: 44px;
  block-size: 44px;
  color: var(--color-ink-b);
  background: var(--color-tint-b);
  border-radius: var(--radius-control);
}

.home__trust-item strong {
  display: block;
  font-size: var(--font-size-16);
  font-weight: 800;
  color: var(--color-text);
}

.home__trust-item p {
  margin-block-start: var(--space-1);
  font-size: var(--font-size-14);
  line-height: 1.85;
  color: var(--color-text-muted);
}

/* The account band closes the page; it carries the one saffron action and the
   quieter dashboard link beside it. */
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

.home__account-copy {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-inline-size: 0;
}

.home__account-title {
  margin: 0;
  font-size: var(--font-size-22);
  font-weight: 800;
  line-height: 1.5;
  color: var(--color-hero-text);
}

.home__account strong {
  font-size: var(--font-size-18);
  line-height: 1.6;
}

.home__account p {
  max-inline-size: 44ch;
  color: var(--color-hero-muted);
  font-size: var(--font-size-14);
  line-height: 1.9;
}

.home__actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

/* On the dark band a white secondary button is the loudest thing on the page,
   so the quiet action takes the hero palette instead. */
.home__account .btn--secondary {
  --btn-fg: var(--color-hero-text);
  --btn-bg: transparent;
  --btn-bg-hover: color-mix(in srgb, var(--color-hero-text) 14%, transparent);
  color: var(--color-hero-text);
  background: transparent;
  border-color: var(--color-hero-border);
  box-shadow: none;
}

.home__account .btn--secondary:hover:not(:disabled) {
  box-shadow: none;
}

@media (max-width: 699.98px) {
  .home__intro-lead {
    display: none;
  }
}

@media (min-width: 700px) {
  .home__intro {
    align-items: flex-start;
    gap: var(--space-6);
  }

  .home__intro h1 {
    font-size: var(--font-size-36);
  }

  .home__intro-lead {
    display: block;
  }

  .home__mascot {
    display: block;
  }

  .home__trust {
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--space-6);
    padding-block: var(--space-2);
  }

  .home__trust-item {
    flex-direction: column;
    gap: var(--space-2);
  }

  .home__trust-item + .home__trust-item {
    padding-block-start: 0;
    border-block-start: 0;
  }
}

@media (min-width: 1280px) {
  .home__intro h1 {
    font-size: var(--font-size-display);
    line-height: 1.3;
  }

  .home__mascot {
    inline-size: 104px;
    block-size: 104px;
  }
}
</style>
