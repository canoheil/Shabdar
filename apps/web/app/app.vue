<template>
  <NuxtLayout>
    <NuxtPage />
  </NuxtLayout>
</template>

<script setup lang="ts">
import arabicFontUrl from '@fontsource-variable/vazirmatn/files/vazirmatn-arabic-wght-normal.woff2?url'
import latinFontUrl from '@fontsource-variable/vazirmatn/files/vazirmatn-latin-wght-normal.woff2?url'

// Use the same Vite URLs as the stylesheet so each font has one response.
// Preloading the actually used subsets prevents a late swap from moving LCP.
useHead({
  link: [arabicFontUrl, latinFontUrl].map((href, index) => ({
    key: `vazirmatn-${index}`,
    rel: 'preload',
    as: 'font',
    type: 'font/woff2',
    href,
    crossorigin: 'anonymous',
  })),
})
</script>
