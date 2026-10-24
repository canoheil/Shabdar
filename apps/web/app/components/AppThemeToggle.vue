<template>
  <label class="theme-toggle">
    <span class="theme-toggle__label">پوسته</span>
    <select v-model="colorMode.preference" class="theme-toggle__select">
      <option value="system">خودکار</option>
      <option value="light">روشن</option>
      <option value="dark">تیره</option>
    </select>
  </label>
</template>

<script setup lang="ts">
const colorMode = useColorMode()
</script>

<style scoped>
.theme-toggle {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  color: var(--color-text-muted);
  font-size: 0.875rem;
}

.theme-toggle__label {
  font-weight: 500;
}

.theme-toggle__select {
  min-block-size: 44px;
  min-inline-size: 6.5rem;
  padding-inline: var(--space-3);
  color: var(--color-text);
  background: var(--color-surface);
  border: 1px solid var(--color-control-border);
  border-radius: var(--radius-control);
  cursor: pointer;
}
</style>
