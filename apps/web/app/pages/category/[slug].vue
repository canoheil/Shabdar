<template>
  <section class="clean-list">

    <ListingBrowser
      :locked-category="slug"
      kicker="دستهٔ انتخاب‌شده"
      :heading="`آگهی‌های ${categoryName}`"
    >
      <template #intro>
        <div class="clean-list__intro">
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
        </div>
      </template>
    </ListingBrowser>
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

.clean-list__intro {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.clean-list__breadcrumb {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-14);
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
}

.clean-list h1,
.clean-list p {
  margin: 0;
}

.clean-list__kicker {
  color: var(--color-accent);
  font-size: var(--font-size-12);
  font-weight: 800;
}

.clean-list__copy {
  max-inline-size: 44rem;
  color: var(--color-text-muted);
  font-size: var(--font-size-14);
  line-height: 1.95;
}
</style>
