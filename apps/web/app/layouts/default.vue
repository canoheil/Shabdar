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
      <div class="shell__theme">
        <AppThemeToggle />
      </div>
      <nav class="shell__nav" aria-label="ناوبری اصلی">
        <ul class="shell__nav-list">
          <li class="shell__nav-item">
            <NuxtLink to="/" class="shell__nav-action">
              <span class="shell__nav-icon">
                <AppIcon class="shell__nav-idle" name="house" :size="22" />
                <AppIcon class="shell__nav-current" name="house-fill" :size="22" />
              </span>
              <span>خانه</span>
            </NuxtLink>
          </li>
          <li class="shell__nav-item">
            <button type="button" class="shell__nav-action" @click="focusHomeSearch">
              <span class="shell__nav-icon">
                <AppIcon name="magnifying-glass" :size="22" />
              </span>
              <span>جستجو</span>
            </button>
          </li>
          <li class="shell__nav-item">
            <NuxtLink to="/post" class="shell__nav-action shell__nav-action--post">
              <span class="shell__nav-icon">
                <AppIcon name="plus-circle-fill" :size="22" />
              </span>
              <span>ثبت</span>
            </NuxtLink>
          </li>
          <li class="shell__nav-item">
            <NuxtLink to="/me/conversations" class="shell__nav-action">
              <span class="shell__nav-icon">
                <AppIcon class="shell__nav-idle" name="chat-circle" :size="22" />
                <AppIcon class="shell__nav-current" name="chat-circle-fill" :size="22" />
              </span>
              <span>پیام‌ها</span>
            </NuxtLink>
          </li>
          <li class="shell__nav-item">
            <NuxtLink to="/me/favorites" class="shell__nav-action">
              <span class="shell__nav-icon">
                <AppIcon class="shell__nav-idle" name="heart" :size="22" />
                <AppIcon class="shell__nav-current" name="heart-fill" :size="22" />
              </span>
              <span>ذخیره‌ها</span>
            </NuxtLink>
          </li>
          <li class="shell__nav-item">
            <NuxtLink :to="accountTarget" class="shell__nav-action">
              <span class="shell__nav-icon">
                <AppIcon class="shell__nav-idle" name="user-circle" :size="22" />
                <AppIcon class="shell__nav-current" name="user-circle-fill" :size="22" />
              </span>
              <span>{{ accountLabel }}</span>
            </NuxtLink>
          </li>
          <template v-if="user?.is_admin">
            <li v-for="item in adminLinks" :key="item.to" class="shell__nav-item shell__nav-item--admin">
              <NuxtLink :to="item.to" class="shell__nav-action">
                <span class="shell__nav-icon">
                  <AppIcon name="shield-check" :size="22" />
                </span>
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
      <nav class="shell__footer-links" aria-label="پیوندهای حقوقی و اطلاع‌رسانی">
        <NuxtLink to="/privacy">حریم خصوصی</NuxtLink>
        <span aria-hidden="true">·</span>
        <NuxtLink to="/terms">قوانین استفاده</NuxtLink>
        <span aria-hidden="true">·</span>
        <NuxtLink to="/advertising">تبلیغات</NuxtLink>
      </nav>
    </footer>
  </div>
</template>

<script setup lang="ts">
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
