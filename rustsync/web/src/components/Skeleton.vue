<script setup>
defineProps({
  rows: { type: Number, default: 3 },
  animated: { type: Boolean, default: true },
})
</script>

<template>
  <div class="skeleton-wrapper">
    <div v-for="n in rows" :key="n" class="skeleton-row" :class="{ animated }">
      <div class="skeleton-line skeleton-line--long" />
      <div class="skeleton-line skeleton-line--short" />
    </div>
  </div>
</template>

<style lang="scss" scoped>
.skeleton-wrapper {
  padding: 16px;
}

.skeleton-row {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin-bottom: 20px;
  padding: 16px;
  background: var(--el-fill-color-lighter);
  border-radius: 8px;
}

.skeleton-line {
  height: 14px;
  background: var(--el-fill-color);
  border-radius: 4px;
  position: relative;
  overflow: hidden;

  &--long {
    width: 100%;
  }

  &--short {
    width: 60%;
  }
}

.skeleton-row.animated .skeleton-line::after {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: linear-gradient(
    90deg,
    transparent,
    var(--el-fill-color-light),
    transparent
  );
  animation: skeleton-shimmer 1.5s infinite;
}

@keyframes skeleton-shimmer {
  0% { transform: translateX(-100%); }
  100% { transform: translateX(100%); }
}
</style>