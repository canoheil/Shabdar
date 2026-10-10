<template>
  <section class="home">
    <h1>شبدار</h1>
    <p>بازارچهٔ آگهی‌های محلی — نسخهٔ در حال ساخت.</p>

    <!-- Auth state (plan Task 7): session cookie rides SSR via
         useRequestHeaders; /me 401s when logged out. -->
    <template v-if="pending">
      <p>در حال بارگذاری…</p>
    </template>
    <template v-else-if="me">
      <p>خوش آمدید، {{ me.user.display_name ?? 'کاربر شبدار' }}</p>
      <button type="button" class="btn" :disabled="loggingOut" @click="logout">
        خروج
      </button>
    </template>
    <template v-else>
      <p>برای ثبت آگهی وارد شوید.</p>
      <NuxtLink to="/login" class="btn">ورود</NuxtLink>
    </template>
  </section>
</template>

<script setup lang="ts">
import type { ApiUser } from '~/types'

const { data: me, pending, refresh } = await useFetch<{ user: ApiUser }>(
  '/api/v1/me',
  // The cookie must ride SSR requests (ARCHITECTURE.md §3).
  { headers: useRequestHeaders(['cookie']) },
)

const loggingOut = ref(false)

async function logout() {
  loggingOut.value = true
  try {
    await $fetch('/api/v1/auth/logout', { method: 'POST' })
    await refresh()
  } finally {
    loggingOut.value = false
  }
}
</script>

<style scoped>
.home {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  align-items: flex-start;
}
</style>
