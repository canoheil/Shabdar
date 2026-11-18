<template>
  <section class="clean-list">
    <ListingBrowser
      :locked-category="slug"
      emerald-hero
      kicker="نتیجهٔ جستجو در دسته"
      heading="آگهی‌های این دسته"
    >
      <template #intro>
        <StorefrontIntro :category-name="categoryName" />
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
