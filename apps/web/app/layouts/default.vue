<template>
  <div class="shell">
    <header class="shell__header">
      <NuxtLink to="/" class="shell__brand">
        <BrandMascot />
        <span>شبدر</span>
      </NuxtLink>
      <nav class="shell__nav" aria-label="ناوبری اصلی">
        <NuxtLink to="/post" class="shell__nav-link">ثبت آگهی</NuxtLink>
        <NuxtLink
          :to="authStatus === 'anonymous' ? '/login' : '/me'"
          class="shell__nav-link"
        >
          {{ authStatus === 'authenticated' ? 'داشبورد' : authStatus === 'anonymous' ? 'ورود' : 'حساب' }}
        </NuxtLink>
      </nav>
      <AppThemeToggle />
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
const { status: authStatus, refresh: refreshAuth } = await useAuth()
</script>

<style scoped>
.shell__brand {
  gap: var(--space-2);
}

.shell__brand .brand-mascot {
  inline-size: 32px;
  block-size: 32px;
}

.shell__auth-state {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: center;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  color: var(--color-info);
  background: var(--color-info-bg);
  border-block-end: 1px solid var(--color-info);
}

.shell__auth-retry {
  min-block-size: 44px;
  padding-inline: var(--space-3);
  font: inherit;
  font-weight: 600;
  color: inherit;
  background: transparent;
  border: 1px solid currentColor;
  border-radius: var(--radius-control);
  cursor: pointer;
}

.shell__nav-link {
  display: inline-flex;
  align-items: center;
  min-block-size: 44px; /* touch target (ui-ux-pro-max) */
  padding-inline: 0.75rem;
  font-weight: 600;
  text-decoration: none;
}
</style>
