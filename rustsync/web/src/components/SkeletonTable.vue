<script setup>
defineProps({
  rows: { type: Number, default: 5 },
  cols: { type: Number, default: 4 },
})
</script>

<template>
  <div class="skeleton-table">
    <div class="skeleton-table-header">
      <div v-for="c in cols" :key="'h' + c" class="skeleton-cell" />
    </div>
    <div v-for="r in rows" :key="'r' + r" class="skeleton-table-row">
      <div v-for="c in cols" :key="'c' + c" class="skeleton-cell" />
    </div>
  </div>
</template>

<style lang="scss" scoped>
.skeleton-table {
  padding: 16px;

  &-header {
    display: flex;
    gap: 8px;
    margin-bottom: 12px;
    padding: 12px 0;
    border-bottom: 2px solid var(--el-border-color-lighter);

    .skeleton-cell {
      height: 16px;
      background: var(--el-fill-color-darker);
    }
  }

  &-row {
    display: flex;
    gap: 8px;
    margin-bottom: 8px;
    padding: 10px 0;
    border-bottom: 1px solid var(--el-border-color-extra-light);
  }

  .skeleton-cell {
    flex: 1;
    height: 14px;
    background: var(--el-fill-color);
    border-radius: 4px;
    position: relative;
    overflow: hidden;

    &::after {
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
  }
}

@keyframes skeleton-shimmer {
  0% { transform: translateX(-100%); }
  100% { transform: translateX(100%); }
}
</style>