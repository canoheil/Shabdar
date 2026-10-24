// Nuxt 4 config for the Shabdar web app (docs/PHASE2_PLAN.md Task 5).
// SSR on :3000; the browser only ever talks to Nuxt (ARCHITECTURE.md §3).
export default defineNuxtConfig({
  ssr: true,

  app: {
    head: {
      // Persian RTL on every page (ARCHITECTURE.md §8).
      htmlAttrs: { lang: 'fa', dir: 'rtl' },
      title: 'شبدر',
    },
  },

  runtimeConfig: {
    // Same-origin /api proxy target (Task 6). Override: NUXT_API_BASE.
    apiBase: 'http://127.0.0.1:8080',
  },

  modules: ['@nuxtjs/color-mode'],

  colorMode: {
    preference: 'system',
    fallback: 'light',
    classSuffix: '',
  },

  css: [
    '@fontsource-variable/vazirmatn/wght.css',
    '~/assets/css/tokens.css',
    '~/assets/css/main.css',
  ],

  compatibilityDate: '2026-10-10',
})
