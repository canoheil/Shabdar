<template>
  <label class="theme-toggle">
    <AppIcon class="theme-toggle__sun" name="sun" :size="22" />
    <AppIcon class="theme-toggle__moon" name="moon" :size="22" />
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
