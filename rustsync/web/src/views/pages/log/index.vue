<script setup>
import { computed, onMounted, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { useI18n } from "vue-i18n";
import request from "@/utils/request";

const { t } = useI18n();

const loading = ref(false);
const logContent = ref("");
const totalLines = ref(0);
const lineCount = ref(500);
const autoRefresh = ref(false);
let refreshTimer = null;

async function loadLog() {
  loading.value = true;
  try {
    const res = await request({
      url: "/svr/log/read",
      params: { lines: lineCount.value },
      method: "get",
    });
    logContent.value = res.data?.content || "";
    totalLines.value = res.data?.total_lines || 0;
  } catch {
    ElMessage.error(t("log.readFail"));
  } finally {
    loading.value = false;
  }
}

async function clearLog() {
  try {
    await ElMessageBox.confirm(t("log.clearConfirm"), t("common.warning"), {
      confirmButtonText: t("common.confirm"),
      cancelButtonText: t("common.cancel"),
      type: "warning",
    });
  } catch {
    return;
  }
  try {
    await request({
      url: "/svr/log/clear",
      method: "post",
    });
    ElMessage.success(t("log.cleared"));
    loadLog();
  } catch {
    ElMessage.error(t("log.clearFail"));
  }
}

function toggleAutoRefresh() {
  autoRefresh.value = !autoRefresh.value;
  if (autoRefresh.value) {
    refreshTimer = setInterval(loadLog, 5000);
  } else {
    clearInterval(refreshTimer);
    refreshTimer = null;
  }
}

function scrollToBottom() {
  const el = document.querySelector(".log-viewer-content");
  if (el) el.scrollTop = el.scrollHeight;
}

onMounted(() => {
  loadLog();
});
</script>

<template>
  <div class="log">
    <div class="top-box">
      <div class="top-box-title">{{ t("log.title") }}</div>
    </div>

    <div class="toolbar">
      <div class="toolbar-left">
        <el-select v-model="lineCount" @change="loadLog" size="small" style="width: 120px">
          <el-option :value="100" :label="t('log.recentLines', { n: 100 })" />
          <el-option :value="500" :label="t('log.recentLines', { n: 500 })" />
          <el-option :value="1000" :label="t('log.recentLines', { n: 1000 })" />
          <el-option :value="5000" :label="t('log.recentLines', { n: 5000 })" />
        </el-select>
        <el-button size="small" @click="loadLog">
          <el-icon><Refresh /></el-icon> {{ t("log.refresh") }}
        </el-button>
        <el-button size="small" :type="autoRefresh ? 'warning' : ''" @click="toggleAutoRefresh">
          <el-icon><Timer /></el-icon> {{ autoRefresh ? t("log.stopAutoRefresh") : t("log.autoRefresh") }}
        </el-button>
        <el-button size="small" type="danger" @click="clearLog">
          <el-icon><Delete /></el-icon> {{ t("log.clear") }}
        </el-button>
      </div>
      <div class="toolbar-right">
        <span class="log-info">{{ t("log.totalLines", { n: totalLines }) }}</span>
      </div>
    </div>

    <div class="log-viewer" v-loading="loading">
      <pre class="log-viewer-content">{{ logContent || t("log.empty") }}</pre>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.log {
  width: 100%;
  height: 100%;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  padding: 16px;
}

.top-box {
  margin-bottom: 12px;
  .top-box-title {
    font-size: 20px;
    font-weight: 600;
    color: var(--text-primary);
  }
}

.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 10px;
  flex-wrap: wrap;
  gap: 8px;

  .toolbar-left {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  .log-info {
    color: var(--text-muted);
    font-size: 13px;
  }
}

.log-viewer {
  flex: 1;
  overflow: hidden;
  background: var(--log-viewer-bg);
  border-radius: 6px;
  border: 1px solid var(--border-subtle);

  .log-viewer-content {
    height: 100%;
    margin: 0;
    padding: 12px;
    overflow: auto;
    font-family: "Cascadia Code", "Fira Code", "Consolas", monospace;
    font-size: 13px;
    line-height: 1.6;
    color: var(--log-viewer-text);
    white-space: pre-wrap;
    word-break: break-all;
  }
}

@media (max-width: 768px) {
  .log {
    padding: 8px;
  }

  .toolbar {
    .toolbar-left {
      width: 100%;
      .el-button {
        font-size: 12px;
        padding: 5px 10px;
      }
    }
  }
}
</style>