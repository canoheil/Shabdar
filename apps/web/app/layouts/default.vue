<template>
  <div class="shell">
    <header class="shell__header">
      <NuxtLink to="/" class="shell__brand">
        <BrandMascot />
        <span class="shell__brand-copy">
          <span>شبدر</span>
          <small class="shell__brand-tagline" aria-hidden="true">بازارچهٔ نزدیک شما</small>
        </span>
      </NuxtLink>
      <p class="shell__sidebar-note">
        از دل محله، برای آدم‌های خودمان؛ کشف کنید، گفت‌وگو کنید و آگهی بگذارید.
      </p>
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
              <span>ثبت</span>
            </NuxtLink>
          </li>
          <li class="shell__nav-item">
            <NuxtLink to="/me/conversations" class="shell__nav-action">
              <!-- Inline bubble: the sixth action must not add an icon-package
                   import to the shell chunk every public page loads. -->
              <svg
                width="22"
                height="22"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="1.8"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
                focusable="false"
              >
                <path d="M21 11.5a8.4 8.4 0 0 1-9 8.4 9.6 9.6 0 0 1-3.2-.5L3 21l1.6-4.3A8.2 8.2 0 0 1 3 11.5 8.4 8.4 0 0 1 12 3a8.4 8.4 0 0 1 9 8.5Z" />
              </svg>
              <span>پیام‌ها</span>
            </NuxtLink>
          </li>
          <li class="shell__nav-item">
            <NuxtLink to="/me/favorites" class="shell__nav-action">
              <PhHeart :size="22" aria-hidden="true" />
              <span>ذخیره‌ها</span>
            </NuxtLink>
          </li>
          <li class="shell__nav-item">
            <NuxtLink :to="accountTarget" class="shell__nav-action">
              <PhUserCircle :size="22" aria-hidden="true" />
              <span>{{ accountLabel }}</span>
            </NuxtLink>
          </li>
          <template v-if="user?.is_admin">
            <li v-for="item in adminLinks" :key="item.to" class="shell__nav-item shell__nav-item--admin">
              <NuxtLink :to="item.to" class="shell__nav-action">
                <PhShieldCheck :size="22" aria-hidden="true" />
                <span>{{ item.label }}</span>
              </NuxtLink>
            </li>
          </template>
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

const adminLinks = [
  { to: '/admin/queue', label: 'صف بررسی' },
  { to: '/admin/reports', label: 'گزارش‌ها' },
  { to: '/admin/moderation', label: 'دیدگاه‌ها' },
  { to: '/admin/moderation/rules', label: 'قوانین' },
  { to: '/admin/advertising', label: 'تبلیغات' },
  { to: '/admin/users', label: 'کاربران' },
]

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
