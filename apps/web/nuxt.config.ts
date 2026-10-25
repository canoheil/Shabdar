// Nuxt 4 config for the Shabdar web app (docs/PHASE2_PLAN.md Task 5).
// SSR on :3000; the browser only ever talks to Nuxt (ARCHITECTURE.md §3).
export default defineNuxtConfig({
  ssr: true,

  // Keep the small, route-relevant CSS payload in the SSR document. This
  // avoids conditional component stylesheets becoming render-blocking requests.
  features: {
    inlineStyles: true,
  },

  nitro: {
    // The production Node server must transfer the same compressed assets
    // that the route budget measures; this is especially important on mobile.
    compressPublicAssets: {
      gzip: true,
      brotli: true,
    },
  },

  vite: {
    $client: {
      build: {
        rollupOptions: {
          output: {
            manualChunks(id) {
              // Keep route pages split, but avoid a separate HTTP request for
              // every tiny shared control on the production HTTP/1.1 server.
              if (id === '\0plugin-vue:export-helper') return 'shell'
              if (id.endsWith('/app/app.vue')) return 'shell'
              if (/\/app\/(components|layouts|composables)\//.test(id)) return 'shell'
            },
          },
        },
      },
    },
  },

  app: {
    head: {
      // Persian RTL on every page (ARCHITECTURE.md §8).
      htmlAttrs: { lang: 'fa', dir: 'rtl' },
      title: 'شبدر',
      link: [
        { rel: 'icon', type: 'image/svg+xml', href: '/favicon.svg' },
      ],
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
    disableTransition: true,
  },

  css: [
    '@fontsource-variable/vazirmatn/wght.css',
    '~/assets/css/tokens.css',
    '~/assets/css/main.css',
  ],

  experimental: {
    defaults: {
      nuxtLink: {
        // Do not make the current LCP compete with every visible destination.
        // Pointer/keyboard intent still prefetches before actual navigation.
        prefetchOn: { visibility: false, interaction: true },
      },
    },
  },

  compatibilityDate: '2026-10-10',
})
