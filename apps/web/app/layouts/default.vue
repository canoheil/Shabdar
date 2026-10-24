<template>
  <div class="shell">
    <header class="shell__header">
      <NuxtLink to="/" class="shell__brand">
        <BrandMascot />
        <span>شبدر</span>
      </NuxtLink>
      <div class="shell__theme">
        <AppThemeToggle />
      </div>
      <nav class="shell__nav" aria-label="ناوبری اصلی">
        <ul class="shell__nav-list">
          <li class="shell__nav-item">
            <NuxtLink to="/" class="shell__nav-action">
              <PhHouse :size="22" aria-hidden="true" />
              <span>خانه</span>
            </NuxtLink>
          </li>
          <li class="shell__nav-item">
            <button type="button" class="shell__nav-action" @click="focusHomeSearch">
              <PhMagnifyingGlass :size="22" aria-hidden="true" />
              <span>جستجو</span>
            </button>
          </li>
          <li class="shell__nav-item">
            <NuxtLink to="/post" class="shell__nav-action">
              <PhPlusCircle :size="22" aria-hidden="true" />
              <span>ثبت آگهی</span>
            </NuxtLink>
          </li>
          <li class="shell__nav-item">
            <NuxtLink to="/me/favorites" class="shell__nav-action">
              <PhHeart :size="22" aria-hidden="true" />
              <span>علاقه‌مندی‌ها</span>
            </NuxtLink>
          </li>
          <li class="shell__nav-item">
            <NuxtLink :to="accountTarget" class="shell__nav-action">
              <PhUserCircle :size="22" aria-hidden="true" />
              <span>{{ accountLabel }}</span>
            </NuxtLink>
          </li>
          <li v-if="user?.is_admin" class="shell__nav-item shell__nav-item--admin">
            <NuxtLink to="/admin/queue" class="shell__nav-action">
              <PhShieldCheck :size="22" aria-hidden="true" />
              <span>صف بررسی</span>
            </NuxtLink>
          </li>
        </ul>
      </nav>
    </header>
    <div v-if="authStatus === 'unavailable'" class="shell__auth-state" role="alert">
      <span>وضعیت ورود مشخص نیست؛ دوباره تلاش کنید.</span>
      <button type="button" class="shell__auth-retry" @click="refreshAuth">
        تلاش دوباره
      </button>
    </div>
    <main class="shell__main">
      <slot />
    </main>
    <footer class="shell__footer">
      <p>شبدر — بازارچهٔ آگهی‌های محلی</p>
    </footer>
  </div>
</template>

<script setup lang="ts">
import {
  PhHeart,
  PhHouse,
  PhMagnifyingGlass,
  PhPlusCircle,
  PhShieldCheck,
  PhUserCircle,
} from '@phosphor-icons/vue'

const route = useRoute()
const { user, status: authStatus, refresh: refreshAuth } = await useAuth()

const accountTarget = computed(() => authStatus.value === 'anonymous' ? '/login' : '/me')
const accountLabel = computed(() => {
  if (authStatus.value === 'authenticated') return 'داشبورد'
  if (authStatus.value === 'anonymous') return 'ورود'
  return 'حساب'
})

async function focusHomeSearch(): Promise<void> {
  if (route.path !== '/') {
    await navigateTo({ path: '/', hash: '#search' })
    return
  }
  await nextTick()
  document.getElementById('listing-search')?.focus()
}
</script>

<style scoped>
.shell__brand .brand-mascot {
  inline-size: 36px;
  block-size: 36px;
  flex: 0 0 auto;
}

.shell__nav-action svg {
  flex: 0 0 auto;
}
</style>
