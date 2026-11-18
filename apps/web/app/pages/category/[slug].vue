<template>
  <section class="clean-list">
    <ListingBrowser
      :locked-category="slug"
      kicker="نتیجهٔ جستجو در دسته"
      heading="آگهی‌های این دسته"
    >
      <template #intro>
        <div class="clean-list__intro">
          <nav class="clean-list__breadcrumb" aria-label="مسیر">
            <NuxtLink to="/">خانه</NuxtLink>
            <span aria-hidden="true">/</span>
            <span aria-current="page">{{ categoryName }}</span>
          </nav>

          <header class="clean-list__header">
            <span class="clean-list__mark" aria-hidden="true"><AppIcon :name="categoryIcon(slug)" :size="30" /></span>
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

<style src="~/assets/css/discovery.css"></style>
