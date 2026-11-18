<template>
  <section class="clean-list">
    <ListingBrowser
      :locked-city="city.slug"
      :locked-province="city.province_slug"
      kicker="نتیجهٔ جستجو در شهر"
      heading="آگهی‌های این شهر"
    >
      <template #intro>
        <div class="clean-list__intro">
          <nav class="clean-list__breadcrumb" aria-label="مسیر">
            <NuxtLink to="/">خانه</NuxtLink>
            <span aria-hidden="true">/</span>
            <span>{{ city.province_name_fa }}</span>
            <span aria-hidden="true">/</span>
            <span aria-current="page">{{ city.name_fa }}</span>
          </nav>

          <header class="clean-list__header">
            <span class="clean-list__mark" aria-hidden="true"><AppIcon name="map-pin" :size="30" /></span>
            <p class="clean-list__kicker">آگهی‌های شهر</p>
            <h1>آگهی‌های {{ city.name_fa }}</h1>
            <p class="clean-list__copy">
              آگهی‌های فعال {{ city.name_fa }}، استان {{ city.province_name_fa }}؛
              با جستجو، دسته و بازهٔ قیمت محدودترش کنید.
            </p>
          </header>
        </div>
      </template>
    </ListingBrowser>
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

<style src="~/assets/css/discovery.css"></style>
