<template>
  <section class="clean-list">
    <nav class="clean-list__breadcrumb" aria-label="مسیر">
      <NuxtLink to="/">خانه</NuxtLink>
      <span aria-hidden="true">/</span>
      <span>{{ city.province_name_fa }}</span>
      <span aria-hidden="true">/</span>
      <span aria-current="page">{{ city.name_fa }}</span>
    </nav>

    <header class="clean-list__header">
      <p class="clean-list__kicker">آگهی‌های شهر</p>
      <h1>آگهی‌های {{ city.name_fa }}</h1>
      <p class="clean-list__copy">
        آگهی‌های فعال {{ city.name_fa }}، استان {{ city.province_name_fa }}؛
        با جستجو، دسته و بازهٔ قیمت محدودترش کنید.
      </p>
    </header>

    <ListingBrowser
      :locked-city="city.slug"
      :locked-province="city.province_slug"
      kicker="شهر انتخاب‌شده"
      :heading="`آگهی‌های ${city.name_fa}`"
    />
  </section>
</template>

<script setup lang="ts">
import type { ApiCityDetail } from '~/types'

const route = useRoute()
const slug = computed(() => String(route.params.slug ?? ''))

// City slugs are unique per province only; the API resolves the collision
// deterministically so this page and its canonical always name the same city.
const { data } = await useFetch<{ data: ApiCityDetail }>(
  () => `/api/v1/locations/cities/${encodeURIComponent(slug.value)}`,
)
if (!data.value?.data) {
  throw createError({ statusCode: 404, statusMessage: 'City not found', fatal: true })
}
const city = computed(() => data.value!.data)

useHead(() => ({
  title: `آگهی‌های ${city.value.name_fa} | شبدر`,
  meta: [
    {
      name: 'description',
      content: `آگهی‌های فعال شهر ${city.value.name_fa}، استان ${city.value.province_name_fa} در شبدر.`,
    },
    { name: 'robots', content: 'index, follow' },
  ],
  link: [{ rel: 'canonical', href: `/city/${city.value.slug}` }],
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
