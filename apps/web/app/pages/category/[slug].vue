<template>
  <section class="clean-list">
    <nav class="clean-list__breadcrumb" aria-label="مسیر">
      <NuxtLink to="/">خانه</NuxtLink>
      <span aria-hidden="true">/</span>
      <span aria-current="page">{{ categoryName }}</span>
    </nav>

    <header class="clean-list__header">
      <p class="clean-list__kicker">دستهٔ آگهی</p>
      <h1>آگهی‌های {{ categoryName }}</h1>
      <p class="clean-list__copy">
        همهٔ آگهی‌های فعال این دسته در سراسر ایران؛ با جستجو، شهر و بازهٔ قیمت
        محدودترش کنید.
      </p>
    </header>

    <ListingBrowser
      :locked-category="slug"
      kicker="دستهٔ انتخاب‌شده"
      :heading="`آگهی‌های ${categoryName}`"
    />
  </section>
</template>

<script setup lang="ts">
import type { ApiCategory } from '~/types'

const route = useRoute()
const slug = computed(() => String(route.params.slug ?? ''))

// Validated during SSR against the real reference data: an unknown slug is a
// server-rendered 404, never a silently empty feed.
const { data: categories } = await useFetch<{ data: ApiCategory[] }>('/api/v1/categories')
const category = computed(() => categories.value?.data.find((item) => item.slug === slug.value) ?? null)
if (!category.value) {
  throw createError({ statusCode: 404, statusMessage: 'Category not found', fatal: true })
}
const categoryName = computed(() => category.value?.name_fa ?? '')

useHead(() => ({
  title: `آگهی‌های ${categoryName.value} | شبدر`,
  meta: [
    { name: 'description', content: `آگهی‌های فعال دستهٔ ${categoryName.value} در شبدر، بازارچهٔ محلی ایران.` },
    { name: 'robots', content: 'index, follow' },
  ],
  link: [{ rel: 'canonical', href: `/category/${slug.value}` }],
}))
</script>

<style scoped>
.clean-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
  min-inline-size: 0;
}

.clean-list__breadcrumb {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  font-size: 0.875rem;
  color: var(--color-text-muted);
}

.clean-list__breadcrumb a {
  display: inline-flex;
  align-items: center;
  /* «خانه» is a short label; the site-wide gate needs 44px in both axes. */
  min-inline-size: 44px;
  min-block-size: 44px;
}

.clean-list__header {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-5);
  color: var(--color-hero-text);
  background:
    radial-gradient(circle at 8% 0%, rgb(110 231 183 / 0.16), transparent 36%),
    linear-gradient(135deg, var(--color-hero) 0%, var(--color-hero-deep) 84%);
  border: 1px solid var(--color-hero-border);
  border-radius: var(--radius-panel);
}

.clean-list h1,
.clean-list p {
  margin: 0;
}

.clean-list h1 {
  font-size: clamp(1.5rem, 4vw, 2.1rem);
  line-height: 1.45;
}

.clean-list__kicker {
  color: var(--color-hero-accent);
  font-size: 0.8125rem;
  font-weight: 700;
}

.clean-list__copy {
  max-inline-size: 44rem;
  color: var(--color-hero-muted);
  font-size: 0.9375rem;
  line-height: 1.95;
}
</style>
